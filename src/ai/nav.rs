//! Waypoint graph built automatically from a map's tile grid.
//!
//! A node is any cell a fighter can stand in. Edges say how to get between
//! nodes: walk, jump (with double jump), fall off a ledge, or drop through a
//! one-way platform. Bots path over this graph with A*.

use crate::data::{MapDef, TileKind};
use macroquad::math::{Vec2, vec2};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Highest a bot will try to climb in one jump, in tiles (double jump reaches ~5.5).
const MAX_RISE: i32 = 4;
/// Horizontal reach of an upward jump.
const MAX_REACH: i32 = 4;
/// Widest gap a bot will jump across.
const MAX_GAP: i32 = 5;
/// Deepest drop a bot will jump down to.
const MAX_JUMP_DROP: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Walk,
    Jump,
    Fall,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub to: usize,
    pub kind: EdgeKind,
    pub cost: f32,
}

#[derive(Debug, Clone)]
pub struct NavGraph {
    pub nodes: Vec<(i32, i32)>,
    pub edges: Vec<Vec<Edge>>,
    index: HashMap<(i32, i32), usize>,
}

/// Cells a body can occupy.
fn open(k: TileKind) -> bool {
    matches!(k, TileKind::Air | TileKind::Water)
}

/// Cells a body can pass while jumping (one-way platforms let you through).
fn clear(k: TileKind) -> bool {
    matches!(k, TileKind::Air | TileKind::Water | TileKind::OneWay)
}

fn standable(map: &MapDef, x: i32, y: i32) -> bool {
    open(map.tile(x, y))
        && open(map.tile(x, y - 1))
        && matches!(map.tile(x, y + 1), TileKind::Solid | TileKind::OneWay)
}

impl NavGraph {
    pub fn build(map: &MapDef) -> Self {
        let mut nodes = Vec::new();
        let mut index = HashMap::new();
        for y in 0..map.height as i32 {
            for x in 0..map.width as i32 {
                if standable(map, x, y) {
                    index.insert((x, y), nodes.len());
                    nodes.push((x, y));
                }
            }
        }
        let mut graph = Self {
            edges: vec![Vec::new(); nodes.len()],
            nodes,
            index,
        };
        for n in 0..graph.nodes.len() {
            let edges = graph.edges_from(map, n);
            graph.edges[n] = edges;
        }
        graph
    }

    fn edges_from(&self, map: &MapDef, n: usize) -> Vec<Edge> {
        let (x, y) = self.nodes[n];
        let mut out = Vec::new();
        let mut add = |to: usize, kind: EdgeKind, cost: f32| {
            if !out.iter().any(|e: &Edge| e.to == to) {
                out.push(Edge { to, kind, cost });
            }
        };

        for dx in [-1, 1] {
            let nx = x + dx;
            if let Some(&to) = self.index.get(&(nx, y)) {
                add(to, EdgeKind::Walk, 1.0);
            } else if open(map.tile(nx, y)) && open(map.tile(nx, y - 1)) {
                // Walk off the ledge and fall straight down.
                if let Some(to) = self.fall_target(map, nx, y) {
                    let (_, ty) = self.nodes[to];
                    add(to, EdgeKind::Fall, 1.0 + (ty - y) as f32 * 0.3);
                }
            }
        }

        if map.tile(x, y + 1) == TileKind::OneWay
            && let Some(to) = self.fall_target(map, x, y + 1)
        {
            add(to, EdgeKind::Drop, 1.5);
        }

        for ty in (y - MAX_RISE)..=(y + MAX_JUMP_DROP) {
            for tx in (x - MAX_GAP)..=(x + MAX_GAP) {
                let Some(&to) = self.index.get(&(tx, ty)) else {
                    continue;
                };
                if to == n {
                    continue;
                }
                let rise = y - ty;
                let reach = (tx - x).abs();
                let ok = if rise > 0 {
                    reach <= MAX_REACH && self.clear_jump(map, x, y, tx, ty)
                } else {
                    reach >= 2 && self.clear_jump(map, x, y, tx, ty)
                };
                if ok {
                    add(
                        to,
                        EdgeKind::Jump,
                        2.0 + reach as f32 + rise.max(0) as f32 * 0.5,
                    );
                }
            }
        }
        out
    }

    /// First standable cell found falling down column `x` from row `y`.
    fn fall_target(&self, map: &MapDef, x: i32, y: i32) -> Option<usize> {
        let mut yy = y;
        while yy < map.height as i32 {
            if let Some(&to) = self.index.get(&(x, yy)) {
                return Some(to);
            }
            if !clear(map.tile(x, yy)) {
                return None;
            }
            yy += 1;
        }
        None
    }

    /// Rough check that a jump arc from (x, y) to (tx, ty) is not blocked.
    fn clear_jump(&self, map: &MapDef, x: i32, y: i32, tx: i32, ty: i32) -> bool {
        // A body covers two rows: its feet row and the one above.
        // Level or downward jumps arc one row above the start, which also
        // carries them over spikes in the gap.
        let arc = if ty >= y { 1 } else { 0 };
        let top = y.min(ty) - arc;
        // Rise in the start column up to the arc height.
        if !(top - 1..y).all(|r| clear(map.tile(x, r))) {
            return false;
        }
        // Travel across at that height.
        let (a, b) = if tx > x { (x, tx) } else { (tx, x) };
        if !(a..=b).all(|c| (top - 1..=top).all(|r| clear(map.tile(c, r)))) {
            return false;
        }
        // Come down in the target column.
        (top - 1..=ty).all(|r| clear(map.tile(tx, r)))
    }

    /// Where a fighter's feet stand on node `n`.
    pub fn pos(&self, n: usize) -> Vec2 {
        let (x, y) = self.nodes[n];
        vec2(x as f32 + 0.5, y as f32 + 1.0)
    }

    /// The node under (or closest to) a feet position.
    pub fn nearest(&self, feet: Vec2) -> Option<usize> {
        let cell = (feet.x.floor() as i32, (feet.y - 0.05).floor() as i32);
        if let Some(&n) = self.index.get(&cell) {
            return Some(n);
        }
        (0..self.nodes.len()).min_by(|&a, &b| {
            let da = self.pos(a) - feet;
            let db = self.pos(b) - feet;
            // Prefer nodes at or below the feet (reachable by falling).
            let wa = da.x.abs() + da.y.abs() * if da.y < 0.0 { 2.0 } else { 1.0 };
            let wb = db.x.abs() + db.y.abs() * if db.y < 0.0 { 2.0 } else { 1.0 };
            wa.total_cmp(&wb)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> MapDef {
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

    #[test]
    fn nodes_are_standable_cells() {
        let g = NavGraph::build(&map());
        assert!(g.index.contains_key(&(1, 8)), "floor");
        assert!(g.index.contains_key(&(3, 5)), "on the one-way platform");
        assert!(g.index.contains_key(&(7, 2)), "on top of the block");
        assert!(
            !g.index.contains_key(&(7, 4)),
            "under the block is not standable"
        );
    }

    #[test]
    fn can_path_up_to_the_block_and_back() {
        let g = NavGraph::build(&map());
        let floor = g.index[&(1, 8)];
        let top = g.index[&(7, 2)];
        let up = g.find_path(floor, top).expect("path up");
        assert!(up.iter().any(|(_, k)| *k == EdgeKind::Jump));
        let down = g.find_path(top, floor).expect("path down");
        assert_eq!(down.last().unwrap().0, floor);
    }

    #[test]
    fn one_way_platforms_can_be_dropped_through() {
        let g = NavGraph::build(&map());
        let plat = g.index[&(3, 5)];
        assert!(
            g.edges[plat]
                .iter()
                .any(|e| e.kind == EdgeKind::Drop && g.nodes[e.to] == (3, 8))
        );
    }
}
