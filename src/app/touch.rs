//! On-screen controls for touch screens: a move stick under the left thumb,
//! and a Fire and a Jump button under the right.
//!
//! The move stick floats: a thumb landing anywhere on the left side becomes
//! its centre, and dragging past the rim pulls the centre along. Left /
//! right walks, down crouches or drops through a hatch (and up jumps too).
//!
//! Fire works like the attack button of many phone shooters: hold it and
//! the game aims at the nearest enemy in sight (see `ai::assist`); drag
//! away from it to aim by hand. Either way it fires while held.

use crate::i18n::{t, tf};
use crate::sim::World;
use macroquad::prelude::*;

/// Sideways push needed to walk, as a share of the stick's reach.
const WALK_PUSH: f32 = 0.3;
/// Upward push that jumps, downward push that crouches.
const JUMP_PUSH: f32 = 0.6;
const CROUCH_PUSH: f32 = 0.55;
/// How far a thumb on Fire must drag, as a share of the button's radius,
/// before it aims by hand. Past it, the aim stays manual until the thumb
/// lifts.
const MANUAL_PUSH: f32 = 0.35;
/// How far, as a share of the radius, a hand-aimed drag reads as a full
/// push (for how far the camera looks ahead).
const FULL_PUSH: f32 = 1.6;
/// A touch this close to Fire or Jump (as a share of the radius) still
/// grabs it: thumbs land roughly.
const FIRE_GRAB: f32 = 1.5;
const JUMP_GRAB: f32 = 1.3;
/// Share of the screen width, from the left, that belongs to the move stick.
const MOVE_SIDE: f32 = 0.45;
/// Most weapon slots shown.
const MAX_SLOTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Pause,
    Scores,
    Reload,
    Switch,
    Ability,
    Jump,
    /// A weapon-bar slot: the weapon's index.
    Weapon(usize),
}

/// Where everything sits on a screen of this size, in pixels.
#[derive(Debug, Clone)]
pub struct Layout {
    pub buttons: Vec<(Button, Rect)>,
    /// Where the move stick rests while no thumb is on it.
    pub move_home: Vec2,
    /// How far the move stick's knob travels from its centre.
    pub radius: f32,
    /// The Fire button.
    pub fire: Vec2,
    pub fire_r: f32,
    /// The Jump button (round, inside its `buttons` square).
    pub jump: Vec2,
    pub jump_r: f32,
    /// Touches starting above this line never grab a stick.
    pub top: f32,
    pub width: f32,
}

impl Layout {
    /// `weapons`: the owned weapons, in bar order. `ability`: a held
    /// teleport charge, to show its button.
    pub fn new(size: Vec2, s: f32, weapons: &[usize], ability: bool) -> Self {
        let (sw, sh) = (size.x, size.y);
        let radius = 70.0 * s;
        let fire_r = 64.0 * s;
        let fire = vec2(sw - 36.0 * s - fire_r, sh - 30.0 * s - fire_r);
        let jump_r = 46.0 * s;
        let jump = fire + vec2(-fire_r - jump_r - 26.0 * s, 26.0 * s);
        let mut buttons = vec![
            (
                Button::Pause,
                Rect::new(14.0 * s, 14.0 * s, 48.0 * s, 48.0 * s),
            ),
            (
                Button::Scores,
                Rect::new(70.0 * s, 14.0 * s, 48.0 * s, 48.0 * s),
            ),
            (
                Button::Jump,
                Rect::new(jump.x - jump_r, jump.y - jump_r, 2.0 * jump_r, 2.0 * jump_r),
            ),
        ];
        // Reload and Switch above Fire, Step beside them when held.
        let (bw, bh) = (104.0 * s, 46.0 * s);
        let by = fire.y - fire_r - 22.0 * s - bh;
        let mut x = sw - 16.0 * s - bw;
        buttons.push((Button::Switch, Rect::new(x, by, bw, bh)));
        x -= bw + 10.0 * s;
        buttons.push((Button::Reload, Rect::new(x, by, bw, bh)));
        if ability {
            x -= bw + 10.0 * s;
            buttons.push((Button::Ability, Rect::new(x, by, bw, bh)));
        }
        // The owned weapons, bottom centre, kept clear of Jump (the HUD's
        // ammo line sits just above, see `render::hud`).
        let shown = &weapons[..weapons.len().min(MAX_SLOTS)];
        let gap = 6.0 * s;
        let room = 2.0 * (jump.x - jump_r * JUMP_GRAB - 12.0 * s - sw / 2.0);
        let n = shown.len().max(1) as f32;
        let ww = (84.0 * s).min((room + gap) / n - gap);
        let wh = 46.0 * s;
        let total = n * (ww + gap) - gap;
        let wx = (sw - total) / 2.0;
        let bar_top = sh - wh - 10.0 * s;
        for (k, &w) in shown.iter().enumerate() {
            let r = Rect::new(wx + k as f32 * (ww + gap), bar_top, ww, wh);
            buttons.push((Button::Weapon(w), r));
        }
        Self {
            buttons,
            move_home: vec2(40.0 * s + radius, sh - 40.0 * s - radius),
            radius,
            fire,
            fire_r,
            jump,
            jump_r,
            top: 72.0 * s,
            width: sw,
        }
    }

    /// The layout for this frame's screen and the player's kit.
    pub fn for_player(world: &World, player: usize) -> Self {
        let f = &world.fighters[player];
        let owned: Vec<usize> = (0..world.data.weapons.len())
            .filter(|&w| f.owns(w))
            .collect();
        Self::new(
            vec2(screen_width(), screen_height()),
            super::ui::scale(),
            &owned,
            f.teleport_charges > 0,
        )
    }

    /// The button under `p`. Fire and Jump are round and forgiving.
    fn button_at(&self, p: Vec2) -> Option<Button> {
        if p.distance(self.jump) <= self.jump_r * JUMP_GRAB {
            return Some(Button::Jump);
        }
        self.buttons
            .iter()
            .find(|(_, r)| r.contains(p))
            .map(|(b, _)| *b)
    }

    fn on_fire(&self, p: Vec2) -> bool {
        p.distance(self.fire) <= self.fire_r * FIRE_GRAB
    }
}

#[derive(Debug, Clone, Copy)]
struct Stick {
    id: u64,
    origin: Vec2,
    pos: Vec2,
}

impl Stick {
    /// The push as a vector of length 0..1, screen-down positive.
    fn push(&self, radius: f32) -> Vec2 {
        ((self.pos - self.origin) / radius).clamp_length_max(1.0)
    }

    /// Follow the thumb; past the rim, drag the centre along.
    fn moved(&mut self, p: Vec2, radius: f32) {
        self.pos = p;
        let d = p - self.origin;
        if d.length() > radius {
            self.origin = p - d.normalize() * radius;
        }
    }
}

/// A thumb on Fire: where it went down, where it is, and whether it has
/// dragged far enough to aim by hand.
#[derive(Debug, Clone, Copy)]
struct FireTouch {
    id: u64,
    origin: Vec2,
    pos: Vec2,
    manual: bool,
}

/// One touch as the controls see it.
#[derive(Debug, Clone, Copy)]
pub struct Finger {
    pub id: u64,
    pub pos: Vec2,
    /// Lifted (or cancelled) this frame.
    pub up: bool,
}

/// What the thumbs are doing, plus presses not yet handed to the sim.
#[derive(Debug, Default)]
pub struct TouchControls {
    move_stick: Option<Stick>,
    fire: Option<FireTouch>,
    /// Buttons held down, by touch.
    held: Vec<(u64, Button)>,
    /// The hand-aimed direction (unit, screen-down positive) while Fire is
    /// dragged.
    manual_dir: Option<Vec2>,
    up_zone: bool,
    down_zone: bool,
    /// Edges since the last `take_*` call.
    jumped: bool,
    dropped: bool,
    taps: Vec<Button>,
    /// Fingers already down when the controls were released (say, the one
    /// that tapped Resume): ignored until they lift.
    ignored: Vec<u64>,
    /// The move stick's reach and Fire's radius, from the last layout.
    radius: f32,
    fire_r: f32,
}

impl TouchControls {
    /// Feed this frame's touches.
    pub fn update(&mut self, fingers: &[Finger], layout: &Layout) {
        let r = layout.radius;
        self.radius = r;
        self.fire_r = layout.fire_r;
        // Fingers that vanished without a goodbye are lifted too.
        let gone = |id: u64| !fingers.iter().any(|f| f.id == id && !f.up);
        self.held.retain(|(id, _)| !gone(*id));
        self.ignored.retain(|id| !gone(*id));
        if self.move_stick.is_some_and(|s| gone(s.id)) {
            self.move_stick = None;
        }
        if self.fire.is_some_and(|f| gone(f.id)) {
            self.fire = None;
            self.manual_dir = None;
        }

        for f in fingers {
            if self.ignored.contains(&f.id) {
                continue;
            }
            if let Some(s) = self.move_stick.as_mut().filter(|s| s.id == f.id) {
                s.moved(f.pos, r);
                continue;
            }
            if let Some(fire) = self.fire.as_mut().filter(|t| t.id == f.id) {
                fire.pos = f.pos;
                continue;
            }
            if self.held.iter().any(|(id, _)| *id == f.id) {
                continue;
            }
            // A new finger: Fire, a button, or the move stick.
            if layout.on_fire(f.pos) && self.fire.is_none() && !f.up {
                self.fire = Some(FireTouch {
                    id: f.id,
                    origin: f.pos,
                    pos: f.pos,
                    manual: false,
                });
            } else if let Some(b) = layout.button_at(f.pos) {
                self.taps.push(b);
                if b == Button::Jump {
                    self.jumped = true;
                }
                if !f.up {
                    self.held.push((f.id, b));
                }
            } else if f.up || f.pos.y < layout.top {
                // A tap too short to steer, or the top strip: nothing.
            } else if f.pos.x < layout.width * MOVE_SIDE && self.move_stick.is_none() {
                self.move_stick = Some(Stick {
                    id: f.id,
                    origin: f.pos,
                    pos: f.pos,
                });
            }
        }

        let push = self.move_push();
        let up = push.y < -JUMP_PUSH;
        let down = push.y > CROUCH_PUSH;
        self.jumped |= up && !self.up_zone;
        self.dropped |= down && !self.down_zone;
        self.up_zone = up;
        self.down_zone = down;

        if let Some(fire) = self.fire.as_mut() {
            let d = (fire.pos - fire.origin) / layout.fire_r;
            if d.length() > MANUAL_PUSH {
                fire.manual = true;
                self.manual_dir = Some(d.normalize());
            }
        }
    }

    fn move_push(&self) -> Vec2 {
        self.move_stick.map_or(Vec2::ZERO, |s| s.push(self.radius))
    }

    /// -1, 0 or 1.
    pub fn move_x(&self) -> f32 {
        let x = self.move_push().x;
        if x > WALK_PUSH {
            1.0
        } else if x < -WALK_PUSH {
            -1.0
        } else {
            0.0
        }
    }

    pub fn jump_held(&self) -> bool {
        self.up_zone || self.holding(Button::Jump)
    }

    pub fn crouch(&self) -> bool {
        self.down_zone
    }

    /// Fire is held.
    pub fn firing(&self) -> bool {
        self.fire.is_some()
    }

    /// The hand-aimed direction while Fire is dragged; `None` while it is
    /// held still (aim assist) or not held.
    pub fn manual_dir(&self) -> Option<Vec2> {
        self.fire.filter(|f| f.manual).and(self.manual_dir)
    }

    /// How far Fire is dragged, 0..1, for how far the camera looks ahead.
    pub fn aim_push(&self) -> f32 {
        self.fire.filter(|f| f.manual).map_or(0.0, |f| {
            ((f.pos - f.origin).length() / (self.fire_r * FULL_PUSH)).min(1.0)
        })
    }

    pub fn holding(&self, b: Button) -> bool {
        self.held.iter().any(|(_, h)| *h == b)
    }

    pub fn take_jump(&mut self) -> bool {
        std::mem::take(&mut self.jumped)
    }

    pub fn take_drop(&mut self) -> bool {
        std::mem::take(&mut self.dropped)
    }

    pub fn take_taps(&mut self) -> Vec<Button> {
        std::mem::take(&mut self.taps)
    }

    /// Let go of everything (on pausing and resuming). Fingers still down
    /// are ignored until they lift.
    pub fn release(&mut self, fingers: &[Finger]) {
        *self = Self {
            radius: self.radius,
            fire_r: self.fire_r,
            ignored: fingers.iter().filter(|f| !f.up).map(|f| f.id).collect(),
            ..Self::default()
        };
    }

    /// Draw the stick and buttons.
    pub fn draw(&self, layout: &Layout, world: &World, player: usize, s: f32) {
        let r = layout.radius;
        let f = &world.fighters[player];
        let (origin, knob, active) = self
            .move_stick
            .map_or((layout.move_home, layout.move_home, false), |st| {
                (st.origin, st.origin + st.push(r) * r, true)
            });
        let a = if active { 1.0 } else { 0.55 };
        draw_circle(origin.x, origin.y, r, Color::new(0.0, 0.0, 0.0, 0.22 * a));
        draw_circle_lines(
            origin.x,
            origin.y,
            r,
            2.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.35 * a),
        );
        draw_circle(
            knob.x,
            knob.y,
            r * 0.42,
            Color::new(1.0, 1.0, 1.0, 0.45 * a),
        );

        // Fire: red, brighter while held, with the thumb's drag shown.
        let fc = layout.fire;
        let (fill, edge) = if self.firing() {
            (
                Color::new(0.85, 0.2, 0.18, 0.55),
                Color::new(1.0, 0.5, 0.45, 0.95),
            )
        } else {
            (
                Color::new(0.55, 0.1, 0.1, 0.35),
                Color::new(1.0, 0.45, 0.4, 0.6),
            )
        };
        draw_circle(fc.x, fc.y, layout.fire_r, fill);
        draw_circle_lines(fc.x, fc.y, layout.fire_r, 2.5 * s, edge);
        label(
            t("Fire"),
            Rect::new(
                fc.x - layout.fire_r,
                fc.y - 12.0 * s,
                2.0 * layout.fire_r,
                24.0 * s,
            ),
            22.0 * s,
            WHITE,
        );
        if let Some(dir) = self.manual_dir() {
            let tip = fc + dir * layout.fire_r * (0.55 + 0.45 * self.aim_push());
            draw_line(
                fc.x,
                fc.y,
                tip.x,
                tip.y,
                4.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.8),
            );
            draw_circle(tip.x, tip.y, 8.0 * s, WHITE);
        }

        for (b, rect) in &layout.buttons {
            let down = self.holding(*b);
            match b {
                Button::Jump => {
                    let jc = layout.jump;
                    let fill = if down {
                        Color::new(1.0, 1.0, 1.0, 0.45)
                    } else {
                        Color::new(0.0, 0.0, 0.0, 0.42)
                    };
                    draw_circle(jc.x, jc.y, layout.jump_r, fill);
                    draw_circle_lines(
                        jc.x,
                        jc.y,
                        layout.jump_r,
                        2.0 * s,
                        Color::new(1.0, 1.0, 1.0, 0.45),
                    );
                    label(t("Jump"), *rect, 19.0 * s, WHITE);
                }
                Button::Weapon(w) => {
                    let d = &world.data.weapons[*w];
                    let current = *w == f.current;
                    let cc = crate::render::category_color(d.category);
                    let fill = if down {
                        Color::new(1.0, 1.0, 1.0, 0.4)
                    } else if current {
                        Color::new(1.0, 1.0, 1.0, 0.22)
                    } else {
                        Color::new(0.0, 0.0, 0.0, 0.45)
                    };
                    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
                    if current {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0 * s, cc);
                    }
                    label(t(&d.short_name()), *rect, 16.0 * s, cc);
                }
                _ => {
                    let fill = if down {
                        Color::new(1.0, 1.0, 1.0, 0.45)
                    } else {
                        Color::new(0.0, 0.0, 0.0, 0.42)
                    };
                    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
                    draw_rectangle_lines(
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        1.5 * s,
                        Color::new(1.0, 1.0, 1.0, 0.35),
                    );
                    let white = Color::new(1.0, 1.0, 1.0, 0.9);
                    match b {
                        Button::Pause => {
                            let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
                            for dx in [-7.0, 3.0] {
                                draw_rectangle(
                                    cx + dx * s,
                                    cy - 10.0 * s,
                                    4.0 * s,
                                    20.0 * s,
                                    white,
                                );
                            }
                        }
                        Button::Scores => {
                            let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
                            for dy in [-8.0, -1.0, 6.0] {
                                draw_rectangle(
                                    cx - 11.0 * s,
                                    cy + dy * s,
                                    22.0 * s,
                                    3.0 * s,
                                    white,
                                );
                            }
                        }
                        Button::Reload => label(t("Reload"), *rect, 20.0 * s, white),
                        Button::Switch => label(t("Switch"), *rect, 20.0 * s, white),
                        Button::Ability => label(
                            &tf("Step x{}", &[&f.teleport_charges]),
                            *rect,
                            20.0 * s,
                            Color::new(0.75, 0.6, 1.0, 1.0),
                        ),
                        Button::Jump | Button::Weapon(_) => {}
                    }
                }
            }
        }
    }
}

/// Corner brackets around the enemy aim assist is locked on to.
pub fn draw_lock(world: &World, cam: &crate::render::GameCamera, target: usize, s: f32) {
    let b = world.fighters[target].aabb();
    let lo = cam.world_to_screen(b.min) - vec2(6.0, 6.0) * s;
    let hi = cam.world_to_screen(b.max) + vec2(6.0, 6.0) * s;
    let len = 10.0 * s;
    let c = Color::new(1.0, 0.35, 0.3, 0.9);
    for (x, y, dx, dy) in [
        (lo.x, lo.y, 1.0, 1.0),
        (hi.x, lo.y, -1.0, 1.0),
        (lo.x, hi.y, 1.0, -1.0),
        (hi.x, hi.y, -1.0, -1.0),
    ] {
        draw_line(x, y, x + dx * len, y, 2.5 * s, c);
        draw_line(x, y, x, y + dy * len, 2.5 * s, c);
    }
}

/// Text centred in a rectangle.
fn label(text: &str, r: Rect, size: f32, color: Color) {
    let d = measure_text(text, None, size as u16, 1.0);
    let x = r.x + (r.w - d.width) / 2.0;
    let y = r.y + (r.h + d.offset_y) / 2.0 - size * 0.06;
    draw_text(text, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_text(text, x, y, size, color);
}

/// This frame's touches, from macroquad. Screenshot runs can hold
/// pretend fingers down: `ARCANE_SHOT_FINGERS=x,y;x,y` in screen fractions.
pub fn fingers() -> Vec<Finger> {
    if let Ok(list) = std::env::var("ARCANE_SHOT_FINGERS") {
        let size = vec2(screen_width(), screen_height());
        return list
            .split(';')
            .filter_map(|p| {
                let (x, y) = p.split_once(',')?;
                Some(vec2(x.trim().parse().ok()?, y.trim().parse().ok()?) * size)
            })
            .enumerate()
            .map(|(i, pos)| Finger {
                id: 1000 + i as u64,
                pos,
                up: false,
            })
            .collect();
    }
    touches()
        .into_iter()
        .map(|t| Finger {
            id: t.id,
            pos: t.position,
            up: matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout::new(vec2(1200.0, 540.0), 1.0, &[0, 3], false)
    }

    fn down(id: u64, p: Vec2) -> Finger {
        Finger {
            id,
            pos: p,
            up: false,
        }
    }

    fn at(id: u64, x: f32, y: f32) -> Finger {
        down(id, vec2(x, y))
    }

    #[test]
    fn left_thumb_walks_and_crouches() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[at(1, 200.0, 400.0)], &l);
        assert_eq!(t.move_x(), 0.0);
        t.update(&[at(1, 260.0, 400.0)], &l);
        assert_eq!(t.move_x(), 1.0);
        t.update(&[at(1, 200.0, 460.0)], &l);
        assert!(t.crouch() && t.take_drop());
        t.update(&[], &l);
        assert_eq!(t.move_x(), 0.0);
        assert!(!t.crouch());
    }

    #[test]
    fn dragging_past_the_rim_pulls_the_stick_along() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[at(1, 200.0, 400.0)], &l);
        t.update(&[at(1, 500.0, 400.0)], &l);
        assert_eq!(t.move_x(), 1.0);
        // Coming back a little reverses at once: the centre followed.
        t.update(&[at(1, 400.0, 400.0)], &l);
        assert_eq!(t.move_x(), -1.0);
    }

    #[test]
    fn jump_button_jumps_on_each_tap_and_holds_for_height() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(2, l.jump)], &l);
        assert!(t.take_jump() && t.jump_held());
        t.update(&[down(2, l.jump)], &l);
        assert!(!t.take_jump() && t.jump_held());
        // Lift and tap again: the double jump.
        t.update(&[], &l);
        assert!(!t.jump_held());
        t.update(&[down(3, l.jump + vec2(10.0, 5.0))], &l);
        assert!(t.take_jump());
    }

    #[test]
    fn holding_fire_is_aim_assist_and_dragging_aims_by_hand() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(4, l.fire)], &l);
        assert!(t.firing() && t.manual_dir().is_none());
        // A small wobble stays assisted.
        t.update(&[down(4, l.fire + vec2(8.0, -6.0))], &l);
        assert!(t.manual_dir().is_none());
        // A real drag aims that way, and stays manual on coming back.
        t.update(&[down(4, l.fire + vec2(-60.0, 0.0))], &l);
        assert_eq!(t.manual_dir(), Some(vec2(-1.0, 0.0)));
        assert!(t.firing() && t.aim_push() > 0.5);
        t.update(&[down(4, l.fire + vec2(-10.0, 0.0))], &l);
        assert!(t.manual_dir().is_some());
        // Lifting stops firing and forgets the hand aim.
        t.update(&[], &l);
        assert!(!t.firing() && t.manual_dir().is_none());
    }

    #[test]
    fn both_thumbs_at_once_and_buttons_win_over_the_stick() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[at(1, 200.0, 400.0), down(2, l.fire)], &l);
        t.update(&[at(1, 120.0, 400.0), down(2, l.fire)], &l);
        assert_eq!(t.move_x(), -1.0);
        assert!(t.firing());
        // A third finger on a weapon slot picks it, without moving a stick.
        let (_, slot) = l
            .buttons
            .iter()
            .find(|(b, _)| *b == Button::Weapon(3))
            .unwrap();
        let tap = down(3, slot.center());
        t.update(&[at(1, 120.0, 400.0), down(2, l.fire), tap], &l);
        assert_eq!(t.take_taps(), vec![Button::Weapon(3)]);
        assert_eq!(t.move_x(), -1.0);
        assert!(t.firing());
        // A second thumb on the left does not steal the move stick.
        t.update(&[at(1, 120.0, 400.0), at(4, 300.0, 300.0)], &l);
        assert_eq!(t.move_x(), -1.0);
    }

    #[test]
    fn buttons_stay_clear_of_each_other() {
        for (w, h) in [(1200.0, 540.0), (960.0, 540.0), (1170.0, 540.0)] {
            let all: Vec<usize> = (0..10).collect();
            let l = Layout::new(vec2(w, h), 1.0, &all, true);
            let jump_box = Rect::new(
                l.jump.x - l.jump_r * JUMP_GRAB,
                l.jump.y - l.jump_r * JUMP_GRAB,
                2.0 * l.jump_r * JUMP_GRAB,
                2.0 * l.jump_r * JUMP_GRAB,
            );
            for (b, r) in &l.buttons {
                if *b != Button::Jump {
                    assert!(!r.overlaps(&jump_box), "{b:?} overlaps Jump at {w}x{h}");
                    assert!(!l.on_fire(r.center()), "{b:?} sits on Fire at {w}x{h}");
                }
                assert!(r.x >= 0.0 && r.x + r.w <= w, "{b:?} off screen at {w}x{h}");
            }
        }
    }

    #[test]
    fn a_tap_shorter_than_a_frame_still_presses_a_button() {
        let l = layout();
        let mut t = TouchControls::default();
        let (_, pause) = l.buttons[0];
        t.update(
            &[Finger {
                id: 7,
                pos: pause.center(),
                up: true,
            }],
            &l,
        );
        assert_eq!(t.take_taps(), vec![Button::Pause]);
        assert!(!t.holding(Button::Pause));
    }

    #[test]
    fn a_finger_down_at_release_is_ignored_until_it_lifts() {
        let l = layout();
        let mut t = TouchControls::default();
        t.release(&[at(5, 200.0, 400.0)]);
        t.update(&[at(5, 300.0, 400.0)], &l);
        assert_eq!(t.move_x(), 0.0);
        t.update(&[], &l);
        t.update(&[at(5, 200.0, 400.0)], &l);
        t.update(&[at(5, 300.0, 400.0)], &l);
        assert_eq!(t.move_x(), 1.0);
    }

    #[test]
    fn the_top_strip_never_grabs_the_stick() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[at(1, 300.0, 30.0)], &l);
        t.update(&[at(1, 400.0, 300.0)], &l);
        assert_eq!(t.move_x(), 0.0);
    }
}
