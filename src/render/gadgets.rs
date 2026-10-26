//! Drawing thrusters, stations, the powerup spot and powerup glows.

use super::{GameCamera, category_color};
use crate::data::ObjectKind;
use crate::i18n::{t, tf};
use crate::sim::{Fighter, ObjectState, World};
use macroquad::prelude::*;

pub const SHIELD_COLOR: Color = Color::new(0.35, 0.85, 1.0, 1.0);
const THRUST_COLOR: Color = Color::new(0.3, 1.0, 0.75, 1.0);

pub fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

/// World-space drawing of every map object (camera already set).
pub fn draw_objects(world: &World) {
    let t = world.time;
    for o in &world.objects {
        // Art is laid out in a one-tile box whose bottom-centre is the feet position.
        let (x, y) = (o.pos.x - 0.5, o.pos.y - 1.0);
        match o.def.kind {
            ObjectKind::Thruster => draw_thruster(x, y, t),
            ObjectKind::WeaponStation | ObjectKind::ShieldStation => draw_station(world, o, t),
            ObjectKind::PowerupSpot => draw_powerup_spot(world, o, t),
        }
    }
}

fn draw_thruster(x: f32, y: f32, t: f32) {
    draw_rectangle(
        x + 0.05,
        y + 0.78,
        0.9,
        0.22,
        Color::from_rgba(40, 44, 56, 255),
    );
    draw_rectangle(x + 0.12, y + 0.72, 0.76, 0.08, THRUST_COLOR);
    // Rising chevrons.
    for k in 0..3 {
        let phase = ((t * 1.6 + k as f32 / 3.0) % 1.0).abs();
        let cy = y + 0.6 - phase * 1.4;
        let a = (1.0 - phase) * 0.8;
        let c = Color::new(THRUST_COLOR.r, THRUST_COLOR.g, THRUST_COLOR.b, a);
        draw_line(x + 0.2, cy + 0.18, x + 0.5, cy, 0.07, c);
        draw_line(x + 0.5, cy, x + 0.8, cy + 0.18, 0.07, c);
    }
}

fn draw_station(world: &World, o: &ObjectState, t: f32) {
    let (x, y) = (o.pos.x - 0.5, o.pos.y - 1.0);
    let accent = match (o.def.kind, o.weapon) {
        (ObjectKind::ShieldStation, _) => SHIELD_COLOR,
        (_, Some(w)) => category_color(world.data.weapons[w].category),
        _ => Color::new(0.7, 0.7, 0.75, 1.0),
    };
    // Base plate.
    draw_rectangle(
        x + 0.05,
        y + 0.84,
        0.9,
        0.16,
        Color::from_rgba(46, 50, 64, 255),
    );
    draw_rectangle(x + 0.15, y + 0.8, 0.7, 0.05, accent);
    if o.ready {
        // Pod raised and open, item floating above it.
        let bob = (t * 3.0).sin() * 0.05;
        draw_rectangle(
            x + 0.2,
            y + 0.55,
            0.6,
            0.26,
            Color::from_rgba(60, 66, 84, 255),
        );
        draw_rectangle_lines(x + 0.2, y + 0.55, 0.6, 0.26, 0.04, accent);
        let c = o.item_pos() + vec2(0.0, bob - 0.15);
        if o.def.kind == ObjectKind::ShieldStation {
            let big = o.def.shield_amount() >= 100.0;
            let r = if big { 0.26 } else { 0.18 };
            draw_poly(c.x, c.y, 6, r, 30.0, Color::new(0.35, 0.85, 1.0, 0.85));
            draw_poly_lines(c.x, c.y, 6, r + 0.06, 30.0, 0.04, WHITE);
        } else {
            draw_rectangle(
                c.x - 0.3,
                c.y - 0.1,
                0.6,
                0.2,
                Color::from_rgba(40, 42, 50, 255),
            );
            draw_rectangle(c.x - 0.1, c.y - 0.04, 0.4, 0.08, accent);
        }
        let glow = 0.25 + 0.15 * (t * 4.0).sin();
        draw_rectangle(
            x + 0.2,
            y + 0.3,
            0.6,
            0.25,
            Color::new(accent.r, accent.g, accent.b, glow * 0.4),
        );
    } else {
        // Charging: closed pod with a progress bar.
        draw_rectangle(
            x + 0.25,
            y + 0.66,
            0.5,
            0.15,
            Color::from_rgba(52, 56, 70, 255),
        );
        draw_rectangle(x + 0.1, y + 0.6, 0.8, 0.04, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(x + 0.1, y + 0.6, 0.8 * o.progress(), 0.04, accent);
    }
}

fn draw_powerup_spot(world: &World, o: &ObjectState, t: f32) {
    let (x, y) = (o.pos.x - 0.5, o.pos.y - 1.0);
    draw_rectangle(
        x + 0.15,
        y + 0.85,
        0.7,
        0.15,
        Color::from_rgba(70, 60, 90, 255),
    );
    draw_rectangle(
        x + 0.3,
        y + 0.78,
        0.4,
        0.08,
        Color::from_rgba(110, 95, 140, 255),
    );
    let Some(p) = o.powerup.filter(|_| o.ready) else {
        // Waiting: a faint ring, tinted if this spot always gives the same powerup.
        let ring = match o.fixed_powerup {
            Some(p) => {
                let c = rgb(world.data.powerups.powerups[p].color);
                Color::new(c.r, c.g, c.b, 0.3)
            }
            None => Color::new(1.0, 1.0, 1.0, 0.12),
        };
        draw_circle_lines(x + 0.5, y + 0.4, 0.22, 0.03, ring);
        return;
    };
    let col = rgb(world.data.powerups.powerups[p].color);
    let c = vec2(x + 0.5, y + 0.25 + (t * 2.5).sin() * 0.08);
    let pulse = 0.5 + 0.5 * (t * 5.0).sin();
    draw_circle(
        c.x,
        c.y,
        0.55 + pulse * 0.1,
        Color::new(col.r, col.g, col.b, 0.18),
    );
    draw_circle(c.x, c.y, 0.32, col);
    draw_circle(c.x - 0.08, c.y - 0.08, 0.1, Color::new(1.0, 1.0, 1.0, 0.6));
    draw_circle_lines(
        c.x,
        c.y,
        0.4,
        0.04,
        Color::new(1.0, 1.0, 1.0, 0.5 + pulse * 0.3),
    );
}

/// Colored glow around a fighter for each active powerup (and a held Shadowstep).
pub fn draw_powerup_glow(world: &World, f: &Fighter) {
    let mut colors: Vec<Color> = f
        .buffs
        .iter()
        .filter_map(|b| world.data.powerups.powerups.get(b.powerup))
        .map(|p| rgb(p.color))
        .collect();
    if let Some(p) = f
        .teleport_powerup
        .and_then(|p| world.data.powerups.powerups.get(p))
    {
        colors.push(rgb(p.color));
    }
    if colors.is_empty() {
        return;
    }
    let b = f.aabb();
    let size = b.max - b.min;
    let pulse = 0.5 + 0.5 * (world.time * 6.0).sin();
    for (k, c) in colors.iter().enumerate() {
        let pad = 0.08 + k as f32 * 0.07;
        draw_rectangle(
            b.min.x,
            b.min.y,
            size.x,
            size.y,
            Color::new(c.r, c.g, c.b, 0.22 + pulse * 0.12),
        );
        draw_rectangle_lines(
            b.min.x - pad,
            b.min.y - pad,
            size.x + pad * 2.0,
            size.y + pad * 2.0,
            0.07,
            Color::new(c.r, c.g, c.b, 0.6 + pulse * 0.4),
        );
    }
}

/// Small shield bubble outline when a fighter has shield.
pub fn draw_shield_hint(f: &Fighter) {
    if f.shield <= 0.0 {
        return;
    }
    let c = f.center();
    let a = 0.15 + 0.35 * (f.shield / 100.0);
    draw_circle_lines(
        c.x,
        c.y,
        1.15,
        0.05,
        Color::new(SHIELD_COLOR.r, SHIELD_COLOR.g, SHIELD_COLOR.b, a),
    );
}

/// Screen-space labels over objects.
pub fn draw_object_labels(world: &World, cam: &GameCamera) {
    let s = (screen_height() / 720.0).max(0.6);
    let size = (13.0 * s) as u16;
    for o in &world.objects {
        let label = match (o.def.kind, o.ready) {
            (ObjectKind::WeaponStation, true) => {
                o.weapon.map(|w| t(&world.data.weapons[w].name).to_owned())
            }
            (ObjectKind::ShieldStation, true) => Some(tf(
                "+{} Shield",
                &[&format!("{:.0}", o.def.shield_amount())],
            )),
            (ObjectKind::PowerupSpot, true) => o
                .powerup
                .map(|p| t(&world.data.powerups.powerups[p].name).to_owned()),
            (ObjectKind::PowerupSpot, false) => Some(match o.fixed_powerup {
                Some(p) => format!(
                    "{}  {:.0}",
                    t(&world.data.powerups.powerups[p].name),
                    o.timer.ceil()
                ),
                None => format!("{:.0}", o.timer.ceil()),
            }),
            _ => None,
        };
        let Some(label) = label else { continue };
        let at = cam.world_to_screen(o.item_pos() + vec2(0.0, -0.75));
        if at.y > screen_height() - 64.0 * s {
            continue;
        }
        let dims = measure_text(&label, None, size, 1.0);
        draw_text(
            &label,
            at.x - dims.width / 2.0,
            at.y,
            size as f32,
            Color::new(1.0, 1.0, 1.0, 0.8),
        );
    }
}
