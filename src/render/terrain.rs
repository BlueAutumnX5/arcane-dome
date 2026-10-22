//! Code-drawn polygon terrain, props and themed backgrounds.
//!
//! `MapArt` is built once per map: polygons are triangulated, their edges
//! sorted into tops / undersides / sides, and texture speckles scattered
//! inside them. Drawing then just replays that data each frame.

use crate::data::{Backdrop, Decor, DecorKind, MapDef, Skyline};
use crate::geo::{Material, ShapeKind, point_in_polygon, triangulate};
use macroquad::prelude::*;

/// Deterministic 0..1 noise, so the art never flickers between frames.
fn hash(a: i32, b: i32, salt: u32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x8DA6_B343)
        ^ (b as u32).wrapping_mul(0xD816_3841)
        ^ salt.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

fn shade(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

fn lerp_c(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Fill, lip (top edge) and shadow colours of a material.
fn palette(m: Material) -> (Color, Color, Color) {
    let c = |r, g, b| Color::from_rgba(r, g, b, 255);
    match m {
        Material::Rock => (c(62, 66, 84), c(118, 124, 148), c(40, 43, 58)),
        Material::Metal => (c(74, 80, 94), c(170, 180, 196), c(44, 48, 58)),
        Material::Wood => (c(118, 84, 52), c(176, 132, 84), c(74, 50, 30)),
        Material::Ice => (c(150, 196, 226), c(240, 250, 255), c(96, 140, 182)),
        Material::Sand => (c(186, 146, 92), c(226, 194, 138), c(136, 102, 60)),
        Material::Moss => (c(58, 66, 54), c(96, 168, 70), c(36, 42, 34)),
        Material::Sludge => (c(90, 102, 52), c(142, 156, 80), c(60, 70, 36)),
        Material::Glass => (c(150, 200, 220), c(230, 245, 255), c(90, 130, 150)),
        Material::Lava => (c(220, 80, 30), c(255, 200, 80), c(150, 40, 20)),
        Material::Spikes => (c(150, 150, 160), c(220, 220, 230), c(90, 40, 40)),
        Material::Water => (c(50, 120, 210), c(160, 210, 255), c(30, 70, 140)),
        Material::Void => (c(14, 15, 20), c(14, 15, 20), c(10, 10, 14)),
    }
}

struct EdgeArt {
    a: Vec2,
    b: Vec2,
    normal: Vec2,
}

struct ShapeArt {
    kind: ShapeKind,
    material: Material,
    tris: Vec<[Vec2; 3]>,
    tops: Vec<EdgeArt>,
    bottoms: Vec<EdgeArt>,
    sides: Vec<EdgeArt>,
    /// Texture speckles: position, radius, brightness.
    speckles: Vec<(Vec2, f32, f32)>,
    min: Vec2,
    max: Vec2,
    seed: i32,
}

/// Prepared drawing data for one map.
pub struct MapArt {
    shapes: Vec<ShapeArt>,
    back: Vec<Decor>,
    front: Vec<Decor>,
}

impl MapArt {
    pub fn new(map: &MapDef) -> Self {
        let mut shapes = Vec::new();
        for (si, s) in map.terrain.shapes.iter().enumerate() {
            if s.material == Material::Void {
                continue;
            }
            let n = s.points.len();
            let mut art = ShapeArt {
                kind: s.kind,
                material: s.material,
                tris: triangulate(&s.points),
                tops: Vec::new(),
                bottoms: Vec::new(),
                sides: Vec::new(),
                speckles: Vec::new(),
                min: s
                    .points
                    .iter()
                    .fold(Vec2::splat(f32::MAX), |m, p| m.min(*p)),
                max: s
                    .points
                    .iter()
                    .fold(Vec2::splat(f32::MIN), |m, p| m.max(*p)),
                seed: si as i32,
            };
            for i in 0..n {
                let (a, b) = (s.points[i], s.points[(i + 1) % n]);
                let d = b - a;
                if d.length_squared() < 1e-6 {
                    continue;
                }
                let mut normal = vec2(d.y, -d.x).normalize();
                if point_in_polygon((a + b) * 0.5 + normal * 1e-3, &s.points) {
                    normal = -normal;
                }
                let e = EdgeArt { a, b, normal };
                if normal.y < -0.55 {
                    art.tops.push(e);
                } else if normal.y > 0.55 {
                    art.bottoms.push(e);
                } else {
                    art.sides.push(e);
                }
            }
            // Scatter speckles inside the polygon (about 0.7 per square tile).
            let size = art.max - art.min;
            let count = ((size.x * size.y * 0.7) as i32).min(1500);
            for k in 0..count {
                let p = art.min
                    + vec2(
                        hash(si as i32, k, 1) * size.x,
                        hash(si as i32, k, 2) * size.y,
                    );
                if point_in_polygon(p, &s.points) {
                    art.speckles.push((
                        p,
                        0.03 + hash(si as i32, k, 3) * 0.06,
                        hash(si as i32, k, 4),
                    ));
                }
            }
            shapes.push(art);
        }
        let (front, back): (Vec<Decor>, Vec<Decor>) =
            map.decor.iter().cloned().partition(|d| d.front);
        Self {
            shapes,
            back,
            front,
        }
    }

    /// Props behind everything, then legacy interiors, then solid terrain.
    pub fn draw_back(&self, map: &MapDef, view_min: Vec2, view_max: Vec2, time: f32) {
        for d in &self.back {
            draw_decor(d, map, time);
        }
        draw_legacy_interior(map, view_min, view_max, time);
        for s in &self.shapes {
            if s.max.x < view_min.x
                || s.min.x > view_max.x
                || s.max.y < view_min.y
                || s.min.y > view_max.y
            {
                continue;
            }
            match s.kind {
                ShapeKind::Solid => draw_solid(s, time),
                ShapeKind::Hatch => draw_hatch(s),
                ShapeKind::Hazard => draw_hazard(s, time),
                ShapeKind::Water => {}
            }
        }
    }

    /// Water and front props, drawn over the fighters.
    pub fn draw_front(&self, map: &MapDef, time: f32) {
        for s in self.shapes.iter().filter(|s| s.kind == ShapeKind::Water) {
            let (fill, lip, _) = palette(Material::Water);
            for t in &s.tris {
                draw_triangle(t[0], t[1], t[2], alpha(fill, 0.42));
            }
            for e in &s.tops {
                let wave = (time * 2.0).sin() * 0.03;
                draw_line(
                    e.a.x,
                    e.a.y + wave,
                    e.b.x,
                    e.b.y + wave,
                    0.07,
                    alpha(lip, 0.75),
                );
            }
        }
        for d in &self.front {
            draw_decor(d, map, time);
        }
    }
}

fn draw_solid(s: &ShapeArt, time: f32) {
    let (fill, lip, dark) = palette(s.material);
    for t in &s.tris {
        draw_triangle(t[0], t[1], t[2], fill);
    }
    for &(p, r, v) in &s.speckles {
        let c = if v > 0.5 {
            shade(fill, 0.82)
        } else {
            shade(fill, 1.12)
        };
        match s.material {
            Material::Metal => draw_circle(p.x, p.y, 0.035, shade(fill, 0.7)),
            Material::Wood => draw_line(
                p.x - r * 2.0,
                p.y,
                p.x + r * 2.0,
                p.y,
                0.02,
                shade(fill, 0.75),
            ),
            Material::Ice => draw_line(
                p.x,
                p.y,
                p.x + r * 3.0,
                p.y - r * 2.0,
                0.015,
                alpha(lip, 0.5),
            ),
            _ => draw_circle(p.x, p.y, r, c),
        }
    }
    // Shadowed undersides with drips or icicles.
    for e in &s.bottoms {
        draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.16, dark);
        let len = (e.b - e.a).length();
        let n = (len * 1.6) as i32;
        for k in 0..n {
            let u = (k as f32 + 0.5) / n as f32;
            let p = e.a.lerp(e.b, u);
            let h = hash(s.seed, k, 7);
            match s.material {
                Material::Ice if h > 0.3 => {
                    let l = 0.2 + h * 0.5;
                    draw_triangle(
                        p - vec2(0.08, 0.0),
                        p + vec2(0.08, 0.0),
                        p + vec2(0.0, l),
                        alpha(lip, 0.9),
                    );
                }
                Material::Sludge if h > 0.55 => {
                    let l = 0.15 + h * 0.35;
                    draw_rectangle(p.x - 0.035, p.y, 0.07, l, fill);
                    draw_circle(p.x, p.y + l, 0.06, lip);
                    let period = 2.5 + h * 3.0;
                    let phase = (time / period + h) % 1.0;
                    if phase < 0.25 {
                        let f = phase / 0.25;
                        draw_circle(p.x, p.y + l + f * f * 1.6, 0.045, alpha(lip, 1.0 - f));
                    }
                }
                Material::Moss if h > 0.6 => {
                    let l = 0.3 + h * 0.6;
                    draw_line(
                        p.x,
                        p.y,
                        p.x + 0.05,
                        p.y + l,
                        0.035,
                        Color::from_rgba(70, 120, 50, 255),
                    );
                }
                _ => {}
            }
        }
    }
    for e in &s.sides {
        draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.06, shade(dark, 1.1));
    }
    // Lit tops: material-specific trim along every walkable edge.
    for e in &s.tops {
        let len = (e.b - e.a).length();
        let inward = -e.normal;
        match s.material {
            Material::Rock | Material::Sand => {
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.1, lip);
                let n = (len * 2.5) as i32;
                for k in 0..n {
                    let u = (k as f32 + 0.5) / n as f32;
                    let p = e.a.lerp(e.b, u) + inward * 0.04;
                    draw_circle(p.x, p.y, 0.05 + hash(s.seed, k, 9) * 0.05, lip);
                }
            }
            Material::Metal => {
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.12, lip);
                let p = inward * 0.13;
                draw_line(
                    e.a.x + p.x,
                    e.a.y + p.y,
                    e.b.x + p.x,
                    e.b.y + p.y,
                    0.03,
                    shade(lip, 0.7),
                );
            }
            Material::Wood => {
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.1, lip);
            }
            Material::Ice => {
                // Snow cap.
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.22, WHITE);
                let n = (len * 2.0) as i32;
                for k in 0..n {
                    let u = (k as f32 + 0.5) / n as f32;
                    let p = e.a.lerp(e.b, u) + inward * 0.06;
                    draw_circle(p.x, p.y, 0.09 + hash(s.seed, k, 11) * 0.06, WHITE);
                }
            }
            Material::Moss => {
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.12, lip);
                let n = (len * 5.0) as i32;
                for k in 0..n {
                    let u = (k as f32 + hash(s.seed, k, 12)) / n as f32;
                    let p = e.a.lerp(e.b, u.min(1.0));
                    let tip = p
                        + e.normal * (0.12 + hash(s.seed, k, 13) * 0.18)
                        + vec2(hash(s.seed, k, 14) * 0.1 - 0.05, 0.0);
                    draw_triangle(
                        p - vec2(0.04, 0.0),
                        p + vec2(0.04, 0.0),
                        tip,
                        shade(lip, 0.85 + hash(s.seed, k, 15) * 0.3),
                    );
                }
            }
            Material::Sludge => {
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.12, lip);
                let n = (len * 2.5) as i32;
                for k in 0..n {
                    let u = (k as f32 + 0.5) / n as f32;
                    let p = e.a.lerp(e.b, u) + inward * 0.03;
                    draw_circle(p.x, p.y, 0.07 + hash(s.seed, k, 16) * 0.07, lip);
                }
            }
            Material::Lava => {
                let glow = 0.7 + 0.3 * (time * 3.0).sin();
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.14, alpha(lip, glow));
            }
            _ => draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.1, lip),
        }
    }
}

/// Hatches: a grate you can see through, so it reads as passable from below.
fn draw_hatch(s: &ShapeArt) {
    let (fill, lip, dark) = palette(s.material);
    for t in &s.tris {
        draw_triangle(t[0], t[1], t[2], alpha(fill, 0.55));
    }
    let mut x = s.min.x + 0.12;
    while x < s.max.x {
        draw_line(x, s.min.y, x, s.max.y, 0.05, alpha(dark, 0.9));
        x += 0.25;
    }
    for e in &s.tops {
        draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.07, lip);
    }
    for e in &s.bottoms {
        draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.04, dark);
    }
}

fn draw_hazard(s: &ShapeArt, time: f32) {
    let (fill, lip, dark) = palette(s.material);
    match s.material {
        Material::Lava => {
            for t in &s.tris {
                draw_triangle(t[0], t[1], t[2], fill);
            }
            for &(p, r, v) in &s.speckles {
                let pulse = 0.5 + 0.5 * (time * 2.0 + v * 6.0).sin();
                draw_circle(p.x, p.y, r * 1.6, alpha(lip, 0.3 + 0.4 * pulse));
            }
            for (i, e) in s.tops.iter().enumerate() {
                // Heat shimmer rising off the surface.
                let flicker = 0.85 + 0.15 * (time * 3.1 + i as f32).sin();
                for k in 0..4 {
                    let (h0, h1) = (k as f32 * 0.35, (k + 1) as f32 * 0.35);
                    let c = alpha(lip, 0.16 / (k + 1) as f32 * flicker);
                    let (a0, b0) = (e.a - vec2(0.0, h0), e.b - vec2(0.0, h0));
                    let (a1, b1) = (e.a - vec2(0.0, h1), e.b - vec2(0.0, h1));
                    draw_triangle(a0, b0, b1, c);
                    draw_triangle(a0, b1, a1, c);
                }
                // Bubbles that swell and pop at the surface.
                let len = (e.b - e.a).length();
                let n = (len * 0.9) as i32 + 1;
                for k in 0..n {
                    let phase = hash(i as i32, k, 11);
                    let cycle = (time * (0.5 + phase * 0.4) + phase * 7.0).fract();
                    let along =
                        (hash(i as i32, k, 12) + (time * 0.05 + phase).floor() * 0.37).fract();
                    let p = e.a.lerp(e.b, 0.05 + along * 0.9) + vec2(0.0, 0.12);
                    let r = 0.05 + cycle * 0.14;
                    draw_circle(p.x, p.y - cycle * 0.1, r, alpha(lip, 0.9 * (1.0 - cycle)));
                }
                draw_line(e.a.x, e.a.y, e.b.x, e.b.y, 0.12, lip);
            }
        }
        Material::Void => {}
        _ => {
            // Spikes: a dark base with a row of teeth along each top edge.
            for t in &s.tris {
                draw_triangle(t[0], t[1], t[2], dark);
            }
            for e in &s.tops {
                let len = (e.b - e.a).length();
                let n = ((len * 3.0) as i32).max(1);
                for k in 0..n {
                    let a = e.a.lerp(e.b, k as f32 / n as f32);
                    let b = e.a.lerp(e.b, (k + 1) as f32 / n as f32);
                    let tip = (a + b) * 0.5 + e.normal * 0.45;
                    draw_triangle(a, b, tip, if k % 2 == 0 { fill } else { lip });
                }
            }
        }
    }
}

fn draw_legacy_interior(map: &MapDef, view_min: Vec2, view_max: Vec2, time: f32) {
    if !map.has_backdrop() {
        return;
    }
    let wall = Color::new(0.13, 0.14, 0.19, 1.0);
    let seam = Color::new(0.09, 0.1, 0.14, 1.0);
    for y in view_min.y.floor() as i32..=view_max.y.ceil() as i32 {
        for x in view_min.x.floor() as i32..=view_max.x.ceil() as i32 {
            let b = map.backdrop(x, y);
            if b == Backdrop::None {
                continue;
            }
            let (fx, fy) = (x as f32, y as f32);
            draw_rectangle(fx, fy, 1.0, 1.0, shade(wall, 0.92 + hash(x, y, 1) * 0.1));
            if x % 2 == 0 {
                draw_rectangle(fx, fy, 0.05, 1.0, seam);
            }
            if b == Backdrop::Window {
                let glow = 0.75 + 0.1 * (time * 0.7 + hash(x, y, 2) * 6.0).sin();
                draw_rectangle(
                    fx + 0.2,
                    fy + 0.25,
                    0.6,
                    0.45,
                    Color::new(0.45 * glow, 0.62 * glow, 0.7 * glow, 1.0),
                );
            }
        }
    }
}

/// One decorative prop. Sizes are in tiles; (x, y) is the prop's bottom-left
/// for standing props, or its top-left for hanging ones (chains, vines).
fn draw_decor(d: &Decor, map: &MapDef, time: f32) {
    let tint = d
        .color
        .as_deref()
        .and_then(crate::data::parse_color)
        .map(rgb);
    let far = rgb(map.theme.far);
    let (x, y, w, h) = (d.x, d.y, d.w, d.h);
    let flip = if d.flip { -1.0 } else { 1.0 };
    let seed = (x * 7.0) as i32 ^ (y * 13.0) as i32;
    match d.kind {
        DecorKind::Wall => {
            let c = tint.unwrap_or(lerp_c(far, Color::new(0.1, 0.1, 0.12, 1.0), 0.5));
            draw_rectangle(x, y - h, w, h, c);
            let mut sx = x + 2.0;
            while sx < x + w {
                draw_line(sx, y - h, sx, y, 0.06, shade(c, 0.75));
                sx += 2.0;
            }
            let mut sy = y - h + 3.0;
            while sy < y {
                draw_line(x, sy, x + w, sy, 0.08, shade(c, 0.7));
                sy += 3.0;
            }
        }
        DecorKind::Window => {
            let glow = 0.8 + 0.1 * (time * 0.6 + x).sin();
            let glass = tint.unwrap_or(Color::new(0.5, 0.7, 0.8, 1.0));
            draw_rectangle(
                x - 0.1,
                y - h - 0.1,
                w + 0.2,
                h + 0.2,
                Color::new(0.08, 0.09, 0.12, 1.0),
            );
            draw_rectangle(x, y - h, w, h, shade(glass, glow));
            draw_line(
                x + w * 0.5,
                y - h,
                x + w * 0.5,
                y,
                0.08,
                Color::new(0.08, 0.09, 0.12, 1.0),
            );
            draw_line(
                x,
                y - h * 0.5,
                x + w,
                y - h * 0.5,
                0.08,
                Color::new(0.08, 0.09, 0.12, 1.0),
            );
        }
        DecorKind::Girder => {
            let c = tint.unwrap_or(Color::from_rgba(70, 76, 90, 255));
            if w >= h {
                draw_rectangle(x, y - h, w, 0.12, c);
                draw_rectangle(x, y - 0.12, w, 0.12, c);
                let n = (w / h.max(0.4)).ceil() as i32;
                for k in 0..n {
                    let x0 = x + w * k as f32 / n as f32;
                    let x1 = x + w * (k + 1) as f32 / n as f32;
                    let (ya, yb) = if k % 2 == 0 { (y - h, y) } else { (y, y - h) };
                    draw_line(x0, ya, x1, yb, 0.08, c);
                }
            } else {
                draw_rectangle(x, y - h, 0.12, h, c);
                draw_rectangle(x + w - 0.12, y - h, 0.12, h, c);
                let n = (h / w.max(0.4)).ceil() as i32;
                for k in 0..n {
                    let y0 = y - h + h * k as f32 / n as f32;
                    let y1 = y - h + h * (k + 1) as f32 / n as f32;
                    let (xa, xb) = if k % 2 == 0 { (x, x + w) } else { (x + w, x) };
                    draw_line(xa, y0, xb, y1, 0.08, c);
                }
            }
        }
        DecorKind::Pipe => {
            let c = tint.unwrap_or(Color::from_rgba(96, 100, 112, 255));
            let (thick, horizontal) = if w >= h {
                (h.min(0.5), true)
            } else {
                (w.min(0.5), false)
            };
            if horizontal {
                draw_rectangle(x, y - thick, w, thick, c);
                draw_line(
                    x,
                    y - thick * 0.7,
                    x + w,
                    y - thick * 0.7,
                    0.05,
                    shade(c, 1.3),
                );
                let mut jx = x + 1.5;
                while jx < x + w {
                    draw_rectangle(
                        jx - 0.08,
                        y - thick - 0.06,
                        0.16,
                        thick + 0.12,
                        shade(c, 0.8),
                    );
                    jx += 3.0;
                }
            } else {
                draw_rectangle(x, y - h, thick, h, c);
                draw_line(
                    x + thick * 0.3,
                    y - h,
                    x + thick * 0.3,
                    y,
                    0.05,
                    shade(c, 1.3),
                );
                let mut jy = y - 1.5;
                while jy > y - h {
                    draw_rectangle(x - 0.06, jy - 0.08, thick + 0.12, 0.16, shade(c, 0.8));
                    jy -= 3.0;
                }
            }
        }
        DecorKind::Lamp => {
            let c = tint.unwrap_or(Color::from_rgba(255, 220, 150, 255));
            draw_rectangle(x - 0.05, y - h, 0.1, h, Color::from_rgba(50, 54, 64, 255));
            let top = vec2(x + 0.35 * flip, y - h);
            draw_line(
                x,
                y - h,
                top.x,
                top.y,
                0.08,
                Color::from_rgba(50, 54, 64, 255),
            );
            let flicker = 0.85 + 0.15 * (time * 9.0 + x).sin().max(0.0);
            draw_circle(top.x, top.y + 0.15, 1.4, alpha(c, 0.08 * flicker));
            draw_circle(top.x, top.y + 0.15, 0.6, alpha(c, 0.18 * flicker));
            draw_circle(top.x, top.y + 0.15, 0.14, alpha(c, flicker));
        }
        DecorKind::Tree => {
            let trunk = Color::from_rgba(70, 50, 36, 255);
            let leaf = tint.unwrap_or(Color::from_rgba(56, 110, 60, 255));
            draw_rectangle(x + w * 0.42, y - h * 0.6, w * 0.16, h * 0.6, trunk);
            for k in 0..6 {
                let cx = x + w * (0.2 + hash(seed, k, 1) * 0.6);
                let cy = y - h * (0.55 + hash(seed, k, 2) * 0.4);
                let r = w * (0.18 + hash(seed, k, 3) * 0.14);
                draw_circle(cx, cy, r, shade(leaf, 0.8 + hash(seed, k, 4) * 0.35));
            }
        }
        DecorKind::Crystal => {
            let c = tint.unwrap_or(Color::from_rgba(150, 220, 255, 255));
            let glow = 0.75 + 0.25 * (time * 1.5 + x).sin();
            draw_circle(
                x + w * 0.5,
                y - h * 0.4,
                w.max(h) * 0.8,
                alpha(c, 0.08 * glow),
            );
            for k in 0..3 {
                let bx = x + w * (0.2 + 0.3 * k as f32);
                let hh = h * (0.6 + hash(seed, k, 5) * 0.4);
                let lean = (hash(seed, k, 6) - 0.5) * 0.4;
                let half = w * 0.12;
                let tip = vec2(bx + lean, y - hh);
                draw_triangle(vec2(bx - half, y), vec2(bx + half, y), tip, alpha(c, 0.85));
                draw_line(bx, y, tip.x, tip.y, 0.03, alpha(WHITE, 0.6 * glow));
            }
        }
        DecorKind::Chain => {
            let c = tint.unwrap_or(Color::from_rgba(110, 112, 120, 255));
            let mut cy = y;
            let mut k = 0;
            while cy < y + h {
                if k % 2 == 0 {
                    draw_circle_lines(x, cy + 0.12, 0.1, 0.04, c);
                } else {
                    draw_line(x, cy, x, cy + 0.24, 0.05, c);
                }
                cy += 0.22;
                k += 1;
            }
        }
        DecorKind::Banner => {
            let c = tint.unwrap_or(Color::from_rgba(150, 50, 60, 255));
            draw_line(
                x - 0.1,
                y - h,
                x + w + 0.1,
                y - h,
                0.08,
                Color::from_rgba(60, 60, 70, 255),
            );
            let sway = (time * 1.3 + x).sin() * 0.05;
            draw_rectangle(x, y - h, w, h * 0.85, c);
            draw_triangle(
                vec2(x, y - h * 0.15),
                vec2(x + w, y - h * 0.15),
                vec2(x + w * 0.5 + sway, y),
                c,
            );
            draw_rectangle(x + w * 0.42, y - h, w * 0.16, h * 0.85, shade(c, 1.3));
        }
        DecorKind::Tank => {
            let c = tint.unwrap_or(Color::from_rgba(90, 98, 110, 255));
            let r = w.min(h) * 0.5;
            draw_rectangle(x, y - h + r, w, h - 2.0 * r, c);
            draw_circle(x + w * 0.5, y - h + r, r.min(w * 0.5), c);
            draw_circle(x + w * 0.5, y - r, r.min(w * 0.5), c);
            draw_rectangle(x + w * 0.15, y - h * 0.7, w * 0.08, h * 0.4, shade(c, 1.35));
            draw_circle(
                x + w * 0.65,
                y - h * 0.55,
                w * 0.12,
                Color::from_rgba(220, 220, 200, 255),
            );
        }
        DecorKind::Fence => {
            let c = tint.unwrap_or(Color::from_rgba(100, 80, 56, 255));
            let mut px = x;
            while px <= x + w + 0.01 {
                draw_rectangle(px - 0.05, y - h, 0.1, h, c);
                px += 1.0;
            }
            draw_line(x, y - h * 0.75, x + w, y - h * 0.75, 0.07, c);
            draw_line(x, y - h * 0.35, x + w, y - h * 0.35, 0.07, c);
        }
        DecorKind::Vines => {
            let c = tint.unwrap_or(Color::from_rgba(60, 120, 56, 255));
            for k in 0..((w * 2.0) as i32).max(1) {
                let vx = x + (k as f32 + 0.5) * 0.5;
                let len = h * (0.5 + hash(seed, k, 8) * 0.5);
                let sway = (time * 0.8 + vx).sin() * 0.06;
                draw_line(vx, y, vx + sway, y + len, 0.04, c);
                let mut ly = y + 0.3;
                while ly < y + len {
                    draw_circle(vx + sway * (ly - y) / len + 0.06, ly, 0.06, shade(c, 1.2));
                    ly += 0.4;
                }
            }
        }
        DecorKind::Cactus => {
            let c = tint.unwrap_or(Color::from_rgba(72, 128, 72, 255));
            let t = w * 0.3;
            draw_rectangle(x + w * 0.5 - t * 0.5, y - h, t, h, c);
            draw_circle(x + w * 0.5, y - h, t * 0.5, c);
            draw_rectangle(x + w * 0.15, y - h * 0.65, t * 0.8, h * 0.3, c);
            draw_rectangle(x + w * 0.15, y - h * 0.4, w * 0.4, t * 0.6, c);
            draw_rectangle(x + w * 0.62, y - h * 0.8, t * 0.8, h * 0.35, c);
            draw_rectangle(x + w * 0.45, y - h * 0.5, w * 0.4, t * 0.6, c);
        }
    }
}

/// Sky gradient, sun and far skyline, drawn in screen space.
pub fn draw_sky(map: &MapDef, cam_x: f32, ppt: f32, time: f32) {
    draw_sky_sized(map, screen_width(), screen_height(), cam_x, ppt, time);
}

/// The sky over a `sw` x `sh` area whose top-left is the origin.
pub fn draw_sky_sized(map: &MapDef, sw: f32, sh: f32, cam_x: f32, ppt: f32, time: f32) {
    let t = &map.theme;
    let (top, bottom) = (rgb(t.sky_top), rgb(t.sky_bottom));
    let bands = 32;
    for i in 0..bands {
        let k = i as f32 / bands as f32;
        draw_rectangle(
            0.0,
            sh * k,
            sw,
            sh / bands as f32 + 1.0,
            lerp_c(top, bottom, k),
        );
    }
    if let Some(sun) = t.sun {
        let c = rgb(t.sun_color);
        let (x, y) = (sun.x * sw, sun.y * sh);
        let r = sh * 0.09;
        draw_circle(x, y, r * 3.0, alpha(c, 0.08));
        draw_circle(x, y, r * 1.7, alpha(c, 0.16));
        draw_circle(x, y, r, c);
    }

    let far = rgb(t.far);
    let parallax = 0.25;
    let scroll = cam_x * ppt * parallax;
    let seed = map
        .id
        .bytes()
        .fold(7i32, |a, b| a.wrapping_mul(31).wrapping_add(b as i32));
    match t.skyline {
        Skyline::None => {}
        Skyline::City | Skyline::Spires | Skyline::Girders => {
            let spacing = 120.0 * ppt / 32.0;
            let first = (scroll / spacing).floor() as i32 - 1;
            let count = (sw / spacing) as i32 + 3;
            for k in 0..count {
                let idx = first + k;
                let x = idx as f32 * spacing - scroll;
                let hh = sh * (0.2 + hash(seed, idx, 1) * 0.4);
                let w = spacing * (0.5 + hash(seed, idx, 2) * 0.35);
                match t.skyline {
                    Skyline::Spires => {
                        draw_triangle(
                            vec2(x, sh),
                            vec2(x + w, sh),
                            vec2(x + w * 0.5, sh - hh * 1.3),
                            far,
                        );
                    }
                    Skyline::Girders => {
                        draw_rectangle(x + w * 0.4, sh - hh, w * 0.2, hh, far);
                        draw_rectangle(x, sh - hh, w, w * 0.12, far);
                        draw_line(x, sh - hh + w * 0.12, x + w * 0.4, sh - hh * 0.6, 3.0, far);
                    }
                    _ => {
                        draw_rectangle(x, sh - hh, w, hh, far);
                        draw_rectangle(x + w * 0.2, sh - hh - 16.0, w * 0.15, 16.0, far);
                    }
                }
            }
        }
        Skyline::Mountains => {
            let spacing = 260.0 * ppt / 32.0;
            let first = (scroll / spacing).floor() as i32 - 1;
            for k in 0..((sw / spacing) as i32 + 3) {
                let idx = first + k;
                let x = idx as f32 * spacing - scroll;
                let hh = sh * (0.25 + hash(seed, idx, 3) * 0.3);
                draw_triangle(
                    vec2(x - spacing * 0.3, sh),
                    vec2(x + spacing * 1.3, sh),
                    vec2(x + spacing * 0.5, sh - hh),
                    far,
                );
                let cap = vec2(x + spacing * 0.5, sh - hh);
                draw_triangle(
                    cap,
                    cap + vec2(-spacing * 0.12, hh * 0.15),
                    cap + vec2(spacing * 0.12, hh * 0.15),
                    lerp_c(far, WHITE, 0.25),
                );
            }
        }
        Skyline::Clouds => {
            let spacing = 200.0 * ppt / 32.0;
            let drift = time * 6.0;
            let first = ((scroll + drift) / spacing).floor() as i32 - 1;
            for k in 0..((sw / spacing) as i32 + 3) {
                let idx = first + k;
                let x = idx as f32 * spacing - scroll - drift;
                let y = sh * (0.25 + hash(seed, idx, 4) * 0.5);
                for b in 0..4 {
                    let r = spacing * (0.12 + hash(seed, idx * 4 + b, 5) * 0.1);
                    draw_circle(
                        x + b as f32 * r * 0.9,
                        y - hash(seed, idx * 4 + b, 6) * r * 0.5,
                        r,
                        alpha(far, 0.55),
                    );
                }
            }
        }
    }
}
