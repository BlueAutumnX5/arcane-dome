use super::{BLUE, Effects, GameCamera, RED, category_color, fighter_color};
use crate::data::ProjectileKind;
use crate::sim::{MAX_HEALTH, Phase, World};
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

fn text_right(t: &str, rx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    text(t, rx - d.width, y, size, color);
}

fn clock(secs: f32) -> String {
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
    let line = if !mode.respawn {
        let alive = |t: u8| {
            world
                .fighters
                .iter()
                .filter(|f| f.alive && f.team == Some(t))
                .count()
        };
        if mode.is_team() {
            format!(
                "Round {}/{}   Blue {} - {} Red   (alive {} v {})",
                world.round,
                mode.rounds.unwrap_or(1),
                world.round_wins[0],
                world.round_wins[1],
                alive(0),
                alive(1)
            )
        } else {
            format!("Round {}/{}", world.round, mode.rounds.unwrap_or(1))
        }
    } else if mode.is_team() {
        format!(
            "Blue {}  -  {} Red     to {}",
            world.scores[0],
            world.scores[1],
            mode.score_limit.unwrap_or(0)
        )
    } else {
        let mine = world.scores[world.side_of(player)];
        let (leader, best) = (0..world.num_sides())
            .map(|s| (s, world.scores[s]))
            .max_by_key(|&(s, v)| (v, std::cmp::Reverse(s)))
            .unwrap_or((0, 0));
        format!(
            "You {}    Leader {} {}    to {}",
            mine,
            world.side_name(leader),
            best,
            mode.score_limit.unwrap_or(0)
        )
    };
    text_center(
        &line,
        cx,
        58.0 * s,
        20.0 * s,
        Color::new(1.0, 1.0, 1.0, 0.9),
    );
    if mode.is_team() {
        draw_rectangle(cx - 150.0 * s, 64.0 * s, 8.0 * s, 4.0 * s, BLUE);
        draw_rectangle(cx + 142.0 * s, 64.0 * s, 8.0 * s, 4.0 * s, RED);
    }

    // Kill feed, top right.
    for (i, l) in effects.feed.iter().enumerate() {
        let a = (1.0 - (l.age - 5.0).max(0.0)).clamp(0.0, 1.0);
        let col = if l.mine {
            Color::new(1.0, 0.9, 0.5, a)
        } else {
            Color::new(1.0, 1.0, 1.0, a * 0.85)
        };
        text_right(
            &l.text,
            sw - 16.0 * s,
            28.0 * s + i as f32 * 22.0 * s,
            18.0 * s,
            col,
        );
    }

    // Status messages, center.
    match world.phase {
        Phase::MatchOver { winner } => {
            let msg = match winner {
                Some(w) if world.side_of(player) == w => "VICTORY".to_owned(),
                Some(w) => format!("{} WINS", world.side_name(w).to_uppercase()),
                None => "DRAW".to_owned(),
            };
            text_center(&msg, cx, sh * 0.22, 56.0 * s, WHITE);
        }
        Phase::RoundOver { winner, .. } => {
            let msg = match winner {
                Some(w) => format!("{} takes the round", world.side_name(w)),
                None => "Round drawn".to_owned(),
            };
            text_center(&msg, cx, sh * 0.3, 40.0 * s, WHITE);
        }
        Phase::Playing if !f.alive => {
            let msg = if mode.respawn {
                format!("Respawning in {:.1}", f.respawn_timer.max(0.0))
            } else {
                "Eliminated - waiting for the next round".to_owned()
            };
            text_center(&msg, cx, sh * 0.35, 32.0 * s, WHITE);
        }
        Phase::Playing => {
            if def.projectile.kind == ProjectileKind::Beam && slot.mag == 0 && !f.is_reloading() {
                text_center("Press R to reload", cx, sh * 0.62, 20.0 * s, WHITE);
            }
        }
    }
}

pub fn draw_scoreboard(world: &World, player: Option<usize>) {
    let s = ui_scale();
    let sw = screen_width();
    let sh = screen_height();
    let w = 620.0 * s;
    let rows = world.fighters.len();
    let h = (110.0 + rows as f32 * 28.0) * s;
    let x = (sw - w) / 2.0;
    let y = (sh - h) / 2.0;
    draw_rectangle(x, y, w, h, Color::new(0.05, 0.06, 0.09, 0.88));
    draw_rectangle_lines(x, y, w, h, 2.0 * s, Color::new(1.0, 1.0, 1.0, 0.2));
    let title = format!("{}  -  {}", world.mode.display_name(), world.map.name);
    text_center(&title, sw / 2.0, y + 36.0 * s, 26.0 * s, WHITE);

    let col_name = x + 30.0 * s;
    let col_k = x + w - 230.0 * s;
    let col_d = x + w - 150.0 * s;
    let col_s = x + w - 70.0 * s;
    let hy = y + 70.0 * s;
    let head = Color::new(1.0, 1.0, 1.0, 0.55);
    text("NAME", col_name, hy, 16.0 * s, head);
    text("KILLS", col_k, hy, 16.0 * s, head);
    text("DEATHS", col_d, hy, 16.0 * s, head);
    text("SCORE", col_s, hy, 16.0 * s, head);

    let mut order: Vec<usize> = (0..rows).collect();
    order.sort_by_key(|&i| {
        let f = &world.fighters[i];
        (f.team, std::cmp::Reverse(f.kills), f.deaths, i)
    });
    for (row, &i) in order.iter().enumerate() {
        let f = &world.fighters[i];
        let ry = hy + (row as f32 + 1.0) * 28.0 * s;
        if Some(i) == player {
            draw_rectangle(
                x + 10.0 * s,
                ry - 19.0 * s,
                w - 20.0 * s,
                26.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.08),
            );
        }
        let c = fighter_color(f);
        draw_rectangle(col_name - 16.0 * s, ry - 13.0 * s, 8.0 * s, 14.0 * s, c);
        let name_col = if f.alive {
            WHITE
        } else {
            Color::new(1.0, 1.0, 1.0, 0.5)
        };
        text(&f.name, col_name, ry, 20.0 * s, name_col);
        text(&f.kills.to_string(), col_k, ry, 20.0 * s, WHITE);
        text(&f.deaths.to_string(), col_d, ry, 20.0 * s, WHITE);
        let score = if world.mode.is_team() {
            f.kills as i32
        } else {
            world.scores[world.side_of(i)]
        };
        text(&score.to_string(), col_s, ry, 20.0 * s, WHITE);
    }
}
