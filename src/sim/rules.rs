//! Scoring, win conditions and rounds. Driven entirely by `ModeDef` fields.

use super::world::{Event, MAX_TEAMS, TEAM_NAMES};
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
        }
        for i in 0..self.fighters.len() {
            self.spawn_fighter(i);
        }
    }
}
