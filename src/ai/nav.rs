//! Waypoint graph built automatically from a map's polygon terrain.
//!
//! Nodes are standing spots sampled along every walkable surface. Edges say
//! how to get between them: walk, jump (with double jump), fall off a ledge,
//! drop through a hatch, or ride a thruster. Every edge is checked by
//! sweeping a fighter-sized box along it, so bots never plan a move that
//! terrain would block. Bots path over this graph with A*.

use crate::data::{MapDef, ObjectKind};
use crate::geo::ShapeKind;
use crate::sim::STAND_SIZE;
use macroquad::math::{Vec2, vec2};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Spacing of standing spots along a surface.
const NODE_SPACING: f32 = 0.8;
/// Highest a bot will try to climb in one jump (double jump reaches ~5.5).
const MAX_RISE: f32 = 4.0;
/// Peak height of a jump arc above the higher end.
const ARC_HEADROOM: f32 = 1.0;
/// Thruster landings must be within this far below the top of the flight.
const TOP_BAND: f32 = 2.5;
/// Widest gap a bot will jump.
const MAX_JUMP_REACH: f32 = 5.5;
/// Deepest drop a bot will jump down to.
const MAX_JUMP_DROP: f32 = 4.0;
/// Nodes this close (and this level) count as a walk apart.
const WALK_REACH: f32 = 1.3;
const WALK_RISE: f32 = 0.9;
/// Bucket size for neighbour lookups.
const BUCKET: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Walk,
    Jump,
    Fall,
    Drop,
    /// Launched by a thruster pad.
    Thrust,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub to: usize,
    pub kind: EdgeKind,
    pub cost: f32,
}

#[derive(Debug, Clone)]
pub struct NavGraph {
    /// Feet position of each standing spot.
    pub nodes: Vec<Vec2>,
    pub edges: Vec<Vec<Edge>>,
    /// Node stands on a hatch (and so can drop through).
    hatch: Vec<bool>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

fn bucket_of(p: Vec2) -> (i32, i32) {
    ((p.x / BUCKET).floor() as i32, (p.y / BUCKET).floor() as i32)
}

/// Does a standing body fit with its feet at `p`? Only the bottom of the box
/// is raised a little, so a body resting on a slope's uphill side does not
/// count as overlapping it, while headroom checks stay exact.
fn fits(map: &MapDef, p: Vec2) -> bool {
    let half = STAND_SIZE.x * 0.5 - 0.03;
    let min = vec2(p.x - half, p.y - STAND_SIZE.y + 0.03);
    let max = vec2(p.x + half, p.y - 0.08);
    !map.terrain.box_blocked(min, max)
}

/// Where a standing body really rests near `p` (see `MapDef::rest_height`).
fn rest(map: &MapDef, p: Vec2) -> Vec2 {
    let y = map
        .rest_height(p, STAND_SIZE.x * 0.5)
        .filter(|y| (y - p.y).abs() < 0.5)
        .unwrap_or(p.y);
    vec2(p.x, y)
}

/// Is every point along the polyline free for a body?
fn path_clear(map: &MapDef, points: &[Vec2]) -> bool {
    points.windows(2).all(|w| {
        let (a, b) = (w[0], w[1]);
        let n = ((b - a).length() / 0.3).ceil().max(1.0) as i32;
        (0..=n).all(|k| fits(map, a.lerp(b, k as f32 / n as f32)))
    })
}

fn hazard_at(map: &MapDef, p: Vec2) -> bool {
    map.terrain
        .hazard_dps(p - vec2(0.3, 1.7), p + vec2(0.3, -0.05))
        > 0.0
}

impl NavGraph {
    pub fn build(map: &MapDef) -> Self {
        // 1. Standing spots along walkable surfaces.
        let mut nodes = Vec::new();
        let mut hatch = Vec::new();
        for e in map.terrain.edges.iter().filter(|e| e.walkable()) {
            let len = (e.b - e.a).length();
            let n = (len / NODE_SPACING).ceil().max(1.0) as i32;
            for k in 0..n {
                let t = (k as f32 + 0.5) / n as f32;
                let p = rest(map, e.a.lerp(e.b, t));
                let inside =
                    p.x > 0.3 && p.x < map.width - 0.3 && p.y > 1.5 && p.y <= map.height + 0.01;
                if inside && fits(map, p) && !hazard_at(map, p) {
                    nodes.push(p);
                    hatch.push(e.kind == ShapeKind::Hatch);
                }
            }
        }
        let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in nodes.iter().enumerate() {
            buckets.entry(bucket_of(*p)).or_default().push(i);
        }
        let mut graph = Self {
            edges: vec![Vec::new(); nodes.len()],
            nodes,
            hatch,
            buckets,
        };

        let pads: Vec<(Vec2, f32)> = map
            .objects
            .iter()
            .filter(|o| o.kind == ObjectKind::Thruster)
            .map(|o| (o.feet(), o.launch_height()))
            .collect();

        for n in 0..graph.nodes.len() {
            let p = graph.nodes[n];
            let mut edges = graph.edges_from(map, n);
            // On a thruster pad, pressing jump launches instead of jumping.
            if let Some(&(pad, height)) = pads
                .iter()
                .find(|(pad, _)| (pad.x - p.x).abs() < 0.6 && (pad.y - p.y).abs() < 0.5)
            {
                edges.retain(|e| e.kind != EdgeKind::Jump);
                edges.extend(graph.thrust_edges(map, n, pad, height));
            }
            graph.edges[n] = edges;
        }
        graph
    }

    /// Nodes within `r` of `p`.
    fn near(&self, p: Vec2, r: f32) -> Vec<usize> {
        let (c0, r0) = bucket_of(p - Vec2::splat(r));
        let (c1, r1) = bucket_of(p + Vec2::splat(r));
        let mut out = Vec::new();
        for by in r0..=r1 {
            for bx in c0..=c1 {
                if let Some(v) = self.buckets.get(&(bx, by)) {
                    out.extend(
                        v.iter()
                            .copied()
                            .filter(|&i| self.nodes[i].distance(p) <= r),
                    );
                }
            }
        }
        out
    }

    fn edges_from(&self, map: &MapDef, n: usize) -> Vec<Edge> {
        let p = self.nodes[n];
        let mut out: Vec<Edge> = Vec::new();
        let add = |out: &mut Vec<Edge>, to: usize, kind: EdgeKind, cost: f32| {
            if to != n && !out.iter().any(|e| e.to == to) {
                out.push(Edge { to, kind, cost });
            }
        };

        // Walking: close, level-ish neighbours with floor all the way.
        for m in self.near(p, WALK_REACH + WALK_RISE) {
            let q = self.nodes[m];
            if m == n || (q.x - p.x).abs() > WALK_REACH || (q.y - p.y).abs() > WALK_RISE {
                continue;
            }
            if walkable_between(map, p, q) {
                add(&mut out, m, EdgeKind::Walk, p.distance(q));
            }
        }
        let walk_neighbours: Vec<usize> = out.iter().map(|e| e.to).collect();

        // Falling off a ledge end, left or right.
        for dir in [-1.0f32, 1.0] {
            let has_side = walk_neighbours
                .iter()
                .any(|&m| (self.nodes[m].x - p.x) * dir > 0.2);
            if has_side {
                continue;
            }
            let off = vec2(p.x + dir * 0.9, p.y - 0.05);
            if !fits(map, off) {
                continue;
            }
            if let Some(land_y) = map.ground_below(off, 0.0, 40.0)
                && land_y > p.y + 0.3
            {
                let land = vec2(off.x, land_y);
                if path_clear(map, &[off, land - vec2(0.0, 0.05)])
                    && let Some(m) = self.closest(land, 1.2)
                {
                    add(&mut out, m, EdgeKind::Fall, 1.0 + (land_y - p.y) * 0.3);
                }
            }
        }

        // Dropping through a hatch.
        if self.hatch[n] {
            let below = vec2(p.x, p.y + 0.4);
            if let Some(land_y) = map.ground_below(below, 0.0, 40.0) {
                let land = vec2(p.x, land_y);
                if let Some(m) = self.closest(land, 1.2)
                    && m != n
                {
                    add(&mut out, m, EdgeKind::Drop, 1.5 + (land_y - p.y) * 0.2);
                }
            }
        }

        // Jumping: up onto higher ledges, across gaps, or down to lower ones.
        for m in self.near(p, MAX_JUMP_REACH + MAX_RISE) {
            if m == n || out.iter().any(|e| e.to == m) {
                continue;
            }
            let q = self.nodes[m];
            let rise = p.y - q.y;
            let reach = (q.x - p.x).abs();
            if rise > MAX_RISE || -rise > MAX_JUMP_DROP || reach > MAX_JUMP_REACH {
                continue;
            }
            if rise <= 0.5 && reach < WALK_REACH {
                continue; // that would be a walk
            }
            // Arc: up to a peak above the higher end, across, then down.
            // Under a low ceiling a flatter arc still makes it.
            let clear = [ARC_HEADROOM, 0.5, 0.15, 0.04].into_iter().any(|headroom| {
                let apex = p.y.min(q.y) - headroom;
                path_clear(map, &[p, vec2(p.x, apex), vec2(q.x, apex), q])
            });
            if clear {
                add(
                    &mut out,
                    m,
                    EdgeKind::Jump,
                    // Jumps are slower and riskier than walking: price them higher.
                    3.0 + reach * 1.2 + rise.max(0.0) * 0.8,
                );
            }
        }
        out
    }

    /// Landing spots a thruster can carry a body to.
    fn thrust_edges(&self, map: &MapDef, n: usize, pad: Vec2, height: f32) -> Vec<Edge> {
        let p = self.nodes[n];
        let max_rise = height - 0.3;
        // A launch flies straight up to the top of its flight: only floors
        // near there are landings. (Swerving out early at a middle floor
        // hits the floor above; those floors are reached by dropping back
        // down the hatches or by the side ledges.)
        let min_rise = (max_rise - TOP_BAND).max(1.0);
        let mut out = Vec::new();
        for m in self.near(p, max_rise + 3.0) {
            let q = self.nodes[m];
            let rise = p.y - q.y;
            if m == n || rise < min_rise || rise > max_rise || (q.x - pad.x).abs() > 3.0 {
                continue;
            }
            // Straight up the shaft (hatches are open from below), then across.
            let top = q.y - 0.3;
            let path = [vec2(pad.x, p.y - 0.1), vec2(pad.x, top), vec2(q.x, top), q];
            if path_clear(map, &path) {
                out.push(Edge {
                    to: m,
                    kind: EdgeKind::Thrust,
                    cost: 2.0 + rise * 0.15 + (q.x - pad.x).abs() * 0.5,
                });
            }
        }
        out
    }

    fn closest(&self, p: Vec2, r: f32) -> Option<usize> {
        self.near(p, r).into_iter().min_by(|&a, &b| {
            self.nodes[a]
                .distance(p)
                .total_cmp(&self.nodes[b].distance(p))
        })
    }

    /// Where a fighter's feet stand on node `n`.
    pub fn pos(&self, n: usize) -> Vec2 {
        self.nodes[n]
    }

    /// The node under (or closest to) a feet position.
    pub fn nearest(&self, feet: Vec2) -> Option<usize> {
        if let Some(n) = self.closest(feet, 1.0) {
            return Some(n);
        }
        (0..self.nodes.len()).min_by(|&a, &b| {
            let da = self.nodes[a] - feet;
            let db = self.nodes[b] - feet;
            // Prefer nodes at or below the feet (reachable by falling).
            let wa = da.x.abs() + da.y.abs() * if da.y < 0.0 { 2.0 } else { 1.0 };
            let wb = db.x.abs() + db.y.abs() * if db.y < 0.0 { 2.0 } else { 1.0 };
            wa.total_cmp(&wb)
        })
    }

    /// Nodes that can both reach `start` and be reached from it.
    pub fn mutually_reachable(&self, start: usize) -> Vec<bool> {
        let n = self.nodes.len();
        let mut back: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (a, edges) in self.edges.iter().enumerate() {
            for e in edges {
                back[e.to].push(a);
            }
        }
        let flood = |next: &dyn Fn(usize) -> Vec<usize>| {
            let mut seen = vec![false; n];
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(a) = stack.pop() {
                for b in next(a) {
                    if !seen[b] {
                        seen[b] = true;
                        stack.push(b);
                    }
                }
            }
            seen
        };
        let fwd = flood(&|a| self.edges[a].iter().map(|e| e.to).collect());
        let bwd = flood(&|a| back[a].clone());
        (0..n).map(|i| fwd[i] && bwd[i]).collect()
    }

    /// Cheapest travel cost from any of `sources` to every node (Dijkstra).
    pub fn costs_from(&self, sources: &[usize]) -> Vec<f32> {
        let mut best = vec![f32::INFINITY; self.nodes.len()];
        let mut open = BinaryHeap::new();
        for &s in sources {
            best[s] = 0.0;
            open.push(Reverse((0u32, s)));
        }
        while let Some(Reverse((c, n))) = open.pop() {
            if c as f32 / 100.0 > best[n] + 1e-3 {
                continue;
            }
            for e in &self.edges[n] {
                let g = best[n] + e.cost;
                if g < best[e.to] {
                    best[e.to] = g;
                    open.push(Reverse(((g * 100.0) as u32, e.to)));
                }
            }
        }
        best
    }

    /// A* from `from` to `to`. Returns the nodes to visit (excluding `from`)
    /// with the edge kind used to reach each.
    pub fn find_path(&self, from: usize, to: usize) -> Option<Vec<(usize, EdgeKind)>> {
        if from == to {
            return Some(Vec::new());
        }
        let goal = self.pos(to);
        let h = |n: usize| self.pos(n).distance(goal);
        let mut best = vec![f32::INFINITY; self.nodes.len()];
        let mut came: Vec<Option<(usize, EdgeKind)>> = vec![None; self.nodes.len()];
        let mut open = BinaryHeap::new();
        best[from] = 0.0;
        open.push(Reverse(((h(from) * 100.0) as u32, from)));
        while let Some(Reverse((_, n))) = open.pop() {
            if n == to {
                let mut path = Vec::new();
                let mut cur = to;
                while let Some((prev, kind)) = came[cur] {
                    path.push((cur, kind));
                    cur = prev;
                }
                path.reverse();
                return Some(path);
            }
            for e in &self.edges[n] {
                let g = best[n] + e.cost;
                if g < best[e.to] {
                    best[e.to] = g;
                    came[e.to] = Some((n, e.kind));
                    open.push(Reverse((((g + h(e.to)) * 100.0) as u32, e.to)));
                }
            }
        }
        None
    }
}

/// Can a body walk from `a` to `b` with floor under it the whole way?
fn walkable_between(map: &MapDef, a: Vec2, b: Vec2) -> bool {
    let n = ((b - a).length() / 0.25).ceil().max(1.0) as i32;
    (0..=n).all(|k| {
        let q = a.lerp(b, k as f32 / n as f32);
        match map.ground_below(q, 0.45, 0.6) {
            Some(_) => {
                let r = rest(map, q);
                fits(map, r) && !hazard_at(map, r)
            }
            None => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::{Material, Shape};

    fn rows_map() -> MapDef {
        MapDef::from_rows(&[
            "############",
            "#..........#",
            "#..........#",
            "#.....####.#",
            "#..........#",
            "#..........#",
            "#.===......#",
            "#..........#",
            "#..........#",
            "############",
        ])
    }

    fn node_near(g: &NavGraph, p: Vec2) -> usize {
        g.closest(p, 0.6)
            .unwrap_or_else(|| panic!("no node near {p}"))
    }

    #[test]
    fn nodes_sit_on_walkable_surfaces() {
        let g = NavGraph::build(&rows_map());
        node_near(&g, vec2(1.5, 9.0)); // floor
        node_near(&g, vec2(3.0, 6.0)); // hatch platform
        node_near(&g, vec2(7.5, 3.0)); // top of the block
    }

    #[test]
    fn can_path_up_to_the_block_and_back() {
        let g = NavGraph::build(&rows_map());
        let floor = node_near(&g, vec2(1.5, 9.0));
        let top = node_near(&g, vec2(7.5, 3.0));
        let up = g.find_path(floor, top).expect("path up");
        assert!(up.iter().any(|(_, k)| *k == EdgeKind::Jump));
        let down = g.find_path(top, floor).expect("path down");
        assert_eq!(down.last().unwrap().0, floor);
    }

    #[test]
    fn hatches_can_be_dropped_through() {
        let g = NavGraph::build(&rows_map());
        let plat = node_near(&g, vec2(3.0, 6.0));
        assert!(
            g.edges[plat]
                .iter()
                .any(|e| e.kind == EdgeKind::Drop && g.nodes[e.to].y > 8.5)
        );
    }

    #[test]
    fn slopes_are_walked_not_jumped() {
        let shapes = vec![Shape {
            kind: ShapeKind::Solid,
            material: Material::Rock,
            points: vec![
                vec2(0.0, 10.0),
                vec2(6.0, 10.0),
                vec2(12.0, 7.0),
                vec2(20.0, 7.0),
                vec2(20.0, 12.0),
                vec2(0.0, 12.0),
            ],
            dps: 0.0,
        }];
        let map = MapDef::from_shapes(shapes, 20.0, 12.0);
        let g = NavGraph::build(&map);
        let low = node_near(&g, vec2(3.0, 10.0));
        let high = node_near(&g, vec2(16.0, 7.0));
        let path = g.find_path(low, high).expect("path up the slope");
        assert!(path.iter().all(|(_, k)| *k == EdgeKind::Walk), "{path:?}");
    }

    #[test]
    fn solid_floors_block_jumps_from_below() {
        // A solid shelf directly overhead with no way around: unreachable.
        let map = MapDef::from_rows(&[
            "############",
            "#..........#",
            "#..........#",
            "############",
            "#..........#",
            "#..........#",
            "############",
        ]);
        let g = NavGraph::build(&map);
        let low = node_near(&g, vec2(5.5, 6.0));
        let high = node_near(&g, vec2(5.5, 3.0));
        assert!(g.find_path(low, high).is_none());
    }
}
