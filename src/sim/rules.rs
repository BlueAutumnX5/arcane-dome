//! Scoring, win conditions and rounds. Driven entirely by `ModeDef` fields.

use super::world::{ElimStyle, Event, MAX_TEAMS, TEAM_NAMES};
use super::{ObjectState, World};

/// Seconds between elimination rounds.
const INTERMISSION: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Playing,
    /// Elimination only: short pause before the next round.
    RoundOver {
        timer: f32,
        winner: Option<usize>,
    },
    MatchOver {
        winner: Option<usize>,
    },
}

impl World {
    /// Teams in team modes, otherwise every fighter is their own side.
    pub fn num_sides(&self) -> usize {
        self.scores.len()
    }

    pub fn side_of(&self, fighter: usize) -> usize {
        if self.mode.is_team() {
            self.fighters[fighter].team.unwrap_or(0) as usize
        } else {
            fighter
        }
    }

    pub fn allies(&self, a: usize, b: usize) -> bool {
        a == b || (self.mode.is_team() && self.fighters[a].team == self.fighters[b].team)
    }

    pub fn side_name(&self, side: usize) -> String {
        if self.mode.is_team() {
            TEAM_NAMES[self.team_color(side)].into()
        } else {
            self.fighters[side].name.clone()
        }
    }

    /// A team's colour, as an index into the team palette.
    pub fn team_color(&self, team: usize) -> usize {
        // Checked when the world was built: all different and in range.
        self.config.team_colors[team % MAX_TEAMS] as usize
    }

    /// Do dead fighters come back (respawn modes and Elimination with lives)?
    pub fn respawns(&self) -> bool {
        self.mode.respawn || self.uses_lives()
    }

    /// Elimination played with lives instead of rounds.
    pub fn uses_lives(&self) -> bool {
        !self.mode.respawn && self.style != ElimStyle::Rounds
    }

    /// A death in Elimination with lives: it costs the fighter (or their
    /// team's pool) a life; with none left to pay, the fighter is out.
    pub(crate) fn spend_life(&mut self, victim: usize) {
        if !self.uses_lives() || self.lives_limit.is_none() {
            return;
        }
        let side = self.side_of(victim);
        let out = match self.style {
            ElimStyle::Attrition => {
                // The pool pays for the respawn; an empty pool means no more.
                let pool = &mut self.team_lives[side];
                let empty = *pool == 0;
                *pool = pool.saturating_sub(1);
                empty
            }
            _ => {
                let lives = &mut self.lives_left[victim];
                *lives = lives.saturating_sub(1);
                *lives == 0
            }
        };
        self.fighters[victim].out = out;
    }

    /// Lives a side still has: its pool (Attrition) or its fighters' lives,
    /// counting fighters still alive.
    pub fn side_lives(&self, side: usize) -> u32 {
        match self.style {
            ElimStyle::Attrition => {
                let standing = self
                    .fighters
                    .iter()
                    .filter(|f| f.alive && self.side_of(f.id) == side)
                    .count() as u32;
                self.team_lives[side] + standing
            }
            _ => self
                .fighters
                .iter()
                .filter(|f| self.side_of(f.id) == side)
                .map(|f| if f.out { 0 } else { self.lives_left[f.id] })
                .sum(),
        }
    }

    pub fn rounds_to_win(&self) -> u32 {
        self.mode.rounds.unwrap_or(1) / 2 + 1
    }

    /// The side with the strictly highest value, or `None` on a tie.
    fn unique_best(values: &[i64]) -> Option<usize> {
        let best = *values.iter().max()?;
        let mut leaders = values.iter().enumerate().filter(|(_, v)| **v == best);
        let first = leaders.next().map(|(i, _)| i);
        if leaders.next().is_some() {
            None
        } else {
            first
        }
    }

    pub fn leader(&self) -> Option<usize> {
        let s: Vec<i64> = self.scores.iter().map(|&v| v as i64).collect();
        Self::unique_best(&s)
    }

    pub(crate) fn check_rules(&mut self) {
        if self.phase != Phase::Playing {
            return;
        }
        if self.uses_lives() {
            // The match goes on while two sides still have someone in it.
            let mut in_play = vec![false; self.num_sides()];
            for f in self.fighters.iter().filter(|f| !f.out) {
                in_play[self.side_of(f.id)] = true;
            }
            let left = in_play.iter().filter(|&&b| b).count();
            if left <= 1 {
                let w = in_play.iter().position(|&b| b);
                self.finish(w);
            } else if self.time_left <= 0.0 {
                // Time: most lives left wins (most kills, with unlimited lives).
                let by: Vec<i64> = (0..self.num_sides())
                    .map(|s| match self.lives_limit {
                        Some(_) if in_play[s] => self.side_lives(s) as i64,
                        Some(_) => -1,
                        None => self.scores[s] as i64,
                    })
                    .collect();
                let w = Self::unique_best(&by);
                self.finish(w);
            }
            return;
        }
        if self.mode.respawn {
            let limit = self.mode.score_limit.unwrap_or(u32::MAX) as i64;
            let reached = (0..self.num_sides()).find(|&s| self.scores[s] as i64 >= limit);
            if let Some(s) = reached {
                self.finish(Some(s));
            } else if self.time_left <= 0.0 {
                let w = self.leader();
                self.finish(w);
            }
            return;
        }

        let mut alive = vec![0i64; self.num_sides()];
        for f in self.fighters.iter().filter(|f| f.alive) {
            alive[self.side_of(f.id)] += 1;
        }
        let sides_alive = alive.iter().filter(|&&n| n > 0).count();
        let winner = if sides_alive <= 1 {
            alive.iter().position(|&n| n > 0)
        } else if self.time_left <= 0.0 {
            Self::unique_best(&alive)
        } else {
            return;
        };
        if let Some(w) = winner {
            self.round_wins[w] += 1;
        }
        self.events.push(Event::RoundOver { winner });
        let rounds = self.mode.rounds.unwrap_or(1);
        let best = self.round_wins.iter().copied().max().unwrap_or(0);
        if best >= self.rounds_to_win() || self.round >= rounds {
            let wins: Vec<i64> = self.round_wins.iter().map(|&v| v as i64).collect();
            self.finish(Self::unique_best(&wins));
        } else {
            self.phase = Phase::RoundOver {
                timer: INTERMISSION,
                winner,
            };
        }
    }

    fn finish(&mut self, winner: Option<usize>) {
        self.phase = Phase::MatchOver { winner };
        self.events.push(Event::MatchOver { winner });
    }

    pub(crate) fn next_round(&mut self) {
        self.round += 1;
        self.time_left = self.mode.time_limit_s;
        self.phase = Phase::Playing;
        self.start_round();
    }

    /// Fresh round: everyone respawns with a new loadout, pickups reset.
    pub(crate) fn start_round(&mut self) {
        self.countdown = self.config.countdown_s;
        self.projectiles.clear();
        for p in &mut self.pickups {
            p.active = true;
            p.timer = 0.0;
        }
        let data = std::sync::Arc::clone(&self.data);
        for o in &mut self.objects {
            *o = ObjectState::new(&o.def, &data);
        }
        for f in &mut self.fighters {
            f.alive = false;
            f.out = false;
        }
        // Spawn in a shuffled order: each spawn avoids the enemies already
        // placed, so whoever went first (always Blue, before) set up where
        // everyone else would land and fight.
        let mut order: Vec<usize> = (0..self.fighters.len()).collect();
        for k in (1..order.len()).rev() {
            let j = self.rng.below(k + 1);
            order.swap(k, j);
        }
        for i in order {
            self.spawn_fighter(i);
        }
    }
}
