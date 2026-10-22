use super::fighter::*;
use super::physics::{line_of_sight, raycast};
use super::world::Event;
use super::{DT, Input, Phase, World};
use crate::data::{Falloff, ProjectileKind, Special, TileKind, WeaponDef};
use macroquad::math::{Vec2, vec2};
use std::sync::Arc;

/// A moving shot: bullet, grenade or rocket.
#[derive(Debug, Clone, PartialEq)]
pub struct Projectile {
    pub owner: usize,
    pub weapon: usize,
    pub kind: ProjectileKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub travelled: f32,
    pub bounces_left: u32,
    pub pierce_left: u32,
    pub hit: Vec<usize>,
    pub alive: bool,
}

/// Credit a kill to whoever damaged the victim within this many seconds
/// (covers deaths by spikes or burning after the shooter is gone).
const ASSIST_WINDOW: f32 = 5.0;
const MELEE_HALF_ARC: f32 = 60.0;
const SELF_DAMAGE: f32 = 0.5;
/// Max projectile movement per sub-step, in tiles.
const PROJECTILE_STEP: f32 = 0.2;

impl World {
    pub(crate) fn handle_weapons(&mut self, i: usize, input: &Input) {
        let data = Arc::clone(&self.data);
        let n = data.weapons.len();
        let f = &mut self.fighters[i];

        if let Some(s) = input.select
            && s < n
            && f.owns(s)
            && s != f.current
        {
            switch_to(f, s);
        }
        if input.cycle != 0 {
            let dir = input.cycle.signum();
            let mut w = f.current as i32;
            for _ in 0..n {
                w = (w + dir).rem_euclid(n as i32);
                if f.owns(w as usize) {
                    break;
                }
            }
            if w as usize != f.current {
                switch_to(f, w as usize);
            }
        }

        let def = &data.weapons[f.current];
        if f.reload_timer > 0.0 {
            f.reload_timer -= DT;
            if f.reload_timer <= 0.0 {
                finish_reload(f, def);
            }
        }
        if input.reload {
            start_reload(f, def);
        }
        if input.fire {
            self.try_fire(i);
        }
    }

    fn try_fire(&mut self, i: usize) {
        let data = Arc::clone(&self.data);
        let f = &mut self.fighters[i];
        let w = f.current;
        let def = &data.weapons[w];
        if f.fire_cooldown > 0.0 || f.is_reloading() {
            return;
        }
        if def.uses_ammo() {
            if f.inventory[w].mag == 0 {
                if !start_reload(f, def)
                    && let Some(other) = best_loaded_weapon(f, &data.weapons)
                {
                    switch_to(f, other);
                }
                return;
            }
            f.inventory[w].mag -= 1;
        }
        f.fire_cooldown += 1.0 / def.fire_rate_rps;

        let origin = f.eye();
        let base = f.aim.y.atan2(f.aim.x);
        let spread = (def.spread_deg + f.bloom).to_radians();
        f.bloom = (f.bloom + def.recoil * 2.0).min(MAX_BLOOM_DEG);
        if def.projectile.kind == ProjectileKind::Beam {
            f.beam_age = 0.0;
        }
        let owner_vel = f.vel;
        self.events.push(Event::Shot {
            fighter: i,
            weapon: w,
        });

        let kind = def.projectile.kind;
        if kind == ProjectileKind::MeleeArc {
            self.melee(i, w, origin);
            return;
        }
        for _ in 0..def.total_pellets() {
            let a = base + self.rng.range(-0.5, 0.5) * spread;
            let dir = vec2(a.cos(), a.sin());
            match kind {
                ProjectileKind::Hitscan | ProjectileKind::Beam => self.hitscan(i, w, origin, dir),
                _ => {
                    let inherit = if kind == ProjectileKind::Grenade {
                        owner_vel * 0.4
                    } else {
                        Vec2::ZERO
                    };
                    self.projectiles.push(Projectile {
                        owner: i,
                        weapon: w,
                        kind,
                        pos: origin,
                        vel: dir * def.projectile.speed + inherit,
                        life: def.projectile.lifetime_s,
                        travelled: 0.0,
                        bounces_left: def.projectile.bounces,
                        pierce_left: def.projectile.pierce,
                        hit: Vec::new(),
                        alive: true,
                    });
                }
            }
        }
    }

    /// Instant ray: stops at the first wall, passes through up to `pierce` bodies.
    fn hitscan(&mut self, i: usize, w: usize, origin: Vec2, dir: Vec2) {
        let data = Arc::clone(&self.data);
        let def = &data.weapons[w];
        let wall = raycast(&self.map, origin, dir, def.range_tiles);
        let mut hits: Vec<(f32, usize)> = self
            .fighters
            .iter()
            .filter(|o| o.alive && o.id != i && !self.allies(i, o.id))
            .filter_map(|o| o.aabb().ray_hit(origin, dir, wall).map(|t| (t, o.id)))
            .collect();
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        let max_hits = def.projectile.pierce as usize + 1;
        let end_t = if hits.len() >= max_hits {
            hits[max_hits - 1].0
        } else {
            wall
        };
        for &(t, j) in hits.iter().take(max_hits) {
            let point = origin + dir * t;
            let headshot = self.fighters[j].is_headshot_point(point);
            let dmg = def.damage * if headshot { def.headshot_mult } else { 1.0 };
            self.damage(j, Some(i), Some(w), dmg, headshot, true);
        }
        self.events.push(Event::Tracer {
            from: origin,
            to: origin + dir * end_t,
            kind: def.projectile.kind,
        });
    }

    fn melee(&mut self, i: usize, w: usize, origin: Vec2) {
        let data = Arc::clone(&self.data);
        let def = &data.weapons[w];
        let aim = self.fighters[i].aim;
        let targets: Vec<usize> = self
            .fighters
            .iter()
            .filter(|o| o.alive && o.id != i && !self.allies(i, o.id))
            .filter(|o| {
                let d = o.center() - origin;
                d.length() <= def.range_tiles
                    && aim.angle_between(d).to_degrees().abs() <= MELEE_HALF_ARC
                    && line_of_sight(&self.map, origin, o.center())
            })
            .map(|o| o.id)
            .collect();
        for j in targets {
            self.damage(j, Some(i), Some(w), def.damage, false, true);
        }
        self.events.push(Event::Tracer {
            from: origin,
            to: origin + aim * def.range_tiles,
            kind: ProjectileKind::MeleeArc,
        });
    }

    pub(crate) fn update_projectiles(&mut self) {
        let data = Arc::clone(&self.data);
        let mut projectiles = std::mem::take(&mut self.projectiles);
        for p in projectiles.iter_mut() {
            self.step_projectile(p, &data.weapons[p.weapon]);
        }
        projectiles.retain(|p| p.alive);
        projectiles.append(&mut self.projectiles);
        self.projectiles = projectiles;
    }

    fn step_projectile(&mut self, p: &mut Projectile, def: &WeaponDef) {
        let pd = &def.projectile;
        p.life -= DT;
        if pd.homing_strength > 0.0
            && let Some(target) = self.homing_target(p)
        {
            let speed = p.vel.length();
            let cur = p.vel / speed.max(1e-4);
            let want = (target - p.pos).normalize_or(cur);
            let turn = (pd.homing_strength * DT).min(1.0);
            p.vel = (cur + (want - cur) * turn).normalize_or(cur) * speed;
        }

        let steps = ((p.vel.length() * DT / PROJECTILE_STEP).ceil() as i32).max(1);
        let sub_dt = DT / steps as f32;
        for _ in 0..steps {
            p.vel.y += pd.gravity * sub_dt;
            let next = p.pos + p.vel * sub_dt;
            if self.solid_at(next) {
                if p.bounces_left > 0 {
                    let hit_x = self.solid_at(vec2(next.x, p.pos.y));
                    let hit_y = self.solid_at(vec2(p.pos.x, next.y));
                    if hit_x {
                        p.vel.x = -p.vel.x * 0.6;
                    }
                    if hit_y {
                        p.vel.y = -p.vel.y * 0.5;
                        p.vel.x *= 0.8;
                    }
                    if !hit_x && !hit_y {
                        p.vel = -p.vel * 0.5;
                    }
                    // Gentle rolling bounces do not use up the bounce budget.
                    if p.vel.length() > 4.0 {
                        p.bounces_left -= 1;
                    }
                    continue;
                }
                self.end_projectile(p, def);
                return;
            }
            p.travelled += (next - p.pos).length();
            p.pos = next;

            for j in 0..self.fighters.len() {
                let o = &self.fighters[j];
                if !o.alive
                    || j == p.owner
                    || p.hit.contains(&j)
                    || self.allies(p.owner, j)
                    || !o.aabb().expand(0.12).contains(p.pos)
                {
                    continue;
                }
                let headshot = o.is_headshot_point(p.pos);
                let dmg = def.damage * if headshot { def.headshot_mult } else { 1.0 };
                self.damage(j, Some(p.owner), Some(p.weapon), dmg, headshot, true);
                if pd.explosion.is_some() {
                    self.end_projectile(p, def);
                    return;
                }
                if p.pierce_left > 0 {
                    p.pierce_left -= 1;
                    p.hit.push(j);
                } else {
                    p.alive = false;
                    return;
                }
            }
        }
        let out_of_range = p.kind == ProjectileKind::Bullet && p.travelled >= def.range_tiles;
        if p.life <= 0.0 || out_of_range {
            self.end_projectile(p, def);
        }
    }

    fn solid_at(&self, p: Vec2) -> bool {
        self.map.tile(p.x.floor() as i32, p.y.floor() as i32) == TileKind::Solid
    }

    fn end_projectile(&mut self, p: &mut Projectile, def: &WeaponDef) {
        p.alive = false;
        if def.projectile.explosion.is_some() {
            self.explode(p.pos, p.owner, p.weapon);
        }
    }

    fn homing_target(&self, p: &Projectile) -> Option<Vec2> {
        let dir = p.vel.normalize_or_zero();
        self.fighters
            .iter()
            .filter(|o| o.alive && o.id != p.owner && !self.allies(p.owner, o.id))
            .map(|o| o.center())
            .filter(|c| {
                let d = *c - p.pos;
                d.length() < 20.0 && d.normalize_or_zero().dot(dir) > 0.3
            })
            .min_by(|a, b| a.distance(p.pos).total_cmp(&b.distance(p.pos)))
    }

    /// Splash damage and knockback. Walls block splash; the owner takes half.
    pub fn explode(&mut self, pos: Vec2, owner: usize, weapon: usize) {
        let data = Arc::clone(&self.data);
        let Some(ex) = &data.weapons[weapon].projectile.explosion else {
            return;
        };
        self.events.push(Event::Explosion {
            pos,
            radius: ex.radius_tiles,
        });
        for j in 0..self.fighters.len() {
            let o = &self.fighters[j];
            if !o.alive {
                continue;
            }
            let body = o.aabb();
            let d = body.distance_to(pos);
            if d > ex.radius_tiles || (d > 0.5 && !line_of_sight(&self.map, pos, body.center())) {
                continue;
            }
            let is_self = j == owner;
            if !is_self && self.allies(owner, j) {
                continue;
            }
            let strength = match ex.falloff {
                Falloff::Linear => 1.0 - d / ex.radius_tiles,
                Falloff::None => 1.0,
            };
            let push = ((body.center() - pos).normalize_or(vec2(0.0, -1.0)) + vec2(0.0, -0.35))
                .normalize_or(vec2(0.0, -1.0));
            let o = &mut self.fighters[j];
            o.vel += push * ex.knockback * strength;
            o.jump_rising = false;
            let dmg = ex.damage * strength * if is_self { SELF_DAMAGE } else { 1.0 };
            if dmg > 0.0 {
                self.damage(j, Some(owner), Some(weapon), dmg, false, true);
            }
        }
    }

    /// Apply damage, on-hit specials and death. Friendly fire is off in team modes.
    pub fn damage(
        &mut self,
        victim: usize,
        attacker: Option<usize>,
        weapon: Option<usize>,
        amount: f32,
        headshot: bool,
        specials: bool,
    ) {
        if matches!(self.phase, Phase::MatchOver { .. }) {
            return;
        }
        let v = &self.fighters[victim];
        if !v.alive || v.spawn_protect > 0.0 {
            return;
        }
        if let Some(a) = attacker
            && a != victim
            && self.allies(a, victim)
        {
            return;
        }
        let data = Arc::clone(&self.data);
        let on_hit: &[Special] = match (specials, weapon) {
            (true, Some(w)) => &data.weapons[w].specials,
            _ => &[],
        };
        let mut amount = amount;
        for s in on_hit {
            if let Special::Crit { chance, mult } = *s
                && self.rng.chance(chance)
            {
                amount *= mult;
            }
        }

        let time = self.time;
        let v = &mut self.fighters[victim];
        v.health -= amount;
        if let Some(a) = attacker
            && a != victim
        {
            v.last_attacker = Some((a, time));
        }
        let pos = v.center();
        self.events.push(Event::Hit {
            victim,
            attacker,
            amount,
            headshot,
            pos,
        });

        if let (Some(a), Some(w)) = (attacker, weapon)
            && a != victim
        {
            for s in on_hit {
                match *s {
                    Special::Lifesteal { ratio } => {
                        let att = &mut self.fighters[a];
                        if att.alive {
                            att.health = (att.health + amount * ratio).min(MAX_HEALTH);
                        }
                    }
                    Special::Burn { dps, duration_s } => {
                        self.fighters[victim].burn = Some(Burn {
                            dps,
                            time_left: duration_s,
                            source: a,
                            weapon: w,
                        });
                    }
                    Special::Stun { duration_s } => {
                        let v = &mut self.fighters[victim];
                        v.stun = v.stun.max(duration_s);
                    }
                    Special::Freeze { duration_s, slow } => {
                        let v = &mut self.fighters[victim];
                        v.freeze = v.freeze.max(duration_s);
                        v.freeze_slow = slow;
                    }
                    _ => {}
                }
            }
        }

        if self.fighters[victim].health <= 0.0 {
            self.kill(victim, attacker, weapon, headshot);
        }
    }

    fn kill(
        &mut self,
        victim: usize,
        attacker: Option<usize>,
        weapon: Option<usize>,
        headshot: bool,
    ) {
        let time = self.time;
        let v = &mut self.fighters[victim];
        v.alive = false;
        v.health = 0.0;
        v.deaths += 1;
        v.respawn_timer = RESPAWN_DELAY;
        v.burn = None;
        v.vel = Vec2::ZERO;
        let credited = match attacker {
            Some(a) if a != victim => Some(a),
            _ => v
                .last_attacker
                .filter(|(_, t)| time - t < ASSIST_WINDOW)
                .map(|(a, _)| a),
        }
        .filter(|&a| !self.allies(a, victim));

        let playing = self.phase == Phase::Playing;
        if let Some(k) = credited {
            self.fighters[k].kills += 1;
            if playing {
                let side = self.side_of(k);
                self.scores[side] += 1;
            }
        } else if playing && self.mode.respawn && !self.mode.is_team() {
            // Free-for-all suicide penalty.
            let side = self.side_of(victim);
            self.scores[side] -= 1;
        }
        self.events.push(Event::Kill {
            victim,
            killer: credited,
            weapon,
            headshot,
        });
    }
}

fn switch_to(f: &mut Fighter, w: usize) {
    f.current = w;
    f.reload_timer = 0.0;
    f.fire_cooldown = f.fire_cooldown.max(SWITCH_DELAY);
}

fn start_reload(f: &mut Fighter, def: &WeaponDef) -> bool {
    let slot = f.inventory[f.current];
    let can = def.uses_ammo()
        && !f.is_reloading()
        && slot.mag < def.magazine
        && (slot.reserve > 0 || def.infinite_reserve());
    if can {
        f.reload_timer = def.reload_time_s.max(DT);
    }
    can
}

fn finish_reload(f: &mut Fighter, def: &WeaponDef) {
    let slot = &mut f.inventory[f.current];
    let need = def.magazine - slot.mag;
    let take = if def.infinite_reserve() {
        need
    } else {
        need.min(slot.reserve)
    };
    slot.mag += take;
    if !def.infinite_reserve() {
        slot.reserve -= take;
    }
    f.reload_timer = 0.0;
}

/// Some other owned weapon that can still shoot, preferring later (stronger) slots.
fn best_loaded_weapon(f: &Fighter, weapons: &[WeaponDef]) -> Option<usize> {
    (0..weapons.len()).rev().find(|&w| {
        let s = f.inventory[w];
        w != f.current
            && s.owned
            && (!weapons[w].uses_ammo()
                || s.mag > 0
                || s.reserve > 0
                || weapons[w].infinite_reserve())
    })
}
