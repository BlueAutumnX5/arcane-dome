//! Freeform polygon terrain.
//!
//! A map's terrain is a list of simple polygons (any angle, convex or not).
//! Collision works on the polygons' boundary edges, never on internal seams,
//! so joined shapes stay perfectly smooth to walk and slide along.

mod poly;

pub use poly::{point_in_polygon, ray_segment, segment_hits_box, triangulate};

use macroquad::math::{Vec2, vec2};
use serde::Deserialize;

/// How a shape behaves in play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    /// Blocks bodies and bullets from every side.
    Solid,
    /// A hatch in a floor: blocks only from above (you can fly up through
    /// it, then stand on it). Bullets pass through.
    Hatch,
    /// Swimmable water.
    Water,
    /// Hurts anyone touching it (spikes, lava, a bottomless drop).
    Hazard,
}

/// What a shape looks like. Only affects drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    #[default]
    Rock,
    Metal,
    Wood,
    Ice,
    Sand,
    Moss,
    Sludge,
    Glass,
    Lava,
    Spikes,
    Water,
    Void,
}

/// One polygon of terrain.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub material: Material,
    pub points: Vec<Vec2>,
    /// Damage per second for hazards.
    pub dps: f32,
}

/// A boundary edge with its outward normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub a: Vec2,
    pub b: Vec2,
    pub normal: Vec2,
    pub kind: ShapeKind,
    pub shape: usize,
}

impl Edge {
    /// Floor you can stand on (faces mostly up).
    pub fn walkable(&self) -> bool {
        self.normal.y < -WALKABLE
    }
}

/// Surfaces steeper than this (normal.y above -WALKABLE) are walls.
/// 0.55 allows slopes up to about 56 degrees.
pub const WALKABLE: f32 = 0.55;

/// Uniform grid of edge indices for fast nearby lookups.
#[derive(Debug, Clone)]
struct EdgeGrid {
    cell: f32,
    cols: i32,
    rows: i32,
    cells: Vec<Vec<usize>>,
}

const GRID_CELL: f32 = 2.0;
/// Thickness of the invisible walls around the map.
const BORDER: f32 = 4.0;

#[derive(Debug, Clone)]
pub struct Terrain {
    pub shapes: Vec<Shape>,
    /// Bounding box of each shape, for skipping far-away ones quickly.
    bounds: Vec<(Vec2, Vec2)>,
    pub edges: Vec<Edge>,
    pub width: f32,
    pub height: f32,
    grid: EdgeGrid,
}

impl Terrain {
    /// Build terrain. Solid walls are added just outside the map bounds so
    /// nothing can ever leave it.
    pub fn new(mut shapes: Vec<Shape>, width: f32, height: f32) -> Self {
        let (w, h, b) = (width, height, BORDER);
        for (x0, y0, x1, y1) in [
            (-b, -b, w + b, 0.0),
            (-b, h, w + b, h + b),
            (-b, 0.0, 0.0, h),
            (w, 0.0, w + b, h),
        ] {
            shapes.push(Shape {
                kind: ShapeKind::Solid,
                material: Material::Void,
                points: vec![vec2(x0, y0), vec2(x1, y0), vec2(x1, y1), vec2(x0, y1)],
                dps: 0.0,
            });
        }

        let mut edges = Vec::new();
        for (si, s) in shapes.iter().enumerate() {
            if !matches!(s.kind, ShapeKind::Solid | ShapeKind::Hatch) {
                continue;
            }
            let n = s.points.len();
            for i in 0..n {
                let (a, b) = (s.points[i], s.points[(i + 1) % n]);
                let d = b - a;
                if d.length_squared() < 1e-8 {
                    continue;
                }
                let mut normal = vec2(d.y, -d.x).normalize();
                let mid = (a + b) * 0.5;
                if point_in_polygon(mid + normal * 1e-3, &s.points) {
                    normal = -normal;
                }
                edges.push(Edge {
                    a,
                    b,
                    normal,
                    kind: s.kind,
                    shape: si,
                });
            }
        }

        let cols = ((w + 2.0 * b) / GRID_CELL).ceil() as i32 + 1;
        let rows = ((h + 2.0 * b) / GRID_CELL).ceil() as i32 + 1;
        let mut grid = EdgeGrid {
            cell: GRID_CELL,
            cols,
            rows,
            cells: vec![Vec::new(); (cols * rows) as usize],
        };
        for (i, e) in edges.iter().enumerate() {
            let (c0, r0) = grid.cell_of(e.a.min(e.b));
            let (c1, r1) = grid.cell_of(e.a.max(e.b));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    grid.cells[(r * cols + c) as usize].push(i);
                }
            }
        }
        let bounds = shapes
            .iter()
            .map(|s| {
                let min = s
                    .points
                    .iter()
                    .fold(Vec2::splat(f32::MAX), |m, p| m.min(*p));
                let max = s
                    .points
                    .iter()
                    .fold(Vec2::splat(f32::MIN), |m, p| m.max(*p));
                (min, max)
            })
            .collect();
        Self {
            shapes,
            bounds,
            edges,
            width,
            height,
            grid,
        }
    }

    /// Indices of edges whose grid cells touch the box `min..max`.
    pub fn edges_in(&self, min: Vec2, max: Vec2) -> Vec<usize> {
        let (c0, r0) = self.grid.cell_of(min);
        let (c1, r1) = self.grid.cell_of(max);
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                for &i in &self.grid.cells[(r * self.grid.cols + c) as usize] {
                    if !out.contains(&i) {
                        out.push(i);
                    }
                }
            }
        }
        out
    }

    /// Nearest hit of a ray against edges that face it and pass `accept`.
    /// Returns the distance and the edge index.
    pub fn cast(
        &self,
        origin: Vec2,
        dir: Vec2,
        max: f32,
        accept: impl Fn(&Edge) -> bool,
    ) -> Option<(f32, usize)> {
        let end = origin + dir * max;
        let mut best: Option<(f32, usize)> = None;
        for i in self.edges_in(origin.min(end), origin.max(end)) {
            let e = &self.edges[i];
            if e.normal.dot(dir) >= 0.0 || !accept(e) {
                continue;
            }
            if let Some(t) = ray_segment(origin, dir, e.a, e.b)
                && t <= max
                && best.is_none_or(|(bt, _)| t < bt)
            {
                best = Some((t, i));
            }
        }
        best
    }

    /// Distance along `dir` (normalized) to the first solid surface, up to `max`.
    /// Hatches do not stop bullets.
    pub fn raycast_solid(&self, origin: Vec2, dir: Vec2, max: f32) -> f32 {
        if self.point_solid(origin) {
            return 0.0;
        }
        self.cast(origin, dir, max, |e| e.kind == ShapeKind::Solid)
            .map_or(max, |(t, _)| t)
    }

    /// Is the point inside any solid shape?
    pub fn point_solid(&self, p: Vec2) -> bool {
        self.near(p, p)
            .any(|s| s.kind == ShapeKind::Solid && point_in_polygon(p, &s.points))
    }

    /// Shapes whose bounding box overlaps `min..max`.
    fn near(&self, min: Vec2, max: Vec2) -> impl Iterator<Item = &Shape> + '_ {
        self.shapes
            .iter()
            .zip(&self.bounds)
            .filter(move |(_, (bmin, bmax))| {
                bmin.x <= max.x && bmax.x >= min.x && bmin.y <= max.y && bmax.y >= min.y
            })
            .map(|(s, _)| s)
    }

    /// Does the box overlap any shape of `kind`?
    pub fn box_touches(&self, min: Vec2, max: Vec2, kind: ShapeKind) -> bool {
        let center = (min + max) * 0.5;
        self.near(min, max).any(|s| {
            if s.kind != kind {
                return false;
            }
            if point_in_polygon(center, &s.points) {
                return true;
            }
            let n = s.points.len();
            (0..n).any(|i| segment_hits_box(s.points[i], s.points[(i + 1) % n], min, max))
        })
    }

    /// The damage rate of any hazard the box touches (0 if none).
    pub fn hazard_dps(&self, min: Vec2, max: Vec2) -> f32 {
        let center = (min + max) * 0.5;
        self.near(min, max)
            .filter(|s| s.kind == ShapeKind::Hazard)
            .filter(|s| {
                point_in_polygon(center, &s.points) || {
                    let n = s.points.len();
                    (0..n).any(|i| segment_hits_box(s.points[i], s.points[(i + 1) % n], min, max))
                }
            })
            .map(|s| s.dps)
            .fold(0.0, f32::max)
    }

    /// Does a box overlap solid terrain? Used to test whether a body fits.
    pub fn box_blocked(&self, min: Vec2, max: Vec2) -> bool {
        let center = (min + max) * 0.5;
        if self.point_solid(center) {
            return true;
        }
        self.edges_in(min, max)
            .into_iter()
            .map(|i| &self.edges[i])
            .any(|e| e.kind == ShapeKind::Solid && segment_hits_box(e.a, e.b, min, max))
    }
}

impl EdgeGrid {
    fn cell_of(&self, p: Vec2) -> (i32, i32) {
        let c = ((p.x + BORDER) / self.cell).floor() as i32;
        let r = ((p.y + BORDER) / self.cell).floor() as i32;
        (c.clamp(0, self.cols - 1), r.clamp(0, self.rows - 1))
    }
}
