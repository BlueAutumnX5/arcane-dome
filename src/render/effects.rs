use crate::data::ProjectileKind;
use crate::sim::{Event, World};
use macroquad::prelude::*;

struct Tracer {
    from: Vec2,
    to: Vec2,
    kind: ProjectileKind,
    age: f32,
}

struct Blast {
    pos: Vec2,
    radius: f32,
    age: f32,
}

pub struct FeedLine {
    /// Pieces of text with their colours (names in team colours).
    pub parts: Vec<(String, Color)>,
    pub mine: bool,
    pub age: f32,
}

/// Short-lived visuals created from sim events.
#[derive(Default)]
pub struct Effects {
    tracers: Vec<Tracer>,
    /// Where bullets hit walls and floors; the oldest goes past MAX_MARKS.
    marks: std::collections::VecDeque<Vec2>,
    blasts: Vec<Blast>,
    pub feed: Vec<FeedLine>,
    /// Seconds left on the "you hit someone" crosshair marker.
    pub hitmarker: f32,
    pub hitmarker_headshot: bool,
    /// Seconds left on the "you got hurt" screen flash.
    pub hurt: f32,
}

const TRACER_LIFE: f32 = 0.1;
const BEAM_LIFE: f32 = 0.08;
const BLAST_LIFE: f32 = 0.35;
const FEED_LIFE: f32 = 6.0;
/// Most bullet marks kept on walls and floors.
pub const MAX_MARKS: usize = 512;

impl Effects {
    pub fn consume(&mut self, world: &World, player: Option<usize>) {
        for e in &world.events {
            match e {
                Event::Tracer { from, to, kind } => self.tracers.push(Tracer {
                    from: *from,
                    to: *to,
                    kind: *kind,
                    age: 0.0,
                }),
                Event::Mark { pos } => {
                    if self.marks.len() >= MAX_MARKS {
                        self.marks.pop_front();
                    }
                    self.marks.push_back(*pos);
                }
                Event::Explosion { pos, radius } => self.blasts.push(Blast {
                    pos: *pos,
                    radius: *radius,
                    age: 0.0,
                }),
                Event::Thrust { pos, .. } => self.blasts.push(Blast {
                    pos: *pos - vec2(0.0, 0.3),
                    radius: 0.9,
                    age: 0.0,
                }),
                Event::Teleport { from, to, .. } => {
                    for p in [*from, *to] {
                        self.blasts.push(Blast {
                            pos: p - vec2(0.0, 0.9),
                            radius: 1.4,
                            age: 0.0,
                        });
                    }
                }
                Event::Hit {
                    victim,
                    attacker,
                    headshot,
                    ..
                } => {
                    if *attacker == player && player.is_some() && Some(*victim) != player {
                        self.hitmarker = 0.15;
                        self.hitmarker_headshot = *headshot;
                    }
                    if Some(*victim) == player {
                        self.hurt = 0.25;
                    }
                }
                Event::Kill {
                    victim,
                    killer,
                    weapon,
                    headshot,
                } => {
                    let name = |i: usize| {
                        let f = &world.fighters[i];
                        (f.name.clone(), super::fighter_color(f))
                    };
                    let plain = Color::new(1.0, 1.0, 1.0, 0.85);
                    let mut parts = Vec::new();
                    match killer {
                        Some(k) => {
                            let w = weapon
                                .map(|w| world.data.weapons[w].name.as_str())
                                .unwrap_or("hazard");
                            parts.push(name(*k));
                            parts.push((format!("[{w}]"), plain));
                            parts.push(name(*victim));
                            if *headshot {
                                parts.push(("(headshot)".into(), plain));
                            }
                        }
                        None => {
                            parts.push(name(*victim));
                            parts.push(("died".into(), plain));
                        }
                    }
                    let mine = player.is_some() && (*killer == player || Some(*victim) == player);
                    self.feed.push(FeedLine {
                        parts,
                        mine,
                        age: 0.0,
                    });
                    if self.feed.len() > 6 {
                        self.feed.remove(0);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        for t in &mut self.tracers {
            t.age += dt;
        }
        self.tracers.retain(|t| {
            t.age
                < if t.kind == ProjectileKind::Beam {
                    BEAM_LIFE
                } else {
                    TRACER_LIFE
                }
        });
        for b in &mut self.blasts {
            b.age += dt;
        }
        self.blasts.retain(|b| b.age < BLAST_LIFE);
        for f in &mut self.feed {
            f.age += dt;
        }
        self.feed.retain(|f| f.age < FEED_LIFE);
        self.hitmarker = (self.hitmarker - dt).max(0.0);
        self.hurt = (self.hurt - dt).max(0.0);
    }

    /// Draw in world space (camera already set).
    /// Bullet marks on walls and floors (world space, drawn over the
    /// terrain and under everything else).
    pub fn draw_marks(&self) {
        for p in &self.marks {
            draw_circle(p.x, p.y, 0.13, Color::new(0.0, 0.0, 0.0, 0.22));
            draw_circle(p.x, p.y, 0.07, Color::new(0.06, 0.05, 0.05, 0.85));
        }
    }

    pub fn draw_world(&self) {
        for t in &self.tracers {
            let (color, width, life) = match t.kind {
                ProjectileKind::Beam => (Color::new(1.0, 0.85, 0.45, 1.0), 0.22, BEAM_LIFE),
                ProjectileKind::MeleeArc => (Color::new(1.0, 1.0, 1.0, 1.0), 0.1, TRACER_LIFE),
                _ => (Color::new(0.7, 0.95, 1.0, 1.0), 0.09, TRACER_LIFE),
            };
            let a = 1.0 - t.age / life;
            draw_line(
                t.from.x,
                t.from.y,
                t.to.x,
                t.to.y,
                width,
                Color::new(color.r, color.g, color.b, a * 0.9),
            );
            if t.kind == ProjectileKind::Beam {
                draw_line(
                    t.from.x,
                    t.from.y,
                    t.to.x,
                    t.to.y,
                    width * 0.35,
                    Color::new(1.0, 1.0, 0.95, a),
                );
                draw_circle(t.to.x, t.to.y, 0.22, Color::new(1.0, 0.8, 0.4, a * 0.7));
            }
        }
        for b in &self.blasts {
            let k = b.age / BLAST_LIFE;
            let r = b.radius * (0.35 + 0.65 * k);
            draw_circle(
                b.pos.x,
                b.pos.y,
                r,
                Color::new(1.0, 0.55, 0.2, 0.45 * (1.0 - k)),
            );
            draw_circle(
                b.pos.x,
                b.pos.y,
                r * 0.55,
                Color::new(1.0, 0.9, 0.6, 0.6 * (1.0 - k)),
            );
            draw_circle_lines(
                b.pos.x,
                b.pos.y,
                r,
                0.08,
                Color::new(1.0, 0.8, 0.5, 0.7 * (1.0 - k)),
            );
        }
    }
}
