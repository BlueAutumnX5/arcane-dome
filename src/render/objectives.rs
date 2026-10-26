//! Zones and flags in the world, their labels, and the arrows that point
//! to them from the screen edge.

use super::{GameCamera, TEAM_COLORS, edge_arrow, fighter_color};
use crate::data::Rules;
use crate::i18n::t;
use crate::sim::{FlagState, World};
use macroquad::prelude::*;

/// Zones nobody holds.
const NEUTRAL: Color = Color::new(0.85, 0.9, 1.0, 1.0);

/// A side's colour: its team's, or in free-for-all the fighter's own.
pub fn side_color(world: &World, side: usize) -> Color {
    if world.mode.is_team() {
        TEAM_COLORS[world.team_color(side)]
    } else {
        fighter_color(&world.fighters[side])
    }
}

/// "A", "B", "C"... left to right.
pub fn zone_letter(k: usize) -> String {
    char::from(b'A' + (k % 26) as u8).to_string()
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// The zones in play: the hill only, in King of the Hill.
fn zones_shown(world: &World) -> Vec<usize> {
    match (world.mode.rules, world.hill) {
        (Rules::KingOfTheHill, Some(h)) => vec![h],
        (Rules::KingOfTheHill, None) => Vec::new(),
        _ => (0..world.zones.len()).collect(),
    }
}

/// Each zone as a tinted box in its holder's colour, with a bar along the
/// floor filling as someone captures it. A contested zone's edge flashes.
pub fn draw_zones(world: &World) {
    let pulse = 0.5 + 0.5 * (world.time * 4.0).sin();
    for k in zones_shown(world) {
        let z = &world.zones[k];
        let (a, b) = z.def.rect();
        let size = b - a;
        let c = z.owner.map_or(NEUTRAL, |s| side_color(world, s));
        let fill = if z.owner.is_some() { 0.16 } else { 0.07 };
        draw_rectangle(a.x, a.y, size.x, size.y, with_alpha(c, fill + 0.05 * pulse));
        let contested = z.present.len() > 1;
        let edge = if contested && pulse > 0.5 {
            WHITE
        } else {
            with_alpha(c, 0.85)
        };
        draw_rectangle_lines(a.x, a.y, size.x, size.y, 0.1, edge);
        if let Some(s) = z.capturer
            && z.progress > 0.0
        {
            draw_rectangle(
                a.x,
                b.y - 0.22,
                size.x,
                0.22,
                Color::new(0.0, 0.0, 0.0, 0.5),
            );
            draw_rectangle(
                a.x,
                b.y - 0.22,
                size.x * z.progress.min(1.0),
                0.22,
                side_color(world, s),
            );
        }
    }
}

/// A flag on its pole, standing at `foot`, flying toward `dir` (1 right,
/// -1 left).
fn draw_flag(foot: Vec2, dir: f32, c: Color, time: f32, height: f32) {
    let top = foot - vec2(0.0, height);
    draw_line(
        foot.x,
        foot.y,
        top.x,
        top.y,
        0.09,
        Color::new(0.85, 0.87, 0.9, 1.0),
    );
    let wave = (time * 6.0 + foot.x).sin() * 0.12;
    let cloth = height * 0.42;
    draw_triangle(
        top,
        top + vec2(dir * cloth * 1.6, cloth * 0.45 + wave),
        top + vec2(0.0, cloth),
        c,
    );
}

/// Flags at home, lying where they fell, or on their carriers' backs; and
/// each team's base plate.
pub fn draw_flags(world: &World) {
    for f in &world.flags {
        let c = TEAM_COLORS[world.team_color(f.side)];
        draw_rectangle(f.home.x - 0.7, f.home.y - 0.14, 1.4, 0.14, c);
        match f.carrier {
            Some(i) => {
                let who = &world.fighters[i];
                let back = vec2(who.feet.x - who.facing * 0.35, who.feet.y - 1.1);
                draw_flag(back, -who.facing, c, world.time, 1.5);
            }
            None => {
                if !f.at_home() {
                    // An empty pole marks home while the flag is away.
                    draw_line(
                        f.home.x,
                        f.home.y,
                        f.home.x,
                        f.home.y - 2.4,
                        0.06,
                        with_alpha(c, 0.35),
                    );
                }
                draw_flag(f.pos, 1.0, c, world.time, 2.4);
            }
        }
    }
}

/// Zone letters (or "HILL"), seconds before a dropped flag goes home, and
/// edge arrows to what the player should head for.
pub fn draw_labels(world: &World, cam: &GameCamera, player: Option<usize>, s: f32) {
    let size = 20.0 * s;
    for k in zones_shown(world) {
        let z = &world.zones[k];
        let (a, b) = z.def.rect();
        let label = if world.mode.rules == Rules::KingOfTheHill {
            t("HILL").to_owned()
        } else {
            zone_letter(k)
        };
        let at = cam.world_to_screen(vec2((a.x + b.x) / 2.0, a.y));
        let w = measure_text(&label, None, size as u16, 1.0).width;
        let c = z.owner.map_or(NEUTRAL, |side| side_color(world, side));
        draw_text(
            &label,
            at.x - w / 2.0 + 1.5,
            at.y - 6.0 * s + 1.5,
            size,
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        draw_text(&label, at.x - w / 2.0, at.y - 6.0 * s, size, c);
    }
    for f in &world.flags {
        if let Some(left) = f.return_in {
            let text = format!("{}", left.ceil() as i32);
            let at = cam.world_to_screen(f.pos - vec2(0.0, 2.8));
            let w = measure_text(&text, None, (16.0 * s) as u16, 1.0).width;
            draw_text(&text, at.x - w / 2.0, at.y, 16.0 * s, WHITE);
        }
    }
    let Some(p) = player.filter(|&p| world.fighters[p].alive) else {
        return;
    };
    match world.mode.rules {
        Rules::KingOfTheHill => {
            if let Some(h) = world.hill {
                let z = &world.zones[h];
                let c = z.owner.map_or(NEUTRAL, |side| side_color(world, side));
                edge_arrow(cam, z.def.feet() - vec2(0.0, z.def.h / 2.0), c, s);
            }
        }
        Rules::CaptureTheFlag => {
            let side = world.side_of(p);
            let mine: Option<&FlagState> = world.flags.iter().find(|f| f.side == side);
            let c = TEAM_COLORS[world.team_color(side)];
            if world.carried_flag(p).is_some() {
                // Home with it.
                if let Some(f) = mine {
                    edge_arrow(cam, f.home - vec2(0.0, 1.0), c, s);
                }
            } else if let Some(f) = mine.filter(|f| !f.at_home()) {
                // Our flag is out there.
                edge_arrow(cam, f.pos - vec2(0.0, 1.0), c, s);
            }
        }
        _ => {}
    }
}
