//! Axis-aligned boxes against the tile grid, plus grid raycasts.
//!
//! Bodies are described by their feet (bottom-center) position and size.
//! Movement is resolved one axis at a time in small sub-steps, so fast
//! bodies cannot tunnel through one-tile walls.

use crate::data::{MapDef, TileKind};
use macroquad::math::{Vec2, vec2};

/// Tolerance so a body resting exactly on a tile edge is not "inside" it.
pub const SKIN: f32 = 1e-3;
/// Largest distance a body moves per sub-step (well under one tile).
const MAX_STEP: f32 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb {
    pub fn from_feet(feet: Vec2, size: Vec2) -> Self {
        Self {
            min: vec2(feet.x - size.x * 0.5, feet.y - size.y),
            max: vec2(feet.x + size.x * 0.5, feet.y),
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn expand(&self, by: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(by),
            max: self.max + Vec2::splat(by),
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// Distance from `p` to the closest point of the box (0 inside).
    pub fn distance_to(&self, p: Vec2) -> f32 {
        let closest = p.clamp(self.min, self.max);
        (p - closest).length()
    }

    /// Ray entry distance (slab test). `dir` must be normalized.
    pub fn ray_hit(&self, origin: Vec2, dir: Vec2, max_t: f32) -> Option<f32> {
        let mut t0 = 0.0f32;
        let mut t1 = max_t;
        for axis in 0..2 {
            let (o, d, lo, hi) = (origin[axis], dir[axis], self.min[axis], self.max[axis]);
            if d.abs() < 1e-8 {
                if o < lo || o > hi {
                    return None;
                }
            } else {
                let mut a = (lo - o) / d;
                let mut b = (hi - o) / d;
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                t0 = t0.max(a);
                t1 = t1.min(b);
                if t0 > t1 {
                    return None;
                }
            }
        }
        Some(t0)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MoveResult {
    pub landed: bool,
    pub hit_wall: bool,
    pub hit_ceiling: bool,
}

/// Move a body by `vel * dt`, stopping at solid tiles. One-way tiles only
/// block downward movement, and only when the body started above them and
/// is not dropping through. Blocked velocity components are zeroed.
pub fn move_body(
    map: &MapDef,
    feet: &mut Vec2,
    size: Vec2,
    vel: &mut Vec2,
    dt: f32,
    drop_through: bool,
) -> MoveResult {
    let delta = *vel * dt;
    let steps = ((delta.x.abs().max(delta.y.abs()) / MAX_STEP).ceil() as i32).max(1);
    let step = delta / steps as f32;
    let mut res = MoveResult::default();
    for _ in 0..steps {
        if step.x != 0.0 && !res.hit_wall && sweep_x(map, feet, size, step.x) {
            res.hit_wall = true;
            vel.x = 0.0;
        }
        if step.y != 0.0 && !res.landed && !res.hit_ceiling {
            match sweep_y(map, feet, size, step.y, drop_through) {
                Some(true) => {
                    res.landed = true;
                    vel.y = 0.0;
                }
                Some(false) => {
                    res.hit_ceiling = true;
                    vel.y = 0.0;
                }
                None => {}
            }
        }
    }
    res
}

fn rows_of(feet: Vec2, size: Vec2) -> (i32, i32) {
    (
        (feet.y - size.y + SKIN).floor() as i32,
        (feet.y - SKIN).floor() as i32,
    )
}

fn cols_of(feet: Vec2, size: Vec2) -> (i32, i32) {
    let half = size.x * 0.5;
    (
        (feet.x - half + SKIN).floor() as i32,
        (feet.x + half - SKIN).floor() as i32,
    )
}

/// Returns true if blocked by a wall.
fn sweep_x(map: &MapDef, feet: &mut Vec2, size: Vec2, dx: f32) -> bool {
    let half = size.x * 0.5;
    let (row0, row1) = rows_of(*feet, size);
    let solid_col = |c: i32| (row0..=row1).any(|r| map.tile(c, r) == TileKind::Solid);
    if dx > 0.0 {
        let old = feet.x + half;
        let new = old + dx;
        // Columns whose left edge lies in [old, new).
        for c in (old - SKIN).ceil() as i32..=(new.ceil() as i32 - 1) {
            if solid_col(c) {
                feet.x = c as f32 - half;
                return true;
            }
        }
    } else {
        let old = feet.x - half;
        let new = old + dx;
        // Columns whose right edge lies in (new, old].
        let mut c = (old + SKIN).floor() as i32 - 1;
        while c >= new.floor() as i32 {
            if solid_col(c) {
                feet.x = (c + 1) as f32 + half;
                return true;
            }
            c -= 1;
        }
    }
    feet.x += dx;
    false
}

/// Returns Some(true) when landing, Some(false) when bumping a ceiling.
fn sweep_y(map: &MapDef, feet: &mut Vec2, size: Vec2, dy: f32, drop_through: bool) -> Option<bool> {
    let (col0, col1) = cols_of(*feet, size);
    if dy > 0.0 {
        let old = feet.y;
        let new = old + dy;
        for r in (old - SKIN).ceil() as i32..=(new.ceil() as i32 - 1) {
            let blocks = (col0..=col1).any(|c| match map.tile(c, r) {
                TileKind::Solid => true,
                TileKind::OneWay => !drop_through,
                _ => false,
            });
            if blocks {
                feet.y = r as f32;
                return Some(true);
            }
        }
    } else {
        let old = feet.y - size.y;
        let new = old + dy;
        let mut r = (old + SKIN).floor() as i32 - 1;
        while r >= new.floor() as i32 {
            if (col0..=col1).any(|c| map.tile(c, r) == TileKind::Solid) {
                feet.y = (r + 1) as f32 + size.y;
                return Some(false);
            }
            r -= 1;
        }
    }
    feet.y += dy;
    None
}

/// Does the body overlap any tile of `kind`?
pub fn overlaps_kind(map: &MapDef, feet: Vec2, size: Vec2, kind: TileKind) -> bool {
    let (row0, row1) = rows_of(feet, size);
    let (col0, col1) = cols_of(feet, size);
    (row0..=row1).any(|r| (col0..=col1).any(|c| map.tile(c, r) == kind))
}

/// True if the body rests on one-way tiles only (so it can drop through).
pub fn standing_on_one_way(map: &MapDef, feet: Vec2, size: Vec2) -> bool {
    let r = (feet.y + SKIN).floor() as i32;
    if (feet.y - r as f32).abs() > 2.0 * SKIN {
        return false;
    }
    let (col0, col1) = cols_of(feet, size);
    let kinds: Vec<TileKind> = (col0..=col1).map(|c| map.tile(c, r)).collect();
    kinds.contains(&TileKind::OneWay) && !kinds.contains(&TileKind::Solid)
}

/// Distance along `dir` (normalized) to the first solid tile, capped at `max_dist`.
/// One-way platforms do not stop shots.
pub fn raycast(map: &MapDef, origin: Vec2, dir: Vec2, max_dist: f32) -> f32 {
    let mut cell = (origin.x.floor() as i32, origin.y.floor() as i32);
    if map.tile(cell.0, cell.1) == TileKind::Solid {
        return 0.0;
    }
    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let delta_x = if dir.x != 0.0 {
        (1.0 / dir.x).abs()
    } else {
        f32::INFINITY
    };
    let delta_y = if dir.y != 0.0 {
        (1.0 / dir.y).abs()
    } else {
        f32::INFINITY
    };
    let mut next_x = if dir.x > 0.0 {
        (cell.0 as f32 + 1.0 - origin.x) * delta_x
    } else {
        (origin.x - cell.0 as f32) * delta_x
    };
    let mut next_y = if dir.y > 0.0 {
        (cell.1 as f32 + 1.0 - origin.y) * delta_y
    } else {
        (origin.y - cell.1 as f32) * delta_y
    };
    loop {
        let t = if next_x < next_y {
            cell.0 += step_x;
            let t = next_x;
            next_x += delta_x;
            t
        } else {
            cell.1 += step_y;
            let t = next_y;
            next_y += delta_y;
            t
        };
        if t >= max_dist {
            return max_dist;
        }
        if map.tile(cell.0, cell.1) == TileKind::Solid {
            return t;
        }
    }
}

pub fn line_of_sight(map: &MapDef, a: Vec2, b: Vec2) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return true;
    }
    raycast(map, a, d / len, len) >= len - 1e-3
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = Vec2::new(0.7, 1.8);
    const DT: f32 = 1.0 / 60.0;

    fn room() -> MapDef {
        MapDef::from_rows(&[
            "##########",
            "#........#",
            "#........#",
            "#...==...#",
            "#........#",
            "#........#",
            "#....#...#",
            "##########",
        ])
    }

    fn settle(map: &MapDef, feet: &mut Vec2, vel: &mut Vec2, frames: usize) -> MoveResult {
        let mut last = MoveResult::default();
        for _ in 0..frames {
            vel.y += 48.0 * DT;
            last = move_body(map, feet, SIZE, vel, DT, false);
        }
        last
    }

    #[test]
    fn falls_and_lands_exactly_on_floor() {
        let map = room();
        let mut feet = vec2(2.5, 3.0);
        let mut vel = Vec2::ZERO;
        let res = settle(&map, &mut feet, &mut vel, 120);
        assert!(res.landed);
        assert_eq!(feet.y, 7.0);
        assert_eq!(vel.y, 0.0);
    }

    #[test]
    fn walls_stop_horizontal_movement() {
        let map = room();
        let mut feet = vec2(2.0, 7.0);
        let mut vel = vec2(-20.0, 0.0);
        for _ in 0..30 {
            move_body(&map, &mut feet, SIZE, &mut vel, DT, false);
            vel.x = -20.0;
        }
        assert!((feet.x - (1.0 + SIZE.x * 0.5)).abs() < 1e-5, "{feet}");
    }

    #[test]
    fn small_step_blocks_from_the_side() {
        // The single-tile bump at (5, 6) is solid; walking right must stop at it.
        let map = room();
        let mut feet = vec2(3.0, 7.0);
        let mut vel = vec2(10.0, 0.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.5, false);
        assert!(res.hit_wall);
        assert!((feet.x - (5.0 - SIZE.x * 0.5)).abs() < 1e-5);
    }

    #[test]
    fn diagonal_into_corner_resolves_both_axes() {
        let map = room();
        let mut feet = vec2(8.0, 6.0);
        let mut vel = vec2(30.0, 30.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.2, false);
        assert!(res.hit_wall && res.landed);
        assert!((feet.x - (9.0 - SIZE.x * 0.5)).abs() < 1e-5);
        assert_eq!(feet.y, 7.0);
    }

    #[test]
    fn ceiling_stops_upward_movement() {
        let map = room();
        let mut feet = vec2(2.5, 4.0);
        let mut vel = vec2(0.0, -40.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.2, false);
        assert!(res.hit_ceiling);
        assert!((feet.y - (1.0 + SIZE.y)).abs() < 1e-5);
    }

    #[test]
    fn one_way_lets_you_jump_up_through() {
        let map = room();
        let mut feet = vec2(4.5, 7.0);
        let mut vel = vec2(0.0, -12.5);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.2, false);
        assert!(!res.hit_ceiling);
        assert!(
            (feet.y - 4.5).abs() < 1e-4,
            "passed into the platform: {feet}"
        );
    }

    #[test]
    fn one_way_catches_you_from_above_unless_dropping() {
        let map = room();
        let mut feet = vec2(4.5, 2.0);
        let mut vel = vec2(0.0, 10.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.3, false);
        assert!(res.landed);
        assert_eq!(feet.y, 3.0);
        assert!(standing_on_one_way(&map, feet, SIZE));

        let mut vel = vec2(0.0, 10.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.1, true);
        assert!(!res.landed);
        assert!(feet.y > 3.0);
    }

    #[test]
    fn fast_bodies_do_not_tunnel() {
        let map = MapDef::from_rows(&["#####", "#...#", "#...#", "#...#", "#####"]);
        let mut feet = vec2(2.0, 3.5);
        let mut vel = vec2(500.0, 0.0);
        move_body(&map, &mut feet, Vec2::new(0.5, 0.5), &mut vel, 0.5, false);
        assert!((feet.x - 3.75).abs() < 1e-5, "{feet}");
    }

    #[test]
    fn raycast_hits_walls_and_ignores_one_way() {
        let map = room();
        let d = raycast(&map, vec2(1.5, 2.5), vec2(1.0, 0.0), 50.0);
        assert!((d - 7.5).abs() < 1e-5);
        let d = raycast(&map, vec2(4.5, 5.5), vec2(0.0, -1.0), 50.0);
        assert!(
            (d - 4.5).abs() < 1e-5,
            "one-way at row 3 must not stop it: {d}"
        );
        assert!(line_of_sight(&map, vec2(1.5, 1.5), vec2(8.5, 5.5)));
        assert!(!line_of_sight(&map, vec2(4.5, 6.5), vec2(6.5, 6.5)));
    }

    #[test]
    fn aabb_ray_hit() {
        let b = Aabb::from_feet(vec2(5.0, 5.0), SIZE);
        let t = b.ray_hit(vec2(0.0, 4.0), vec2(1.0, 0.0), 100.0).unwrap();
        assert!((t - (5.0 - SIZE.x * 0.5)).abs() < 1e-5);
        assert!(b.ray_hit(vec2(0.0, 1.0), vec2(1.0, 0.0), 100.0).is_none());
    }
}
