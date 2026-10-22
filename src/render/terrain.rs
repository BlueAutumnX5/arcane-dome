//! Code-drawn terrain: rounded, lumpy rock; ramps; dripping sludge; and
//! interior back walls. No image files: every tile is shaped from simple
//! shapes with a per-tile hash so neighbours never look stamped out.

use crate::data::{Backdrop, MapDef, TileKind};
use macroquad::prelude::*;

const ROCK: Color = Color::new(0.23, 0.25, 0.32, 1.0);
const ROCK_DARK: Color = Color::new(0.16, 0.17, 0.23, 1.0);
const ROCK_LIP: Color = Color::new(0.43, 0.45, 0.55, 1.0);
const SLUDGE: Color = Color::new(0.35, 0.39, 0.2, 1.0);
const SLUDGE_DARK: Color = Color::new(0.24, 0.27, 0.14, 1.0);
const SLUDGE_LIP: Color = Color::new(0.55, 0.6, 0.31, 1.0);
const WALL: Color = Color::new(0.13, 0.14, 0.19, 1.0);
const WALL_SEAM: Color = Color::new(0.09, 0.1, 0.14, 1.0);
const PLATFORM: Color = Color::new(0.67, 0.55, 0.35, 1.0);
const SPIKES: Color = Color::new(0.82, 0.27, 0.27, 1.0);

/// Corner rounding for rock and sludge edges that face open space.
const INSET: f32 = 0.12;

/// Deterministic 0..1 noise per cell (and salt), so terrain looks the same every frame.
fn hash(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ salt.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn shade(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

/// Does this tile read as part of the same rock mass?
fn massive(k: TileKind) -> bool {
    k.is_solid() || k.is_slope()
}

/// Interior back walls, drawn before the terrain.
pub fn draw_interior(map: &MapDef, x0: i32, x1: i32, y0: i32, y1: i32, time: f32) {
    if !map.has_backdrop() {
        return;
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let b = map.backdrop(x, y);
            if b == Backdrop::None {
                continue;
            }
            let (fx, fy) = (x as f32, y as f32);
            draw_rectangle(fx, fy, 1.0, 1.0, shade(WALL, 0.92 + hash(x, y, 1) * 0.1));
            if x % 2 == 0 {
                draw_rectangle(fx, fy, 0.05, 1.0, WALL_SEAM);
            }
            if y % 4 == 0 {
                draw_rectangle(fx, fy, 1.0, 0.08, WALL_SEAM);
            }
            if b == Backdrop::Window {
                let glow = 0.75 + 0.1 * (time * 0.7 + hash(x, y, 2) * 6.0).sin();
                draw_rectangle(
                    fx + 0.15,
                    fy + 0.2,
                    0.7,
                    0.55,
                    Color::new(0.06, 0.07, 0.1, 1.0),
                );
                draw_rectangle(
                    fx + 0.2,
                    fy + 0.25,
                    0.6,
                    0.45,
                    Color::new(0.45 * glow, 0.62 * glow, 0.7 * glow, 1.0),
                );
                draw_rectangle(
                    fx + 0.48,
                    fy + 0.25,
                    0.04,
                    0.45,
                    Color::new(0.06, 0.07, 0.1, 1.0),
                );
            }
        }
    }
}

/// All terrain tiles in the visible range (camera already set).
pub fn draw_terrain(map: &MapDef, x0: i32, x1: i32, y0: i32, y1: i32, time: f32) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
                continue;
            }
            match map.tile(x, y) {
                TileKind::Solid => draw_mass(map, x, y, ROCK, ROCK_DARK, ROCK_LIP, false),
                TileKind::Sludge => {
                    draw_mass(map, x, y, SLUDGE, SLUDGE_DARK, SLUDGE_LIP, true);
                    draw_drips(map, x, y, time);
                }
                k @ (TileKind::SlopeUpRight | TileKind::SlopeUpLeft) => draw_ramp(map, x, y, k),
                TileKind::OneWay => draw_platform(map, x, y),
                TileKind::Spikes => draw_spikes(x, y),
                TileKind::Air | TileKind::Water => {}
            }
        }
    }
}

/// A rock or sludge cell: filled toward solid neighbours, rounded and lumpy
/// where it faces open space.
fn draw_mass(map: &MapDef, x: i32, y: i32, base: Color, dark: Color, lip: Color, blobby: bool) {
    let (fx, fy) = (x as f32, y as f32);
    let up = massive(map.tile(x, y - 1));
    let down = massive(map.tile(x, y + 1));
    let left = massive(map.tile(x - 1, y));
    let right = massive(map.tile(x + 1, y));
    let body = shade(base, 0.9 + hash(x, y, 3) * 0.2);

    let l = if left { 0.0 } else { INSET };
    let r = if right { 0.0 } else { INSET };
    let t = if up { 0.0 } else { INSET };
    let b = if down { 0.0 } else { INSET };
    // Cross shape plus round corners gives a soft outline where rock meets air.
    draw_rectangle(fx + l, fy, 1.0 - l - r, 1.0, body);
    draw_rectangle(fx, fy + t, 1.0, 1.0 - t - b, body);
    for (cx, cy, open_x, open_y) in [
        (fx + INSET, fy + INSET, !left, !up),
        (fx + 1.0 - INSET, fy + INSET, !right, !up),
        (fx + INSET, fy + 1.0 - INSET, !left, !down),
        (fx + 1.0 - INSET, fy + 1.0 - INSET, !right, !down),
    ] {
        // Corners touching rock are already covered by the cross.
        if open_x && open_y {
            draw_circle(cx, cy, INSET, body);
        }
    }

    // Speckles and the occasional crack.
    for k in 0..3u32 {
        let px = fx + 0.15 + hash(x, y, 10 + k) * 0.7;
        let py = fy + 0.15 + hash(x, y, 20 + k) * 0.7;
        draw_circle(px, py, 0.025 + hash(x, y, 30 + k) * 0.04, shade(dark, 0.95));
    }
    if hash(x, y, 40) > 0.82 {
        let a = vec2(fx + 0.2 + hash(x, y, 41) * 0.6, fy + 0.15);
        let m = a + vec2(hash(x, y, 42) * 0.3 - 0.15, 0.35);
        let e = m + vec2(hash(x, y, 43) * 0.3 - 0.15, 0.35);
        draw_line(a.x, a.y, m.x, m.y, 0.03, dark);
        draw_line(m.x, m.y, e.x, e.y, 0.03, dark);
    }

    // Shadowed underside.
    if !down {
        draw_rectangle(
            fx + l,
            fy + 1.0 - 0.18,
            1.0 - l - r,
            0.18 - b * 0.5,
            shade(dark, 0.9),
        );
    }
    // Lumpy lit top edge.
    if !up {
        let bumps = if blobby { 4 } else { 3 };
        for k in 0..bumps {
            let u = (k as f32 + 0.5) / bumps as f32;
            let rad = if blobby { 0.1 } else { 0.07 } + hash(x, y, 50 + k as u32) * 0.07;
            let cx = fx + u;
            let cy = fy + INSET + 0.02 - hash(x, y, 60 + k as u32) * 0.06;
            draw_circle(cx, cy, rad, lip);
        }
        draw_rectangle(fx + l, fy + INSET, 1.0 - l - r, 0.07, lip);
    }
    // Blobby edges on the open sides of sludge.
    if blobby {
        for (open, side) in [(!left, 0.0), (!right, 1.0)] {
            if open {
                let cy = fy + 0.3 + hash(x, y, 70 + side as u32) * 0.4;
                draw_circle(fx + side, cy, 0.09 + hash(x, y, 71) * 0.05, body);
            }
        }
    }
}

/// Hanging drips under sludge, with a droplet falling now and then.
fn draw_drips(map: &MapDef, x: i32, y: i32, time: f32) {
    if map.tile(x, y + 1) != TileKind::Air {
        return;
    }
    let (fx, fy) = (x as f32, y as f32 + 1.0);
    for k in 0..2u32 {
        if hash(x, y, 80 + k) < 0.35 {
            continue;
        }
        let dx = fx + 0.2 + hash(x, y, 82 + k) * 0.6;
        let len = 0.12 + hash(x, y, 84 + k) * 0.35;
        draw_rectangle(dx - 0.035, fy - 0.05, 0.07, len, SLUDGE);
        draw_circle(dx, fy + len - 0.04, 0.06, SLUDGE_LIP);
        // A droplet falls every few seconds, at its own pace.
        let period = 2.5 + hash(x, y, 86 + k) * 3.0;
        let phase = (time / period + hash(x, y, 88 + k)) % 1.0;
        if phase < 0.25 {
            let fall = phase / 0.25;
            let dy = fy + len + fall * fall * 1.6;
            draw_circle(
                dx,
                dy,
                0.045,
                Color::new(SLUDGE_LIP.r, SLUDGE_LIP.g, SLUDGE_LIP.b, 1.0 - fall),
            );
        }
    }
}

/// A ramp: solid triangle with a lumpy lit surface along the slope.
fn draw_ramp(map: &MapDef, x: i32, y: i32, kind: TileKind) {
    let (fx, fy) = (x as f32, y as f32);
    let body = shade(ROCK, 0.9 + hash(x, y, 3) * 0.2);
    let (low, high) = match kind {
        TileKind::SlopeUpRight => (vec2(fx, fy + 1.0), vec2(fx + 1.0, fy)),
        _ => (vec2(fx + 1.0, fy + 1.0), vec2(fx, fy)),
    };
    let foot = vec2(high.x, fy + 1.0);
    draw_triangle(low, high, foot, body);
    // Fill the strip under the ramp if rock continues below.
    if !massive(map.tile(x, y + 1)) {
        draw_rectangle(fx, fy + 0.85, 1.0, 0.15, shade(ROCK_DARK, 0.9));
    }
    for k in 0..4u32 {
        let u = (k as f32 + 0.5) / 4.0;
        let p = low.lerp(high, u) + vec2(0.0, 0.05);
        draw_circle(p.x, p.y, 0.06 + hash(x, y, 90 + k) * 0.05, ROCK_LIP);
    }
    draw_line(low.x, low.y, high.x, high.y, 0.06, ROCK_LIP);
}

/// Thin platform: a plank with a darker underside and small brackets.
fn draw_platform(map: &MapDef, x: i32, y: i32) {
    let (fx, fy) = (x as f32, y as f32);
    draw_rectangle(
        fx,
        fy,
        1.0,
        0.2,
        shade(PLATFORM, 0.92 + hash(x, y, 5) * 0.12),
    );
    draw_rectangle(fx, fy + 0.2, 1.0, 0.07, shade(PLATFORM, 0.55));
    let left_end = map.tile(x - 1, y) != TileKind::OneWay;
    let right_end = map.tile(x + 1, y) != TileKind::OneWay;
    for (end, bx) in [(left_end, fx + 0.12), (right_end, fx + 0.82)] {
        if end {
            draw_triangle(
                vec2(bx, fy + 0.27),
                vec2(bx + 0.06, fy + 0.27),
                vec2(bx + 0.03, fy + 0.5),
                shade(PLATFORM, 0.45),
            );
        }
    }
}

fn draw_spikes(x: i32, y: i32) {
    let (fx, fy) = (x as f32, y as f32);
    for k in 0..3 {
        let bx = fx + k as f32 / 3.0;
        draw_triangle(
            vec2(bx, fy + 1.0),
            vec2(bx + 1.0 / 3.0, fy + 1.0),
            vec2(bx + 1.0 / 6.0, fy + 0.35),
            SPIKES,
        );
    }
}
