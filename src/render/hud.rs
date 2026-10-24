use super::{Effects, GameCamera, RED, TEAM_COLORS, category_color, fighter_color};
use crate::data::ProjectileKind;
use crate::sim::{GO_SHOWN_S, MAX_HEALTH, MAX_SHIELD, Phase, World};
use macroquad::prelude::*;

fn ui_scale() -> f32 {
    (screen_height() / 720.0).max(0.6)
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

fn text_right(t: &str, rx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    text(t, rx - d.width, y, size, color);
}

/// "to 25", or "no limit" when the score limit is off.
fn limit_text(limit: Option<u32>) -> String {
    limit.map_or("no limit".into(), |l| format!("to {l}"))
}

/// The name of the Elimination style being played.
fn style_text(world: &World) -> String {
    match world.style {
        crate::sim::ElimStyle::Attrition => "Attrition".into(),
        crate::sim::ElimStyle::LastManStanding => "Last Man Standing".into(),
        crate::sim::ElimStyle::Rounds => {
            format!("Round {}/{}", world.round, world.mode.rounds.unwrap_or(1))
        }
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

pub fn draw_hud(world: &World, effects: &Effects, player: usize) {
    let s = ui_scale();
    let sw = screen_width();
    let sh = screen_height();
    let f = &world.fighters[player];

    // Blood: builds up as health drops (static, up to a quarter strength at
    // 1 HP), then plays out in full when the fighter dies. One pattern per
    // life, so the death splatter grows out of the stains already there.
    if !f.alive && world.phase == Phase::Playing {
        let life = f.deaths.saturating_sub(1);
        draw_splatter(f.id, life, 1.0, Some(world.time - f.died_at));
    } else if f.alive && f.health < MAX_HEALTH {
        let hurt = ((MAX_HEALTH - f.health) / (MAX_HEALTH - 1.0)).clamp(0.0, 1.0);
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

    // Health, bottom left.
    let (hx, hy, hw, hh) = (20.0 * s, sh - 44.0 * s, 240.0 * s, 22.0 * s);
    draw_rectangle(hx, hy, hw, hh, Color::new(0.0, 0.0, 0.0, 0.55));
    let frac = (f.health / MAX_HEALTH).clamp(0.0, 1.0);
    let hc = if frac > 0.5 {
        Color::new(0.35, 0.85, 0.45, 1.0)
    } else if frac > 0.25 {
        Color::new(0.95, 0.75, 0.25, 1.0)
    } else {
        Color::new(0.95, 0.3, 0.25, 1.0)
    };
    draw_rectangle(hx, hy, hw * frac, hh, hc);
    text(
        &format!("{:.0}", f.health.max(0.0)),
        hx + 8.0 * s,
        hy + 17.0 * s,
        22.0 * s,
        WHITE,
    );

    // Shield, just above health.
    if f.shield > 0.0 {
        let sy = hy - 10.0 * s;
        draw_rectangle(hx, sy, hw, 6.0 * s, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(
            hx,
            sy,
            hw * (f.shield / MAX_SHIELD).clamp(0.0, 1.0),
            6.0 * s,
            super::gadgets::SHIELD_COLOR,
        );
        text(
            &format!("Shield {:.0}", f.shield),
            hx + hw + 8.0 * s,
            sy + 8.0 * s,
            16.0 * s,
            super::gadgets::SHIELD_COLOR,
        );
    }

    // Active powerups, stacked above health.
    let mut row = 0.0;
    for b in &f.buffs {
        if let Some(p) = world.data.powerups.powerups.get(b.powerup) {
            let c = super::gadgets::rgb(p.color);
            let y = hy - (34.0 + row * 24.0) * s;
            draw_rectangle(hx, y, 14.0 * s, 14.0 * s, c);
            text(
                &format!("{}  {:.0}s", p.name, b.time_left.ceil()),
                hx + 22.0 * s,
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
        let y = hy - (34.0 + row * 24.0) * s;
        draw_rectangle(hx, y, 14.0 * s, 14.0 * s, c);
        text(
            &format!("{} x{}  [F]", p.name, f.teleport_charges),
            hx + 22.0 * s,
            y + 13.0 * s,
            18.0 * s,
            c,
        );
    }

    // Ammo, bottom right.
    let def = &world.data.weapons[f.current];
    let slot = f.inventory[f.current];
    let rx = sw - 20.0 * s;
    text_right(
        &def.name,
        rx,
        sh - 52.0 * s,
        22.0 * s,
        category_color(def.category),
    );
    let ammo = if !def.uses_ammo() {
        "-".to_owned()
    } else if def.infinite_reserve() {
        format!("{} / inf", slot.mag)
    } else {
        format!("{} / {}", slot.mag, slot.reserve)
    };
    text_right(&ammo, rx, sh - 22.0 * s, 30.0 * s, WHITE);
    if f.is_reloading() {
        let total = def.reload_time_s.max(0.01);
        let p = 1.0 - f.reload_timer / total;
        let w = 140.0 * s;
        draw_rectangle(
            rx - w,
            sh - 78.0 * s,
            w,
            5.0 * s,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
        draw_rectangle(rx - w, sh - 78.0 * s, w * p, 5.0 * s, WHITE);
        text_right("RELOADING", rx, sh - 84.0 * s, 16.0 * s, WHITE);
    }

    // Weapon bar, bottom center.
    let n = world.data.weapons.len().min(10);
    let bw = 66.0 * s;
    let bh = 38.0 * s;
    let gap = 4.0 * s;
    let total = n as f32 * (bw + gap) - gap;
    let bx = (sw - total) / 2.0;
    let by = sh - bh - 14.0 * s;
    for w in 0..n {
        let x = bx + w as f32 * (bw + gap);
        let d = &world.data.weapons[w];
        let owned = f.owns(w);
        let current = w == f.current;
        let bg = if current {
            Color::new(1.0, 1.0, 1.0, 0.22)
        } else {
            Color::new(0.0, 0.0, 0.0, 0.45)
        };
        draw_rectangle(x, by, bw, bh, bg);
        let cc = category_color(d.category);
        if current {
            draw_rectangle_lines(x, by, bw, bh, 2.0 * s, cc);
        }
        let alpha = if owned { 1.0 } else { 0.28 };
        let key = if w == 9 {
            "0".to_owned()
        } else {
            (w + 1).to_string()
        };
        text(
            &key,
            x + 4.0 * s,
            by + 13.0 * s,
            14.0 * s,
            Color::new(1.0, 1.0, 1.0, alpha * 0.7),
        );
        let name = d.short_name();
        let size = 14.0 * s;
        let dims = measure_text(&name, None, size as u16, 1.0);
        text(
            &name,
            x + (bw - dims.width) / 2.0,
            by + 31.0 * s,
            size,
            Color::new(cc.r, cc.g, cc.b, alpha),
        );
    }

    // Score and timer, top center.
    let cx = sw / 2.0;
    let mode = &world.mode;
    let time = if matches!(world.phase, Phase::RoundOver { .. }) {
        "--:--".to_owned()
    } else {
        clock(world.time_left)
    };
    text_center(&time, cx, 34.0 * s, 30.0 * s, WHITE);
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
                let label = if mode.respawn {
                    format!("{} {}", world.side_name(t), world.scores[t])
                } else if world.uses_lives() {
                    match world.lives_limit {
                        Some(_) => format!("{} {} lives", world.side_name(t), world.side_lives(t)),
                        None => format!("{} {}", world.side_name(t), world.scores[t]),
                    }
                } else {
                    format!(
                        "{} {} ({} up)",
                        world.side_name(t),
                        world.round_wins[t],
                        alive(t)
                    )
                };
                (label, TEAM_COLORS[world.team_color(t)])
            })
            .collect();
        let goal = if mode.respawn {
            limit_text(mode.score_limit)
        } else if world.uses_lives() {
            style_text(world)
        } else {
            format!("Round {}/{}", world.round, mode.rounds.unwrap_or(1))
        };
        parts.push((goal, Color::new(1.0, 1.0, 1.0, 0.75)));
        segments_center(&parts, cx, 58.0 * s, 20.0 * s, 22.0 * s);
    }
    let line = if mode.is_team() {
        String::new()
    } else if world.uses_lives() {
        let left = (0..world.num_sides())
            .filter(|&s| !world.fighters[s].out)
            .count();
        match world.lives_limit {
            Some(_) => format!(
                "Lives {}    {} still in    {}",
                world.lives_left[player],
                left,
                style_text(world)
            ),
            None => format!("Kills {}    {}", world.scores[player], style_text(world)),
        }
    } else if !mode.respawn {
        format!("Round {}/{}", world.round, mode.rounds.unwrap_or(1))
    } else {
        let mine = world.scores[world.side_of(player)];
        let (leader, best) = (0..world.num_sides())
            .map(|s| (s, world.scores[s]))
            .max_by_key(|&(s, v)| (v, std::cmp::Reverse(s)))
            .unwrap_or((0, 0));
        format!(
            "You {}    Leader {} {}    {}",
            mine,
            world.side_name(leader),
            best,
            limit_text(mode.score_limit)
        )
    };
    text_center(
        &line,
        cx,
        58.0 * s,
        20.0 * s,
        Color::new(1.0, 1.0, 1.0, 0.9),
    );

    // Kill feed, top right.
    for (i, l) in effects.feed.iter().enumerate() {
        let a = (1.0 - (l.age - 5.0).max(0.0)).clamp(0.0, 1.0);
        let size = 18.0 * s;
        let gap = 7.0 * s;
        let y = 28.0 * s + i as f32 * 22.0 * s;
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
            ("GO!".to_owned(), k, Color::new(0.55, 1.0, 0.6, 1.0))
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
                format!("Round {}", world.round)
            } else {
                "Get ready".to_owned()
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
                Some(w) if world.side_of(player) == w => "VICTORY".to_owned(),
                Some(w) => format!("{} WINS", world.side_name(w).to_uppercase()),
                None => "DRAW".to_owned(),
            };
            text_center(&msg, cx, sh * 0.22, 56.0 * s, side_color(world, winner));
        }
        Phase::RoundOver { winner, .. } => {
            let msg = match winner {
                Some(w) => format!("{} takes the round", world.side_name(w)),
                None => "Round drawn".to_owned(),
            };
            text_center(&msg, cx, sh * 0.3, 40.0 * s, side_color(world, winner));
        }
        Phase::Playing if !f.alive => {
            // The countdown shows only for the last few seconds.
            let msg = if f.out {
                Some("Out of lives - spectating".to_owned())
            } else if !world.respawns() {
                Some("Eliminated - waiting for the next round".to_owned())
            } else if f.respawn_timer <= RESPAWN_SHOWN_S {
                Some(format!(
                    "Respawn in {}",
                    f.respawn_timer.max(0.0).ceil() as i32
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
                text_center("Press R to reload", cx, sh * 0.62, 20.0 * s, WHITE);
            }
        }
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
    text(&f.name, x + 30.0 * s, ry, 20.0 * s, name_col);
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
    let title = format!("{}  -  {}", world.mode.match_name(), world.map.name);
    text_center(&title, sw / 2.0, y + 36.0 * s, 26.0 * s, WHITE);
    let cols = [x + w - 230.0 * s, x + w - 150.0 * s, x + w - 70.0 * s];
    let hy = y + 70.0 * s;
    let head = Color::new(1.0, 1.0, 1.0, 0.55);
    text("NAME", x + 30.0 * s, hy, 16.0 * s, head);
    for (label, cx) in ["KILLS", "DEATHS", "SCORE"].iter().zip(cols) {
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
    let title = format!("{}  -  {}", world.mode.match_name(), world.map.name);
    text_center(&title, sw / 2.0, y + 38.0 * s, 26.0 * s, WHITE);

    let head = Color::new(1.0, 1.0, 1.0, 0.55);
    for (t, list) in members.iter().enumerate() {
        let px = x + gap + (t % 2) as f32 * (panel_w + gap);
        let py = y + 56.0 * s + (t / 2) as f32 * (panel_h + gap);
        let c = TEAM_COLORS[world.team_color(t)];
        draw_rectangle(px, py, panel_w, panel_h, Color::new(c.r, c.g, c.b, 0.07));
        draw_rectangle(px, py, panel_w, 34.0 * s, Color::new(c.r, c.g, c.b, 0.2));
        draw_rectangle(px, py, 5.0 * s, panel_h, c);
        let total = if world.mode.respawn {
            world.scores[t]
        } else if world.uses_lives() {
            match world.lives_limit {
                Some(_) => world.side_lives(t) as i32,
                None => world.scores[t],
            }
        } else {
            world.round_wins[t] as i32
        };
        text(
            &format!("{} Team", world.side_name(t)),
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
        text("NAME", px + 30.0 * s, hy, 14.0 * s, head);
        for (label, cx) in ["K", "D", "PTS"].iter().zip(cols) {
            text(label, cx, hy, 14.0 * s, head);
        }
        for (row, &i) in list.iter().enumerate() {
            let ry = hy + (row as f32 + 1.0) * row_h;
            let pts = world.fighters[i].kills as i32;
            score_row(world, i, player, px, panel_w, ry, cols, pts);
        }
    }
}
