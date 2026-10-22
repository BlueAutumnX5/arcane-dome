//! Map gadgets (thrusters, weapon/shield stations, the powerup spot),
//! shields, powerup buffs and teleporting.

use super::fighter::*;
use super::world::{Event, give_weapon};
use super::{DT, World};
use crate::data::{Category, GameData, MapDef, MapObject, ObjectKind, PowerupEffect, TileKind};
use macroquad::math::{Vec2, vec2};
use std::sync::Arc;

/// At this much shield (or more) a hit is blocked as hard as it can be.
const SHIELD_FULL_AT: f32 = 50.0;
/// Share of each hit a strong shield absorbs.
const SHIELD_MAX_BLOCK: f32 = 0.7;
/// How close a fighter must be to grab what a station offers.
const TOUCH_RANGE: f32 = 0.35;
const THRUST_COOLDOWN: f32 = 0.4;

/// Split a hit between shield and health: `(to_shield, to_health)`.
///
/// With 50+ shield, 70% of the hit is absorbed, so big hits are blunted.
/// Below 50 the absorbed share slopes down with the shield (25 shield
/// absorbs 35%, 1 shield only 1.4%). Absorbed damage drains the shield.
pub fn absorb(shield: f32, damage: f32) -> (f32, f32) {
    let share = (shield / SHIELD_FULL_AT).clamp(0.0, 1.0) * SHIELD_MAX_BLOCK;
    let absorbed = (damage * share).min(shield.max(0.0));
    (absorbed, damage - absorbed)
}

/// Live state of one map object.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectState {
    pub def: MapObject,
    /// Bottom-center of the object's cell (where a fighter's feet would be).
    pub pos: Vec2,
    /// Something is deployed and can be taken.
    pub ready: bool,
    /// Seconds until the next deployment.
    pub timer: f32,
    /// Length of the current wait, for progress bars.
    pub delay: f32,
    /// Weapon station: the weapon on offer.
    pub weapon: Option<usize>,
    /// Powerup spot: the powerup on offer.
    pub powerup: Option<usize>,
    /// Powerup spot: the one powerup this spot always offers, if fixed.
    pub fixed_powerup: Option<usize>,
}

impl ObjectState {
    pub fn new(def: &MapObject, data: &GameData) -> Self {
        let first_powerup_s = data.powerups.first_spawn_s;
        let fixed_powerup = def
            .powerup
            .as_deref()
            .and_then(|id| data.powerups.powerups.iter().position(|p| p.id == id));
        let (ready, timer) = match def.kind {
            ObjectKind::Thruster | ObjectKind::WeaponStation => (true, 0.0),
            ObjectKind::ShieldStation => (false, def.delay_range().0 * 0.5),
            ObjectKind::PowerupSpot => (false, first_powerup_s),
        };
        Self {
            def: def.clone(),
            pos: vec2(def.x as f32 + 0.5, def.y as f32 + 1.0),
            ready,
            timer,
            delay: timer.max(0.01),
            weapon: None,
            powerup: None,
            fixed_powerup,
        }
    }

    /// 0 right after being taken, 1 when ready.
    pub fn progress(&self) -> f32 {
        if self.ready {
            1.0
        } else {
            (1.0 - self.timer / self.delay).clamp(0.0, 1.0)
        }
    }

    /// Where the offered item floats.
    pub fn item_pos(&self) -> Vec2 {
        self.pos - vec2(0.0, 0.55)
    }
}

/// Every cell a fighter can stand in (as feet positions), except thruster pads.
pub fn stand_cells(map: &MapDef) -> Vec<Vec2> {
    let pads: Vec<(i32, i32)> = map
        .objects
        .iter()
        .filter(|o| o.kind == ObjectKind::Thruster)
        .map(|o| (o.x, o.y))
        .collect();
    let mut out = Vec::new();
    for y in 1..map.height as i32 {
        for x in 0..map.width as i32 {
            let standable = map.tile(x, y) == TileKind::Air
                && map.tile(x, y - 1) == TileKind::Air
                && map.tile(x, y + 1).is_floor();
            if standable && !pads.contains(&(x, y)) {
                out.push(vec2(x as f32 + 0.5, y as f32 + 1.0));
            }
        }
    }
    out
}

impl World {
    fn buff_effects(&self, i: usize) -> impl Iterator<Item = PowerupEffect> + '_ {
        self.fighters[i]
            .buffs
            .iter()
            .filter_map(|b| self.data.powerups.powerups.get(b.powerup))
            .map(|p| p.effect)
    }

    pub fn damage_mult(&self, i: usize) -> f32 {
        self.buff_effects(i)
            .map(|e| match e {
                PowerupEffect::Damage { mult } => mult,
                _ => 1.0,
            })
            .product()
    }

    pub fn fire_rate_mult(&self, i: usize) -> f32 {
        self.buff_effects(i)
            .map(|e| match e {
                PowerupEffect::FireRate { mult } => mult,
                _ => 1.0,
            })
            .product()
    }

    pub fn is_invulnerable(&self, i: usize) -> bool {
        self.buff_effects(i)
            .any(|e| matches!(e, PowerupEffect::Invulnerable))
    }

    pub(crate) fn update_buffs(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        for b in &mut f.buffs {
            b.time_left -= DT;
        }
        f.buffs.retain(|b| b.time_left > 0.0);
    }

    /// Launch a fighter touching a thruster pad straight up.
    pub(crate) fn check_thrusters(&mut self, i: usize) {
        let f = &self.fighters[i];
        if !f.alive || f.thrust_cooldown > 0.0 || f.vel.y < -1.0 {
            return;
        }
        let body = f.aabb();
        let pad = self.objects.iter().find(|o| {
            if o.def.kind != ObjectKind::Thruster {
                return false;
            }
            let (x, y) = (o.def.x as f32, o.def.y as f32);
            body.min.x < x + 0.9
                && body.max.x > x + 0.1
                && body.max.y > y + 0.6
                && body.min.y < y + 1.0
        });
        let Some(pad) = pad else { return };
        let height = pad.def.launch_height();
        let pos = pad.pos;
        let f = &mut self.fighters[i];
        f.vel.y = -(2.0 * GRAVITY * (height + 0.4)).sqrt();
        f.on_ground = false;
        f.crouching = false;
        f.coyote = 0.0;
        f.air_jumps = 1;
        f.jump_rising = false;
        f.thrust_cooldown = THRUST_COOLDOWN;
        self.events.push(Event::Thrust { fighter: i, pos });
    }

    /// Spend a held teleport charge, if any.
    pub(crate) fn use_ability(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        if !f.alive || f.teleport_charges == 0 {
            return;
        }
        f.teleport_charges -= 1;
        if f.teleport_charges == 0 {
            f.teleport_powerup = None;
        }
        self.teleport(i);
    }

    /// Jump to a random platform, preferring spots far from here and from enemies.
    pub fn teleport(&mut self, i: usize) {
        if self.stand_cells.is_empty() {
            return;
        }
        let from = self.fighters[i].feet;
        let enemies: Vec<Vec2> = self
            .fighters
            .iter()
            .filter(|o| o.alive && !self.allies(i, o.id))
            .map(|o| o.feet)
            .collect();
        let good: Vec<Vec2> = self
            .stand_cells
            .iter()
            .copied()
            .filter(|c| c.distance(from) > 10.0 && enemies.iter().all(|e| e.distance(*c) > 5.0))
            .collect();
        let pool = if good.is_empty() {
            self.stand_cells.clone()
        } else {
            good
        };
        let to = pool[self.rng.below(pool.len())];
        let f = &mut self.fighters[i];
        f.feet = to;
        f.vel = Vec2::ZERO;
        f.on_ground = false;
        f.crouching = false;
        self.events.push(Event::Teleport {
            fighter: i,
            from,
            to,
        });
    }

    pub(crate) fn update_objects(&mut self) {
        let data = Arc::clone(&self.data);
        let extras: Vec<usize> = (0..data.weapons.len())
            .filter(|&w| data.weapons[w].category != Category::Starter)
            .collect();
        for k in 0..self.objects.len() {
            let kind = self.objects[k].def.kind;
            if kind == ObjectKind::Thruster {
                continue;
            }
            if !self.objects[k].ready {
                let o = &mut self.objects[k];
                o.timer -= DT;
                if o.timer > 0.0 {
                    continue;
                }
                o.ready = true;
                self.events.push(Event::Deployed { object: k });
            }
            // Roll what is on offer the first time it is needed.
            match kind {
                ObjectKind::WeaponStation if self.objects[k].weapon.is_none() => {
                    if extras.is_empty() {
                        continue;
                    }
                    let w = extras[self.rng.below(extras.len())];
                    self.objects[k].weapon = Some(w);
                }
                ObjectKind::PowerupSpot if self.objects[k].powerup.is_none() => {
                    let n = data.powerups.powerups.len();
                    if n == 0 {
                        continue;
                    }
                    let p = match self.objects[k].fixed_powerup {
                        Some(p) => p,
                        None => self.rng.below(n),
                    };
                    self.objects[k].powerup = Some(p);
                }
                _ => {}
            }

            let item = self.objects[k].item_pos();
            for i in 0..self.fighters.len() {
                let f = &self.fighters[i];
                if !f.alive || !f.aabb().expand(TOUCH_RANGE).contains(item) {
                    continue;
                }
                if self.take_object(i, k) {
                    let delay = if kind == ObjectKind::PowerupSpot {
                        data.powerups.respawn_s
                    } else {
                        let (lo, hi) = self.objects[k].def.delay_range();
                        self.rng.range(lo, hi)
                    };
                    let o = &mut self.objects[k];
                    o.ready = false;
                    o.weapon = None;
                    o.powerup = None;
                    o.timer = delay;
                    o.delay = delay.max(0.01);
                    self.events.push(Event::Taken {
                        object: k,
                        fighter: i,
                    });
                    break;
                }
            }
        }
    }

    /// Give fighter `i` what object `k` offers. False if it would be wasted.
    fn take_object(&mut self, i: usize, k: usize) -> bool {
        let data = Arc::clone(&self.data);
        let o = &self.objects[k];
        match o.def.kind {
            ObjectKind::WeaponStation => match o.weapon {
                Some(w) => give_weapon(&mut self.fighters[i], &data.weapons[w], w),
                None => false,
            },
            ObjectKind::ShieldStation => {
                let amount = o.def.shield_amount();
                let f = &mut self.fighters[i];
                if f.shield >= MAX_SHIELD {
                    return false;
                }
                f.shield = (f.shield + amount).min(MAX_SHIELD);
                true
            }
            ObjectKind::PowerupSpot => match o.powerup {
                Some(p) => {
                    self.apply_powerup(i, p);
                    true
                }
                None => false,
            },
            ObjectKind::Thruster => false,
        }
    }

    pub fn apply_powerup(&mut self, i: usize, p: usize) {
        let data = Arc::clone(&self.data);
        let def = &data.powerups.powerups[p];
        match def.effect {
            PowerupEffect::Teleport {
                on_pickup: true, ..
            } => self.teleport(i),
            PowerupEffect::Teleport { charges, .. } => {
                let f = &mut self.fighters[i];
                f.teleport_charges += charges;
                f.teleport_powerup = Some(p);
            }
            _ => {
                let f = &mut self.fighters[i];
                match f.buffs.iter_mut().find(|b| b.powerup == p) {
                    Some(b) => b.time_left = def.duration_s,
                    None => f.buffs.push(Buff {
                        powerup: p,
                        time_left: def.duration_s,
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::absorb;

    #[test]
    fn strong_shields_block_most_of_a_big_hit() {
        let (s, h) = absorb(100.0, 90.0);
        assert!((s - 63.0).abs() < 1e-4 && (h - 27.0).abs() < 1e-4);
    }

    #[test]
    fn weak_shields_block_less() {
        let (s, _) = absorb(25.0, 20.0);
        assert!((s - 7.0).abs() < 1e-4);
        let (s, h) = absorb(1.0, 20.0);
        assert!(s < 0.3 && h > 19.7);
    }

    #[test]
    fn shield_cannot_absorb_more_than_it_has() {
        let (s, h) = absorb(10.0, 1000.0);
        assert_eq!(s, 10.0);
        assert_eq!(h, 990.0);
    }
}
