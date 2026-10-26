//! Aim assist for touch screens: holding the fire button picks the nearest
//! enemy the player can see and works out where to point, so a thumb that
//! is busy firing does not also have to aim.

use crate::data::WeaponDef;
use crate::sim::{Fighter, World};
use macroquad::math::Vec2;

/// How far past the weapon's listed range assist still looks, as a share.
const REACH: f32 = 1.25;
/// Never less than this many tiles (short-range weapons still see a
/// little way).
const MIN_REACH: f32 = 12.0;
/// A locked target keeps the lock unless another is this much closer (as a
/// share of the locked one's distance), so the aim does not flick about.
const STICKY: f32 = 0.7;
/// Share of a target's movement to lead by: a thumb's guess, not perfect.
const LEAD: f32 = 0.6;

/// The enemy assist is locked on to, kept from tick to tick.
#[derive(Debug, Default, Clone, Copy)]
pub struct AutoAim {
    pub target: Option<usize>,
}

impl AutoAim {
    /// Pick (or keep) a target for fighter `me` and return where to aim, or
    /// `None` if no enemy is in sight and in reach.
    pub fn update(&mut self, world: &World, me: usize) -> Option<Vec2> {
        let f = &world.fighters[me];
        if !f.alive {
            self.target = None;
            return None;
        }
        let def = &world.data.weapons[f.current];
        let reach = (def.range_tiles * REACH).max(MIN_REACH);
        let eye = f.eye();
        let in_sight: Vec<(usize, f32)> = world
            .fighters
            .iter()
            .filter(|o| o.id != me && o.alive && !world.allies(me, o.id))
            .map(|o| (o.id, eye.distance(o.aabb().center())))
            .filter(|&(id, d)| d <= reach && world.can_see(me, world.fighters[id].aabb().center()))
            .collect();
        let nearest = in_sight.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
        let locked = self
            .target
            .and_then(|t| in_sight.iter().copied().find(|&(id, _)| id == t));
        self.target = match (locked, nearest) {
            (Some((t, d)), Some((n, nd))) if nd >= d * STICKY || n == t => Some(t),
            (_, Some((n, _))) => Some(n),
            _ => None,
        };
        self.target.map(|t| aim_at(f, &world.fighters[t], def))
    }
}

/// Where to point to hit `them` with this weapon: their middle, and for
/// shots that fly and fall, a little ahead of them and above, for the drop.
fn aim_at(me: &Fighter, them: &Fighter, def: &WeaponDef) -> Vec2 {
    let eye = me.eye();
    let mut point = them.aabb().center();
    let pd = &def.projectile;
    if pd.kind.is_physical() && pd.speed > 0.0 {
        let flight = eye.distance(point) / pd.speed;
        point += them.vel * flight * LEAD;
        point.y -= 0.5 * def.gravity() * flight * flight;
    }
    point
}
