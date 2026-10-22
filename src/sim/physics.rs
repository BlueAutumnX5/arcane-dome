//! Bodies moving through freeform polygon terrain.
//!
//! A body is an axis-aligned box described by its feet (bottom-centre) and
//! size. It senses terrain with short rays against polygon *boundary* edges:
//!
//! - feet rays (left, centre, right) find the floor, so slopes of any
//!   walkable angle just work and small lips are stepped onto;
//! - wall rays along the leading side stop horizontal movement at steep
//!   surfaces (above step height while walking, full height while airborne);
//! - head rays stop upward movement at ceilings.
//!
//! Rays ignore edges facing away from them, so a body that somehow ends up
//! inside a shape can always move back out. Hatches only act as floors, and
//! only for bodies that were above them.

use crate::data::MapDef;
use crate::geo::ShapeKind;
use macroquad::math::{Vec2, vec2};

/// Gap kept between a body and walls.
pub const SKIN: f32 = 0.02;
/// Largest distance a body moves per sub-step.
const MAX_STEP: f32 = 0.2;
/// Height a walking body steps up onto without jumping.
const STEP_UP: f32 = 0.35;
/// How far down a walking body is pulled to stay on a downward slope.
const STICK_DIST: f32 = 0.45;
/// Feet rays sit this far in from the body's sides.
const FEET_INSET: f32 = 0.06;
/// Rise per unit of run of the steepest walkable slope (just above what the
/// walkable normal limit allows).
const MAX_SLOPE: f32 = 1.6;

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

/// Move a body by `vel * dt` through the terrain. Blocked velocity
/// components are zeroed.
///
/// `stick` means "was standing on the ground": the body steps up small lips
/// and stays glued to downward slopes. `drop_through` lets it fall through
/// hatches.
pub fn move_body(
    map: &MapDef,
    feet: &mut Vec2,
    size: Vec2,
    vel: &mut Vec2,
    dt: f32,
    drop_through: bool,
    stick: bool,
) -> MoveResult {
    let delta = *vel * dt;
    let steps = ((delta.x.abs().max(delta.y.abs()) / MAX_STEP).ceil() as i32).max(1);
    let step = delta / steps as f32;
    let mut res = MoveResult::default();
    let mut grounded = stick;

    for _ in 0..steps {
        // Horizontal.
        if step.x != 0.0 && !res.hit_wall {
            let dir = step.x.signum();
            let want = step.x.abs();
            match wall_distance(map, *feet, size, dir, want + SKIN, grounded) {
                Some(d) if d - SKIN < want => {
                    // A negative distance means the side already overlaps the
                    // wall (say, a floor corner met while dropping through a
                    // hatch): this pushes the body back out.
                    feet.x += dir * (d - SKIN);
                    res.hit_wall = true;
                    vel.x = 0.0;
                }
                _ => feet.x += step.x,
            }
            // Moving sideways into a rising slope (even in the air, say after
            // a knock-back) must ride up onto it rather than sink in.
            let climb = step.x.abs() * MAX_SLOPE + SKIN;
            if let Some(y) = floor_at(map, *feet, size, climb, climb, feet.y, drop_through)
                && y < feet.y - 1e-4
            {
                feet.y = y;
                if vel.y > 0.0 {
                    vel.y = 0.0;
                }
            }
        }

        // Vertical.
        if step.y < 0.0 && !res.hit_ceiling {
            let want = -step.y;
            match ceiling_distance(map, *feet, size, want + SKIN) {
                Some(d) if d - SKIN < want => {
                    feet.y -= (d - SKIN).max(0.0);
                    res.hit_ceiling = true;
                    vel.y = 0.0;
                }
                _ => feet.y += step.y,
            }
            grounded = false;
        } else if step.y >= 0.0 {
            let prev = feet.y;
            // Walking bodies look a little above their feet to step onto lips.
            let lift = if grounded { STEP_UP } else { SKIN };
            let reach = step.y + lift;
            match floor_at(map, *feet, size, lift, reach, prev, drop_through) {
                Some(y) if y <= feet.y + step.y + 1e-4 => {
                    feet.y = y;
                    vel.y = vel.y.min(0.0);
                    res.landed = true;
                    grounded = true;
                }
                _ => {
                    feet.y += step.y;
                    grounded = false;
                }
            }
        }
    }

    // Stay glued to the ground when walking down slopes or off small drops.
    if stick && !res.landed && vel.y >= 0.0 {
        let prev = feet.y;
        if let Some(y) = floor_at(map, *feet, size, SKIN, STICK_DIST, prev, drop_through) {
            feet.y = y;
            vel.y = 0.0;
            res.landed = true;
        }
    }
    res
}

/// Distance from the body's side to the nearest wall in direction `dir`
/// (+1 / -1). Rays start at the body's centre, so a wall already inside the
/// body gives a negative distance instead of being missed.
fn wall_distance(
    map: &MapDef,
    feet: Vec2,
    size: Vec2,
    dir: f32,
    max: f32,
    grounded: bool,
) -> Option<f32> {
    let half = size.x * 0.5;
    let bottom = if grounded {
        feet.y - STEP_UP - 0.01
    } else {
        feet.y - SKIN
    };
    let top = feet.y - size.y + SKIN;
    let rays = 4;
    (0..rays)
        .filter_map(|i| {
            let y = bottom + (top - bottom) * i as f32 / (rays - 1) as f32;
            map.terrain
                .cast(vec2(feet.x, y), vec2(dir, 0.0), half + max, |e| {
                    e.kind == ShapeKind::Solid && !e.walkable()
                })
                .map(|(t, _)| t - half)
        })
        .reduce(f32::min)
}

/// Distance to the nearest ceiling above the head.
fn ceiling_distance(map: &MapDef, feet: Vec2, size: Vec2, max: f32) -> Option<f32> {
    // Nearly the full width (unlike the feet), so a body rising past the
    // corner of a floor cannot clip into it.
    let half = size.x * 0.5 - SKIN * 0.5;
    let head = feet.y - size.y;
    [-half, 0.0, half]
        .into_iter()
        .filter_map(|dx| {
            map.terrain
                .cast(vec2(feet.x + dx, head), vec2(0.0, -1.0), max, |e| {
                    e.kind == ShapeKind::Solid
                })
                .map(|(t, _)| t)
        })
        .reduce(f32::min)
}

/// The highest floor under the body's feet rays, searching from `lift`
/// above the feet down to `reach` below that point. Hatches count only if
/// the body was above them (`prev` feet height) and is not dropping.
fn floor_at(
    map: &MapDef,
    feet: Vec2,
    size: Vec2,
    lift: f32,
    reach: f32,
    prev: f32,
    drop_through: bool,
) -> Option<f32> {
    let half = size.x * 0.5 - FEET_INSET;
    let from_y = feet.y - lift;
    let mut best: Option<f32> = None;
    for dx in [-half, 0.0, half] {
        let origin = vec2(feet.x + dx, from_y);
        let solid = map
            .terrain
            .cast(origin, vec2(0.0, 1.0), reach, |e| {
                e.kind == ShapeKind::Solid && e.walkable()
            })
            .map(|(t, _)| from_y + t);
        let hatch = if drop_through {
            None
        } else {
            map.terrain
                .cast(origin, vec2(0.0, 1.0), reach, |e| {
                    e.kind == ShapeKind::Hatch && e.walkable()
                })
                .map(|(t, _)| from_y + t)
                .filter(|&y| y >= prev - 0.05)
        };
        for y in [solid, hatch].into_iter().flatten() {
            if best.is_none_or(|b| y < b) {
                best = Some(y);
            }
        }
    }
    best
}

/// Does the body overlap solid terrain? (A hair of slack so standing on a
/// floor or touching a wall does not count.)
pub fn overlaps_solid(map: &MapDef, feet: Vec2, size: Vec2) -> bool {
    let b = Aabb::from_feet(feet, size);
    map.terrain
        .box_blocked(b.min + vec2(0.03, 0.03), b.max - vec2(0.03, 0.03))
}

/// Is the body in water?
pub fn in_water(map: &MapDef, feet: Vec2, size: Vec2) -> bool {
    let b = Aabb::from_feet(feet, size);
    map.terrain.box_touches(b.min, b.max, ShapeKind::Water)
}

/// Damage per second from hazards the body touches.
pub fn hazard_dps(map: &MapDef, feet: Vec2, size: Vec2) -> f32 {
    let b = Aabb::from_feet(feet, size);
    map.terrain.hazard_dps(b.min, b.max)
}

/// True if the body stands on a hatch (and not also on solid ground), so it
/// can drop through.
pub fn standing_on_hatch(map: &MapDef, feet: Vec2, size: Vec2) -> bool {
    let probe = |hatch: bool| {
        let half = size.x * 0.5 - FEET_INSET;
        [-half, 0.0, half].into_iter().any(|dx| {
            let o = vec2(feet.x + dx, feet.y - 0.05);
            map.terrain
                .cast(o, vec2(0.0, 1.0), 0.12, |e| {
                    e.walkable() && (e.kind == ShapeKind::Hatch) == hatch
                })
                .is_some()
        })
    };
    probe(true) && !probe(false)
}

/// Distance along `dir` (normalized) to the first solid surface, capped at
/// `max_dist`. Hatches do not stop shots.
pub fn raycast(map: &MapDef, origin: Vec2, dir: Vec2, max_dist: f32) -> f32 {
    map.terrain.raycast_solid(origin, dir, max_dist)
}

pub fn line_of_sight(map: &MapDef, a: Vec2, b: Vec2) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return true;
    }
    raycast(map, a, d / len, len) >= len - 1e-3
}

/// Is this point inside solid terrain?
pub fn point_blocked(map: &MapDef, p: Vec2) -> bool {
    map.terrain.point_solid(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::{Material, Shape};

    const SIZE: Vec2 = Vec2::new(0.7, 1.8);
    const DT: f32 = 1.0 / 60.0;
    const G: f32 = 48.0;

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

    /// Simulate like a fighter: gravity, constant horizontal speed, sticking.
    fn run(map: &MapDef, feet: &mut Vec2, vx: f32, frames: usize) -> Vec<MoveResult> {
        let mut vel = Vec2::ZERO;
        let mut on_ground = false;
        let mut out = Vec::new();
        for _ in 0..frames {
            vel.x = vx;
            vel.y += G * DT;
            let res = move_body(map, feet, SIZE, &mut vel, DT, false, on_ground);
            on_ground = res.landed;
            out.push(res);
        }
        out
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn falls_and_lands_on_the_floor() {
        let map = room();
        let mut feet = vec2(2.5, 3.0);
        let res = run(&map, &mut feet, 0.0, 120);
        assert!(res.last().unwrap().landed);
        assert!(approx(feet.y, 7.0), "{feet}");
    }

    #[test]
    fn walls_stop_horizontal_movement() {
        let map = room();
        let mut feet = vec2(2.5, 7.0);
        let res = run(&map, &mut feet, -20.0, 30);
        assert!(res.iter().any(|r| r.hit_wall));
        assert!(approx(feet.x, 1.0 + SIZE.x * 0.5 + SKIN), "{feet}");
    }

    #[test]
    fn one_tile_bump_is_a_wall_not_a_step() {
        // The bump at column 5 is a full tile high, above STEP_UP.
        let map = room();
        let mut feet = vec2(3.0, 7.0);
        run(&map, &mut feet, 8.0, 60);
        assert!(feet.x < 5.0 - SIZE.x * 0.5 + 0.01, "{feet}");
        assert!(approx(feet.y, 7.0));
    }

    #[test]
    fn ceiling_stops_upward_movement() {
        let map = room();
        let mut feet = vec2(2.5, 4.0);
        let mut vel = vec2(0.0, -40.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.2, false, false);
        assert!(res.hit_ceiling);
        assert!(approx(feet.y - SIZE.y, 1.0 + SKIN), "{feet}");
    }

    #[test]
    fn hatches_let_you_fly_up_through_then_stand_on_them() {
        let map = room();
        let mut feet = vec2(4.5, 7.0);
        // Rise 4.1 tiles: past the hatch top (y=3) but short of the ceiling.
        let mut vel = vec2(0.0, -16.4);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.25, false, false);
        assert!(!res.hit_ceiling, "hatch is open from below");
        assert!(feet.y < 3.0, "flew up past the hatch: {feet}");
        // Now fall back down onto it.
        let res = run(&map, &mut feet, 0.0, 60);
        assert!(res.last().unwrap().landed);
        assert!(approx(feet.y, 3.0), "standing on the hatch: {feet}");
        assert!(standing_on_hatch(&map, feet, SIZE));
        // Dropping through.
        let mut vel = vec2(0.0, 5.0);
        let res = move_body(&map, &mut feet, SIZE, &mut vel, 0.1, true, true);
        assert!(!res.landed && feet.y > 3.0, "{feet}");
    }

    #[test]
    fn fast_bodies_do_not_tunnel() {
        let map = MapDef::from_rows(&["#####", "#...#", "#...#", "#...#", "#####"]);
        let mut feet = vec2(2.0, 4.0);
        let mut vel = vec2(500.0, 0.0);
        move_body(
            &map,
            &mut feet,
            Vec2::new(0.5, 0.5),
            &mut vel,
            0.5,
            false,
            true,
        );
        assert!(feet.x <= 4.0 - 0.25 + 1e-3, "{feet}");
    }

    fn hill() -> MapDef {
        // Flat ground with a 30-degree-ish ramp up to a plateau.
        let shapes = vec![
            Shape {
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
            },
            Shape {
                kind: ShapeKind::Solid,
                material: Material::Rock,
                points: vec![
                    vec2(2.0, 4.0),
                    vec2(5.0, 4.0),
                    vec2(5.0, 5.0),
                    vec2(2.0, 5.0),
                ],
                dps: 0.0,
            },
        ];
        MapDef::from_shapes(shapes, 20.0, 12.0)
    }

    #[test]
    fn walks_up_a_slope_onto_the_plateau() {
        let map = hill();
        let mut feet = vec2(2.0, 10.0);
        let res = run(&map, &mut feet, 7.0, 150);
        assert!(feet.x > 15.0, "{feet}");
        assert!(approx(feet.y, 7.0), "{feet}");
        assert!(
            res.iter().skip(5).all(|r| r.landed),
            "never left the ground"
        );
    }

    #[test]
    fn walks_down_a_slope_without_hopping() {
        let map = hill();
        let mut feet = vec2(16.0, 7.0);
        let res = run(&map, &mut feet, -7.0, 150);
        assert!(feet.x < 4.0, "{feet}");
        assert!(approx(feet.y, 10.0), "{feet}");
        assert!(res.iter().skip(2).all(|r| r.landed), "stayed on the slope");
    }

    #[test]
    fn mid_slope_height_follows_the_surface() {
        let map = hill();
        let mut feet = vec2(9.0, 6.0);
        run(&map, &mut feet, 0.0, 60);
        // Surface at x=9 is y=8.5; the uphill feet ray holds the body a bit higher.
        assert!(feet.y <= 8.5 + 1e-3 && feet.y > 8.2, "{feet}");
    }

    #[test]
    fn raycasts_hit_walls_and_ignore_hatches() {
        let map = room();
        let d = raycast(&map, vec2(1.5, 2.5), vec2(1.0, 0.0), 50.0);
        assert!(approx(d, 7.5), "{d}");
        let d = raycast(&map, vec2(4.5, 5.5), vec2(0.0, -1.0), 50.0);
        assert!(approx(d, 4.5), "hatch at row 3 must not stop it: {d}");
        assert!(line_of_sight(&map, vec2(1.5, 1.5), vec2(8.5, 5.5)));
        assert!(!line_of_sight(&map, vec2(4.5, 6.5), vec2(6.5, 6.5)));
    }

    #[test]
    fn aabb_ray_hit() {
        let b = Aabb::from_feet(vec2(5.0, 5.0), SIZE);
        let t = b.ray_hit(vec2(0.0, 4.0), vec2(1.0, 0.0), 100.0).unwrap();
        assert!(approx(t, 5.0 - SIZE.x * 0.5));
        assert!(b.ray_hit(vec2(0.0, 1.0), vec2(1.0, 0.0), 100.0).is_none());
    }
}
