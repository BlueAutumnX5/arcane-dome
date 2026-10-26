//! On-screen controls for touch screens: a move stick under the left
//! thumb, an aim stick under the right, and a few buttons.
//!
//! Both sticks float: a thumb landing anywhere on its half of the screen
//! becomes the stick's centre, and dragging past the rim pulls the centre
//! along. Move stick: left / right walks, up jumps (flick up again for the
//! double jump), down crouches or drops through a hatch. Aim stick: the
//! direction aims; pushing past the inner ring fires.

use crate::sim::World;
use macroquad::prelude::*;

/// Sideways push needed to walk, as a share of the stick's reach.
const WALK_PUSH: f32 = 0.3;
/// Upward push that jumps, downward push that crouches.
const JUMP_PUSH: f32 = 0.5;
const CROUCH_PUSH: f32 = 0.55;
/// Push below which the aim stick does not turn the aim (a resting thumb).
const AIM_DEAD: f32 = 0.2;
/// Push that fires: the inner ring.
pub const FIRE_PUSH: f32 = 0.65;
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
    /// A weapon-bar slot: the weapon's index.
    Weapon(usize),
}

/// Where everything sits on a screen of this size, in pixels.
#[derive(Debug, Clone)]
pub struct Layout {
    pub buttons: Vec<(Button, Rect)>,
    /// Where each stick rests while no thumb is on it.
    pub move_home: Vec2,
    pub aim_home: Vec2,
    /// How far a stick's knob travels from its centre.
    pub radius: f32,
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
        let mut buttons = vec![
            (
                Button::Pause,
                Rect::new(14.0 * s, 14.0 * s, 48.0 * s, 48.0 * s),
            ),
            (
                Button::Scores,
                Rect::new(70.0 * s, 14.0 * s, 48.0 * s, 48.0 * s),
            ),
        ];
        // Actions in a row above the aim stick.
        let (bw, bh) = (104.0 * s, 50.0 * s);
        let by = sh - 300.0 * s;
        let mut x = sw - 16.0 * s - bw;
        buttons.push((Button::Switch, Rect::new(x, by, bw, bh)));
        x -= bw + 10.0 * s;
        buttons.push((Button::Reload, Rect::new(x, by, bw, bh)));
        if ability {
            x -= bw + 10.0 * s;
            buttons.push((Button::Ability, Rect::new(x, by, bw, bh)));
        }
        // The owned weapons, bottom centre (the HUD's ammo line sits just
        // above, see `render::hud`).
        let (ww, wh, gap) = (84.0 * s, 46.0 * s, 6.0 * s);
        let shown = &weapons[..weapons.len().min(MAX_SLOTS)];
        let total = shown.len() as f32 * (ww + gap) - gap;
        let wx = (sw - total) / 2.0;
        let bar_top = sh - wh - 10.0 * s;
        for (k, &w) in shown.iter().enumerate() {
            let r = Rect::new(wx + k as f32 * (ww + gap), bar_top, ww, wh);
            buttons.push((Button::Weapon(w), r));
        }
        Self {
            buttons,
            move_home: vec2(40.0 * s + radius, sh - 40.0 * s - radius),
            aim_home: vec2(sw - 40.0 * s - radius, sh - 40.0 * s - radius),
            radius,
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

    pub fn button_at(&self, p: Vec2) -> Option<Button> {
        self.buttons
            .iter()
            .find(|(_, r)| r.contains(p))
            .map(|(b, _)| *b)
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
    aim_stick: Option<Stick>,
    /// Buttons held down, by touch.
    held: Vec<(u64, Button)>,
    /// Last aim direction (unit, screen-down positive), kept after the
    /// thumb lifts.
    aim_dir: Option<Vec2>,
    up_zone: bool,
    down_zone: bool,
    /// Edges since the last `take_*` call.
    jumped: bool,
    dropped: bool,
    taps: Vec<Button>,
    /// Fingers already down when the controls were released (say, the one
    /// that tapped Resume): ignored until they lift.
    ignored: Vec<u64>,
    /// The sticks' reach, from the last layout.
    radius: f32,
}

impl TouchControls {
    /// Feed this frame's touches.
    pub fn update(&mut self, fingers: &[Finger], layout: &Layout) {
        let r = layout.radius;
        self.radius = r;
        // Fingers that vanished without a goodbye are lifted too.
        let gone = |id: u64| !fingers.iter().any(|f| f.id == id && !f.up);
        self.held.retain(|(id, _)| !gone(*id));
        self.ignored.retain(|id| !gone(*id));
        if self.move_stick.is_some_and(|s| gone(s.id)) {
            self.move_stick = None;
        }
        if self.aim_stick.is_some_and(|s| gone(s.id)) {
            self.aim_stick = None;
        }

        for f in fingers {
            if self.ignored.contains(&f.id) {
                continue;
            }
            let known = self.held.iter().any(|(id, _)| *id == f.id)
                || self.move_stick.is_some_and(|s| s.id == f.id)
                || self.aim_stick.is_some_and(|s| s.id == f.id);
            if known {
                if let Some(s) = self.move_stick.as_mut().filter(|s| s.id == f.id) {
                    s.moved(f.pos, r);
                }
                if let Some(s) = self.aim_stick.as_mut().filter(|s| s.id == f.id) {
                    s.moved(f.pos, r);
                }
                continue;
            }
            // A new finger: a button, else the stick for its half.
            if let Some(b) = layout.button_at(f.pos) {
                self.taps.push(b);
                if !f.up {
                    self.held.push((f.id, b));
                }
            } else if f.up || f.pos.y < layout.top {
                // A tap too short to steer, or the top strip: nothing.
            } else if f.pos.x < layout.width * MOVE_SIDE {
                if self.move_stick.is_none() {
                    self.move_stick = Some(Stick {
                        id: f.id,
                        origin: f.pos,
                        pos: f.pos,
                    });
                }
            } else if self.aim_stick.is_none() {
                self.aim_stick = Some(Stick {
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

        if let Some(s) = self.aim_stick {
            let v = s.push(r);
            if v.length() > AIM_DEAD {
                self.aim_dir = Some(v.normalize());
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
        self.up_zone
    }

    pub fn crouch(&self) -> bool {
        self.down_zone
    }

    /// The aim direction, if the player has aimed yet.
    pub fn aim_dir(&self) -> Option<Vec2> {
        self.aim_dir
    }

    /// How hard the aim stick is pushed, 0..1 (0 with no thumb on it).
    pub fn aim_push(&self) -> f32 {
        self.aim_stick.map_or(0.0, |s| s.push(self.radius).length())
    }

    pub fn firing(&self) -> bool {
        self.aim_push() >= FIRE_PUSH
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
            aim_dir: self.aim_dir,
            radius: self.radius,
            ignored: fingers.iter().filter(|f| !f.up).map(|f| f.id).collect(),
            ..Self::default()
        };
    }

    /// Draw the sticks and buttons.
    pub fn draw(&self, layout: &Layout, world: &World, player: usize, s: f32) {
        let r = layout.radius;
        let f = &world.fighters[player];
        let move_stick = self
            .move_stick
            .map_or((layout.move_home, layout.move_home, false), |st| {
                (st.origin, st.origin + st.push(r) * r, true)
            });
        draw_stick(move_stick, r, s, None);
        let aim_stick = self
            .aim_stick
            .map_or((layout.aim_home, layout.aim_home, false), |st| {
                (st.origin, st.origin + st.push(r) * r, true)
            });
        draw_stick(aim_stick, r, s, Some(self.firing()));

        for (b, rect) in &layout.buttons {
            let down = self.holding(*b);
            match b {
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
                    label(&d.short_name(), *rect, 16.0 * s, cc);
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
                        Button::Reload => label("Reload", *rect, 20.0 * s, white),
                        Button::Switch => label("Switch", *rect, 20.0 * s, white),
                        Button::Ability => label(
                            &format!("Step x{}", f.teleport_charges),
                            *rect,
                            20.0 * s,
                            Color::new(0.75, 0.6, 1.0, 1.0),
                        ),
                        Button::Weapon(_) => {}
                    }
                }
            }
        }
    }
}

/// (centre, knob, in use); `fire`: the aim stick's firing state.
fn draw_stick((origin, knob, active): (Vec2, Vec2, bool), r: f32, s: f32, fire: Option<bool>) {
    let a = if active { 1.0 } else { 0.55 };
    draw_circle(origin.x, origin.y, r, Color::new(0.0, 0.0, 0.0, 0.22 * a));
    draw_circle_lines(
        origin.x,
        origin.y,
        r,
        2.0 * s,
        Color::new(1.0, 1.0, 1.0, 0.35 * a),
    );
    if let Some(firing) = fire {
        let c = if firing {
            Color::new(1.0, 0.35, 0.3, 0.8)
        } else {
            Color::new(1.0, 0.45, 0.4, 0.35 * a)
        };
        draw_circle_lines(origin.x, origin.y, r * FIRE_PUSH, 1.5 * s, c);
    }
    let knob_c = match fire {
        Some(true) => Color::new(1.0, 0.45, 0.4, 0.75),
        _ => Color::new(1.0, 1.0, 1.0, 0.45 * a),
    };
    draw_circle(knob.x, knob.y, r * 0.42, knob_c);
}

/// Text centred in a rectangle.
fn label(t: &str, r: Rect, size: f32, color: Color) {
    let d = measure_text(t, None, size as u16, 1.0);
    let x = r.x + (r.w - d.width) / 2.0;
    let y = r.y + (r.h + d.offset_y) / 2.0 - size * 0.06;
    draw_text(t, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_text(t, x, y, size, color);
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

    fn down(id: u64, x: f32, y: f32) -> Finger {
        Finger {
            id,
            pos: vec2(x, y),
            up: false,
        }
    }

    #[test]
    fn left_thumb_walks_and_jumps_once_per_flick() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(1, 200.0, 400.0)], &l);
        assert_eq!(t.move_x(), 0.0);
        t.update(&[down(1, 260.0, 400.0)], &l);
        assert_eq!(t.move_x(), 1.0);
        assert!(!t.take_jump());
        // Up: one jump, held while up.
        t.update(&[down(1, 260.0, 340.0)], &l);
        assert!(t.take_jump() && t.jump_held());
        t.update(&[down(1, 262.0, 338.0)], &l);
        assert!(!t.take_jump());
        // Back to the middle and up again: the double jump.
        t.update(&[down(1, 230.0, 360.0)], &l);
        t.update(&[down(1, 230.0, 300.0)], &l);
        assert!(t.take_jump());
        // Lifting the thumb lets go.
        t.update(&[], &l);
        assert_eq!(t.move_x(), 0.0);
        assert!(!t.jump_held());
    }

    #[test]
    fn dragging_past_the_rim_pulls_the_stick_along() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(1, 200.0, 400.0)], &l);
        t.update(&[down(1, 500.0, 400.0)], &l);
        assert_eq!(t.move_x(), 1.0);
        // Coming back a little reverses at once: the centre followed.
        t.update(&[down(1, 400.0, 400.0)], &l);
        assert_eq!(t.move_x(), -1.0);
    }

    #[test]
    fn right_thumb_aims_then_fires_past_the_inner_ring() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(2, 900.0, 400.0)], &l);
        assert!(t.aim_dir().is_none() && !t.firing());
        t.update(&[down(2, 930.0, 400.0)], &l);
        assert_eq!(t.aim_dir(), Some(vec2(1.0, 0.0)));
        assert!(!t.firing());
        t.update(&[down(2, 900.0, 470.0)], &l);
        assert!(t.firing());
        assert!(t.aim_dir().unwrap().y > 0.99);
        // Let go: stops firing, keeps facing that way.
        t.update(&[], &l);
        assert!(!t.firing());
        assert!(t.aim_dir().unwrap().y > 0.99);
    }

    #[test]
    fn both_thumbs_at_once_and_buttons_win_over_sticks() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(1, 200.0, 400.0), down(2, 900.0, 400.0)], &l);
        t.update(&[down(1, 120.0, 400.0), down(2, 900.0, 300.0)], &l);
        assert_eq!(t.move_x(), -1.0);
        assert!(t.firing());
        // A third finger on a weapon slot picks it, without moving a stick.
        let (_, slot) = l
            .buttons
            .iter()
            .find(|(b, _)| *b == Button::Weapon(3))
            .unwrap();
        let tap = down(3, slot.center().x, slot.center().y);
        t.update(&[down(1, 120.0, 400.0), down(2, 900.0, 300.0), tap], &l);
        assert_eq!(t.take_taps(), vec![Button::Weapon(3)]);
        assert_eq!(t.move_x(), -1.0);
        assert!(t.firing());
        // A second thumb on the left half does not steal the move stick.
        t.update(&[down(1, 120.0, 400.0), down(4, 300.0, 300.0)], &l);
        assert_eq!(t.move_x(), -1.0);
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
        t.release(&[down(5, 200.0, 400.0)]);
        t.update(&[down(5, 300.0, 400.0)], &l);
        assert_eq!(t.move_x(), 0.0);
        t.update(&[], &l);
        t.update(&[down(5, 200.0, 400.0)], &l);
        t.update(&[down(5, 300.0, 400.0)], &l);
        assert_eq!(t.move_x(), 1.0);
    }

    #[test]
    fn the_top_strip_never_grabs_a_stick() {
        let l = layout();
        let mut t = TouchControls::default();
        t.update(&[down(1, 600.0, 30.0)], &l);
        t.update(&[down(1, 700.0, 300.0)], &l);
        assert!(t.aim_dir().is_none() && t.aim_push() == 0.0);
    }
}
