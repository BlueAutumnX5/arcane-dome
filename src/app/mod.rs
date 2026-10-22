//! Window, menus and the real-time loop that feeds the fixed-step sim.

mod session;
mod ui;

use crate::data::{GameData, find_assets_dir};
use crate::sim::{Difficulty, MatchConfig};
use macroquad::prelude::*;
use session::Session;
use std::sync::Arc;

const SCORE_LIMITS: [u32; 10] = [5, 10, 15, 20, 25, 30, 40, 50, 75, 100];
const ROUND_COUNTS: [u32; 5] = [1, 3, 5, 7, 9];
const TIME_LIMITS: [f32; 8] = [60.0, 120.0, 180.0, 300.0, 480.0, 600.0, 900.0, 1200.0];

enum Screen {
    Menu { focus: usize },
    Custom { focus: usize },
    Game(Box<Session>),
    Error(String),
}

/// Choices on the custom match screen.
struct Custom {
    map: usize,
    mode: usize,
    score: usize,
    rounds: usize,
    time: usize,
    friendly: u32,
    enemy: u32,
    difficulty: usize,
}

struct App {
    data: Option<Arc<GameData>>,
    screen: Screen,
    custom: Custom,
    seed: u64,
    quit: bool,
}

pub async fn run() {
    let data = match find_assets_dir() {
        None => Err("Could not find the assets folder.\nRun from the project folder or set ARCANE_DOME_ASSETS.".to_owned()),
        Some(dir) => GameData::load(&dir).map(Arc::new).map_err(|e| e.to_string()),
    };
    let mut app = App {
        screen: match &data {
            Ok(_) => Screen::Menu { focus: 0 },
            Err(e) => Screen::Error(e.clone()),
        },
        data: data.ok(),
        custom: Custom {
            map: 0,
            mode: 0,
            score: 4,
            rounds: 2,
            time: 5,
            friendly: 2,
            enemy: 3,
            difficulty: 1,
        },
        seed: macroquad::miniquad::date::now() as u64,
        quit: false,
    };
    app.reset_mode_defaults();

    let shot = debug::Shot::from_env();
    if let Some(s) = &shot {
        app.apply_debug_scene(&s.scene);
    }
    let start = get_time();
    loop {
        app.frame();
        if let Some(s) = &shot
            && get_time() - start > s.after_s
        {
            get_screen_data().export_png(&s.path);
            break;
        }
        if app.quit {
            break;
        }
        next_frame().await;
    }
}

impl App {
    fn frame(&mut self) {
        let dt = get_frame_time().min(0.25);
        let mut next = None;
        match &mut self.screen {
            Screen::Error(msg) => {
                clear_background(Color::from_rgba(30, 20, 24, 255));
                let s = ui::scale();
                ui::text_left(
                    "Game data has problems:",
                    40.0 * s,
                    60.0 * s,
                    30.0 * s,
                    WHITE,
                );
                for (i, line) in msg.lines().take(25).enumerate() {
                    ui::text_left(
                        line,
                        40.0 * s,
                        (100.0 + i as f32 * 24.0) * s,
                        20.0 * s,
                        Color::new(1.0, 0.75, 0.7, 1.0),
                    );
                }
                if is_key_pressed(KeyCode::Escape) {
                    self.quit = true;
                }
            }
            Screen::Menu { focus } => {
                let choice = draw_main_menu(focus);
                match choice {
                    Some(0) => next = Some(self.quick_match()),
                    Some(1) => next = Some(Screen::Custom { focus: 0 }),
                    Some(2) => self.quit = true,
                    _ => {}
                }
            }
            Screen::Custom { focus } => {
                let mut f = *focus;
                match self.custom_screen(&mut f) {
                    Some(true) => next = Some(self.start_custom()),
                    Some(false) => next = Some(Screen::Menu { focus: 1 }),
                    None => next = Some(Screen::Custom { focus: f }),
                }
            }
            Screen::Game(session) => match session.frame(dt) {
                session::Outcome::Continue => {}
                session::Outcome::Rematch => {
                    let cfg = session.config().clone();
                    let cfg = MatchConfig {
                        seed: self.next_seed(),
                        ..cfg
                    };
                    next = Some(self.start(cfg));
                }
                session::Outcome::Menu => next = Some(Screen::Menu { focus: 0 }),
            },
        }
        if let Some(s) = next {
            self.screen = s;
        }
    }

    fn next_seed(&mut self) -> u64 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.seed
    }

    fn start(&self, cfg: MatchConfig) -> Screen {
        let data = self.data.clone().expect("data loaded");
        match Session::new(data, cfg) {
            Ok(s) => Screen::Game(Box::new(s)),
            Err(e) => Screen::Error(e),
        }
    }

    fn quick_match(&mut self) -> Screen {
        let data = self.data.clone().expect("data loaded");
        let map = data.maps[self.next_seed() as usize % data.maps.len()]
            .id
            .clone();
        let cfg = MatchConfig {
            map_id: map,
            mode_id: "deathmatch".into(),
            enemy_bots: 5,
            seed: self.next_seed(),
            ..Default::default()
        };
        self.start(cfg)
    }

    fn modes_for_map(&self) -> Vec<usize> {
        let data = self.data.as_ref().expect("data loaded");
        let map = &data.maps[self.custom.map];
        (0..data.modes.len())
            .filter(|&m| map.modes.contains(&data.modes[m].id))
            .collect()
    }

    /// Reset limits to the selected mode's defaults from modes.json.
    fn reset_mode_defaults(&mut self) {
        let Some(data) = &self.data else { return };
        let modes = self.modes_for_map();
        if modes.is_empty() {
            return;
        }
        self.custom.mode = self.custom.mode.min(modes.len() - 1);
        let mode = &data.modes[modes[self.custom.mode]];
        let nearest = |list: &[f32], v: f32| {
            (0..list.len())
                .min_by(|&a, &b| (list[a] - v).abs().total_cmp(&(list[b] - v).abs()))
                .unwrap_or(0)
        };
        if let Some(s) = mode.score_limit {
            let l: Vec<f32> = SCORE_LIMITS.iter().map(|&v| v as f32).collect();
            self.custom.score = nearest(&l, s as f32);
        }
        if let Some(r) = mode.rounds {
            let l: Vec<f32> = ROUND_COUNTS.iter().map(|&v| v as f32).collect();
            self.custom.rounds = nearest(&l, r as f32);
        }
        self.custom.time = nearest(&TIME_LIMITS, mode.time_limit_s);
    }

    fn custom_config(&mut self) -> MatchConfig {
        let data = self.data.clone().expect("data loaded");
        let modes = self.modes_for_map();
        let mode = &data.modes[modes[self.custom.mode]];
        let c = &self.custom;
        MatchConfig {
            map_id: data.maps[c.map].id.clone(),
            mode_id: mode.id.clone(),
            score_limit: Some(SCORE_LIMITS[c.score]),
            rounds: Some(ROUND_COUNTS[c.rounds]),
            time_limit_s: Some(TIME_LIMITS[c.time]),
            friendly_bots: c.friendly,
            enemy_bots: c.enemy,
            difficulty: Difficulty::ALL[c.difficulty],
            seed: self.next_seed(),
            ..Default::default()
        }
    }

    fn start_custom(&mut self) -> Screen {
        let cfg = self.custom_config();
        self.start(cfg)
    }

    /// Returns Some(true) to start, Some(false) to go back.
    fn custom_screen(&mut self, focus: &mut usize) -> Option<bool> {
        let data = self.data.clone().expect("data loaded");
        clear_background(Color::from_rgba(20, 24, 34, 255));
        let s = ui::scale();
        let sw = screen_width();
        ui::text_center("CUSTOM MATCH", sw / 2.0, 70.0 * s, 44.0 * s, WHITE);

        let modes = self.modes_for_map();
        let mode = &data.modes[modes[self.custom.mode.min(modes.len() - 1)]];
        let team = mode.is_team();
        let c = &self.custom;
        let rows: [(&str, String, bool); 7] = [
            ("Map", data.maps[c.map].name.clone(), true),
            ("Mode", mode.display_name(), true),
            if mode.respawn {
                ("Score limit", SCORE_LIMITS[c.score].to_string(), true)
            } else {
                ("Rounds", ROUND_COUNTS[c.rounds].to_string(), true)
            },
            (
                if mode.respawn {
                    "Time limit"
                } else {
                    "Round time"
                },
                format!(
                    "{}:{:02}",
                    TIME_LIMITS[c.time] as u32 / 60,
                    TIME_LIMITS[c.time] as u32 % 60
                ),
                true,
            ),
            (
                "Friendly bots",
                if team {
                    c.friendly.to_string()
                } else {
                    "-".into()
                },
                team,
            ),
            (
                if team { "Enemy bots" } else { "Bots" },
                c.enemy.to_string(),
                true,
            ),
            (
                "Bot difficulty",
                Difficulty::ALL[c.difficulty].label().into(),
                true,
            ),
        ];
        let n_rows = rows.len();
        let w = 620.0 * s;
        let h = 44.0 * s;
        let x = (sw - w) / 2.0;
        let mut y = 110.0 * s;
        let mut deltas = [0i32; 7];
        for (i, (label, value, enabled)) in rows.iter().enumerate() {
            let d = ui::stepper(label, value, Rect::new(x, y, w, h), *focus == i, *enabled);
            if d != 0 {
                *focus = i;
                deltas[i] = d;
            }
            y += h + 6.0 * s;
        }

        // Keyboard navigation.
        let total = n_rows + 2;
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            *focus = (*focus + 1) % total;
        }
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            *focus = (*focus + total - 1) % total;
        }
        if *focus < n_rows {
            if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                deltas[*focus] = -1;
            }
            if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                deltas[*focus] = 1;
            }
        }

        let step = |v: usize, d: i32, n: usize| (v as i32 + d).rem_euclid(n as i32) as usize;
        let c = &mut self.custom;
        if deltas[0] != 0 {
            c.map = step(c.map, deltas[0], data.maps.len());
        }
        if deltas[1] != 0 {
            c.mode = step(c.mode, deltas[1], modes.len());
        }
        if deltas[2] != 0 {
            if mode.respawn {
                c.score = step(c.score, deltas[2], SCORE_LIMITS.len());
            } else {
                c.rounds = step(c.rounds, deltas[2], ROUND_COUNTS.len());
            }
        }
        if deltas[3] != 0 {
            c.time = step(c.time, deltas[3], TIME_LIMITS.len());
        }
        if deltas[4] != 0 {
            c.friendly = (c.friendly as i32 + deltas[4]).clamp(0, 7) as u32;
        }
        if deltas[5] != 0 {
            c.enemy = (c.enemy as i32 + deltas[5]).clamp(1, 11) as u32;
        }
        if deltas[6] != 0 {
            c.difficulty = step(c.difficulty, deltas[6], Difficulty::ALL.len());
        }
        if deltas[0] != 0 || deltas[1] != 0 {
            self.reset_mode_defaults();
        }

        y += 16.0 * s;
        let bw = 200.0 * s;
        let start = Rect::new(sw / 2.0 - bw - 10.0 * s, y, bw, 50.0 * s);
        let back = Rect::new(sw / 2.0 + 10.0 * s, y, bw, 50.0 * s);
        let go = ui::button("Start", start, *focus == n_rows);
        let ret = ui::button("Back", back, *focus == n_rows + 1);
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if go || (enter && *focus != n_rows + 1) {
            return Some(true);
        }
        if ret || is_key_pressed(KeyCode::Escape) || (enter && *focus == n_rows + 1) {
            return Some(false);
        }
        None
    }

    fn apply_debug_scene(&mut self, scene: &str) {
        if self.data.is_none() {
            return;
        }
        match scene {
            "custom" => self.screen = Screen::Custom { focus: 0 },
            s if s.starts_with("game") => {
                let mut cfg = self.custom_config();
                cfg.seed = 3;
                if let Some(map) = s.strip_prefix("game:") {
                    cfg.map_id = map.to_owned();
                }
                self.screen = self.start(cfg);
            }
            _ => {}
        }
    }
}

fn draw_main_menu(focus: &mut usize) -> Option<usize> {
    clear_background(Color::from_rgba(20, 24, 34, 255));
    let s = ui::scale();
    let sw = screen_width();
    let sh = screen_height();
    // A simple dome motif.
    let c = vec2(sw / 2.0, sh * 0.42);
    for k in 0..4 {
        let r = (150.0 + k as f32 * 26.0) * s;
        draw_circle_lines(
            c.x,
            c.y,
            r,
            2.0,
            Color::new(0.33, 0.63, 1.0, 0.25 - k as f32 * 0.05),
        );
    }
    draw_rectangle(0.0, c.y, sw, sh - c.y, Color::from_rgba(20, 24, 34, 255));
    draw_rectangle(
        sw / 2.0 - 240.0 * s,
        c.y,
        480.0 * s,
        3.0 * s,
        Color::new(0.33, 0.63, 1.0, 0.6),
    );
    ui::text_center("ARCANE DOME", sw / 2.0, sh * 0.3, 84.0 * s, WHITE);
    ui::text_center(
        "platform arena shooter",
        sw / 2.0,
        sh * 0.3 + 40.0 * s,
        24.0 * s,
        Color::new(1.0, 1.0, 1.0, 0.6),
    );

    let labels = ["Quick Match", "Custom Match", "Quit"];
    let bw = 280.0 * s;
    let bh = 50.0 * s;
    let mut choice = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::new(
            (sw - bw) / 2.0,
            sh * 0.5 + i as f32 * (bh + 12.0 * s),
            bw,
            bh,
        );
        if ui::button(l, r, *focus == i) {
            choice = Some(i);
        }
    }
    if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
        *focus = (*focus + 1) % labels.len();
    }
    if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
        *focus = (*focus + labels.len() - 1) % labels.len();
    }
    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
        choice = Some(*focus);
    }
    ui::text_center(
        "Fan project inspired by the Raze series by JuiceTin and AddisonR. Not affiliated.",
        sw / 2.0,
        sh - 20.0 * s,
        16.0 * s,
        Color::new(1.0, 1.0, 1.0, 0.45),
    );
    choice
}

/// Optional screenshot hook for checking visuals without a person at the keyboard:
/// `ARCANE_SHOT=out.png ARCANE_SHOT_SCENE=menu|custom|game[:map_id] ARCANE_SHOT_AFTER=2`.
mod debug {
    pub struct Shot {
        pub path: String,
        pub scene: String,
        pub after_s: f64,
    }

    impl Shot {
        pub fn from_env() -> Option<Self> {
            let path = std::env::var("ARCANE_SHOT").ok()?;
            Some(Self {
                path,
                scene: std::env::var("ARCANE_SHOT_SCENE").unwrap_or_else(|_| "menu".into()),
                after_s: std::env::var("ARCANE_SHOT_AFTER")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1.5),
            })
        }
    }
}
