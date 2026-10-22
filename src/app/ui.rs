//! Tiny immediate-mode widgets for the menus.

use macroquad::prelude::*;

pub fn scale() -> f32 {
    (screen_height() / 720.0).max(0.6)
}

pub fn text_center(t: &str, cx: f32, y: f32, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    draw_text(
        t,
        cx - d.width / 2.0 + 2.0,
        y + 2.0,
        size,
        Color::new(0.0, 0.0, 0.0, 0.5),
    );
    draw_text(t, cx - d.width / 2.0, y, size, color);
}

pub fn text_left(t: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(t, x, y, size, color);
}

fn hovered(r: Rect) -> bool {
    r.contains(mouse_position().into())
}

/// A clickable button. `focused` highlights it for keyboard navigation.
pub fn button(label: &str, r: Rect, focused: bool) -> bool {
    let hot = hovered(r);
    let bg = if hot || focused {
        Color::new(0.33, 0.63, 1.0, 0.35)
    } else {
        Color::new(1.0, 1.0, 1.0, 0.08)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    let border = if focused {
        Color::new(0.33, 0.63, 1.0, 1.0)
    } else {
        Color::new(1.0, 1.0, 1.0, 0.25)
    };
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, border);
    text_center(label, r.x + r.w / 2.0, r.y + r.h * 0.66, r.h * 0.5, WHITE);
    hot && is_mouse_button_pressed(MouseButton::Left)
}

/// A `< value >` row. Returns -1 / +1 when an arrow is clicked.
pub fn stepper(label: &str, value: &str, r: Rect, focused: bool, enabled: bool) -> i32 {
    let s = scale();
    if focused {
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.33, 0.63, 1.0, 0.18));
    }
    let a = if enabled { 1.0 } else { 0.35 };
    text_left(
        label,
        r.x + 16.0 * s,
        r.y + r.h * 0.65,
        r.h * 0.5,
        Color::new(1.0, 1.0, 1.0, 0.8 * a),
    );
    let aw = r.h;
    let left = Rect::new(r.x + r.w * 0.45, r.y, aw, r.h);
    let right = Rect::new(r.x + r.w - aw - 8.0 * s, r.y, aw, r.h);
    let mid = (left.x + left.w + right.x) / 2.0;
    text_center(
        value,
        mid,
        r.y + r.h * 0.65,
        r.h * 0.52,
        Color::new(1.0, 1.0, 1.0, a),
    );
    if !enabled {
        return 0;
    }
    let mut delta = 0;
    for (rect, d, glyph) in [(left, -1, "<"), (right, 1, ">")] {
        let hot = hovered(rect);
        let c = if hot {
            Color::new(0.33, 0.63, 1.0, 1.0)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.7)
        };
        text_center(glyph, rect.x + rect.w / 2.0, r.y + r.h * 0.68, r.h * 0.6, c);
        if hot && is_mouse_button_pressed(MouseButton::Left) {
            delta = d;
        }
    }
    delta
}
