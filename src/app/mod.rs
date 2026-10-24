//! Window, menus and the real-time loop that feeds the fixed-step sim.

mod session;
mod ui;

use crate::data::{GameData, ProjectileKind, find_assets_dir};
use crate::render::TEAM_COLORS;
use crate::sim::{Difficulty, MAX_TEAMS, MatchConfig, TEAM_NAMES};
use macroquad::prelude::*;
use session::Session;
use std::sync::Arc;
use ui::{ACCENT, Side, Ui};

const SCORE_LIMITS: [u32; 10] = [5, 10, 15, 20, 25, 30, 40, 50, 75, 100];
const ROUND_COUNTS: [u32; 5] = [1, 3, 5, 7, 9];
/// Most bots one team can have.
const MAX_TEAM_BOTS: i32 = 7;
const TIME_LIMITS: [f32; 8] = [60.0, 120.0, 180.0, 300.0, 480.0, 600.0, 900.0, 1200.0];
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Copy, PartialEq, Eq)]
enum Info {
    HowTo,
    Weapons,
    About,
}

enum Screen {
    Menu,
    Setup,
    Info(Info),
    Game(Box<Session>),
    Error(String),
}

/// Team settings offered for this map in this mode: 0 (teams off, every
/// mode has it), then the team counts both the map and the mode support.
fn allowed_team_counts(map: &crate::data::MapDef, mode: &crate::data::ModeDef) -> Vec<usize> {
    let most = (mode.teams as usize).min(MAX_TEAMS);
    std::iter::once(0)
        .chain(
            map.team_counts
                .iter()
                .map(|&n| n as usize)
                .filter(|&n| n >= 2 && n <= most),
        )
        .collect()
}

fn teams_label(n: usize) -> String {
    if n >= 2 { n.to_string() } else { "Off".into() }
}

/// Choices on the match setup screen.
struct Setup {
    map: usize,
    mode: usize,
    score: usize,
    rounds: usize,
    time: usize,
    /// Free-for-all bots and their skill.
    ffa_bots: u32,
    ffa_skill: usize,
    /// Team modes: how many teams, and bots and skill per team.
    teams: usize,
    team_bots: [u32; MAX_TEAMS],
    team_skill: [usize; MAX_TEAMS],
    /// Each team's colour (palette index); always all different.
    team_colors: [u8; MAX_TEAMS],
    powerups: bool,
    stations: bool,
    infinite_ammo: bool,
}

struct App {
    data: Option<Arc<GameData>>,
    screen: Screen,
    setup: Setup,
    ui: Ui,
    seed: u64,
    time: f32,
    quit: bool,
    /// Rendered map pictures for the setup screen, by map id.
    thumbs: std::collections::HashMap<String, crate::render::MapThumb>,
}

pub async fn run() {
    let data = match find_assets_dir() {
        None => Err(
            "Could not find the assets folder.\nRun from the project folder or set ARCANE_DOME_ASSETS."
                .to_owned(),
        ),
        Some(dir) => GameData::load(&dir).map(Arc::new).map_err(|e| e.to_string()),
    };
    let mut app = App {
        screen: match &data {
            Ok(_) => Screen::Menu,
            Err(e) => Screen::Error(e.clone()),
        },
        data: data.ok(),
        setup: Setup {
            map: 0,
            mode: 0,
            score: 4,
            rounds: 2,
            time: 5,
            ffa_bots: 3,
            ffa_skill: 1,
            teams: 0,
            team_bots: [2, 3, 3, 3],
            team_skill: [1; MAX_TEAMS],
            team_colors: [0, 1, 2, 3],
            powerups: true,
            stations: true,
            infinite_ammo: false,
        },
        ui: Ui::default(),
        seed: macroquad::miniquad::date::now() as u64,
        time: 0.0,
        quit: false,
        thumbs: Default::default(),
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

fn on_off(b: bool) -> String {
    if b { "On".into() } else { "Off".into() }
}

impl App {
    fn frame(&mut self) {
        let dt = get_frame_time().min(0.25);
        self.time += dt;
        self.ui.begin(dt);
        let next = match &mut self.screen {
            Screen::Error(msg) => {
                let msg = msg.clone();
                self.error_screen(&msg);
                None
            }
            Screen::Menu => self.main_menu(),
            Screen::Setup => self.setup_screen(),
            Screen::Info(page) => {
                let page = *page;
                self.info_screen(page)
            }
            Screen::Game(session) => match session.frame(dt, &mut self.ui) {
                session::Outcome::Continue => None,
                session::Outcome::Rematch => {
                    let cfg = session.config().clone();
                    let cfg = MatchConfig {
                        seed: self.next_seed(),
                        ..cfg
                    };
                    Some(self.start(cfg))
                }
                session::Outcome::Menu => Some(Screen::Menu),
            },
        };
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

    fn data(&self) -> Arc<GameData> {
        self.data.clone().expect("data loaded")
    }

    fn start(&self, cfg: MatchConfig) -> Screen {
        match Session::new(self.data(), cfg) {
            Ok(s) => Screen::Game(Box::new(s)),
            Err(e) => Screen::Error(e),
        }
    }

    fn quick_match(&mut self) -> Screen {
        let data = self.data();
        let map = data.maps[self.next_seed() as usize % data.maps.len()]
            .id
            .clone();
        let cfg = MatchConfig {
            map_id: map,
            mode_id: "deathmatch".into(),
            bots: 5,
            seed: self.next_seed(),
            ..Default::default()
        };
        self.start(cfg)
    }

    fn error_screen(&mut self, msg: &str) {
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

    fn main_menu(&mut self) -> Option<Screen> {
        ui::backdrop(self.time);
        let s = ui::scale();
        let (sw, sh) = (screen_width(), screen_height());

        // Title with a dome arc behind it.
        let c = vec2(sw / 2.0, sh * 0.27);
        for k in 0..3 {
            let r = (120.0 + k as f32 * 22.0) * s;
            draw_circle_lines(
                c.x,
                c.y,
                r,
                2.0,
                Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.22 - k as f32 * 0.06),
            );
        }
        ui::text_center("ARCANE DOME", sw / 2.0, sh * 0.25, 76.0 * s, WHITE);
        ui::text_center(
            "platform arena shooter",
            sw / 2.0,
            sh * 0.25 + 34.0 * s,
            22.0 * s,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.8),
        );

        let bw = 300.0 * s;
        let bh = 40.0 * s;
        let gap = 54.0 * s;
        let y0 = sh * 0.42;
        let lx = 60.0 * s;
        let rx = sw - 60.0 * s - bw;
        let mut next = None;
        if self.ui.tab(
            "m_quick",
            "Quick Match",
            Rect::new(lx, y0, bw, bh),
            Side::Left,
            false,
        ) {
            next = Some(self.quick_match());
        }
        if self.ui.tab(
            "m_setup",
            "Match Setup",
            Rect::new(lx, y0 + gap, bw, bh),
            Side::Left,
            false,
        ) {
            next = Some(Screen::Setup);
        }
        if self.ui.tab(
            "m_quit",
            "Quit",
            Rect::new(lx, y0 + gap * 2.0, bw, bh),
            Side::Left,
            false,
        ) {
            self.quit = true;
        }
        if self.ui.tab(
            "m_howto",
            "How to Play",
            Rect::new(rx, y0, bw, bh),
            Side::Right,
            false,
        ) {
            next = Some(Screen::Info(Info::HowTo));
        }
        if self.ui.tab(
            "m_weapons",
            "Weapon Info",
            Rect::new(rx, y0 + gap, bw, bh),
            Side::Right,
            false,
        ) {
            next = Some(Screen::Info(Info::Weapons));
        }
        if self.ui.tab(
            "m_about",
            "About",
            Rect::new(rx, y0 + gap * 2.0, bw, bh),
            Side::Right,
            false,
        ) {
            next = Some(Screen::Info(Info::About));
        }
        if is_key_pressed(KeyCode::Enter) {
            next = Some(self.quick_match());
        }

        ui::text_right(
            VERSION,
            sw - 30.0 * s,
            sh - 30.0 * s,
            16.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.5),
        );
        ui::text_center(
            "Fan project inspired by the Raze series by JuiceTin and AddisonR. Not affiliated.",
            sw / 2.0,
            sh - 30.0 * s,
            16.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.45),
        );
        next
    }

    fn modes_for_map(&self) -> Vec<usize> {
        let data = self.data();
        let map = &data.maps[self.setup.map];
        (0..data.modes.len())
            .filter(|&m| map.modes.contains(&data.modes[m].id))
            .collect()
    }

    /// Reset limits to the selected mode's defaults from modes.json.
    fn reset_mode_defaults(&mut self) {
        let Some(data) = self.data.clone() else {
            return;
        };
        let modes = self.modes_for_map();
        if modes.is_empty() {
            return;
        }
        self.setup.mode = self.setup.mode.min(modes.len() - 1);
        let mode = &data.modes[modes[self.setup.mode]];
        let nearest = |list: &[f32], v: f32| {
            (0..list.len())
                .min_by(|&a, &b| (list[a] - v).abs().total_cmp(&(list[b] - v).abs()))
                .unwrap_or(0)
        };
        let score = if self.setup.teams >= 2 {
            mode.team_score_limit.or(mode.score_limit)
        } else {
            mode.score_limit
        };
        if let Some(s) = score {
            let l: Vec<f32> = SCORE_LIMITS.iter().map(|&v| v as f32).collect();
            self.setup.score = nearest(&l, s as f32);
        }
        if let Some(r) = mode.rounds {
            let l: Vec<f32> = ROUND_COUNTS.iter().map(|&v| v as f32).collect();
            self.setup.rounds = nearest(&l, r as f32);
        }
        self.setup.time = nearest(&TIME_LIMITS, mode.time_limit_s);
    }

    fn setup_config(&mut self) -> MatchConfig {
        let data = self.data();
        let modes = self.modes_for_map();
        let mode = &data.modes[modes[self.setup.mode]];
        let c = &self.setup;
        let cfg = MatchConfig {
            map_id: data.maps[c.map].id.clone(),
            mode_id: mode.id.clone(),
            score_limit: Some(SCORE_LIMITS[c.score]),
            rounds: Some(ROUND_COUNTS[c.rounds]),
            time_limit_s: Some(TIME_LIMITS[c.time]),
            bots: c.ffa_bots,
            difficulty: Difficulty::ALL[c.ffa_skill],
            teams: c.teams as u8,
            team_bots: c.team_bots,
            team_difficulty: c.team_skill.map(|k| Difficulty::ALL[k]),
            team_colors: c.team_colors,
            powerups: c.powerups,
            stations: c.stations,
            infinite_ammo: c.infinite_ammo,
            ..Default::default()
        };
        MatchConfig {
            seed: self.next_seed(),
            ..cfg
        }
    }

    /// Match setup: map and rules on the left, teams on the right.
    fn setup_screen(&mut self) -> Option<Screen> {
        ui::backdrop(self.time);
        let data = self.data();
        let s = ui::scale();
        let (sw, sh) = (screen_width(), screen_height());
        ui::text_center("MATCH SETUP", sw / 2.0, 58.0 * s, 40.0 * s, WHITE);

        let modes = self.modes_for_map();
        let mode = data.modes[modes[self.setup.mode.min(modes.len() - 1)]].clone();
        let map = &data.maps[self.setup.map];
        let step = |v: usize, d: i32, n: usize| (v as i32 + d).rem_euclid(n as i32) as usize;

        // Left: map preview with prev / next, description, then rules.
        let lx = 40.0 * s;
        let lw = sw * 0.55 - lx;
        let top = 80.0 * s;
        ui::panel(Rect::new(lx, top, lw, sh - top - 100.0 * s), None);
        let pad = 12.0 * s;
        let pv = Rect::new(lx + pad, top + pad, lw - 2.0 * pad, 170.0 * s);
        let thumb = self
            .thumbs
            .entry(map.id.clone())
            .or_insert_with(|| crate::render::MapThumb::new(map, 768));
        ui::map_preview(map, thumb.draw(map), pv);
        let ny = pv.y + pv.h + 26.0 * s;
        let mut map_delta = self.ui.stepper(
            "s_map",
            "Map",
            &map.name,
            Rect::new(pv.x, ny - 20.0 * s, pv.w, 28.0 * s),
            false,
            data.maps.len() > 1,
        );
        if is_key_pressed(KeyCode::Left) {
            map_delta = -1;
        }
        if is_key_pressed(KeyCode::Right) {
            map_delta = 1;
        }
        let dsize = 16.0 * s;
        let mut y = ny + 22.0 * s;
        for line in ui::wrap(&map.description, pv.w, dsize).iter().take(2) {
            ui::text_left(line, pv.x, y, dsize, Color::new(1.0, 1.0, 1.0, 0.7));
            y += dsize + 4.0 * s;
        }
        y += 6.0 * s;

        let rh = 30.0 * s;
        let row = |y: f32| Rect::new(pv.x, y, pv.w, rh);
        let d_mode = self.ui.stepper(
            "s_mode",
            "Match Type",
            &mode.display_name(),
            row(y),
            false,
            true,
        );
        y += rh + 2.0 * s;
        for line in ui::wrap(&mode.description, pv.w, dsize).iter().take(2) {
            y += dsize;
            ui::text_left(
                line,
                pv.x + 10.0 * s,
                y,
                dsize,
                Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.8),
            );
        }
        y += 10.0 * s;
        let (limit_label, limit_value) = if mode.respawn {
            ("Score Limit", SCORE_LIMITS[self.setup.score].to_string())
        } else {
            ("Rounds", ROUND_COUNTS[self.setup.rounds].to_string())
        };
        let d_limit = self
            .ui
            .stepper("s_limit", limit_label, &limit_value, row(y), false, true);
        y += rh + 4.0 * s;
        let t = TIME_LIMITS[self.setup.time] as u32;
        let tl = if mode.respawn {
            "Time Limit"
        } else {
            "Round Time"
        };
        let d_time = self.ui.stepper(
            "s_time",
            tl,
            &format!("{}:{:02}", t / 60, t % 60),
            row(y),
            false,
            true,
        );
        y += rh + 4.0 * s;
        let d_pow = self.ui.stepper(
            "s_pow",
            "Powerups",
            &on_off(self.setup.powerups),
            row(y),
            false,
            true,
        );
        y += rh + 4.0 * s;
        let d_sta = self.ui.stepper(
            "s_sta",
            "Stations",
            &on_off(self.setup.stations),
            row(y),
            false,
            true,
        );
        y += rh + 4.0 * s;
        let d_inf = self.ui.stepper(
            "s_inf",
            "Infinite Clips",
            &on_off(self.setup.infinite_ammo),
            row(y),
            false,
            true,
        );

        // Right: teams (or the opponents in free-for-all).
        let rx = sw * 0.55 + 20.0 * s;
        let rw = sw - rx - 40.0 * s;
        let half = (rw - 30.0 * s) / 2.0;
        let mut d_bots = [0i32; MAX_TEAMS];
        let mut d_skill = [0i32; MAX_TEAMS];
        let mut recolor = None;
        let (mut d_ffa_bots, mut d_ffa_skill) = (0, 0);
        let total;
        let choices = allowed_team_counts(map, &mode);
        let d_teams = self.ui.stepper(
            "s_teams",
            "Teams",
            &teams_label(self.setup.teams),
            Rect::new(rx, top, rw, 34.0 * s),
            false,
            choices.len() > 1,
        );
        if self.setup.teams >= 2 {
            let box_h = 86.0 * s;
            for t in 0..self.setup.teams {
                let y = top + 48.0 * s + t as f32 * (box_h + 10.0 * s);
                let pick = self.setup.team_colors[t] as usize;
                let color = TEAM_COLORS[pick];
                let bots = self.setup.team_bots[t];
                ui::panel(Rect::new(rx, y, rw, box_h), None);
                draw_rectangle(rx, y, 6.0 * s, box_h, color);
                // Click the swatch for the next colour.
                let sw_r = Rect::new(rx + 18.0 * s, y + 10.0 * s, 20.0 * s, 20.0 * s);
                if self.ui.swatch(&format!("s_t{t}_color"), sw_r, color) {
                    recolor = Some(t);
                }
                let title = format!("{} Team", TEAM_NAMES[pick]);
                ui::text_left(&title, rx + 48.0 * s, y + 26.0 * s, 22.0 * s, color);
                let who = if t == 0 {
                    format!("You + {bots} bots")
                } else {
                    format!("{bots} bots")
                };
                ui::text_right(
                    &who,
                    rx + rw - 14.0 * s,
                    y + 26.0 * s,
                    17.0 * s,
                    Color::new(1.0, 1.0, 1.0, 0.75),
                );
                let ry = y + 42.0 * s;
                d_bots[t] = self.ui.stepper(
                    &format!("s_t{t}_bots"),
                    "Bots",
                    &bots.to_string(),
                    Rect::new(rx + 10.0 * s, ry, half, 32.0 * s),
                    false,
                    true,
                );
                d_skill[t] = self.ui.stepper(
                    &format!("s_t{t}_skill"),
                    "Skill",
                    Difficulty::ALL[self.setup.team_skill[t]].label(),
                    Rect::new(rx + 20.0 * s + half, ry, half, 32.0 * s),
                    false,
                    true,
                );
            }
            total = 1 + self.setup.team_bots[..self.setup.teams].iter().sum::<u32>();
        } else {
            let box_h = 140.0 * s;
            let y = top + 48.0 * s;
            let color = TEAM_COLORS[1];
            ui::panel(Rect::new(rx, y, rw, box_h), None);
            draw_rectangle(rx, y, 6.0 * s, box_h, color);
            ui::text_left("Opponents", rx + 18.0 * s, y + 26.0 * s, 22.0 * s, color);
            let who = format!("You against {} bots", self.setup.ffa_bots);
            ui::text_left(
                &who,
                rx + 18.0 * s,
                y + 50.0 * s,
                17.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.75),
            );
            let r1 = Rect::new(rx + 10.0 * s, y + 62.0 * s, rw - 20.0 * s, 32.0 * s);
            let r2 = Rect::new(rx + 10.0 * s, y + 100.0 * s, rw - 20.0 * s, 32.0 * s);
            d_ffa_bots = self.ui.stepper(
                "s_ffa_bots",
                "Bots",
                &self.setup.ffa_bots.to_string(),
                r1,
                false,
                true,
            );
            d_ffa_skill = self.ui.stepper(
                "s_ffa_skill",
                "Difficulty",
                Difficulty::ALL[self.setup.ffa_skill].label(),
                r2,
                false,
                true,
            );
            total = 1 + self.setup.ffa_bots;
        }
        if let Some([lo, hi]) = map.recommended_players {
            let note = format!("{total} players  (map suits {lo}-{hi})");
            let warn = total < lo || total > hi;
            let col = if warn {
                Color::new(1.0, 0.6, 0.4, 0.9)
            } else {
                Color::new(1.0, 1.0, 1.0, 0.6)
            };
            ui::text_left(&note, rx, sh - 120.0 * s, 17.0 * s, col);
        }

        // Apply changes.
        let c = &mut self.setup;
        let mut reset = false;
        if map_delta != 0 {
            c.map = step(c.map, map_delta, data.maps.len());
            reset = true;
        }
        if d_mode != 0 {
            c.mode = step(c.mode, d_mode, modes.len());
            reset = true;
        }
        if d_limit != 0 {
            if mode.respawn {
                c.score = step(c.score, d_limit, SCORE_LIMITS.len());
            } else {
                c.rounds = step(c.rounds, d_limit, ROUND_COUNTS.len());
            }
        }
        if d_time != 0 {
            c.time = step(c.time, d_time, TIME_LIMITS.len());
        }
        if d_pow != 0 {
            c.powerups = !c.powerups;
        }
        if d_sta != 0 {
            c.stations = !c.stations;
        }
        if d_inf != 0 {
            c.infinite_ammo = !c.infinite_ammo;
        }
        // Step through the team counts this map and mode allow, and keep
        // the choice on that list when the map or mode changes.
        let allowed = allowed_team_counts(map, &mode);
        let at = allowed
            .iter()
            .position(|&n| n >= c.teams)
            .unwrap_or(allowed.len() - 1);
        let at = (at as i32 + d_teams).clamp(0, allowed.len() as i32 - 1) as usize;
        let was_team = c.teams >= 2;
        c.teams = allowed[at];
        let score_default_changed = was_team != (c.teams >= 2);
        if let Some(t) = recolor {
            // Next colour; the team that had it takes this team's old one.
            let old = c.team_colors[t];
            let new = (old + 1) % MAX_TEAMS as u8;
            if let Some(other) = c.team_colors.iter().position(|&k| k == new) {
                c.team_colors[other] = old;
            }
            c.team_colors[t] = new;
        }
        for t in 0..MAX_TEAMS {
            // Your team may be just you; every other team needs a bot.
            let min = if t == 0 { 0 } else { 1 };
            c.team_bots[t] = (c.team_bots[t] as i32 + d_bots[t]).clamp(min, MAX_TEAM_BOTS) as u32;
            c.team_skill[t] = step(c.team_skill[t], d_skill[t], Difficulty::ALL.len());
        }
        c.ffa_bots = (c.ffa_bots as i32 + d_ffa_bots).clamp(1, 11) as u32;
        c.ffa_skill = step(c.ffa_skill, d_ffa_skill, Difficulty::ALL.len());
        if reset {
            self.reset_mode_defaults();
        } else if score_default_changed {
            // Teams on or off: the usual score limit differs (say 50 vs 25).
            let mode = &data.modes[modes[self.setup.mode.min(modes.len() - 1)]];
            let score = if self.setup.teams >= 2 {
                mode.team_score_limit.or(mode.score_limit)
            } else {
                mode.score_limit
            };
            if let Some(v) = score {
                self.setup.score = SCORE_LIMITS
                    .iter()
                    .position(|&l| l >= v)
                    .unwrap_or(SCORE_LIMITS.len() - 1);
            }
        }

        // Back (slides left) and Start (slides right).
        let bw = 240.0 * s;
        let bh = 42.0 * s;
        let by = sh - 76.0 * s;
        if self.ui.tab(
            "s_back",
            "Back",
            Rect::new(50.0 * s, by, bw, bh),
            Side::Left,
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            return Some(Screen::Menu);
        }
        if self.ui.tab(
            "s_start",
            "Start",
            Rect::new(sw - 50.0 * s - bw, by, bw, bh),
            Side::Right,
            false,
        ) || is_key_pressed(KeyCode::Enter)
        {
            let cfg = self.setup_config();
            return Some(self.start(cfg));
        }
        None
    }

    fn info_screen(&mut self, page: Info) -> Option<Screen> {
        ui::backdrop(self.time);
        let data = self.data();
        let s = ui::scale();
        let (sw, sh) = (screen_width(), screen_height());
        let title = match page {
            Info::HowTo => "HOW TO PLAY",
            Info::Weapons => "WEAPON INFO",
            Info::About => "ABOUT",
        };
        ui::text_center(title, sw / 2.0, 58.0 * s, 40.0 * s, WHITE);
        let r = Rect::new(60.0 * s, 80.0 * s, sw - 120.0 * s, sh - 180.0 * s);
        ui::panel(r, None);
        let size = 19.0 * s;
        let x = r.x + 24.0 * s;
        let mut y = r.y + 36.0 * s;
        let mut line = |t: &str, c: Color| {
            ui::text_left(t, x, y, size, c);
            y += size + 8.0 * s;
        };
        let soft = Color::new(1.0, 1.0, 1.0, 0.8);
        match page {
            Info::HowTo => {
                for t in [
                    "A / D  move        W or Space  jump (press again in the air to double jump)",
                    "S  crouch          S + jump  drop through a thin platform",
                    "Mouse  aim         Left click  fire        R  reload",
                    "1 - 0, Q / E or mouse wheel  switch weapons",
                    "F  use Shadowstep (when you hold one)",
                    "Tab  scoreboard    P or Esc  pause",
                ] {
                    line(t, WHITE);
                }
                line("", WHITE);
                for t in [
                    "Thrusters launch you high into the air. Step on the glowing pad.",
                    "Weapon stations deploy a random weapon every 30-45 seconds.",
                    "Shield stations are rarer and give +50 or +100 shield. Shield soaks up part of each hit.",
                    "Powerup spots (0-4 per map) each hold one fixed powerup: first at 45 s, then 2 min after pickup:",
                ] {
                    line(t, soft);
                }
                for p in &data.powerups.powerups {
                    let c = Color::from_rgba(p.color[0], p.color[1], p.color[2], 255);
                    let what = if p.duration_s > 0.0 {
                        format!("   {}  ({:.0} s)", p.name, p.duration_s)
                    } else {
                        format!("   {}  (one teleport, press F)", p.name)
                    };
                    line(&what, c);
                }
            }
            Info::Weapons => {
                let cols = [0.0, 0.26, 0.44, 0.56, 0.68, 0.8];
                let head = ["Weapon", "Type", "Damage", "Rate/s", "Clip", "Range"];
                for (k, h) in head.iter().enumerate() {
                    ui::text_left(
                        h,
                        x + cols[k] * (r.w - 48.0 * s),
                        y,
                        size,
                        Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.9),
                    );
                }
                y += size + 10.0 * s;
                for (i, w) in data.weapons.iter().enumerate() {
                    let kind = match w.projectile.kind {
                        ProjectileKind::Hitscan => "Instant",
                        ProjectileKind::Bullet => "Bullet",
                        ProjectileKind::Grenade => "Grenade",
                        ProjectileKind::Rocket => "Rocket",
                        ProjectileKind::Beam => "Beam",
                        ProjectileKind::MeleeArc => "Melee",
                    };
                    let dmg = match &w.projectile.explosion {
                        Some(e) => format!("{:.0} + {:.0} blast", w.damage, e.damage),
                        None if w.total_pellets() > 1 => {
                            format!("{:.0} x{}", w.damage, w.total_pellets())
                        }
                        None => format!("{:.0}", w.damage),
                    };
                    let clip = if w.infinite_reserve() {
                        format!("{} / inf", w.magazine)
                    } else {
                        format!("{} / {}", w.magazine, w.reserve_ammo)
                    };
                    let cells = [
                        format!("{}  {}", if i == 9 { 0 } else { i + 1 }, w.name),
                        kind.to_owned(),
                        dmg,
                        format!("{:.1}", w.fire_rate_rps),
                        clip,
                        format!("{:.0}", w.range_tiles),
                    ];
                    for (k, cell) in cells.iter().enumerate() {
                        ui::text_left(
                            cell,
                            x + cols[k] * (r.w - 48.0 * s),
                            y,
                            size * 0.95,
                            if k == 0 { WHITE } else { soft },
                        );
                    }
                    y += size + 6.0 * s;
                }
            }
            Info::About => {
                for t in [
                    "Arcane Dome is a free, open-source fan project made with Rust and Macroquad.",
                    "",
                    "It is inspired by the Raze series by JuiceTin and AddisonR.",
                    "It is not affiliated with or endorsed by them, and it uses none of their art,",
                    "sounds, code, names or map layouts.",
                    "",
                    "Code: MIT License. See CREDITS.md for libraries and assets.",
                ] {
                    line(t, soft);
                }
            }
        }
        let bw = 240.0 * s;
        if self.ui.tab(
            "i_back",
            "Back",
            Rect::new(50.0 * s, sh - 76.0 * s, bw, 42.0 * s),
            Side::Left,
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            return Some(Screen::Menu);
        }
        None
    }

    fn apply_debug_scene(&mut self, scene: &str) {
        if self.data.is_none() {
            return;
        }
        // ARCANE_SHOT_MODE=elimination ARCANE_SHOT_TEAMS=4 preset the setup.
        if let Ok(id) = std::env::var("ARCANE_SHOT_MODE") {
            let modes = self.modes_for_map();
            let data = self.data();
            if let Some(k) = modes.iter().position(|&m| data.modes[m].id == id) {
                self.setup.mode = k;
                self.reset_mode_defaults();
            }
        }
        if let Some(n) = std::env::var("ARCANE_SHOT_TEAMS")
            .ok()
            .and_then(|n| n.parse().ok())
        {
            self.setup.teams = n;
        }
        if let Some(n) = std::env::var("ARCANE_SHOT_BOTS")
            .ok()
            .and_then(|n| n.parse().ok())
        {
            self.setup.team_bots = [n; MAX_TEAMS];
        }
        // ARCANE_SHOT_COLORS=3,0,2,1 picks team colours (palette indices).
        if let Ok(list) = std::env::var("ARCANE_SHOT_COLORS") {
            let picks = list.split(',').filter_map(|c| c.parse().ok());
            for (t, c) in picks.take(MAX_TEAMS).enumerate() {
                self.setup.team_colors[t] = c;
            }
        }
        match scene {
            "custom" | "setup" => self.screen = Screen::Setup,
            "howto" => self.screen = Screen::Info(Info::HowTo),
            "weapons" => self.screen = Screen::Info(Info::Weapons),
            "about" => self.screen = Screen::Info(Info::About),
            s if s.starts_with("game") => {
                let mut cfg = self.setup_config();
                cfg.seed = 3;
                // game:<map>@x,y also puts the player at x, y.
                let (s, at) = s.split_once('@').unwrap_or((s, ""));
                if let Some(map) = s.strip_prefix("game:") {
                    cfg.map_id = map.to_owned();
                }
                self.screen = self.start(cfg);
                let at = at
                    .split_once(',')
                    .and_then(|(x, y)| Some(vec2(x.parse().ok()?, y.parse().ok()?)));
                if let (Screen::Game(g), Some(at)) = (&mut self.screen, at) {
                    g.debug_place_player(at);
                }
                // ARCANE_SHOT_KILL=1 shows the death screen.
                if let (Screen::Game(g), Ok(_)) =
                    (&mut self.screen, std::env::var("ARCANE_SHOT_KILL"))
                {
                    g.debug_kill_player();
                }
            }
            _ => {}
        }
    }
}

/// Optional screenshot hook for checking visuals without a person at the keyboard:
/// `ARCANE_SHOT=out.png ARCANE_SHOT_SCENE=menu|setup|howto|weapons|about|game[:map_id[@x,y]] ARCANE_SHOT_AFTER=2`,
/// optionally `ARCANE_SHOT_MODE=<mode id> ARCANE_SHOT_TEAMS=<n> ARCANE_SHOT_BOTS=<per team> ARCANE_SHOT_COLORS=3,0,2,1
/// ARCANE_SHOT_TAB=1` (scoreboard).
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
