//! Small immediate-mode widget kit for the menus.
//!
//! Buttons are angled tabs anchored to the left or right side of the screen.
//! Hovering eases the tab to white and slides it outward (left tabs move
//! left, right tabs mirror that and move right); leaving eases it back.

use crate::data::{MapDef, ObjectKind};
use crate::i18n::t;
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
const LOCKED_FILL: Color = Color::new(0.06, 0.09, 0.1, 0.55);
const LOCKED_EDGE: Color = Color::new(0.45, 0.5, 0.52, 0.35);
const BALLOON: Color = Color::new(1.0, 0.86, 0.38, 1.0);
/// How long the mouse rests on something before its description pops up.
const TIP_DELAY: f32 = 0.45;

pub fn scale() -> f32 {
    (screen_height() / crate::platform::ui_height()).max(0.6)
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

thread_local! {
    /// Screenshot runs can move a pretend mouse (see `set_fake_mouse`).
    static FAKE_MOUSE: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
    /// Where the current press began, on screen (touch: a tap only counts
    /// if the finger lifts near there).
    static PRESS_AT: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
    /// How far the page being drawn is scrolled (see `Scroll`).
    static SCROLL_Y: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

/// How far a finger may slide and still tap, in UI units.
const TAP_SLOP: f32 = 14.0;

/// Make the menus see the mouse at `p` (screenshot runs only).
pub fn set_fake_mouse(p: Option<Vec2>) {
    FAKE_MOUSE.with(|m| m.set(p));
}

/// The pointer, on the page being drawn. On a touch screen there is none
/// while no finger is down, so nothing stays lit after a tap.
fn mouse() -> Vec2 {
    if let Some(p) = FAKE_MOUSE.with(|m| m.get()) {
        return p;
    }
    if crate::platform::touch()
        && !is_mouse_button_down(MouseButton::Left)
        && !is_mouse_button_released(MouseButton::Left)
    {
        return vec2(-1e5, -1e5);
    }
    Vec2::from(mouse_position()) + vec2(0.0, SCROLL_Y.with(|y| y.get()))
}

/// A click on something under the pointer (`hot`). With a mouse, the press;
/// on a touch screen, lifting the finger near where it went down, so that
/// dragging a page never presses what it started on.
fn clicked(hot: bool) -> bool {
    if !hot {
        return false;
    }
    if !crate::platform::touch() {
        return is_mouse_button_pressed(MouseButton::Left);
    }
    is_mouse_button_released(MouseButton::Left)
        && PRESS_AT
            .with(|p| p.get())
            .is_some_and(|at| at.distance(mouse_position().into()) <= TAP_SLOP * scale())
}

/// A page taller than the screen, dragged up and down (and scrolled with
/// the wheel). Draw it between `begin` and `end`, laid out as if the screen
/// were `page_h` tall.
#[derive(Default)]
pub struct Scroll {
    pub y: f32,
    drag: Option<f32>,
}

impl Scroll {
    /// Start drawing the page; widgets see the pointer on the page.
    pub fn begin(&mut self, page_h: f32) {
        let most = (page_h - screen_height()).max(0.0);
        // Screenshot runs: ARCANE_SHOT_SCROLL=<0..1> of the way down.
        if let Some(f) = std::env::var("ARCANE_SHOT_SCROLL")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            self.y = most * f;
        }
        let drag = drag_y(&mut self.drag);
        self.y = (self.y - drag - mouse_wheel().1.signum() * 40.0 * scale()).clamp(0.0, most);
        SCROLL_Y.with(|y| y.set(self.y));
        self.resume();
    }

    /// Put the page's view back after something drew off screen (say, a
    /// map picture into its texture), which resets it.
    pub fn resume(&self) {
        let (sw, sh) = (screen_width(), screen_height());
        if self.y > 0.0 {
            crate::render::use_camera(&Camera2D {
                target: vec2(sw / 2.0, sh / 2.0 + self.y),
                zoom: vec2(2.0 / sw, 2.0 / sh),
                ..Default::default()
            });
        }
    }

    /// Back to the screen (for what stays put), with a bar showing where
    /// on the page the screen is.
    pub fn end(&self, page_h: f32) {
        crate::render::use_screen();
        SCROLL_Y.with(|y| y.set(0.0));
        let (sw, sh) = (screen_width(), screen_height());
        if page_h > sh {
            let s = scale();
            let h = sh * sh / page_h;
            let y = self.y / (page_h - sh) * (sh - h);
            draw_rectangle(sw - 6.0 * s, y, 4.0 * s, h, Color::new(1.0, 1.0, 1.0, 0.35));
        }
    }
}

/// Escape, or the Back button on Android.
pub fn back_pressed() -> bool {
    is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Back)
}

/// How far a finger (or the mouse, button held) dragged up or down this
/// frame, in pixels: for scrolling lists by dragging them.
pub fn drag_y(last: &mut Option<f32>) -> f32 {
    let y = mouse_position().1;
    let dy = match *last {
        Some(prev) if is_mouse_button_down(MouseButton::Left) => y - prev,
        _ => 0.0,
    };
    *last = is_mouse_button_down(MouseButton::Left).then_some(y);
    dy
}

fn hovered(r: Rect) -> bool {
    r.contains(mouse())
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

/// Greedy word wrap into lines no wider than `width`. A word wider than a
/// whole line (a long file path) is split wherever it must be.
pub fn wrap(t: &str, width: f32, size: f32) -> Vec<String> {
    let fits = |s: &str| measure_text(s, None, size as u16, 1.0).width <= width;
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in t.split_whitespace() {
        let next = if cur.is_empty() {
            word.to_owned()
        } else {
            format!("{cur} {word}")
        };
        if fits(&next) {
            cur = next;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        for c in word.chars() {
            cur.push(c);
            if !fits(&cur) && cur.chars().count() > 1 {
                cur.pop();
                lines.push(std::mem::replace(&mut cur, c.to_string()));
            }
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
    /// The description under the mouse, and whether something asked for it
    /// this frame (if not, the mouse has left it).
    tip: Option<Tip>,
    tip_seen: bool,
}

struct Tip {
    id: String,
    text: String,
    /// How long the mouse has rested on it.
    age: f32,
    /// A balloon pointing at this button, with this title (like "Coming
    /// Soon!"), instead of a plain description at the mouse.
    soon: Option<(Rect, &'static str)>,
}

impl Ui {
    pub fn begin(&mut self, dt: f32) {
        self.dt = dt;
        self.tip_seen = false;
        if is_mouse_button_pressed(MouseButton::Left) {
            PRESS_AT.with(|p| p.set(Some(mouse_position().into())));
        }
    }

    /// Pop up `text` when the mouse rests on `r`. Call each frame.
    pub fn hint(&mut self, id: &str, r: Rect, text: &str) {
        self.tip_at(id, r, text, None);
    }

    /// A "Coming Soon!" balloon with `text` under it, beside `r`, while the
    /// mouse is on it.
    pub fn soon(&mut self, id: &str, r: Rect, text: &str) {
        self.balloon(id, r, t("Coming Soon!"), text);
    }

    /// A balloon titled `title`, with `text` under it, beside `r`, while
    /// the mouse is on it.
    pub fn balloon(&mut self, id: &str, r: Rect, title: &'static str, text: &str) {
        self.tip_at(id, r, text, Some((r, title)));
    }

    fn tip_at(&mut self, id: &str, r: Rect, text: &str, soon: Option<(Rect, &'static str)>) {
        if !hovered(r) {
            return;
        }
        self.tip_seen = true;
        match &mut self.tip {
            Some(tip) if tip.id == id => tip.age += self.dt,
            _ => {
                self.tip = Some(Tip {
                    id: id.to_owned(),
                    text: text.to_owned(),
                    age: 0.0,
                    soon,
                })
            }
        }
    }

    /// Draw the description asked for this frame, if any. Call last.
    pub fn draw_tip(&mut self) {
        if !self.tip_seen {
            self.tip = None;
        }
        let Some(tip) = &self.tip else {
            return;
        };
        if let Some((anchor, title)) = tip.soon {
            balloon(anchor, title, &tip.text, tip.age);
            return;
        }
        if tip.age < TIP_DELAY {
            return;
        }
        let s = scale();
        let size = 15.0 * s;
        let pad = 8.0 * s;
        let lines = wrap(&tip.text, 280.0 * s, size);
        let w = lines
            .iter()
            .map(|l| measure_text(l, None, size as u16, 1.0).width)
            .fold(0.0, f32::max)
            + 2.0 * pad;
        let h = lines.len() as f32 * (size + 3.0 * s) + 2.0 * pad - 3.0 * s;
        let Vec2 { x: mx, y: my } = mouse();
        // Below and right of the cursor, kept on screen.
        let x = (mx + 14.0 * s)
            .min(screen_width() - w - 4.0 * s)
            .max(4.0 * s);
        let y = if my + 22.0 * s + h > screen_height() {
            my - h - 8.0 * s
        } else {
            my + 22.0 * s
        };
        draw_rectangle(x, y, w, h, Color::new(0.02, 0.05, 0.06, 0.95));
        draw_rectangle_lines(
            x,
            y,
            w,
            h,
            1.5,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.7),
        );
        for (i, line) in lines.iter().enumerate() {
            let ly = y + pad + size * 0.8 + i as f32 * (size + 3.0 * s);
            text_left(line, x + pad, ly, size, WHITE);
        }
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
        self.tab_ex(id, label, r, side, focused, true)
    }

    /// A greyed-out tab for something not built yet: it never lights up or
    /// slides. Pair it with `soon` for the balloon.
    pub fn tab_locked(&mut self, id: &str, label: &str, r: Rect, side: Side) {
        self.tab_ex(id, label, r, side, false, false);
    }

    fn tab_ex(
        &mut self,
        id: &str,
        label: &str,
        r: Rect,
        side: Side,
        focused: bool,
        enabled: bool,
    ) -> bool {
        let s = scale();
        let hot = enabled && hovered(r);
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
        // inner end slanting out toward the bottom); mirrored for the right
        // side.
        let local = [
            vec2(0.0, 0.0),
            vec2(w - slant, 0.0),
            vec2(w, h),
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
        let (fill, edge) = if enabled {
            (
                lerp_color(TAB_FILL, TAB_HOVER, t),
                Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.35 + 0.4 * (1.0 - t)),
            )
        } else {
            (LOCKED_FILL, LOCKED_EDGE)
        };
        fill_poly(&pts, fill);
        stroke_poly(&pts, 1.5, edge);
        // Stripe along the outer edge of the body.
        let stripe = 3.0 * s;
        let inset = 5.0 * s;
        let sx = match side {
            Side::Left => x + inset,
            Side::Right => x + w - inset - stripe,
        };
        let stripe_col = if enabled {
            lerp_color(ACCENT, TEXT_DARK, text_t)
        } else {
            LOCKED_EDGE
        };
        draw_rectangle(sx, y + 4.0 * s, stripe, h - cut - 2.0 * s, stripe_col);

        let size = h * 0.56;
        let color = if enabled {
            lerp_color(WHITE, TEXT_DARK, text_t)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.32)
        };
        let ty = y + h * 0.7;
        match side {
            Side::Left => text_left(label, x + inset + stripe + 12.0 * s, ty, size, color),
            Side::Right => text_right(label, x + w - inset - stripe - 12.0 * s, ty, size, color),
        }
        clicked(hot)
    }

    /// A flat rectangular button in the menu style: hovering eases the fill
    /// to white and flips the text dark, like a tab but without the slide.
    /// `on` lights it in the accent colour (the tool in use). Returns true
    /// when clicked.
    pub fn button(&mut self, id: &str, label: &str, r: Rect, on: bool, enabled: bool) -> bool {
        let s = scale();
        let hot = enabled && hovered(r);
        let t = self.animate(id, if hot || on { 1.0 } else { 0.0 }, TAB_EASE);
        let text_t = self.animate(
            &format!("{id}#text"),
            if hot || on { 1.0 } else { 0.0 },
            TAB_TEXT_EASE,
        );
        let lit = if on && !hot { ACCENT } else { TAB_HOVER };
        draw_rectangle(r.x, r.y, r.w, r.h, lerp_color(TAB_FILL, lit, t));
        draw_rectangle_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            1.5,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.35 + 0.4 * (1.0 - t)),
        );
        let a = if enabled { 1.0 } else { 0.35 };
        let stripe = 3.0 * s;
        let inset = 4.0 * s;
        let mut stripe_col = lerp_color(ACCENT, TEXT_DARK, text_t);
        stripe_col.a *= a;
        draw_rectangle(
            r.x + inset,
            r.y + inset,
            stripe,
            r.h - 2.0 * inset,
            stripe_col,
        );
        let mut color = lerp_color(WHITE, TEXT_DARK, text_t);
        color.a *= a;
        text_left(
            label,
            r.x + inset + stripe + 8.0 * s,
            r.y + r.h * 0.68,
            r.h * 0.56,
            color,
        );
        clicked(hot)
    }

    /// A button that switches something on and off, with a light at its
    /// right end. Returns true when clicked.
    pub fn toggle(&mut self, id: &str, label: &str, r: Rect, value: bool, enabled: bool) -> bool {
        let clicked = self.button(id, label, r, false, enabled);
        let k = r.h * 0.4;
        let (x, y) = (r.x + r.w - k - r.h * 0.3, r.y + (r.h - k) / 2.0);
        if value {
            draw_rectangle(x, y, k, k, ACCENT);
        }
        draw_rectangle_lines(x, y, k, k, 1.5, Color::new(0.5, 0.6, 0.62, 1.0));
        clicked
    }

    /// A clickable colour square with a frame that lights up on hover.
    /// Returns true when clicked.
    pub fn swatch(&mut self, id: &str, r: Rect, color: Color) -> bool {
        let s = scale();
        let hot = hovered(r);
        let t = self.animate(id, if hot { 1.0 } else { 0.0 }, TAB_EASE);
        let grow = 2.0 * s * t;
        draw_rectangle(
            r.x - grow,
            r.y - grow,
            r.w + grow * 2.0,
            r.h + grow * 2.0,
            color,
        );
        draw_rectangle_lines(
            r.x - grow,
            r.y - grow,
            r.w + grow * 2.0,
            r.h + grow * 2.0,
            2.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.35 + 0.65 * t),
        );
        clicked(hot)
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
        // Fingers are blunt: on a touch screen each half of the value side
        // steps that way.
        let (left_hit, right_hit) = if crate::platform::touch() {
            let half = r.w * 0.25;
            (
                Rect::new(r.x + r.w * 0.5, r.y, half, r.h),
                Rect::new(r.x + r.w * 0.75, r.y, half, r.h),
            )
        } else {
            (left, right)
        };
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
        for (rect, hit, d, glyph) in [(left, left_hit, -1, "<"), (right, right_hit, 1, ">")] {
            let hot = hovered(hit);
            let c = if hot {
                ACCENT
            } else {
                Color::new(1.0, 1.0, 1.0, 0.6)
            };
            text_center(glyph, rect.x + rect.w / 2.0, r.y + r.h * 0.7, r.h * 0.6, c);
            if clicked(hot) {
                delta = d;
            }
        }
        delta
    }
}

/// Rectangle with round corners.
fn round_rect(x: f32, y: f32, w: f32, h: f32, rad: f32, color: Color) {
    draw_rectangle(x + rad, y, w - 2.0 * rad, h, color);
    draw_rectangle(x, y + rad, rad, h - 2.0 * rad, color);
    draw_rectangle(x + w - rad, y + rad, rad, h - 2.0 * rad, color);
    for (cx, cy) in [
        (x + rad, y + rad),
        (x + w - rad, y + rad),
        (x + rad, y + h - rad),
        (x + w - rad, y + h - rad),
    ] {
        draw_circle(cx, cy, rad, color);
    }
}

/// A speech balloon titled `title_text` (and `text`) beside `anchor`,
/// its tail pointing at it. Pops in over `age`.
fn balloon(anchor: Rect, title_text: &str, text: &str, age: f32) {
    let s = scale();
    let pop = (age * 7.0).min(1.0);
    let (title, body) = (21.0 * s, 14.0 * s);
    let pad = 10.0 * s;
    let lines = wrap(text, 250.0 * s, body);
    let title_w = measure_text(title_text, None, title as u16, 1.0).width;
    let w = lines
        .iter()
        .map(|l| measure_text(l, None, body as u16, 1.0).width)
        .fold(title_w, f32::max)
        + 2.0 * pad;
    let h = title + 6.0 * s + lines.len() as f32 * (body + 3.0 * s) + 2.0 * pad;
    // Beside the button, toward the middle of the screen.
    let right = anchor.x + anchor.w / 2.0 < screen_width() / 2.0;
    let gap = 16.0 * s;
    let x = if right {
        anchor.x + anchor.w + gap
    } else {
        anchor.x - gap - w
    };
    let cy = anchor.y + anchor.h / 2.0;
    let y = (cy - h / 2.0 - 4.0 * s * (1.0 - pop)).clamp(4.0, screen_height() - h - 4.0);
    let fill = Color::new(BALLOON.r, BALLOON.g, BALLOON.b, pop);
    let (edge, tip) = if right {
        (x, x - gap + 4.0 * s)
    } else {
        (x + w, x + w + gap - 4.0 * s)
    };
    draw_triangle(
        vec2(edge, cy - 7.0 * s),
        vec2(edge, cy + 7.0 * s),
        vec2(tip, cy),
        fill,
    );
    round_rect(x, y, w, h, 8.0 * s, fill);
    let ink = Color::new(TEXT_DARK.r, TEXT_DARK.g, TEXT_DARK.b, pop);
    draw_text(title_text, x + pad, y + pad + title * 0.8, title, ink);
    for (i, line) in lines.iter().enumerate() {
        let ly = y + pad + title + 6.0 * s + body * 0.8 + i as f32 * (body + 3.0 * s);
        draw_text(
            line,
            x + pad,
            ly,
            body,
            Color::new(ink.r, ink.g, ink.b, 0.8 * pop),
        );
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
