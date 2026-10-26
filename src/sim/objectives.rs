//! The objective modes: King of the Hill, Domination, Territories and
//! Capture the Flag.

use super::physics::hazard_dps;
use super::world::Event;
use super::{DT, World};
use crate::data::{Rules, ZoneDef};
use macroquad::math::{Vec2, vec2};

/// Seconds one fighter needs to capture a zone (without a mode setting).
const CAPTURE_S: f32 = 6.0;
/// More fighters capture faster: each extra adds half again, up to double.
const MAX_CREW_SPEED: f32 = 2.0;
/// King of the Hill: seconds before the hill moves (without a mode setting).
pub const HILL_MOVE_S: f32 = 60.0;
/// Domination: points per second for each zone held.
const ZONE_POINTS: f32 = 0.5;
/// Capture the Flag: seconds a dropped flag lies before going home.
const FLAG_RETURN_S: f32 = 15.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneState {
    pub def: ZoneDef,
    /// The side that holds it (King of the Hill: whoever alone is on it).
    pub owner: Option<usize>,
    /// A side partway through capturing it, and how far (0 to 1).
    pub capturer: Option<usize>,
    pub progress: f32,
    /// Sides with someone alive inside, this tick.
    pub present: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagEvent {
    Taken,
    Dropped,
    Returned,
    Captured,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlagState {
    /// The team whose flag it is.
    pub side: usize,
    pub home: Vec2,
    /// Where it is (at its feet on the floor, or on its carrier).
    pub pos: Vec2,
    pub carrier: Option<usize>,
    /// Lying where its carrier fell: seconds before it goes home.
    pub return_in: Option<f32>,
}

impl FlagState {
    pub fn at_home(&self) -> bool {
        self.carrier.is_none() && self.return_in.is_none()
    }
}

impl World {
    /// Zones and flags for this match (from `World::new`).
    pub(crate) fn setup_objectives(&mut self) {
        let rules = self.mode.rules;
        if rules.uses_zones() {
            self.zones = self
                .map
                .zones
                .iter()
                .map(|&def| ZoneState {
                    def,
                    owner: None,
                    capturer: None,
                    progress: 0.0,
                    present: Vec::new(),
                })
                .collect();
        }
        match rules {
            Rules::KingOfTheHill if !self.zones.is_empty() => {
                self.hill = Some(self.rng.below(self.zones.len()));
                self.hill_timer = self.hill_move_s();
            }
            Rules::Territories => self.reset_territories(),
            Rules::CaptureTheFlag => {
                let n = self.num_sides();
                self.flags = (0..n)
                    .filter_map(|t| {
                        let def = self
                            .map
                            .flags
                            .iter()
                            .find(|f| f.team_for(n) == Some(t as u8))?;
                        Some(FlagState {
                            side: t,
                            home: def.feet(),
                            pos: def.feet(),
                            carrier: None,
                            return_in: None,
                        })
                    })
                    .collect();
            }
            _ => {}
        }
        self.score_acc = vec![0.0; self.num_sides()];
    }

    /// Seconds between hill moves; infinite when it stays put.
    pub fn hill_move_s(&self) -> f32 {
        let s = self
            .config
            .hill_move_s
            .or(self.mode.hill_move_s)
            .unwrap_or(HILL_MOVE_S);
        if s > 0.0 && self.zones.len() > 1 {
            s
        } else {
            f32::INFINITY
        }
    }

    fn capture_s(&self) -> f32 {
        self.mode.capture_s.unwrap_or(CAPTURE_S).max(0.1)
    }

    /// One tick of the objective, while the match is on.
    pub(crate) fn update_objectives(&mut self) {
        match self.mode.rules {
            Rules::KingOfTheHill => self.update_hill(),
            Rules::Domination => {
                self.update_zones();
                let rate = self.mode.zone_points_per_s.unwrap_or(ZONE_POINTS);
                for k in 0..self.zones.len() {
                    if let Some(s) = self.zones[k].owner {
                        self.add_points(s, rate * DT);
                    }
                }
            }
            Rules::Territories => {
                self.update_zones();
                let all = self.zones.first().and_then(|z| z.owner);
                if let Some(s) = all
                    && self.zones.iter().all(|z| z.owner == all)
                {
                    self.scores[s] += 1;
                    self.events.push(Event::Pushed { side: s });
                    self.reset_territories();
                }
            }
            Rules::CaptureTheFlag => self.update_flags(),
            _ => {}
        }
    }

    /// Whole points from a running total (Domination and the hill earn
    /// them a fraction at a time).
    fn add_points(&mut self, side: usize, amount: f32) {
        let acc = &mut self.score_acc[side];
        *acc += amount;
        // Sixtieths of a point add up to a hair under the whole one.
        let whole = (*acc + 1e-3).floor();
        *acc -= whole;
        self.scores[side] += whole as i32;
    }

    /// Sides with someone alive in a zone, with how many each.
    fn sides_in(&self, z: &ZoneDef) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = Vec::new();
        for f in self.fighters.iter().filter(|f| f.alive) {
            let b = f.aabb();
            if !z.touches(b.min, b.max) {
                continue;
            }
            let side = self.side_of(f.id);
            match out.iter_mut().find(|(s, _)| *s == side) {
                Some((_, n)) => *n += 1,
                None => out.push((side, 1)),
            }
        }
        out
    }

    fn update_hill(&mut self) {
        let Some(h) = self.hill else {
            return;
        };
        self.hill_timer -= DT;
        if self.hill_timer <= 0.0 && self.zones.len() > 1 {
            // Somewhere else, never the same zone twice in a row.
            let n = self.zones.len();
            let next = (h + 1 + self.rng.below(n - 1)) % n;
            self.zones[h].owner = None;
            self.zones[h].present.clear();
            self.hill = Some(next);
            self.hill_timer = self.hill_move_s();
            self.events.push(Event::HillMoved { zone: next });
            return;
        }
        let present = self.sides_in(&self.zones[h].def);
        let holder = match present.as_slice() {
            [(s, _)] => Some(*s),
            _ => None,
        };
        let z = &mut self.zones[h];
        z.present = present.iter().map(|p| p.0).collect();
        z.owner = holder;
        if let Some(s) = holder {
            self.add_points(s, DT);
        }
    }

    /// Territories: can `side` capture zone `k`? Only next to a zone it
    /// holds, so the line moves one zone at a time.
    pub fn can_push(&self, k: usize, side: usize) -> bool {
        let z = &self.zones;
        if self.mode.rules != Rules::Territories {
            return true;
        }
        z[k].owner != Some(side)
            && ((k > 0 && z[k - 1].owner == Some(side))
                || (k + 1 < z.len() && z[k + 1].owner == Some(side)))
    }

    /// Territories: each team holds the zones at its own end, and the
    /// middle one (with an odd count) is open.
    fn reset_territories(&mut self) {
        let n = self.zones.len();
        // The team whose spawns lie further left starts on the left.
        let mean_x = |t: u8| {
            let xs: Vec<f32> = self
                .map
                .spawns
                .iter()
                .filter(|s| s.team_for(2) == Some(t))
                .map(|s| s.x)
                .collect();
            xs.iter().sum::<f32>() / xs.len().max(1) as f32
        };
        let (left, right) = if mean_x(0) <= mean_x(1) {
            (0, 1)
        } else {
            (1, 0)
        };
        let each = n.saturating_sub(1) / 2;
        for (k, z) in self.zones.iter_mut().enumerate() {
            z.owner = if k < each {
                Some(left)
            } else if k >= n - each {
                Some(right)
            } else {
                None
            };
            z.capturer = None;
            z.progress = 0.0;
        }
    }

    /// Domination and Territories: a side alone in a zone captures it
    /// (faster with more of them); someone from another side stops it.
    fn update_zones(&mut self) {
        let step = DT / self.capture_s();
        for k in 0..self.zones.len() {
            let present = self.sides_in(&self.zones[k].def);
            let lone = match present.as_slice() {
                [(s, n)] => Some((*s, *n)),
                _ => None,
            };
            let allowed = lone.is_some_and(|(s, _)| self.can_push(k, s));
            let z = &mut self.zones[k];
            z.present = present.iter().map(|p| p.0).collect();
            match lone {
                // Contested: nothing moves.
                None if present.len() > 1 => {}
                // Empty, or the owner standing on it: a half-done capture
                // slips back.
                None => z.progress = (z.progress - step * 0.5).max(0.0),
                Some((s, _)) if z.owner == Some(s) => z.progress = (z.progress - step).max(0.0),
                Some(_) if !allowed => {}
                Some((s, crew)) => {
                    let speed = (1.0 + 0.5 * (crew as f32 - 1.0)).min(MAX_CREW_SPEED);
                    if z.capturer.is_some_and(|c| c != s) && z.progress > 0.0 {
                        // Undo the other side's progress first.
                        z.progress = (z.progress - step * speed).max(0.0);
                    } else {
                        z.capturer = Some(s);
                        z.progress += step * speed;
                        if z.progress >= 1.0 {
                            z.owner = Some(s);
                            z.capturer = None;
                            z.progress = 0.0;
                            self.events.push(Event::ZoneTaken { zone: k, side: s });
                        }
                    }
                }
            }
            if self.zones[k].progress <= 0.0 {
                self.zones[k].capturer = None;
            }
        }
    }

    /// The flag this fighter carries, if any.
    pub fn carried_flag(&self, i: usize) -> Option<usize> {
        self.flags.iter().position(|f| f.carrier == Some(i))
    }

    fn update_flags(&mut self) {
        for k in 0..self.flags.len() {
            let f = &mut self.flags[k];
            if let Some(c) = f.carrier {
                f.pos = self.fighters[c].feet;
            } else if let Some(t) = f.return_in.as_mut() {
                *t -= DT;
                if *t <= 0.0 {
                    self.flag_home(k, None);
                }
            }
        }
        for i in 0..self.fighters.len() {
            if !self.fighters[i].alive {
                continue;
            }
            let side = self.side_of(i);
            let reach = self.fighters[i].aabb().expand(0.5);
            for k in 0..self.flags.len() {
                let f = &self.flags[k];
                if f.carrier.is_some() || !reach.contains(f.pos - vec2(0.0, 0.5)) {
                    continue;
                }
                if f.side == side {
                    if !f.at_home() {
                        self.flag_home(k, Some(i));
                    } else if let Some(e) = self.carried_flag(i) {
                        self.scores[side] += 1;
                        self.flags[e].carrier = None;
                        self.flags[e].pos = self.flags[e].home;
                        self.events.push(Event::Flag {
                            flag: e,
                            fighter: Some(i),
                            what: FlagEvent::Captured,
                        });
                    }
                } else if self.carried_flag(i).is_none() {
                    let f = &mut self.flags[k];
                    f.carrier = Some(i);
                    f.return_in = None;
                    self.events.push(Event::Flag {
                        flag: k,
                        fighter: Some(i),
                        what: FlagEvent::Taken,
                    });
                }
            }
        }
    }

    /// Send a flag home: returned by a teammate (`by`) or on its own.
    fn flag_home(&mut self, k: usize, by: Option<usize>) {
        let f = &mut self.flags[k];
        f.carrier = None;
        f.return_in = None;
        f.pos = f.home;
        self.events.push(Event::Flag {
            flag: k,
            fighter: by,
            what: FlagEvent::Returned,
        });
    }

    /// A carrier fell: the flag drops to the floor below, or goes straight
    /// home if there is none or it is in a hazard.
    pub(crate) fn drop_flags(&mut self, victim: usize) {
        let Some(k) = self.carried_flag(victim) else {
            return;
        };
        let feet = self.fighters[victim].feet;
        let floor = self
            .map
            .ground_below(feet, 0.3, self.map.height)
            .map(|y| vec2(feet.x, y))
            .filter(|&p| hazard_dps(&self.map, p, vec2(0.6, 1.0)) <= 0.0);
        match floor {
            Some(p) => {
                let wait = self.mode.flag_return_s.unwrap_or(FLAG_RETURN_S);
                let f = &mut self.flags[k];
                f.carrier = None;
                f.pos = p;
                f.return_in = Some(wait);
                self.events.push(Event::Flag {
                    flag: k,
                    fighter: Some(victim),
                    what: FlagEvent::Dropped,
                });
            }
            None => self.flag_home(k, None),
        }
    }
}
