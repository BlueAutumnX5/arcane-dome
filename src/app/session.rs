//! One running match: reads the keyboard and mouse, steps the sim at a fixed
//! 60 Hz, and draws it.

use super::ui;
use crate::ai::BotSet;
use crate::data::GameData;
use crate::render::{self, Effects, GameCamera};
use crate::sim::{DT, Input, MatchConfig, Phase, World};
use macroquad::prelude::*;
use std::sync::Arc;

pub enum Outcome {
    Continue,
    Rematch,
    Menu,
}

/// Presses seen between sim ticks, so none are lost when the frame rate
/// is higher than the tick rate.
#[derive(Default)]
struct Edges {
    jump: bool,
    reload: bool,
    select: Option<usize>,
    cycle: i32,
}

pub struct Session {
    world: World,
    bots: BotSet,
    player: usize,
    effects: Effects,
    cam: GameCamera,
    acc: f32,
    paused: bool,
    edges: Edges,
    over_for: f32,
}

const SLOT_KEYS: [KeyCode; 10] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
    KeyCode::Key7,
    KeyCode::Key8,
    KeyCode::Key9,
    KeyCode::Key0,
];

impl Session {
    pub fn new(data: Arc<GameData>, cfg: MatchConfig) -> Result<Self, String> {
        let world = World::new(data, cfg)?;
        let bots = BotSet::new(&world);
        let mut cam = GameCamera::new(world.fighters[0].center());
        cam.snap(world.fighters[0].center());
        Ok(Self {
            world,
            bots,
            player: 0,
            effects: Effects::default(),
            cam,
            acc: 0.0,
            paused: false,
            edges: Edges::default(),
            over_for: 0.0,
        })
    }

    pub fn config(&self) -> &MatchConfig {
        &self.world.config
    }

    fn held(keys: &[KeyCode]) -> bool {
        keys.iter().any(|k| is_key_down(*k))
    }

    fn pressed(keys: &[KeyCode]) -> bool {
        keys.iter().any(|k| is_key_pressed(*k))
    }

    pub fn frame(&mut self, dt: f32) -> Outcome {
        let over = matches!(self.world.phase, Phase::MatchOver { .. });
        if !over && Self::pressed(&[KeyCode::P, KeyCode::Escape]) {
            self.paused = !self.paused;
        }
        let mouse: Vec2 = mouse_position().into();

        if !self.paused {
            self.collect_edges();
            self.acc += dt;
            while self.acc >= DT {
                self.acc -= DT;
                self.tick(mouse);
            }
        }
        self.effects.update(if self.paused { 0.0 } else { dt });
        let focus = self.focus_point();
        self.cam.update(&self.world, focus, mouse, dt);

        render::draw_world(&self.world, &self.cam, &self.effects, Some(self.player));
        render::draw_hud(&self.world, &self.effects, self.player);
        if !self.paused && !over {
            render::draw_crosshair(&self.world, &self.cam, &self.effects, self.player, mouse);
        }
        if over || is_key_down(KeyCode::Tab) {
            render::draw_scoreboard(&self.world, Some(self.player));
        }

        if over {
            self.over_for += dt;
            return self.end_buttons();
        }
        if self.paused {
            return self.pause_menu();
        }
        Outcome::Continue
    }

    fn collect_edges(&mut self) {
        if Self::pressed(&[KeyCode::W, KeyCode::Up, KeyCode::Space]) {
            self.edges.jump = true;
        }
        if is_key_pressed(KeyCode::R) {
            self.edges.reload = true;
        }
        for (i, k) in SLOT_KEYS.iter().enumerate() {
            if is_key_pressed(*k) {
                self.edges.select = Some(i);
            }
        }
        if is_key_pressed(KeyCode::Q) {
            self.edges.cycle = -1;
        }
        if is_key_pressed(KeyCode::E) {
            self.edges.cycle = 1;
        }
        let wheel = mouse_wheel().1;
        if wheel > 0.0 {
            self.edges.cycle = -1;
        } else if wheel < 0.0 {
            self.edges.cycle = 1;
        }
    }

    fn tick(&mut self, mouse: Vec2) {
        let left = Self::held(&[KeyCode::A, KeyCode::Left]);
        let right = Self::held(&[KeyCode::D, KeyCode::Right]);
        let edges = std::mem::take(&mut self.edges);
        let player = Input {
            move_x: right as i32 as f32 - left as i32 as f32,
            jump: edges.jump,
            jump_held: Self::held(&[KeyCode::W, KeyCode::Up, KeyCode::Space]),
            crouch: Self::held(&[KeyCode::S, KeyCode::Down]),
            aim: self.cam.screen_to_world(mouse),
            fire: is_mouse_button_down(MouseButton::Left),
            reload: edges.reload,
            select: edges.select,
            cycle: edges.cycle,
        };
        let mut inputs = vec![Input::default(); self.world.fighters.len()];
        inputs[self.player] = player;
        self.bots.think(&self.world, &mut inputs);
        self.world.step(&inputs);
        self.effects.consume(&self.world, Some(self.player));
    }

    /// Follow the player, or a living fighter while the player is dead.
    fn focus_point(&self) -> Vec2 {
        let me = &self.world.fighters[self.player];
        if me.alive || self.world.mode.respawn {
            return me.center();
        }
        self.world
            .fighters
            .iter()
            .filter(|f| f.alive)
            .min_by_key(|f| (!self.world.allies(self.player, f.id), f.id))
            .map_or(me.center(), |f| f.center())
    }

    fn pause_menu(&mut self) -> Outcome {
        let s = ui::scale();
        let sw = screen_width();
        let sh = screen_height();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
        ui::text_center("PAUSED", sw / 2.0, sh * 0.3, 56.0 * s, WHITE);
        let bw = 260.0 * s;
        let bh = 48.0 * s;
        let x = (sw - bw) / 2.0;
        if ui::button("Resume", Rect::new(x, sh * 0.4, bw, bh), false) {
            self.paused = false;
        }
        if ui::button("Restart", Rect::new(x, sh * 0.4 + 60.0 * s, bw, bh), false) {
            return Outcome::Rematch;
        }
        if ui::button(
            "Main Menu",
            Rect::new(x, sh * 0.4 + 120.0 * s, bw, bh),
            false,
        ) {
            return Outcome::Menu;
        }
        ui::text_center(
            "Move A/D  Jump W/Space (twice to double jump)  Crouch S  Fire mouse  Reload R  Weapons 1-0 / Q E / wheel  Scores Tab",
            sw / 2.0,
            sh - 30.0 * s,
            16.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.7),
        );
        Outcome::Continue
    }

    fn end_buttons(&mut self) -> Outcome {
        if self.over_for < 1.0 {
            return Outcome::Continue;
        }
        let s = ui::scale();
        let sw = screen_width();
        let sh = screen_height();
        let bw = 200.0 * s;
        let bh = 46.0 * s;
        let y = sh - 90.0 * s;
        if ui::button(
            "Rematch",
            Rect::new(sw / 2.0 - bw - 10.0 * s, y, bw, bh),
            false,
        ) || is_key_pressed(KeyCode::Enter)
        {
            return Outcome::Rematch;
        }
        if ui::button(
            "Main Menu",
            Rect::new(sw / 2.0 + 10.0 * s, y, bw, bh),
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            return Outcome::Menu;
        }
        Outcome::Continue
    }
}
