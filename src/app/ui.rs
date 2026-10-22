//! Small immediate-mode widget kit for the menus.
//!
//! Buttons are angled tabs anchored to the left or right side of the screen.
//! Hovering eases the tab to white and slides it outward (left tabs move
//! left, right tabs mirror that and move right); leaving eases it back.

use crate::data::{MapDef, ObjectKind};
use macroquad::prelude::*;
use std::collections::HashMap;

pub const ACCENT: Color = Color::new(0.36, 0.86, 0.86, 1.0);
const TAB_FILL: Color = Color::new(0.06, 0.13, 0.15, 0.82);
const TAB_HOVER: Color = Color::new(0.95, 0.97, 0.97, 0.95);
const TEXT_DARK: Color = Color::new(0.07, 0.09, 0.11, 1.0);
/// Easing rate for tab fill and slide (higher is snappier).
const TAB_EASE: f32 = 14.0;
/// Easing rate for tab text colour, fast so it flips before the fill.
const TAB_TEXT_EASE: f32 = 45.0;
const PANEL: Color = Color::new(0.04, 0.09, 0.11, 0.78);

pub fn scale() -> f32 {
    (screen_height() / 720.0).max(0.6)
}

/// Which screen edge a tab belongs to; it slides toward that edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn hovered(r: Rect) -> bool {
    r.contains(mouse_position().into())
}

pub fn text_center(t: &str, cx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(
        t,
        cx - d.width / 2.0 + 1.5,
        y + 1.5,
        size,
        Color::new(0.0, 0.0, 0.0, 0.5),
    );
    draw_text(t, cx - d.width / 2.0, y, size, color);
}

pub fn text_left(t: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(t, x, y, size, color);
}

pub fn text_right(t: &str, rx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(t, rx - d.width, y, size, color);
}

/// Greedy word wrap into lines no wider than `width`.
pub fn wrap(t: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in t.split_whitespace() {
        let next = if cur.is_empty() {
            word.to_owned()
        } else {
            format!("{cur} {word}")
        };
        if measure_text(&next, None, size as u16, 1.0).width > width && !cur.is_empty() {
            lines.push(std::mem::replace(&mut cur, word.to_owned()));
        } else {
            cur = next;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Fill a convex polygon.
fn fill_poly(points: &[Vec2], color: Color) {
    let c = points.iter().fold(Vec2::ZERO, |a, p| a + *p) / points.len() as f32;
    for i in 0..points.len() {
        draw_triangle(c, points[i], points[(i + 1) % points.len()], color);
    }
}

fn stroke_poly(points: &[Vec2], thickness: f32, color: Color) {
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }
}

/// Animation state that survives between frames.
#[derive(Default)]
pub struct Ui {
    hover: HashMap<String, f32>,
    dt: f32,
}

impl Ui {
    pub fn begin(&mut self, dt: f32) {
        self.dt = dt;
    }

    /// Ease an animation value toward `target` at `rate` and return it smoothed.
    fn animate(&mut self, id: &str, target: f32, rate: f32) -> f32 {
        let t = self.hover.entry(id.to_owned()).or_insert(0.0);
        *t += (target - *t) * (1.0 - (-rate * self.dt).exp());
        let x = t.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    /// An angled tab button. Returns true when clicked.
    pub fn tab(&mut self, id: &str, label: &str, r: Rect, side: Side, focused: bool) -> bool {
        let s = scale();
        let hot = hovered(r);
        let target = if hot || focused { 1.0 } else { 0.0 };
        // Fill and slide ease together; the text flips colour much faster,
        // so it changes before the fill has finished (both ways).
        let t = self.animate(id, target, TAB_EASE);
        let text_t = self.animate(&format!("{id}#text"), target, TAB_TEXT_EASE);
        let dir = if side == Side::Left { -1.0 } else { 1.0 };
        let x = r.x + dir * 13.0 * s * t;
        let (y, w, h) = (r.y, r.w, r.h);
        let cut = h * 0.42;
        let slant = h * 0.35;

        // Shape for a left tab (diagonal cut at the outer bottom corner,
        // slanted inner end); mirrored for the right side.
        let local = [
            vec2(0.0, 0.0),
            vec2(w, 0.0),
            vec2(w - slant, h),
            vec2(cut, h),
            vec2(0.0, h - cut),
        ];
        let pts: Vec<Vec2> = local
            .iter()
            .map(|p| match side {
                Side::Left => vec2(x + p.x, y + p.y),
                Side::Right => vec2(x + w - p.x, y + p.y),
            })
            .collect();
        fill_poly(&pts, lerp_color(TAB_FILL, TAB_HOVER, t));
        stroke_poly(
            &pts,
            1.5,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.35 + 0.4 * (1.0 - t)),
        );
        // Stripe along the outer edge of the body.
        let stripe = 3.0 * s;
        let inset = 5.0 * s;
        let sx = match side {
            Side::Left => x + inset,
            Side::Right => x + w - inset - stripe,
        };
        let stripe_col = lerp_color(ACCENT, TEXT_DARK, text_t);
        draw_rectangle(sx, y + 4.0 * s, stripe, h - cut - 2.0 * s, stripe_col);

        // Guide line running back toward the screen center.
        let (lx0, lx1) = match side {
            Side::Left => (x + w - slant, x + w + 40.0 * s),
            Side::Right => (x + slant - 40.0 * s, x + slant),
        };
        draw_line(
            lx0,
            y + h,
            lx1,
            y + h,
            1.5,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.35),
        );

        let size = h * 0.56;
        let color = lerp_color(WHITE, TEXT_DARK, text_t);
        let ty = y + h * 0.7;
        match side {
            Side::Left => text_left(label, x + inset + stripe + 12.0 * s, ty, size, color),
            Side::Right => text_right(label, x + w - inset - stripe - 12.0 * s, ty, size, color),
        }
        hot && is_mouse_button_pressed(MouseButton::Left)
    }

    /// A `label   <  value  >` row. Returns -1 / +1 on arrow clicks.
    pub fn stepper(
        &mut self,
        id: &str,
        label: &str,
        value: &str,
        r: Rect,
        focused: bool,
        enabled: bool,
    ) -> i32 {
        let s = scale();
        let row_hot = hovered(r) && enabled;
        let t = self.animate(id, if row_hot || focused { 1.0 } else { 0.0 }, TAB_EASE);
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h,
            Color::new(1.0, 1.0, 1.0, 0.04 + 0.08 * t),
        );
        draw_line(
            r.x,
            r.y + r.h,
            r.x + r.w,
            r.y + r.h,
            1.0,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.2 + 0.4 * t),
        );
        let a = if enabled { 1.0 } else { 0.35 };
        let size = r.h * 0.5;
        text_left(
            label,
            r.x + 10.0 * s,
            r.y + r.h * 0.68,
            size,
            Color::new(1.0, 1.0, 1.0, 0.75 * a),
        );
        let aw = r.h * 0.9;
        let left = Rect::new(r.x + r.w * 0.5, r.y, aw, r.h);
        let right = Rect::new(r.x + r.w - aw, r.y, aw, r.h);
        let mid = (left.x + left.w + right.x) / 2.0;
        text_center(
            value,
            mid,
            r.y + r.h * 0.68,
            size,
            Color::new(1.0, 1.0, 1.0, a),
        );
        if !enabled {
            return 0;
        }
        let mut delta = 0;
        for (rect, d, glyph) in [(left, -1, "<"), (right, 1, ">")] {
            let hot = hovered(rect);
            let c = if hot {
                ACCENT
            } else {
                Color::new(1.0, 1.0, 1.0, 0.6)
            };
            text_center(glyph, rect.x + rect.w / 2.0, r.y + r.h * 0.7, r.h * 0.6, c);
            if hot && is_mouse_button_pressed(MouseButton::Left) {
                delta = d;
            }
        }
        delta
    }
}

/// A framed panel with an optional title bar.
pub fn panel(r: Rect, title: Option<&str>) {
    let s = scale();
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        1.5,
        Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.35),
    );
    if let Some(t) = title {
        let bh = 30.0 * s;
        draw_rectangle(r.x, r.y, r.w, bh, Color::new(1.0, 1.0, 1.0, 0.07));
        text_center(t, r.x + r.w / 2.0, r.y + bh * 0.72, 22.0 * s, WHITE);
    }
}

/// Menu background: deep teal gradient, a faint grid and a framed border.
pub fn backdrop(time: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let top = Color::new(0.05, 0.12, 0.14, 1.0);
    let bottom = Color::new(0.02, 0.04, 0.06, 1.0);
    let bands = 24;
    for i in 0..bands {
        let t = i as f32 / bands as f32;
        let y = sh * t;
        draw_rectangle(
            0.0,
            y,
            sw,
            sh / bands as f32 + 1.0,
            lerp_color(top, bottom, t),
        );
    }
    let s = scale();
    let step = 40.0 * s;
    let drift = (time * 6.0) % step;
    let grid = Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.05);
    let mut x = -drift;
    while x < sw {
        draw_line(x, 0.0, x, sh, 1.0, grid);
        x += step;
    }
    let mut y = -drift;
    while y < sh {
        draw_line(0.0, y, sw, y, 1.0, grid);
        y += step;
    }
    let m = 14.0 * s;
    draw_rectangle_lines(
        m,
        m,
        sw - 2.0 * m,
        sh - 2.0 * m,
        2.0,
        Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.25),
    );
    let c = 26.0 * s;
    for (cx, cy, dx, dy) in [
        (m, m, 1.0, 1.0),
        (sw - m, m, -1.0, 1.0),
        (m, sh - m, 1.0, -1.0),
        (sw - m, sh - m, -1.0, -1.0),
    ] {
        draw_line(cx, cy, cx + c * dx, cy, 3.0, ACCENT);
        draw_line(cx, cy, cx, cy + c * dy, 3.0, ACCENT);
    }
}

/// Miniature of a map's terrain, water, hazards and objects fitted into `r`.
pub fn map_preview(map: &MapDef, thumb: &Texture2D, r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::from_rgba(8, 10, 16, 255));
    let k = (r.w / map.width).min(r.h / map.height);
    let o = vec2(
        r.x + (r.w - k * map.width) / 2.0,
        r.y + (r.h - k * map.height) / 2.0,
    );
    let to_screen = |p: Vec2| o + p * k;
    draw_texture_ex(
        thumb,
        o.x,
        o.y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(map.width, map.height) * k),
            ..Default::default()
        },
    );
    for obj in &map.objects {
        let c = match obj.kind {
            ObjectKind::Thruster => Color::new(0.3, 1.0, 0.75, 1.0),
            ObjectKind::WeaponStation => Color::new(1.0, 0.6, 0.25, 1.0),
            ObjectKind::ShieldStation => Color::new(0.35, 0.85, 1.0, 1.0),
            ObjectKind::PowerupSpot => Color::new(0.75, 0.45, 1.0, 1.0),
        };
        let p = to_screen(obj.feet() - vec2(0.0, 0.5));
        draw_circle(p.x, p.y, (k * 0.6).max(2.0), c);
    }
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        1.5,
        Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.5),
    );
}
