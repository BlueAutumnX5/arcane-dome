use super::objectives::{side_color as zone_side_color, zone_letter};
use super::{Effects, GameCamera, RED, TEAM_COLORS, category_color, fighter_color};
use crate::data::{ProjectileKind, Rules};
use crate::i18n::{t, tf};
use crate::sim::{GO_SHOWN_S, Phase, World};
use macroquad::prelude::*;

fn ui_scale() -> f32 {
    (screen_height() / crate::platform::ui_height()).max(0.6)
}

fn text(t: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(t, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_text(t, x, y, size, color);
}

fn text_center(t: &str, cx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    text(t, cx - d.width / 2.0, y, size, color);
}

/// Seconds before respawning when "Respawn in N" appears.
const RESPAWN_SHOWN_S: f32 = 3.0;

/// Opacity of the blood at 1 HP (it fades in from full health).
const HURT_BLOOD: f32 = 0.25;

/// A red vignette and splatter, drawn in code. `life` picks the pattern.
/// `dead_for`: seconds since dying, to animate the splats landing and the
/// runs creeping down; `None` draws them all at once, still.
fn draw_splatter(fighter: usize, life: u32, strength: f32, dead_for: Option<f32>) {
    let (sw, sh) = (screen_width(), screen_height());
    let fade = strength * dead_for.map_or(1.0, |t| (t / 0.35).clamp(0.0, 1.0));
    // Drips run only on the death screen; while hurt there are just splats.
    let dying = dead_for.is_some();
    let dead_for = dead_for.unwrap_or(10.0);
    // Vignette: red bands thickening toward the edges.
    for k in 0..6 {
        let t = (k + 1) as f32 / 6.0;
        let band = sh * 0.05 * t;
        let c = Color::new(0.55, 0.02, 0.02, 0.09 * fade);
        draw_rectangle(0.0, 0.0, sw, band, c);
        draw_rectangle(0.0, sh - band, sw, band, c);
        draw_rectangle(0.0, 0.0, band * 1.4, sh, c);
        draw_rectangle(sw - band * 1.4, 0.0, band * 1.4, sh, c);
    }
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.45, 0.0, 0.0, 0.18 * fade));
    // Splats: a blob, a few droplets and a run or two, mostly near the edges.
    let mut seed =
        (fighter as u32 + 1).wrapping_mul(0x9E37_79B9) ^ (life + 1).wrapping_mul(0x85EB_CA6B);
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 10_000) as f32 / 10_000.0
    };
    let unit = sh / 720.0;
    for n in 0..14 {
        // Pick a spot nearer an edge than the middle.
        let (mut x, mut y) = (rand(), rand());
        if n % 2 == 0 {
            x = if x < 0.5 {
                x * 0.35
            } else {
                1.0 - (1.0 - x) * 0.35
            };
        } else {
            y = if y < 0.5 {
                y * 0.35
            } else {
                1.0 - (1.0 - y) * 0.35
            };
        }
        let (x, y) = (x * sw, y * sh);
        let landed = ((dead_for - n as f32 * 0.025) / 0.12).clamp(0.0, 1.0);
        if landed <= 0.0 {
            continue;
        }
        let r = (18.0 + rand() * 46.0) * unit * (0.6 + 0.4 * landed);
        let dark = 0.35 + rand() * 0.25;
        let c = Color::new(dark, 0.0, 0.01, 0.72 * fade);
        draw_circle(x, y, r, c);
        for _ in 0..7 {
            let a = rand() * std::f32::consts::TAU;
            let d = r * (0.9 + rand() * 1.1);
            draw_circle(
                x + a.cos() * d,
                y + a.sin() * d,
                r * (0.08 + rand() * 0.22),
                c,
            );
        }
        // Runs that creep down over the first second.
        let runs = (rand() * 3.0) as i32;
        for _ in 0..runs {
            let rx = x + (rand() - 0.5) * r * 1.2;
            let len = r * (1.0 + rand() * 2.5) * (dead_for / 1.2).clamp(0.0, 1.0);
            let w = r * (0.1 + rand() * 0.12);
            // (Still drawn from the random sequence when skipped, so the
            // splats keep the same places alive and dead.)
            if dying {
                draw_rectangle(rx - w * 0.5, y, w, len, c);
                draw_circle(rx, y + len, w * 0.8, c);
            }
        }
    }
}

/// Coloured pieces of text side by side, `gap` apart, centred on `cx`.
fn segments_center(parts: &[(String, Color)], cx: f32, y: f32, size: f32, gap: f32) {
    let widths: Vec<f32> = parts
        .iter()
        .map(|(t, _)| measure_text(t, None, size as u16, 1.0).width)
        .collect();
    let total = widths.iter().sum::<f32>() + gap * (parts.len().saturating_sub(1)) as f32;
    let mut x = cx - total / 2.0;
    for ((t, c), w) in parts.iter().zip(&widths) {
        text(t, x, y, size, *c);
        x += w + gap;
    }
}

fn text_right(s: &str, rx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    text(s, rx - d.width, y, size, color);
}

/// "to 25", or "no limit" when the score limit is off.
fn limit_text(limit: Option<u32>) -> String {
    limit.map_or(t("no limit").into(), |l| tf("to {}", &[&l]))
}

/// The name of the Elimination style being played.
fn style_text(world: &World) -> String {
    match world.style {
        crate::sim::ElimStyle::Attrition => t("Attrition").into(),
        crate::sim::ElimStyle::LastManStanding => t("Last Man Standing").into(),
        crate::sim::ElimStyle::Rounds => round_text(world),
    }
}

/// "Round 2/5".
fn round_text(world: &World) -> String {
    tf(
        "Round {}/{}",
        &[&world.round, &world.mode.rounds.unwrap_or(1)],
    )
}

/// A side's name: its team colour or the fighter's name, in the current
/// language.
fn side_name(world: &World, side: usize) -> String {
    t(&world.side_name(side)).to_owned()
}

/// "Team Deathmatch", "Elimination" and the like.
fn match_name(world: &World) -> String {
    let mode = t(&world.mode.display_name()).to_owned();
    if world.mode.is_team() {
        tf("Team {}", &[&mode])
    } else {
        mode
    }
}

fn clock(secs: f32) -> String {
    if secs.is_infinite() {
        return "--:--".into();
    }
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Crosshair at the (still visible) system cursor. Its gap opens with the
/// current weapon's spread plus recoil bloom, measured at the cursor distance.
pub fn draw_crosshair(
    world: &World,
    cam: &GameCamera,
    effects: &Effects,
    player: usize,
    mouse: Vec2,
) {
    let f = &world.fighters[player];
    let s = ui_scale();
    let mut gap = 4.0 * s;
    let mut reloading = false;
    if f.alive {
        let def = &world.data.weapons[f.current];
        let half = ((def.spread_deg + f.bloom) * 0.5).to_radians();
        let eye = cam.world_to_screen(f.eye());
        let dist = eye.distance(mouse);
        gap = (half.tan() * dist).clamp(4.0 * s, 120.0 * s);
        reloading = f.is_reloading();
    }
    let len = 7.0 * s;
    let col = if reloading {
        Color::new(1.0, 1.0, 1.0, 0.35)
    } else {
        Color::new(1.0, 1.0, 1.0, 0.95)
    };
    let shadow = Color::new(0.0, 0.0, 0.0, 0.6);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let a = mouse + vec2(dx, dy) * gap;
        let b = mouse + vec2(dx, dy) * (gap + len);
        draw_line(a.x, a.y, b.x, b.y, 4.0 * s, shadow);
        draw_line(a.x, a.y, b.x, b.y, 2.0 * s, col);
    }
    if effects.hitmarker > 0.0 {
        let c = if effects.hitmarker_headshot {
            RED
        } else {
            WHITE
        };
        let r0 = 6.0 * s;
        let r1 = 12.0 * s;
        for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let d = vec2(dx, dy).normalize();
            let a = mouse + d * r0;
            let b = mouse + d * r1;
            draw_line(a.x, a.y, b.x, b.y, 2.0 * s, c);
        }
    }
}

/// `blood: false` leaves out the splatter (the red hit flash stays).
pub fn draw_hud(world: &World, effects: &Effects, player: usize, blood: bool) {
    let s = ui_scale();
    let sw = screen_width();
    let sh = screen_height();
    let f = &world.fighters[player];

    // Blood: builds up as health drops (static, up to a quarter strength at
    // 1 HP), then plays out in full when the fighter dies. One pattern per
    // life, so the death splatter grows out of the stains already there.
    if blood && !f.alive && world.phase == Phase::Playing {
        let life = f.deaths.saturating_sub(1);
        draw_splatter(f.id, life, 1.0, Some(world.time - f.died_at));
    } else if blood && f.alive && f.health < f.max_health {
        let hurt = ((f.max_health - f.health) / (f.max_health - 1.0)).clamp(0.0, 1.0);
        draw_splatter(f.id, f.deaths, HURT_BLOOD * hurt, None);
    }

    if effects.hurt > 0.0 {
        let a = effects.hurt / 0.25 * 0.35;
        let t = 24.0 * s;
        let c = Color::new(0.9, 0.1, 0.1, a);
        draw_rectangle(0.0, 0.0, sw, t, c);
        draw_rectangle(0.0, sh - t, sw, t, c);
        draw_rectangle(0.0, 0.0, t, sh, c);
        draw_rectangle(sw - t, 0.0, t, sh, c);
    }

    let touch = crate::platform::touch();
    let frac = (f.health / f.max_health).clamp(0.0, 1.0);
    let shield = (f.shield / f.max_shield).clamp(0.0, 1.0);
    // Powerups list from here, one row further each.
    let (px, py, pdir) = if touch {
        // With touch controls, health sits top left beside the pause and
        // scores buttons, out from under the thumbs (the touch weapon bar
        // and its buttons are drawn by `app::touch`).
        let (hx, hy, hw, hh) = (128.0 * s, 26.0 * s, 220.0 * s, 22.0 * s);
        draw_rectangle(hx, hy, hw, hh, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(hx, hy, hw * frac, hh, health_color(frac));
        text(
            &format!("{:.0}", f.health.max(0.0)),
            hx + 8.0 * s,
            hy + 17.0 * s,
            22.0 * s,
            WHITE,
        );
        draw_rectangle(hx, hy + hh, hw, 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(hx, hy + hh, hw * shield, 4.0 * s, SHIELD_BAR);
        (hx, hy + hh + 14.0 * s, 1.0)
    } else {
        health_panel(f, frac, shield, s);
        weapon_panel(world, f, s);
        (14.0 * s, 104.0 * s, 1.0)
    };

    // Active powerups, below health.
    let mut row = 0.0;
    for b in &f.buffs {
        if let Some(p) = world.data.powerups.powerups.get(b.powerup) {
            let c = super::gadgets::rgb(p.color);
            let y = py + pdir * row * 24.0 * s;
            draw_rectangle(px, y, 14.0 * s, 14.0 * s, c);
            text(
                &format!("{}  {:.0}s", t(&p.name), b.time_left.ceil()),
                px + 22.0 * s,
                y + 13.0 * s,
                18.0 * s,
                c,
            );
            row += 1.0;
        }
    }
    if f.teleport_charges > 0
        && let Some(p) = f
            .teleport_powerup
            .and_then(|p| world.data.powerups.powerups.get(p))
    {
        let c = super::gadgets::rgb(p.color);
        let y = py + pdir * row * 24.0 * s;
        draw_rectangle(px, y, 14.0 * s, 14.0 * s, c);
        let key = if touch { "" } else { "  [F]" };
        text(
            &format!("{} x{}{key}", t(&p.name), f.teleport_charges),
            px + 22.0 * s,
            y + 13.0 * s,
            18.0 * s,
            c,
        );
    }

    let def = &world.data.weapons[f.current];
    let slot = f.inventory[f.current];
    if touch {
        // Ammo above the touch weapon bar (46 high, 10 from the bottom).
        let ammo = ammo_text(def, slot);
        let y = sh - 66.0 * s;
        let name = measure_text(t(&def.name), None, (22.0 * s) as u16, 1.0).width;
        let count = measure_text(&ammo, None, (26.0 * s) as u16, 1.0).width;
        let gap = 16.0 * s;
        let x = sw / 2.0 - (name + gap + count) / 2.0;
        text(t(&def.name), x, y, 22.0 * s, category_color(def.category));
        text(&ammo, x + name + gap, y, 26.0 * s, WHITE);
        if f.is_reloading() {
            let total = def.reload_time_s.max(0.01);
            let p = 1.0 - f.reload_timer / total;
            let w = 160.0 * s;
            let bx = sw / 2.0 - w / 2.0;
            let by = y - 32.0 * s;
            draw_rectangle(bx, by, w, 5.0 * s, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle(bx, by, w * p, 5.0 * s, WHITE);
        }
    } else {
        weapon_slots(world, f, s);
    }

    // Score and timer: top centre with touch controls; on a PC the timer
    // and match name go top right and the score under the health panel.
    let cx = sw / 2.0;
    let mode = &world.mode;
    let time = if matches!(world.phase, Phase::RoundOver { .. }) {
        "--:--".to_owned()
    } else {
        clock(world.time_left)
    };
    let (score_y, status_y) = if touch {
        text_center(&time, cx, 34.0 * s, 30.0 * s, WHITE);
        (58.0 * s, 84.0 * s)
    } else {
        text_right(&time, sw - 18.0 * s, 32.0 * s, 28.0 * s, WHITE);
        text_right(
            &match_name(world),
            sw - 18.0 * s,
            54.0 * s,
            17.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.75),
        );
        (80.0 * s, 104.0 * s)
    };
    if mode.is_team() {
        // One coloured segment per team: score (or round wins and how many
        // are still standing), then the target.
        let alive = |t: usize| {
            world
                .fighters
                .iter()
                .filter(|f| f.alive && f.team == Some(t as u8))
                .count()
        };
        let mut parts: Vec<(String, Color)> = (0..world.num_sides())
            .map(|t| {
                let name = side_name(world, t);
                let label = if mode.respawn {
                    format!("{} {}", name, world.scores[t])
                } else if world.uses_lives() {
                    match world.lives_limit {
                        Some(_) => tf("{} {} lives", &[&name, &world.side_lives(t)]),
                        None => format!("{} {}", name, world.scores[t]),
                    }
                } else {
                    tf("{} {} ({} up)", &[&name, &world.round_wins[t], &alive(t)])
                };
                (label, TEAM_COLORS[world.team_color(t)])
            })
            .collect();
        let goal = if mode.respawn {
            limit_text(mode.score_limit)
        } else if world.uses_lives() {
            style_text(world)
        } else {
            round_text(world)
        };
        parts.push((goal, Color::new(1.0, 1.0, 1.0, 0.75)));
        segments_center(&parts, cx, score_y, 20.0 * s, 22.0 * s);
    }
    let line = if mode.is_team() {
        String::new()
    } else if world.uses_lives() {
        let left = (0..world.num_sides())
            .filter(|&s| !world.fighters[s].out)
            .count();
        match world.lives_limit {
            Some(_) => tf(
                "Lives {}    {} still in    {}",
                &[&world.lives_left[player], &left, &style_text(world)],
            ),
            None => tf(
                "Kills {}    {}",
                &[&world.scores[player], &style_text(world)],
            ),
        }
    } else if !mode.respawn {
        round_text(world)
    } else {
        let mine = world.scores[world.side_of(player)];
        let (leader, best) = (0..world.num_sides())
            .map(|s| (s, world.scores[s]))
            .max_by_key(|&(s, v)| (v, std::cmp::Reverse(s)))
            .unwrap_or((0, 0));
        tf(
            "You {}    Leader {} {}    {}",
            &[
                &mine,
                &side_name(world, leader),
                &best,
                &limit_text(mode.score_limit),
            ],
        )
    };
    text_center(&line, cx, score_y, 20.0 * s, Color::new(1.0, 1.0, 1.0, 0.9));
    // Juggernaut: who it is. Gun Game: the weapon your rung gives.
    if let Some(j) = world.juggernaut {
        let msg = if j == player {
            t("You are the Juggernaut").to_owned()
        } else {
            tf(
                "{} is the Juggernaut",
                &[&t(&world.fighters[j].name).to_owned()],
            )
        };
        let red = super::fighter_color(&world.fighters[j]);
        text_center(&msg, cx, status_y, 18.0 * s, red);
    } else if !world.ladder.is_empty() {
        let side = world.side_of(player);
        let rung = (world.scores[side].max(0) as usize).min(world.ladder.len() - 1);
        let w = &world.data.weapons[world.rung_weapon(side)];
        let msg = tf(
            "Weapon {}/{}: {}",
            &[&(rung + 1), &world.ladder.len(), &t(&w.name).to_owned()],
        );
        text_center(&msg, cx, status_y, 18.0 * s, category_color(w.category));
    } else {
        objective_status(world, player, cx, status_y, s);
    }

    // Kill feed, top right.
    for (i, l) in effects.feed.iter().enumerate() {
        let a = (1.0 - (l.age - 5.0).max(0.0)).clamp(0.0, 1.0);
        let size = 18.0 * s;
        let gap = 7.0 * s;
        // Under the timer on a PC.
        let top = if touch { 28.0 } else { 80.0 };
        let y = (top + i as f32 * 22.0) * s;
        let widths: Vec<f32> = l
            .parts
            .iter()
            .map(|(t, _)| measure_text(t, None, size as u16, 1.0).width)
            .collect();
        let total = widths.iter().sum::<f32>() + gap * (widths.len().saturating_sub(1)) as f32;
        let mut x = sw - 16.0 * s - total;
        if l.mine {
            // Your own kills and deaths stand out with a bar behind them.
            draw_rectangle(
                x - 6.0 * s,
                y - size * 0.8,
                total + 12.0 * s,
                size * 1.05,
                Color::new(1.0, 0.9, 0.5, 0.18 * a),
            );
        }
        for ((t, c), w) in l.parts.iter().zip(&widths) {
            text(t, x, y, size, Color::new(c.r, c.g, c.b, c.a * a));
            x += w + gap;
        }
    }

    // Countdown before the fight: a big number that shrinks into place each
    // second, then "GO!" fading out.
    if world.phase == Phase::Playing && world.countdown > -GO_SHOWN_S {
        let (msg, k, color) = if world.countdown > 0.0 {
            let k = world.countdown.fract();
            let n = world.countdown.ceil() as i32;
            (n.to_string(), k, WHITE)
        } else {
            let k = 1.0 + world.countdown / GO_SHOWN_S;
            (t("GO!").to_owned(), k, Color::new(0.55, 1.0, 0.6, 1.0))
        };
        let size = (90.0 + 60.0 * k) * s;
        let alpha = (0.35 + 0.65 * k).min(1.0);
        let y = sh * 0.36;
        let shadow = Color::new(0.0, 0.0, 0.0, 0.5 * alpha);
        text_center(&msg, cx + 3.0 * s, y + 3.0 * s, size, shadow);
        text_center(
            &msg,
            cx,
            y,
            size,
            Color::new(color.r, color.g, color.b, alpha),
        );
        if world.countdown > 0.0 {
            let sub = if world.round > 1 {
                tf("Round {}", &[&world.round])
            } else {
                t("Get ready").to_owned()
            };
            text_center(
                &sub,
                cx,
                y + 54.0 * s,
                24.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.8),
            );
        }
    }

    // Status messages, center.
    match world.phase {
        Phase::MatchOver { winner } => {
            let msg = match winner {
                Some(w) if world.side_of(player) == w => t("VICTORY").to_owned(),
                Some(w) => tf("{} WINS", &[&side_name(world, w).to_uppercase()]),
                None => t("DRAW").to_owned(),
            };
            text_center(&msg, cx, sh * 0.22, 56.0 * s, side_color(world, winner));
        }
        Phase::RoundOver { winner, .. } => {
            let msg = match winner {
                Some(w) => tf("{} takes the round", &[&side_name(world, w)]),
                None => t("Round drawn").to_owned(),
            };
            text_center(&msg, cx, sh * 0.3, 40.0 * s, side_color(world, winner));
        }
        Phase::Playing if !f.alive => {
            // The countdown shows only for the last few seconds.
            let msg = if f.out {
                Some(t("Out of lives - spectating").to_owned())
            } else if !world.respawns() {
                Some(t("Eliminated - waiting for the next round").to_owned())
            } else if f.respawn_timer <= RESPAWN_SHOWN_S {
                Some(tf(
                    "Respawn in {}",
                    &[&(f.respawn_timer.max(0.0).ceil() as i32)],
                ))
            } else {
                None
            };
            if let Some(msg) = msg {
                let y = sh * 0.64;
                let gold = Color::new(1.0, 0.82, 0.25, 1.0);
                text_center(
                    &msg,
                    cx + 2.0 * s,
                    y + 2.0 * s,
                    40.0 * s,
                    Color::new(0.0, 0.0, 0.0, 0.6),
                );
                text_center(&msg, cx, y, 40.0 * s, gold);
            }
        }
        Phase::Playing => {
            if def.projectile.kind == ProjectileKind::Beam && slot.mag == 0 && !f.is_reloading() {
                let msg = if touch {
                    t("Tap Reload")
                } else {
                    t("Press R to reload")
                };
                text_center(msg, cx, sh * 0.62, 20.0 * s, WHITE);
            }
        }
    }
}

/// The shield strip's colour, the same at any strength.
const SHIELD_BAR: Color = Color::new(1.0, 0.84, 0.25, 1.0);

/// Health from green (full) through yellow (half) to red (empty).
fn health_color(frac: f32) -> Color {
    let (green, yellow, red) = (
        Color::new(0.3, 0.85, 0.4, 1.0),
        Color::new(0.95, 0.85, 0.25, 1.0),
        Color::new(0.95, 0.25, 0.2, 1.0),
    );
    let mix = |a: Color, b: Color, k: f32| {
        Color::new(
            a.r + (b.r - a.r) * k,
            a.g + (b.g - a.g) * k,
            a.b + (b.b - a.b) * k,
            1.0,
        )
    };
    if frac >= 0.5 {
        mix(yellow, green, (frac - 0.5) * 2.0)
    } else {
        mix(red, yellow, frac * 2.0)
    }
}

const PANEL_BG: Color = Color::new(0.04, 0.06, 0.08, 0.82);
const PANEL_EDGE: Color = Color::new(0.35, 0.85, 0.9, 0.55);

/// "30 / 150": rounds in the magazine and in reserve.
fn ammo_text(def: &crate::data::WeaponDef, slot: crate::sim::Slot) -> String {
    if !def.uses_ammo() {
        "-".to_owned()
    } else if def.infinite_reserve() {
        format!("{} / inf", slot.mag)
    } else {
        format!("{} / {}", slot.mag, slot.reserve)
    }
}

/// Top centre: "Health 97%", the fighter's name and "Shield 69%" over the
/// health bar (green, fading to yellow and red as it empties), with a thin
/// shield strip under it.
fn health_panel(f: &crate::sim::Fighter, frac: f32, shield: f32, s: f32) {
    let w = 460.0 * s;
    let x = (screen_width() - w) / 2.0;
    let y = 6.0 * s;
    let h = 52.0 * s;
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 1.5 * s, PANEL_EDGE);
    // Slanted stripes on both shoulders.
    for k in 0..3 {
        let o = k as f32 * 9.0 * s;
        for (sx, dir) in [(x - 6.0 * s - o, -1.0), (x + w + 6.0 * s + o, 1.0)] {
            draw_triangle(
                vec2(sx, y + 4.0 * s),
                vec2(sx + dir * 5.0 * s, y + 4.0 * s),
                vec2(sx + dir * 9.0 * s, y + 18.0 * s),
                PANEL_EDGE,
            );
        }
    }
    let size = 17.0 * s;
    let ty = y + 18.0 * s;
    text(
        &tf("Health: {}%", &[&((frac * 100.0).round() as i32)]),
        x + 12.0 * s,
        ty,
        size,
        WHITE,
    );
    text_right(
        &tf("Shield: {}%", &[&((shield * 100.0).round() as i32)]),
        x + w - 12.0 * s,
        ty,
        size,
        WHITE,
    );
    let name = t(&f.name);
    let nw = measure_text(name, None, size as u16, 1.0).width + 28.0 * s;
    draw_rectangle(
        x + (w - nw) / 2.0,
        y,
        nw,
        24.0 * s,
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    text_center(name, x + w / 2.0, ty, size, WHITE);
    let (bx, bw) = (x + 10.0 * s, w - 20.0 * s);
    let by = y + 27.0 * s;
    draw_rectangle(bx, by, bw, 12.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(bx, by, bw * frac, 12.0 * s, health_color(frac));
    draw_rectangle(bx, by, bw * frac, 4.0 * s, Color::new(1.0, 1.0, 1.0, 0.18));
    let sy = by + 15.0 * s;
    draw_rectangle(bx, sy, bw, 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(bx, sy, bw * shield, 4.0 * s, SHIELD_BAR);
}

/// Top left: the weapon in hand, with its clip under it (or how far a
/// reload has got).
fn weapon_panel(world: &World, f: &crate::sim::Fighter, s: f32) {
    let def = &world.data.weapons[f.current];
    let slot = f.inventory[f.current];
    let (x, y, w, h) = (12.0 * s, 6.0 * s, 210.0 * s, 78.0 * s);
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 1.5 * s, PANEL_EDGE);
    let icon = Rect::new(x + 40.0 * s, y + 4.0 * s, 130.0 * s, 46.0 * s);
    super::icons::draw_weapon_icon(def.icon(), icon, WHITE);
    let (bx, by, bw, bh) = (x + 8.0 * s, y + 54.0 * s, w - 16.0 * s, 18.0 * s);
    draw_rectangle(bx, by, bw, bh, Color::new(0.0, 0.0, 0.0, 0.6));
    let (fill, label) = if f.is_reloading() {
        let total = def.reload_time_s.max(0.01);
        (1.0 - f.reload_timer / total, t("RELOADING").to_owned())
    } else if def.uses_ammo() {
        (
            slot.mag as f32 / def.magazine.max(1) as f32,
            tf("Clip: {}", &[&ammo_text(def, slot)]),
        )
    } else {
        (1.0, t(&def.name).to_owned())
    };
    draw_rectangle(
        bx,
        by,
        bw * fill.clamp(0.0, 1.0),
        bh,
        Color::new(0.62, 0.14, 0.14, 0.95),
    );
    text_center(&label, bx + bw / 2.0, by + 14.0 * s, 16.0 * s, WHITE);
}

/// Bottom centre: a slot for each weapon. Held ones stand tall with the
/// weapon's name over its picture and an ammo strip along the top (greyed
/// out once empty); the one in hand stands tallest; the rest are low stubs.
/// Gun Game hands out its own weapons, so there it shows just those.
fn weapon_slots(world: &World, f: &crate::sim::Fighter, s: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let gun_game = !world.ladder.is_empty();
    let slots: Vec<usize> = (0..world.data.weapons.len())
        .filter(|&w| f.owns(w) || (world.data.weapons[w].loot && !gun_game))
        .take(10)
        .collect();
    let (bw, bh, gap) = (84.0 * s, 62.0 * s, 6.0 * s);
    let total = slots.len() as f32 * (bw + gap) - gap;
    let x0 = (sw - total) / 2.0;
    let bottom = sh - 8.0 * s;
    for (k, &w) in slots.iter().enumerate() {
        let x = x0 + k as f32 * (bw + gap);
        let d = &world.data.weapons[w];
        // Number keys pick by slot: 1 to 9, then 0.
        let key = match w {
            0..9 => (w + 1).to_string(),
            9 => "0".to_owned(),
            _ => String::new(),
        };
        if !f.owns(w) {
            let h = 16.0 * s;
            draw_rectangle(x, bottom - h, bw, h, Color::new(0.0, 0.0, 0.0, 0.5));
            text(
                &key,
                x + 4.0 * s,
                bottom - 3.0 * s,
                12.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.35),
            );
            continue;
        }
        let current = w == f.current;
        let slot = f.inventory[w];
        let full = d.magazine + d.reserve_ammo;
        let left = if !d.uses_ammo() || d.infinite_reserve() {
            1.0
        } else {
            (slot.mag + slot.reserve) as f32 / full.max(1) as f32
        };
        let empty = left <= 0.0;
        let h = if current { bh + 10.0 * s } else { bh };
        let y = bottom - h;
        let cc = if empty {
            Color::new(0.55, 0.55, 0.58, 1.0)
        } else {
            category_color(d.category)
        };
        let bg = match (current, empty) {
            (_, true) => Color::new(0.35, 0.35, 0.38, 0.6),
            (true, _) => Color::new(0.16, 0.2, 0.24, 0.92),
            _ => PANEL_BG,
        };
        draw_rectangle(x, y, bw, h, bg);
        draw_rectangle_lines(
            x,
            y,
            bw,
            h,
            if current { 2.0 } else { 1.0 } * s,
            if current {
                cc
            } else {
                Color::new(1.0, 1.0, 1.0, 0.2)
            },
        );
        // Ammo left, along the top.
        draw_rectangle(x, y, bw, 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(
            x,
            y,
            bw * left.clamp(0.0, 1.0),
            4.0 * s,
            Color::new(0.85, 0.2, 0.18, 1.0),
        );
        let ink = if empty {
            Color::new(1.0, 1.0, 1.0, 0.45)
        } else {
            WHITE
        };
        text(&key, x + 4.0 * s, y + 17.0 * s, 13.0 * s, ink);
        let name = t(&d.short_name()).to_owned();
        text_center(&name, x + bw / 2.0 + 5.0 * s, y + 18.0 * s, 13.0 * s, ink);
        let icon = Rect::new(x + 8.0 * s, y + 22.0 * s, bw - 16.0 * s, h - 28.0 * s);
        let tint = if empty {
            cc
        } else {
            Color::new(0.5 + cc.r * 0.5, 0.5 + cc.g * 0.5, 0.5 + cc.b * 0.5, 1.0)
        };
        super::icons::draw_weapon_icon(d.icon(), icon, tint);
    }
}

/// Under the score: who holds the hill; each zone's holder and capture
/// progress; or where your flag is.
fn objective_status(world: &World, player: usize, cx: f32, y: f32, s: f32) {
    let faint = Color::new(1.0, 1.0, 1.0, 0.75);
    match world.mode.rules {
        Rules::KingOfTheHill => {
            let Some(h) = world.hill else {
                return;
            };
            let z = &world.zones[h];
            let (mut msg, c) = match z.owner {
                Some(side) => (
                    tf("{} holds the hill", &[&side_name(world, side)]),
                    zone_side_color(world, side),
                ),
                None if z.present.len() > 1 => (t("The hill is contested").to_owned(), WHITE),
                None => (t("The hill is free").to_owned(), faint),
            };
            if world.hill_timer.is_finite() {
                msg += "    ";
                msg += &tf("Moves in {}", &[&clock(world.hill_timer)]);
            }
            text_center(&msg, cx, y, 18.0 * s, c);
        }
        Rules::Domination | Rules::Territories => {
            let n = world.zones.len();
            let (w, h, gap) = (30.0 * s, 22.0 * s, 6.0 * s);
            let x0 = cx - (n as f32 * (w + gap) - gap) / 2.0;
            let top = y - 16.0 * s;
            for (k, z) in world.zones.iter().enumerate() {
                let x = x0 + k as f32 * (w + gap);
                let bg = z.owner.map_or(Color::new(0.0, 0.0, 0.0, 0.5), |side| {
                    zone_side_color(world, side)
                });
                draw_rectangle(x, top, w, h, bg);
                let edge = if z.present.len() > 1 {
                    WHITE
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.35)
                };
                draw_rectangle_lines(x, top, w, h, 1.5 * s, edge);
                text_center(
                    &zone_letter(k),
                    x + w / 2.0,
                    top + 16.0 * s,
                    17.0 * s,
                    WHITE,
                );
                if let Some(side) = z.capturer {
                    draw_rectangle(
                        x,
                        top + h + 2.0 * s,
                        w * z.progress.min(1.0),
                        3.0 * s,
                        zone_side_color(world, side),
                    );
                }
            }
        }
        Rules::CaptureTheFlag => {
            let side = world.side_of(player);
            let mine = world.flags.iter().find(|f| f.side == side);
            let (msg, c) = if world.carried_flag(player).is_some() {
                (
                    t("You have the flag: take it home!"),
                    Color::new(1.0, 0.82, 0.3, 1.0),
                )
            } else {
                match mine {
                    Some(f) if f.carrier.is_some() => (
                        t("Your flag has been taken!"),
                        Color::new(1.0, 0.5, 0.4, 1.0),
                    ),
                    Some(f) if f.return_in.is_some() => {
                        (t("Your flag is down: touch it to send it home"), WHITE)
                    }
                    _ => (t("Your flag is home"), faint),
                }
            };
            text_center(msg, cx, y, 18.0 * s, c);
        }
        _ => {}
    }
}

/// The colour of a side's banner: its team colour in team modes.
fn side_color(world: &World, side: Option<usize>) -> Color {
    match side {
        Some(t) if world.mode.is_team() => TEAM_COLORS[world.team_color(t)],
        _ => WHITE,
    }
}

pub fn draw_scoreboard(world: &World, player: Option<usize>) {
    if world.mode.is_team() {
        draw_team_scoreboard(world, player);
    } else {
        draw_ffa_scoreboard(world, player);
    }
}

/// Fighters sorted best first: most kills, then fewest deaths.
fn ranked(world: &World, keep: impl Fn(usize) -> bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..world.fighters.len()).filter(|&i| keep(i)).collect();
    order.sort_by_key(|&i| {
        let f = &world.fighters[i];
        (std::cmp::Reverse(f.kills), f.deaths, i)
    });
    order
}

/// One fighter's line: colour tag, name, kills, deaths and score.
#[allow(clippy::too_many_arguments)]
fn score_row(
    world: &World,
    i: usize,
    player: Option<usize>,
    x: f32,
    w: f32,
    ry: f32,
    cols: [f32; 3],
    score: i32,
) {
    let s = ui_scale();
    let f = &world.fighters[i];
    if Some(i) == player {
        draw_rectangle(
            x + 6.0 * s,
            ry - 19.0 * s,
            w - 12.0 * s,
            26.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.08),
        );
    }
    draw_rectangle(
        x + 14.0 * s,
        ry - 13.0 * s,
        8.0 * s,
        14.0 * s,
        fighter_color(f),
    );
    let name_col = if f.alive {
        WHITE
    } else {
        Color::new(1.0, 1.0, 1.0, 0.5)
    };
    text(t(&f.name), x + 30.0 * s, ry, 20.0 * s, name_col);
    text(&f.kills.to_string(), cols[0], ry, 20.0 * s, WHITE);
    text(&f.deaths.to_string(), cols[1], ry, 20.0 * s, WHITE);
    text(&score.to_string(), cols[2], ry, 20.0 * s, WHITE);
}

fn draw_ffa_scoreboard(world: &World, player: Option<usize>) {
    let s = ui_scale();
    let (sw, sh) = (screen_width(), screen_height());
    let order = ranked(world, |_| true);
    let w = 620.0 * s;
    let h = (110.0 + order.len() as f32 * 28.0) * s;
    let (x, y) = ((sw - w) / 2.0, (sh - h) / 2.0);
    draw_rectangle(x, y, w, h, Color::new(0.05, 0.06, 0.09, 0.88));
    draw_rectangle_lines(x, y, w, h, 2.0 * s, Color::new(1.0, 1.0, 1.0, 0.2));
    let title = format!("{}  -  {}", match_name(world), t(&world.map.name));
    text_center(&title, sw / 2.0, y + 36.0 * s, 26.0 * s, WHITE);
    let cols = [x + w - 230.0 * s, x + w - 150.0 * s, x + w - 70.0 * s];
    let hy = y + 70.0 * s;
    let head = Color::new(1.0, 1.0, 1.0, 0.55);
    text(t("NAME"), x + 30.0 * s, hy, 16.0 * s, head);
    for (label, cx) in [t("KILLS"), t("DEATHS"), t("SCORE")].iter().zip(cols) {
        text(label, cx, hy, 16.0 * s, head);
    }
    for (row, &i) in order.iter().enumerate() {
        let ry = hy + (row as f32 + 1.0) * 28.0 * s;
        let score = world.scores[world.side_of(i)];
        score_row(world, i, player, x, w, ry, cols, score);
    }
}

/// Team modes: one panel per team, two panels to a row.
fn draw_team_scoreboard(world: &World, player: Option<usize>) {
    let s = ui_scale();
    let (sw, sh) = (screen_width(), screen_height());
    let teams = world.num_sides();
    let members: Vec<Vec<usize>> = (0..teams)
        .map(|t| ranked(world, |i| world.side_of(i) == t))
        .collect();
    let grid_rows = teams.div_ceil(2);
    let most = members.iter().map(Vec::len).max().unwrap_or(0) as f32;
    let (gap, row_h) = (14.0 * s, 26.0 * s);
    let panel_w = 400.0 * s;
    let panel_h = 76.0 * s + most * row_h;
    let w = panel_w * 2.0 + gap * 3.0;
    let h = 64.0 * s + grid_rows as f32 * (panel_h + gap);
    let (x, y) = ((sw - w) / 2.0, (sh - h) / 2.0);
    draw_rectangle(x, y, w, h, Color::new(0.05, 0.06, 0.09, 0.88));
    draw_rectangle_lines(x, y, w, h, 2.0 * s, Color::new(1.0, 1.0, 1.0, 0.2));
    let title = format!("{}  -  {}", match_name(world), t(&world.map.name));
    text_center(&title, sw / 2.0, y + 38.0 * s, 26.0 * s, WHITE);

    let head = Color::new(1.0, 1.0, 1.0, 0.55);
    for (team, list) in members.iter().enumerate() {
        let px = x + gap + (team % 2) as f32 * (panel_w + gap);
        let py = y + 56.0 * s + (team / 2) as f32 * (panel_h + gap);
        let c = TEAM_COLORS[world.team_color(team)];
        draw_rectangle(px, py, panel_w, panel_h, Color::new(c.r, c.g, c.b, 0.07));
        draw_rectangle(px, py, panel_w, 34.0 * s, Color::new(c.r, c.g, c.b, 0.2));
        draw_rectangle(px, py, 5.0 * s, panel_h, c);
        let total = if world.mode.respawn {
            world.scores[team]
        } else if world.uses_lives() {
            match world.lives_limit {
                Some(_) => world.side_lives(team) as i32,
                None => world.scores[team],
            }
        } else {
            world.round_wins[team] as i32
        };
        text(
            &tf("{} Team", &[&side_name(world, team)]),
            px + 16.0 * s,
            py + 24.0 * s,
            22.0 * s,
            c,
        );
        text_right(
            &total.to_string(),
            px + panel_w - 14.0 * s,
            py + 24.0 * s,
            24.0 * s,
            c,
        );
        let cols = [
            px + panel_w - 150.0 * s,
            px + panel_w - 100.0 * s,
            px + panel_w - 46.0 * s,
        ];
        let hy = py + 54.0 * s;
        text(t("NAME"), px + 30.0 * s, hy, 14.0 * s, head);
        for (label, cx) in [t("K"), t("D"), t("PTS")].iter().zip(cols) {
            text(label, cx, hy, 14.0 * s, head);
        }
        for (row, &i) in list.iter().enumerate() {
            let ry = hy + (row as f32 + 1.0) * row_h;
            let pts = world.fighters[i].kills as i32;
            score_row(world, i, player, px, panel_w, ry, cols, pts);
        }
    }
}
