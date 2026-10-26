//! One running match: reads the keyboard and mouse, steps the sim at a fixed
//! 60 Hz, and draws it.

use super::ui::{self, Side, Ui};
use crate::ai::BotSet;
use crate::data::GameData;
use crate::render::{self, Effects, GameCamera, MapArt};
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
    drop: bool,
    reload: bool,
    select: Option<usize>,
    cycle: i32,
    ability: bool,
}

pub struct Session {
    world: World,
    bots: BotSet,
    player: usize,
    effects: Effects,
    cam: GameCamera,
    /// Terrain drawing data, prepared once for this map.
    art: MapArt,
    acc: f32,
    paused: bool,
    edges: Edges,
    over_for: f32,
    /// Blood effects on screen (a player setting).
    pub blood: bool,
    /// The button that leaves the match ("Main Menu", or "Back to Editor"
    /// when test-playing a map).
    pub exit_label: &'static str,
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
        let art = MapArt::new(&world.map);
        Ok(Self {
            world,
            bots,
            player: 0,
            effects: Effects::default(),
            cam,
            art,
            acc: 0.0,
            paused: false,
            edges: Edges::default(),
            over_for: 0.0,
            blood: true,
            exit_label: "Main Menu",
        })
    }

    /// Debug screenshots: set the player's health.
    pub fn debug_set_health(&mut self, hp: f32) {
        self.world.fighters[self.player].health = hp;
    }

    /// Debug screenshots: kill the player, to show the death screen.
    pub fn debug_kill_player(&mut self) {
        let p = self.player;
        self.world.countdown = 0.0;
        self.world.fighters[p].spawn_protect = 0.0;
        self.world.damage(p, None, None, 9999.0, false, false);
    }

    /// Debug screenshots: put the player's feet at `at` and look there.
    pub fn debug_place_player(&mut self, at: Vec2) {
        self.world.fighters[self.player].feet = at;
        self.cam.snap(self.world.fighters[self.player].center());
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

    pub fn frame(&mut self, dt: f32, ui: &mut Ui) -> Outcome {
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

        render::draw_world(
            &self.world,
            &self.cam,
            &self.effects,
            Some(self.player),
            &self.art,
        );
        render::draw_hud(&self.world, &self.effects, self.player, self.blood);
        if !self.paused && !over {
            render::draw_crosshair(&self.world, &self.cam, &self.effects, self.player, mouse);
        }
        let debug_tab = std::env::var_os("ARCANE_SHOT_TAB").is_some();
        if over || is_key_down(KeyCode::Tab) || debug_tab {
            render::draw_scoreboard(&self.world, Some(self.player));
        }

        if over {
            self.over_for += dt;
            return self.end_buttons(ui);
        }
        if self.paused {
            return self.pause_menu(ui);
        }
        Outcome::Continue
    }

    fn collect_edges(&mut self) {
        if Self::pressed(&[KeyCode::W, KeyCode::Up, KeyCode::Space]) {
            self.edges.jump = true;
        }
        if Self::pressed(&[KeyCode::S, KeyCode::Down]) {
            self.edges.drop = true;
        }
        if is_key_pressed(KeyCode::R) {
            self.edges.reload = true;
        }
        if is_key_pressed(KeyCode::F) {
            self.edges.ability = true;
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
            drop: edges.drop,
            aim: self.cam.screen_to_world(mouse),
            fire: is_mouse_button_down(MouseButton::Left),
            reload: edges.reload,
            select: edges.select,
            cycle: edges.cycle,
            ability: edges.ability,
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
        // Out of the match (or waiting for the next round): watch the others.
        if me.alive || (self.world.respawns() && !me.out) {
            return me.center();
        }
        self.world
            .fighters
            .iter()
            .filter(|f| f.alive)
            .min_by_key(|f| (!self.world.allies(self.player, f.id), f.id))
            .map_or(me.center(), |f| f.center())
    }

    fn pause_menu(&mut self, ui: &mut Ui) -> Outcome {
        let s = ui::scale();
        let sw = screen_width();
        let sh = screen_height();
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
        ui::text_center("PAUSED", sw / 2.0, sh * 0.26, 56.0 * s, WHITE);
        let bw = 280.0 * s;
        let bh = 42.0 * s;
        let x = 70.0 * s;
        let y0 = sh * 0.36;
        if ui.tab(
            "pause_resume",
            "Resume",
            Rect::new(x, y0, bw, bh),
            Side::Left,
            false,
        ) {
            self.paused = false;
        }
        if ui.tab(
            "pause_restart",
            "Restart",
            Rect::new(x, y0 + 54.0 * s, bw, bh),
            Side::Left,
            false,
        ) {
            return Outcome::Rematch;
        }
        if ui.tab(
            "pause_menu",
            self.exit_label,
            Rect::new(x, y0 + 108.0 * s, bw, bh),
            Side::Left,
            false,
        ) {
            return Outcome::Menu;
        }
        ui::text_center(
            "Move A/D  Jump W/Space (twice to double jump)  Crouch S  Fire mouse  Reload R  Shadowstep F  Weapons 1-0 / Q E / wheel  Scores Tab",
            sw / 2.0,
            sh - 30.0 * s,
            16.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.7),
        );
        Outcome::Continue
    }

    fn end_buttons(&mut self, ui: &mut Ui) -> Outcome {
        if self.over_for < 1.0 {
            return Outcome::Continue;
        }
        let s = ui::scale();
        let sw = screen_width();
        let sh = screen_height();
        let bw = 240.0 * s;
        let bh = 42.0 * s;
        let y = sh - 80.0 * s;
        if ui.tab(
            "end_menu",
            self.exit_label,
            Rect::new(50.0 * s, y, bw, bh),
            Side::Left,
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            return Outcome::Menu;
        }
        if ui.tab(
            "end_rematch",
            "Rematch",
            Rect::new(sw - 50.0 * s - bw, y, bw, bh),
            Side::Right,
            false,
        ) || is_key_pressed(KeyCode::Enter)
        {
            return Outcome::Rematch;
        }
        Outcome::Continue
    }
}
