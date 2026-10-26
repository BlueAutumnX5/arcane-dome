//! The match types with a twist: Juggernaut and Gun Game.

use super::World;
use super::fighter::{MAX_HEALTH, MAX_SHIELD, SWITCH_DELAY, Slot};
use super::world::Event;
use crate::data::{Category, ProjectileKind};

/// Palette indices: the Juggernaut is red, the hunters blue.
const JUGG_TINT: u8 = 1;
const HUNTER_TINT: u8 = 0;
/// Without numbers in the mode, a Juggernaut gets these.
const JUGG_HEALTH: f32 = 500.0;
const JUGG_SHIELD: f32 = 200.0;

impl World {
    /// Make a random fighter the Juggernaut, never `not`, and someone alive
    /// if anyone is.
    pub(crate) fn pick_juggernaut(&mut self, not: Option<usize>) {
        let others = |alive: bool| -> Vec<usize> {
            self.fighters
                .iter()
                .filter(|f| Some(f.id) != not && (f.alive || !alive))
                .map(|f| f.id)
                .collect()
        };
        let mut pool = others(true);
        if pool.is_empty() {
            pool = others(false);
        }
        if !pool.is_empty() {
            let j = pool[self.rng.below(pool.len())];
            self.make_juggernaut(j);
        }
    }

    /// Hand the role to `j`: full Juggernaut health and shield right away.
    pub(crate) fn make_juggernaut(&mut self, j: usize) {
        if let Some(old) = self.juggernaut {
            let f = &mut self.fighters[old];
            f.max_health = MAX_HEALTH;
            f.max_shield = MAX_SHIELD;
            f.health = f.health.min(MAX_HEALTH);
            f.shield = f.shield.min(MAX_SHIELD);
        }
        self.juggernaut = Some(j);
        for f in &mut self.fighters {
            f.tint = Some(if f.id == j { JUGG_TINT } else { HUNTER_TINT });
        }
        let health = self.mode.juggernaut_health.unwrap_or(JUGG_HEALTH);
        let shield = self.mode.juggernaut_shield.unwrap_or(JUGG_SHIELD);
        let f = &mut self.fighters[j];
        f.max_health = health;
        f.max_shield = shield;
        if f.alive {
            f.health = health;
            f.shield = shield;
            f.burn = None;
        }
        self.events.push(Event::Juggernaut { fighter: j });
    }

    /// Points for a kill, and a new Juggernaut when theirs fell.
    pub(crate) fn juggernaut_kill(&mut self, victim: usize, killer: Option<usize>, playing: bool) {
        let c = &self.config;
        // Both off would leave no way to score.
        let (as_jugg, on_jugg) = match (c.jugg_kills_score, c.killing_jugg_scores) {
            (false, false) => (true, true),
            pair => pair,
        };
        let was_jugg = self.juggernaut == Some(victim);
        if let Some(k) = killer
            && playing
        {
            let points = if was_jugg {
                on_jugg as i32
            } else if self.juggernaut == Some(k) && as_jugg {
                1 + c.jugg_bonus as i32
            } else {
                0
            };
            self.scores[k] += points;
        }
        if was_jugg {
            match killer {
                Some(k) => self.make_juggernaut(k),
                None => self.pick_juggernaut(Some(victim)),
            }
        }
    }

    /// The weapon a side's rung of the ladder gives.
    pub fn rung_weapon(&self, side: usize) -> usize {
        let rung = self.scores[side].max(0) as usize;
        self.ladder[rung.min(self.ladder.len() - 1)]
    }

    /// A ladder that ends on a melee weapon hands it out as a sidearm from
    /// the start, so melee kills (and their knock-backs) can happen on any
    /// rung.
    fn sidearm(&self) -> Option<usize> {
        let &last = self.ladder.last()?;
        let d = &self.data.weapons[last];
        (d.category == Category::Melee || d.projectile.kind == ProjectileKind::MeleeArc)
            .then_some(last)
    }

    /// Give the fighter the weapon of their side's rung and the sidearm
    /// (nothing else), unless that is what they hold already.
    pub(crate) fn arm_for_rung(&mut self, i: usize) {
        let w = self.rung_weapon(self.side_of(i));
        let side = self.sidearm();
        let def = &self.data.weapons[w];
        let f = &mut self.fighters[i];
        let armed = (0..f.inventory.len()).all(|k| f.owns(k) == (k == w || Some(k) == side));
        if armed {
            return;
        }
        f.inventory.fill(Slot::default());
        if let Some(m) = side {
            f.inventory[m].owned = true;
        }
        f.inventory[w] = Slot {
            owned: true,
            mag: def.magazine,
            reserve: def.reserve_ammo,
        };
        f.current = w;
        f.reload_timer = 0.0;
        f.fire_cooldown = f.fire_cooldown.max(SWITCH_DELAY);
    }

    /// Move a side up (or down) the ladder and rearm whoever is alive on it.
    fn climb(&mut self, side: usize, by: i32) {
        let top = self.ladder.len() as i32;
        let rung = (self.scores[side] + by).clamp(0, top);
        if rung == self.scores[side] {
            return;
        }
        self.scores[side] = rung;
        for i in 0..self.fighters.len() {
            if self.fighters[i].alive && self.side_of(i) == side {
                self.arm_for_rung(i);
            }
        }
        self.events.push(Event::Ladder {
            side,
            rung: rung as usize,
            up: by > 0,
        });
    }

    /// A kill climbs the killer's side one rung; a melee kill, or a death
    /// nobody is credited with, can knock the victim's side down one.
    pub(crate) fn gun_game_kill(
        &mut self,
        victim: usize,
        killer: Option<usize>,
        weapon: Option<usize>,
        playing: bool,
    ) {
        if !playing {
            return;
        }
        let melee = weapon.is_some_and(|w| {
            let d = &self.data.weapons[w];
            d.category == Category::Melee || d.projectile.kind == ProjectileKind::MeleeArc
        });
        let down = match killer {
            Some(k) => {
                self.climb(self.side_of(k), 1);
                melee && self.config.melee_demotes
            }
            None => self.config.suicide_demotes,
        };
        if down {
            self.climb(self.side_of(victim), -1);
        }
    }
}
