//! Window, menus and the real-time loop that feeds the fixed-step sim.

mod editor;
mod session;
mod settings;
mod touch;
mod ui;

use crate::data::{GameData, ProjectileKind, Rules, find_assets_dir};
use crate::i18n::{t, tf, tn};
use crate::render::{TEAM_COLORS, resync_texture_bindings, use_screen};
use crate::sim::{Difficulty, ElimStyle, MAX_TEAMS, MatchConfig, TEAM_NAMES};
use editor::Editor;
use macroquad::prelude::*;
use session::Session;
use settings::Settings;
use std::sync::Arc;
use ui::{ACCENT, Side, Ui};

// Limit choices; 0 means Off.
const SCORE_LIMITS: [u32; 17] = [
    0, 1, 2, 3, 5, 10, 15, 20, 25, 30, 40, 50, 75, 100, 150, 200, 300,
];
/// King of the Hill: seconds between hill moves; 0 = it stays put.
const HILL_MOVES: [f32; 5] = [0.0, 30.0, 60.0, 90.0, 120.0];
const ROUND_COUNTS: [u32; 5] = [1, 3, 5, 7, 9];
/// Most bots one team can have.
const MAX_TEAM_BOTS: i32 = 7;
const TIME_LIMITS: [f32; 9] = [0.0, 60.0, 120.0, 180.0, 300.0, 480.0, 600.0, 900.0, 1200.0];
const LIVES: [u32; 6] = [0, 1, 2, 3, 5, 10];
const TEAM_LIVES: [u32; 7] = [0, 5, 10, 15, 20, 30, 50];
const STYLES: [ElimStyle; 3] = [
    ElimStyle::Rounds,
    ElimStyle::LastManStanding,
    ElimStyle::Attrition,
];

fn style_name(st: ElimStyle) -> &'static str {
    match st {
        ElimStyle::Rounds => t("Rounds"),
        ElimStyle::LastManStanding => t("Last Man Standing"),
        ElimStyle::Attrition => t("Attrition"),
    }
}

fn style_blurb(st: ElimStyle) -> &'static str {
    match st {
        ElimStyle::Rounds => {
            t("One life per round. The last side standing takes the round; win the most rounds.")
        }
        ElimStyle::LastManStanding => {
            t("Everyone has their own lives. Out of lives, out of the match: the last one in wins.")
        }
        ElimStyle::Attrition => t(
            "Each team shares a pool of lives. When it runs dry nobody respawns: last team standing wins.",
        ),
    }
}

/// A count, or "Off" for 0.
fn count_or_off(n: u32) -> String {
    if n == 0 {
        t("Off").into()
    } else {
        n.to_string()
    }
}

/// A m:ss time, or "Off" for 0.
fn time_or_off(secs: f32) -> String {
    let secs = secs as u32;
    if secs == 0 {
        t("Off").into()
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));
/// How long a slide from one menu screen to the next takes, in seconds.
const SLIDE_S: f32 = 1.2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Info {
    HowTo,
    Weapons,
    About,
}

/// What the version history page shows.
const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// The game window: the saved fullscreen setting (screenshot runs always
/// get a window), and on Android always the whole screen.
pub fn window_conf() -> Conf {
    let (width, height) = shot_size();
    Conf {
        window_title: "Arcane Dome".to_owned(),
        window_width: width,
        window_height: height,
        high_dpi: true,
        fullscreen: cfg!(target_os = "android")
            || (std::env::var_os("ARCANE_SHOT").is_none() && Settings::load().fullscreen),
        ..Default::default()
    }
}

/// Window size: 1280 x 720, or ARCANE_SHOT_SIZE=<w>x<h> for screenshot runs.
fn shot_size() -> (i32, i32) {
    std::env::var("ARCANE_SHOT_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1280, 720))
}

/// A row on the settings page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingRow {
    Language,
    Fullscreen,
    Slides,
    Blood,
    Fps,
}

/// Where a main menu button goes.
#[derive(Clone, Copy)]
enum Go {
    QuickMatch,
    Editor,
    Info(Info),
    Settings,
    Changes,
    Quit,
}

/// Main menu buttons: id, label, description, and where it goes (None:
/// not built yet, greyed out with a Coming Soon balloon).
type MenuItem = (&'static str, &'static str, &'static str, Option<Go>);

const MENU_LEFT: [MenuItem; 6] = [
    (
        "m_campaign",
        tn("Campaign"),
        tn("Missions against bots, one arena after another."),
        None,
    ),
    (
        "m_quick",
        tn("Quick Match"),
        tn("Pick the map, match type, limits, teams and bots, then play."),
        Some(Go::QuickMatch),
    ),
    (
        "m_editor",
        tn("Map Editor"),
        tn(
            "Build your own maps, or change the ones in the game. Opens the map picked in Quick Match.",
        ),
        Some(Go::Editor),
    ),
    (
        "m_armory",
        tn("Armory"),
        tn("Look over your weapons and gear, and pick what you take into battle."),
        None,
    ),
    (
        "m_achievements",
        tn("Achievements"),
        tn("Goals to chase across your matches, and the ones you have reached."),
        None,
    ),
    ("m_quit", tn("Quit"), tn("Close the game."), Some(Go::Quit)),
];

const MENU_RIGHT: [MenuItem; 5] = [
    (
        "m_profile",
        tn("Profile"),
        tn(
            "Your name, your fighter's personality (what they say in battle), your stats and your look.",
        ),
        None,
    ),
    (
        "m_howto",
        tn("How to Play"),
        tn("Controls and how a match works."),
        Some(Go::Info(Info::HowTo)),
    ),
    (
        "m_weapons",
        tn("Weapon Info"),
        tn("Every weapon and what it does."),
        Some(Go::Info(Info::Weapons)),
    ),
    (
        "m_settings",
        tn("Settings"),
        tn("Language, fullscreen, menu slides, blood effects and the FPS counter."),
        Some(Go::Settings),
    ),
    (
        "m_about",
        tn("About"),
        tn("Who made this game and where it comes from."),
        Some(Go::Info(Info::About)),
    ),
];

enum Screen {
    Menu,
    Setup,
    Info(Info),
    Settings,
    /// The version history (CHANGELOG.md).
    Changes,
    Game(Box<Session>),
    Editor(Box<Editor>),
    /// A test match on the map being edited; the editor waits beside it.
    TestPlay(Box<Session>, Box<Editor>),
    Error(String),
}

impl Screen {
    /// Menu pages slide into each other; matches and errors cut straight in.
    fn slides(&self) -> bool {
        matches!(
            self,
            Screen::Menu
                | Screen::Setup
                | Screen::Info(_)
                | Screen::Settings
                | Screen::Changes
                | Screen::Editor(_)
        )
    }
}

/// A slide between two screens: both are pictures pushed sideways
/// together, fast at first and easing to a stop. Nothing takes input until
/// it ends.
struct Slide {
    from: Texture2D,
    /// The new screen, drawn once on the frame after the switch (so the
    /// click that started the slide does not reach it).
    to: Option<Texture2D>,
    /// Going back to the main menu moves the other way.
    back: bool,
    t: f32,
}

impl Slide {
    fn draw(&self) {
        let (sw, sh) = (screen_width(), screen_height());
        let k = (self.t / SLIDE_S).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - k).powi(4);
        let dir = if self.back { 1.0 } else { -1.0 };
        clear_background(BLACK);
        let shown = [(Some(&self.from), 0.0), (self.to.as_ref(), -dir)];
        for (tex, start) in shown {
            if let Some(tex) = tex {
                let x = (start + dir * eased) * sw;
                draw_texture_ex(
                    tex,
                    x,
                    0.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(sw, sh)),
                        flip_y: true,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

/// What is on screen right now, as a texture.
fn grab_screen() -> Texture2D {
    // Draw everything queued so far before copying.
    unsafe { get_internal_gl().flush() };
    let dpi = screen_dpi_scale();
    let (w, h) = (
        (screen_width() * dpi) as u16,
        (screen_height() * dpi) as u16,
    );
    // A texture of our own: a render target's texture dies with the target.
    let tex = Texture2D::from_rgba8(w, h, &vec![0; w as usize * h as usize * 4]);
    tex.grab_screen();
    resync_texture_bindings();
    tex
}

/// A match started from the editor: leaving it goes back to the editor.
fn test_match(mut s: Session, blood: bool) -> Session {
    s.blood = blood;
    s.exit_label = tn("Back to Editor");
    s
}

/// Team settings offered for this map in this mode: 0 (teams off, unless
/// the mode is for teams only), then the team counts both the map and the
/// mode support.
fn allowed_team_counts(map: &crate::data::MapDef, mode: &crate::data::ModeDef) -> Vec<usize> {
    let most = (mode.teams as usize).min(MAX_TEAMS);
    std::iter::once(0)
        .filter(|_| !mode.team_only)
        .chain(
            map.team_counts
                .iter()
                .map(|&n| n as usize)
                .filter(|&n| n >= 2 && n <= most),
        )
        .collect()
}

fn teams_label(n: usize) -> String {
    if n >= 2 {
        n.to_string()
    } else {
        t("Off").into()
    }
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
    /// Elimination: style (index into STYLES) and lives (index into LIVES
    /// or TEAM_LIVES).
    elim_style: usize,
    lives: usize,
    team_lives: usize,
    powerups: bool,
    stations: bool,
    infinite_ammo: bool,
    /// Juggernaut scoring (at least one of the first two stays on).
    jugg_kills_score: bool,
    killing_jugg_scores: bool,
    jugg_bonus: bool,
    /// Gun Game knock-backs.
    melee_demotes: bool,
    suicide_demotes: bool,
    /// King of the Hill: index into HILL_MOVES.
    hill_move: usize,
}

struct App {
    data: Option<Arc<GameData>>,
    /// The assets folder, where the editor reads and writes maps.
    assets: Option<std::path::PathBuf>,
    screen: Screen,
    slide: Option<Slide>,
    setup: Setup,
    ui: Ui,
    seed: u64,
    time: f32,
    quit: bool,
    settings: Settings,
    /// How far the version history is scrolled, in pixels.
    changes_scroll: f32,
    /// Where the finger was last frame, while dragging the history.
    changes_drag: Option<f32>,
    /// Pages that can be taller than the screen (on a phone).
    setup_scroll: ui::Scroll,
    info_scroll: ui::Scroll,
    /// Rendered map pictures for the setup screen, by map id.
    thumbs: std::collections::HashMap<String, crate::render::MapThumb>,
    /// The FPS counter's memory figures, and when they were read.
    memory: (f64, String),
}

pub async fn run() {
    let settings = Settings::load();
    crate::i18n::set(settings.language);
    let assets = find_assets_dir();
    let data = match &assets {
        None => Err(
            "Could not find the assets folder.\nRun from the project folder or set ARCANE_DOME_ASSETS."
                .to_owned(),
        ),
        Some(dir) => GameData::load(dir).map(Arc::new).map_err(|e| e.to_string()),
    };
    let mut app = App {
        screen: match &data {
            Ok(_) => Screen::Menu,
            Err(e) => Screen::Error(e.clone()),
        },
        data: data.ok(),
        assets,
        slide: None,
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
            elim_style: 0,
            lives: 3,
            team_lives: 3,
            powerups: true,
            stations: true,
            infinite_ammo: false,
            jugg_kills_score: true,
            killing_jugg_scores: true,
            jugg_bonus: false,
            melee_demotes: true,
            suicide_demotes: true,
            hill_move: 2,
        },
        ui: Ui::default(),
        seed: macroquad::miniquad::date::now() as u64,
        time: 0.0,
        quit: false,
        settings,
        changes_scroll: 0.0,
        changes_drag: None,
        setup_scroll: Default::default(),
        info_scroll: Default::default(),
        thumbs: Default::default(),
        memory: (f64::MIN, String::new()),
    };
    app.reset_mode_defaults();

    let shot = debug::Shot::from_env();
    // slide:<scene> starts on the menu and slides to the scene;
    // tour:<scene>,<scene>,... slides through each in turn, 1.6 s apart.
    let mut tour: Vec<String> = Vec::new();
    if let Some(s) = &shot {
        if let Some(to) = s.scene.strip_prefix("slide:") {
            tour.push(to.to_owned());
        } else if let Some(list) = s.scene.strip_prefix("tour:") {
            tour = list.split(',').map(str::to_owned).collect();
        } else {
            app.apply_debug_scene(&s.scene);
        }
    }
    tour.reverse();
    let start = get_time();
    let mut next_stop = 0.0;
    // ARCANE_SHOT_RESIZE=1 keeps resizing the window for the first 3 s, like
    // someone dragging its edge.
    let resize = shot.is_some() && std::env::var_os("ARCANE_SHOT_RESIZE").is_some();
    let mut frames = 0u32;
    loop {
        frames += 1;
        // ARCANE_SHOT_SWEEP=1 moves a pretend mouse down the right-hand
        // menu buttons in the first 2 s.
        if shot.is_some() && std::env::var_os("ARCANE_SHOT_SWEEP").is_some() {
            let t = ((get_time() - start) / 2.0).min(1.0) as f32;
            let s = ui::scale();
            ui::set_fake_mouse(Some(vec2(
                screen_width() - 200.0 * s,
                screen_height() * (0.4 + 0.3 * t) + 18.0 * s,
            )));
        }
        if resize && get_time() - start < 3.0 {
            let k = (frames % 40) as f32;
            request_new_screen_size(900.0 + k * 13.0, 520.0 + k * 7.0);
        }
        app.frame();
        if !tour.is_empty() && get_time() - start >= next_stop {
            let to = tour.pop().expect("tour stop");
            let from = grab_screen();
            app.apply_debug_scene(&to);
            app.slide = Some(Slide {
                from,
                to: None,
                back: to == "menu",
                t: 0.0,
            });
            next_stop += 1.6;
        }
        if let Some(s) = &shot
            && get_time() - start > s.after_s
        {
            get_screen_data().export_png(&s.path);
            break;
        }
        if app.quit {
            // Android keeps a finished game's screen up: end the app instead.
            if cfg!(target_os = "android") {
                std::process::exit(0);
            }
            break;
        }
        next_frame().await;
    }
}

fn on_off(b: bool) -> String {
    if b { t("On").into() } else { t("Off").into() }
}

impl App {
    fn frame(&mut self) {
        let dt = get_frame_time().min(0.25);
        self.time += dt;
        self.ui.begin(dt);
        if is_key_pressed(KeyCode::F11) {
            self.settings.fullscreen = !self.settings.fullscreen;
            set_fullscreen(self.settings.fullscreen);
            self.settings.save();
        }
        if let Some(mut slide) = self.slide.take() {
            slide.t += dt;
            if slide.to.is_none() {
                // Draw the new screen once, out of sight, for its picture.
                self.screen_frame(dt);
                use_screen();
                slide.to = Some(grab_screen());
            }
            if slide.t < SLIDE_S {
                slide.draw();
                self.slide = Some(slide);
                self.draw_fps();
                return;
            }
            // Over: free its pictures (see resync_texture_bindings).
            drop(slide);
            resync_texture_bindings();
        }
        if let Some(next) = self.screen_frame(dt) {
            use_screen();
            // Off on Android for now: grabbing the screen comes out black.
            let slides = self.settings.slides && !cfg!(target_os = "android");
            if slides && self.screen.slides() && next.slides() {
                self.slide = Some(Slide {
                    from: grab_screen(),
                    to: None,
                    back: matches!(next, Screen::Menu),
                    t: 0.0,
                });
            }
            self.screen = next;
        } else {
            use_screen();
            self.ui.draw_tip();
        }
        self.draw_fps();
    }

    fn draw_fps(&mut self) {
        if self.settings.show_fps {
            let s = ui::scale();
            // Memory, read once a second: a figure that keeps climbing
            // points at a leak.
            let now = get_time();
            if now - self.memory.0 >= 1.0 {
                let mb = |b: u64| b / (1 << 20);
                let (ram, gpu) = crate::platform::memory_use();
                let mut text = String::new();
                if let Some(r) = ram {
                    text += &format!("   RAM {} MB", mb(r));
                }
                if let Some(g) = gpu {
                    text += &format!("   GPU {} MB", mb(g));
                }
                self.memory = (now, text);
            }
            // Bottom left, clear of the match HUD's panels.
            ui::text_left(
                &format!("{} FPS{}", get_fps(), self.memory.1),
                8.0 * s,
                screen_height() - 6.0 * s,
                15.0 * s,
                Color::new(1.0, 1.0, 0.6, 0.9),
            );
        }
    }

    /// Run and draw the current screen; returns the screen to switch to.
    fn screen_frame(&mut self, dt: f32) -> Option<Screen> {
        let blood = self.settings.blood;
        // Test-play switches swap the editor between screens; they happen
        // after the match below, once nothing borrows the screen.
        let mut start_test = None;
        let mut back_to_editor = false;
        let next = match &mut self.screen {
            Screen::Error(msg) => {
                let msg = msg.clone();
                self.error_screen(&msg);
                None
            }
            Screen::Menu => self.main_menu(),
            Screen::Setup => self.setup_screen(),
            Screen::Settings => self.settings_screen(),
            Screen::Changes => self.changes_screen(),
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
            Screen::Editor(ed) => match ed.frame(dt, &mut self.ui) {
                editor::Outcome::Continue => None,
                editor::Outcome::Menu => Some(Screen::Menu),
                editor::Outcome::Published(data, id) => {
                    // Quick Match sees the map now; its picture is redrawn.
                    self.data = Some(data);
                    self.thumbs.remove(&id);
                    None
                }
                editor::Outcome::TestPlay(cfg) => {
                    match ed.test_session(cfg) {
                        Ok(s) => start_test = Some(test_match(s, blood)),
                        Err(e) => ed.say(e),
                    }
                    None
                }
            },
            Screen::TestPlay(session, ed) => match session.frame(dt, &mut self.ui) {
                session::Outcome::Continue => None,
                session::Outcome::Rematch => {
                    let cfg = session.config().clone();
                    let cfg = MatchConfig {
                        seed: cfg.seed.wrapping_add(1),
                        ..cfg
                    };
                    match ed.test_session(cfg) {
                        Ok(s) => **session = test_match(s, blood),
                        Err(e) => {
                            ed.say(e);
                            back_to_editor = true;
                        }
                    }
                    None
                }
                session::Outcome::Menu => {
                    back_to_editor = true;
                    None
                }
            },
        };
        // Set directly, not returned: these are not menu page changes, so
        // they must not slide.
        let screen = std::mem::replace(&mut self.screen, Screen::Menu);
        self.screen = match (screen, start_test, back_to_editor) {
            (Screen::Editor(ed), Some(s), _) => Screen::TestPlay(Box::new(s), ed),
            (Screen::TestPlay(_, ed), _, true) => Screen::Editor(ed),
            (screen, _, _) => screen,
        };
        next
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
            Ok(mut s) => {
                s.blood = self.settings.blood;
                Screen::Game(Box::new(s))
            }
            Err(e) => Screen::Error(e),
        }
    }

    /// Open the map picked on the setup screen in the editor.
    fn open_editor(&self) -> Screen {
        let data = self.data();
        let map = &data.maps[self.setup.map.min(data.maps.len() - 1)];
        let Some(assets) = &self.assets else {
            return Screen::Error(t("Could not find the assets folder.").into());
        };
        match Editor::open(data.clone(), assets.clone(), &map.id) {
            Ok(ed) => Screen::Editor(Box::new(ed)),
            Err(e) => Screen::Error(e),
        }
    }

    fn error_screen(&mut self, msg: &str) {
        clear_background(Color::from_rgba(30, 20, 24, 255));
        let s = ui::scale();
        ui::text_left(
            t("Game data has problems:"),
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
        if ui::back_pressed() {
            self.quit = true;
        }
    }

    fn main_menu(&mut self) -> Option<Screen> {
        ui::backdrop(self.time);
        let s = ui::scale();
        let (sw, sh) = (screen_width(), screen_height());

        // On a phone (bigger units) everything sits a little higher, to
        // leave room for the credits under the buttons.
        let touch = crate::platform::touch();
        let lift = if touch { sh * 0.05 } else { 0.0 };

        // Title with a dome arc behind it.
        let c = vec2(sw / 2.0, sh * 0.27 - lift);
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
        ui::text_center("ARCANE DOME", sw / 2.0, sh * 0.25 - lift, 76.0 * s, WHITE);
        ui::text_center(
            t("platform arena shooter"),
            sw / 2.0,
            sh * 0.25 - lift + 34.0 * s,
            22.0 * s,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.8),
        );

        let bw = 300.0 * s;
        let bh = 38.0 * s;
        let gap = if touch { 46.0 * s } else { 50.0 * s };
        let y0 = sh * 0.4 - lift * 1.4;
        let lx = 60.0 * s;
        let rx = sw - 60.0 * s - bw;
        let mut go = None;
        let columns = [
            (&MENU_LEFT[..], lx, Side::Left),
            (&MENU_RIGHT[..], rx, Side::Right),
        ];
        for (items, x, side) in columns {
            for (i, &(id, label, tip, to)) in items.iter().enumerate() {
                let r = Rect::new(x, y0 + gap * i as f32, bw, bh);
                match to {
                    Some(Go::Editor) if crate::platform::touch() => {
                        self.ui.balloon(
                            id,
                            r,
                            t("PC Only"),
                            t("The map editor needs a mouse and keyboard, so for now it is only in the PC version."),
                        );
                        self.ui.tab_locked(id, t(label), r, side);
                    }
                    Some(to) => {
                        self.ui.hint(id, r, t(tip));
                        if self.ui.tab(id, t(label), r, side, false) {
                            go = Some(to);
                        }
                    }
                    None => {
                        self.ui.soon(id, r, t(tip));
                        self.ui.tab_locked(id, t(label), r, side);
                    }
                }
            }
        }

        // Bottom right: the version, and the version history under it.
        let cw = 170.0 * s;
        let changes = Rect::new(sw - 60.0 * s - cw, sh - 66.0 * s, cw, 28.0 * s);
        ui::text_right(
            VERSION,
            changes.x + changes.w - 14.0 * s,
            changes.y - 10.0 * s,
            16.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.5),
        );
        self.ui.hint(
            "m_changes",
            changes,
            t("What is new in each version of the game."),
        );
        if self
            .ui
            .tab("m_changes", t("View Changes"), changes, Side::Right, false)
        {
            go = Some(Go::Changes);
        }
        if is_key_pressed(KeyCode::Enter) {
            go = Some(Go::QuickMatch);
        }

        ui::text_center(
            t("Made by BlueAutumnX5"),
            sw / 2.0,
            sh - 50.0 * s,
            17.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.7),
        );
        ui::text_center(
            t("Fan project inspired by the Raze series by JuiceTin and AddisonR. Not affiliated."),
            sw / 2.0,
            sh - 28.0 * s,
            16.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.45),
        );
        match go? {
            Go::QuickMatch => Some(Screen::Setup),
            Go::Editor => Some(self.open_editor()),
            Go::Info(page) => {
                self.info_scroll = Default::default();
                Some(Screen::Info(page))
            }
            Go::Settings => Some(Screen::Settings),
            Go::Changes => {
                self.changes_scroll = 0.0;
                Some(Screen::Changes)
            }
            Go::Quit => {
                self.quit = true;
                None
            }
        }
    }

    /// Settings: each row switches between On and Off, saved right away.
    fn settings_screen(&mut self) -> Option<Screen> {
        ui::backdrop(self.time);
        let s = ui::scale();
        let (sw, sh) = (screen_width(), screen_height());
        ui::text_center(t("SETTINGS"), sw / 2.0, 58.0 * s, 40.0 * s, WHITE);
        let st = &self.settings;
        let mut rows = vec![
            (
                SettingRow::Language,
                t("Language"),
                st.language.label().to_owned(),
                t("Menus and matches in English or Bahasa Indonesia."),
            ),
            (
                SettingRow::Fullscreen,
                t("Fullscreen (F11)"),
                on_off(st.fullscreen),
                t("Fill the whole screen. F11 switches it any time, in menus or in a match."),
            ),
            (
                SettingRow::Slides,
                t("Menu Slides"),
                on_off(st.slides),
                t("Menu pages slide into each other. Off: they switch at once."),
            ),
            (
                SettingRow::Blood,
                t("Blood Effects"),
                on_off(st.blood),
                t(
                    "Blood on the screen as you get hurt, and on the death screen. The red hit flash stays either way.",
                ),
            ),
            (
                SettingRow::Fps,
                t("FPS Counter"),
                on_off(st.show_fps),
                t("Show frames per second and memory use in the top left corner."),
            ),
        ];
        // A phone is always fullscreen, and has no slides for now.
        if cfg!(target_os = "android") {
            rows.retain(|r| !matches!(r.0, SettingRow::Fullscreen | SettingRow::Slides));
        }
        let w = (640.0 * s).min(sw - 80.0 * s);
        let r = Rect::new(
            (sw - w) / 2.0,
            90.0 * s,
            w,
            (32.0 + 44.0 * rows.len() as f32) * s,
        );
        ui::panel(r, None);
        let mut change = None;
        for (i, (which, label, value, tip)) in rows.into_iter().enumerate() {
            let row = Rect::new(
                r.x + 20.0 * s,
                r.y + 22.0 * s + i as f32 * 44.0 * s,
                r.w - 40.0 * s,
                34.0 * s,
            );
            let id = format!("set_{i}");
            self.ui.hint(&id, row, tip);
            let d = self.ui.stepper(&id, label, &value, row, false, true);
            if d != 0 {
                change = Some((which, d));
            }
        }
        if let Some((which, d)) = change {
            let st = &mut self.settings;
            match which {
                SettingRow::Language => {
                    let all = crate::i18n::Lang::ALL;
                    let at = all.iter().position(|&l| l == st.language).unwrap_or(0);
                    st.language = all[(at as i32 + d).rem_euclid(all.len() as i32) as usize];
                    crate::i18n::set(st.language);
                }
                SettingRow::Fullscreen => {
                    st.fullscreen = !st.fullscreen;
                    set_fullscreen(st.fullscreen);
                }
                SettingRow::Slides => st.slides = !st.slides,
                SettingRow::Blood => st.blood = !st.blood,
                SettingRow::Fps => st.show_fps = !st.show_fps,
            }
            st.save();
        }
        ui::text_center(
            t("Changes are saved right away."),
            sw / 2.0,
            r.y + r.h + 28.0 * s,
            16.0 * s,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.8),
        );
        let bw = 240.0 * s;
        if self.ui.tab(
            "set_back",
            t("Back"),
            Rect::new(50.0 * s, sh - 76.0 * s, bw, 42.0 * s),
            Side::Left,
            false,
        ) || ui::back_pressed()
        {
            return Some(Screen::Menu);
        }
        None
    }

    /// The version history from CHANGELOG.md; the mouse wheel scrolls it.
    fn changes_screen(&mut self) -> Option<Screen> {
        ui::backdrop(self.time);
        let s = ui::scale();
        let (sw, sh) = (screen_width(), screen_height());
        ui::text_center(t("CHANGES"), sw / 2.0, 58.0 * s, 40.0 * s, WHITE);
        let r = Rect::new(60.0 * s, 80.0 * s, sw - 120.0 * s, sh - 180.0 * s);
        ui::panel(r, None);
        let (head, body) = (24.0 * s, 18.0 * s);
        let width = r.w - 60.0 * s;
        // Lay out every line first: (text, size, colour, indent).
        let mut lines: Vec<(String, f32, Color, f32)> = Vec::new();
        for raw in CHANGELOG.lines() {
            if let Some(h) = raw.strip_prefix("## ") {
                if !lines.is_empty() {
                    lines.push((String::new(), body * 0.6, WHITE, 0.0));
                }
                lines.push((h.to_owned(), head, ACCENT, 0.0));
            } else if let Some(item) = raw.strip_prefix("- ") {
                for (k, part) in ui::wrap(item, width - 24.0 * s, body)
                    .into_iter()
                    .enumerate()
                {
                    let text = if k == 0 { format!("- {part}") } else { part };
                    let indent = if k == 0 { 0.0 } else { 16.0 * s };
                    lines.push((text, body, Color::new(1.0, 1.0, 1.0, 0.85), indent));
                }
            }
        }
        let total: f32 = lines.iter().map(|l| l.1 + 8.0 * s).sum();
        let room = r.h - 40.0 * s;
        let most = (total - room).max(0.0);
        let drag = ui::drag_y(&mut self.changes_drag);
        self.changes_scroll =
            (self.changes_scroll - ui::wheel_step() * 40.0 * s - drag).clamp(0.0, most);
        if is_key_down(KeyCode::Down) {
            self.changes_scroll = (self.changes_scroll + 8.0 * s).min(most);
        }
        if is_key_down(KeyCode::Up) {
            self.changes_scroll = (self.changes_scroll - 8.0 * s).max(0.0);
        }
        let mut y = r.y + 30.0 * s - self.changes_scroll;
        for (text, size, color, indent) in &lines {
            y += size + 8.0 * s;
            // Only lines fully inside the panel.
            if y - size >= r.y + 6.0 * s && y <= r.y + r.h - 10.0 * s {
                ui::text_left(text, r.x + 30.0 * s + indent, y - 8.0 * s, *size, *color);
            }
        }
        if most > 0.0 {
            ui::text_right(
                if crate::platform::touch() {
                    t("Drag to scroll")
                } else {
                    t("Mouse wheel to scroll")
                },
                r.x + r.w - 16.0 * s,
                r.y + r.h + 22.0 * s,
                15.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );
        }
        let bw = 240.0 * s;
        if self.ui.tab(
            "ch_back",
            t("Back"),
            Rect::new(50.0 * s, sh - 76.0 * s, bw, 42.0 * s),
            Side::Left,
            false,
        ) || ui::back_pressed()
        {
            return Some(Screen::Menu);
        }
        None
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
        if let Some(h) = mode.hill_move_s {
            self.setup.hill_move = nearest(&HILL_MOVES, h);
        }
        if let Some(l) = mode.lives {
            let list: Vec<f32> = LIVES.iter().map(|&v| v as f32).collect();
            self.setup.lives = nearest(&list, l as f32);
        }
        if let Some(l) = mode.team_lives {
            let list: Vec<f32> = TEAM_LIVES.iter().map(|&v| v as f32).collect();
            self.setup.team_lives = nearest(&list, l as f32);
        }
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
            elim_style: STYLES[c.elim_style],
            lives: Some(match STYLES[c.elim_style] {
                ElimStyle::Attrition => TEAM_LIVES[c.team_lives],
                _ => LIVES[c.lives],
            }),
            bots: c.ffa_bots,
            difficulty: Difficulty::ALL[c.ffa_skill],
            teams: c.teams as u8,
            team_bots: c.team_bots,
            team_difficulty: c.team_skill.map(|k| Difficulty::ALL[k]),
            team_colors: c.team_colors,
            powerups: c.powerups,
            stations: c.stations,
            infinite_ammo: c.infinite_ammo,
            jugg_kills_score: c.jugg_kills_score,
            killing_jugg_scores: c.killing_jugg_scores,
            jugg_bonus: c.jugg_bonus,
            melee_demotes: c.melee_demotes,
            suicide_demotes: c.suicide_demotes,
            hill_move_s: Some(HILL_MOVES[c.hill_move]),
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
        let (sw, screen_h) = (screen_width(), screen_height());
        // Laid out for a screen at least 720 units tall; on a phone (bigger
        // units) that is taller than the screen, and the page scrolls.
        let sh = (720.0 * s).max(screen_h);
        self.setup_scroll.begin(sh);
        ui::text_center(t("QUICK MATCH"), sw / 2.0, 58.0 * s, 40.0 * s, WHITE);

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
        let pv = Rect::new(lx + pad, top + pad, lw - 2.0 * pad, 150.0 * s);
        let thumb = self
            .thumbs
            .entry(map.id.clone())
            .or_insert_with(|| crate::render::MapThumb::new(map, 768));
        let picture = thumb.draw(map);
        self.setup_scroll.resume();
        ui::map_preview(map, picture, pv);
        let ny = pv.y + pv.h + 26.0 * s;
        let mut map_delta = self.ui.stepper(
            "s_map",
            t("Map"),
            t(&map.name),
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
        for line in ui::wrap(t(&map.description), pv.w, dsize).iter().take(2) {
            ui::text_left(line, pv.x, y, dsize, Color::new(1.0, 1.0, 1.0, 0.7));
            y += dsize + 4.0 * s;
        }
        y += 6.0 * s;

        let rh = 30.0 * s;
        let row = |y: f32| Rect::new(pv.x, y, pv.w, rh);
        let d_mode = self.ui.stepper(
            "s_mode",
            t("Match Type"),
            t(&mode.display_name()),
            row(y),
            false,
            true,
        );
        y += rh + 2.0 * s;
        // Elimination: how it is played. Attrition needs teams.
        let style = STYLES[self.setup.elim_style];
        let mut d_style = 0;
        if !mode.respawn {
            d_style = self.ui.stepper(
                "s_style",
                t("Style"),
                style_name(style),
                row(y),
                false,
                true,
            );
            y += rh + 2.0 * s;
        }
        let blurb = if mode.respawn {
            t(&mode.description)
        } else {
            style_blurb(style)
        };
        for line in ui::wrap(blurb, pv.w - 10.0 * s, dsize).iter().take(2) {
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
        let (limit_label, limit_value) = match (mode.respawn, style) {
            (true, _) => (
                t("Score Limit"),
                count_or_off(SCORE_LIMITS[self.setup.score]),
            ),
            (false, ElimStyle::Rounds) => {
                (t("Rounds"), ROUND_COUNTS[self.setup.rounds].to_string())
            }
            (false, ElimStyle::LastManStanding) => {
                (t("Lives"), count_or_off(LIVES[self.setup.lives]))
            }
            (false, ElimStyle::Attrition) => (
                t("Team Lives"),
                count_or_off(TEAM_LIVES[self.setup.team_lives]),
            ),
        };
        // Gun Game ends when a ladder is finished: its knock-back switches
        // take the score limit's place.
        let gun_game = mode.rules == Rules::GunGame;
        let (mut d_limit, mut d_melee, mut d_suicide) = (0, 0, 0);
        if gun_game {
            let rows = [
                (
                    "s_melee",
                    t("Melee Demotes"),
                    self.setup.melee_demotes,
                    t("A kill with a melee weapon also sends the victim back one weapon."),
                ),
                (
                    "s_suicide",
                    t("Suicide Demotes"),
                    self.setup.suicide_demotes,
                    t("Dying by your own fire or to the map sends you back one weapon."),
                ),
            ];
            for (k, (id, label, on, tip)) in rows.into_iter().enumerate() {
                let d = self.ui.stepper(id, label, &on_off(on), row(y), false, true);
                self.ui.hint(&format!("{id}_tip"), row(y), tip);
                if k == 0 {
                    d_melee = d;
                } else {
                    d_suicide = d;
                }
                y += rh + 4.0 * s;
            }
        } else {
            d_limit = self
                .ui
                .stepper("s_limit", limit_label, &limit_value, row(y), false, true);
            y += rh + 4.0 * s;
        }
        // King of the Hill: how often the hill moves (maps with one zone
        // keep it in place anyway).
        let mut d_hill = 0;
        if mode.rules == Rules::KingOfTheHill {
            let secs = HILL_MOVES[self.setup.hill_move];
            let value = if secs > 0.0 {
                tf("Every {} s", &[&secs])
            } else {
                t("Never").to_owned()
            };
            d_hill = self
                .ui
                .stepper("s_hill", t("Hill Moves"), &value, row(y), false, true);
            y += rh + 4.0 * s;
        }
        let tl = if !mode.respawn && style == ElimStyle::Rounds {
            t("Round Time")
        } else {
            t("Time Limit")
        };
        let d_time = self.ui.stepper(
            "s_time",
            tl,
            &time_or_off(TIME_LIMITS[self.setup.time]),
            row(y),
            false,
            true,
        );
        y += rh + 4.0 * s;
        let d_pow = self.ui.stepper(
            "s_pow",
            t("Powerups"),
            &on_off(self.setup.powerups),
            row(y),
            false,
            true,
        );
        y += rh + 4.0 * s;
        let d_sta = self.ui.stepper(
            "s_sta",
            t("Stations"),
            &on_off(self.setup.stations),
            row(y),
            false,
            true,
        );
        y += rh + 4.0 * s;
        let d_inf = self.ui.stepper(
            "s_inf",
            t("Infinite Clips"),
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
            t("Teams"),
            &teams_label(self.setup.teams),
            Rect::new(rx, top, rw, 34.0 * s),
            false,
            choices.len() > 1,
        );
        if self.setup.teams >= 2 {
            let box_h = 86.0 * s;
            for team in 0..self.setup.teams {
                let y = top + 48.0 * s + team as f32 * (box_h + 10.0 * s);
                let pick = self.setup.team_colors[team] as usize;
                let color = TEAM_COLORS[pick];
                let bots = self.setup.team_bots[team];
                ui::panel(Rect::new(rx, y, rw, box_h), None);
                draw_rectangle(rx, y, 6.0 * s, box_h, color);
                // Click the swatch for the next colour.
                let sw_r = Rect::new(rx + 18.0 * s, y + 10.0 * s, 20.0 * s, 20.0 * s);
                if self.ui.swatch(&format!("s_t{team}_color"), sw_r, color) {
                    recolor = Some(team);
                }
                let title = tf("{} Team", &[&t(TEAM_NAMES[pick])]);
                ui::text_left(&title, rx + 48.0 * s, y + 26.0 * s, 22.0 * s, color);
                let who = if team == 0 {
                    tf("You + {} bots", &[&bots])
                } else {
                    tf("{} bots", &[&bots])
                };
                ui::text_right(
                    &who,
                    rx + rw - 14.0 * s,
                    y + 26.0 * s,
                    17.0 * s,
                    Color::new(1.0, 1.0, 1.0, 0.75),
                );
                let ry = y + 42.0 * s;
                d_bots[team] = self.ui.stepper(
                    &format!("s_t{team}_bots"),
                    t("Bots"),
                    &bots.to_string(),
                    Rect::new(rx + 10.0 * s, ry, half, 32.0 * s),
                    false,
                    true,
                );
                d_skill[team] = self.ui.stepper(
                    &format!("s_t{team}_skill"),
                    t("Skill"),
                    t(Difficulty::ALL[self.setup.team_skill[team]].label()),
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
            ui::text_left(t("Opponents"), rx + 18.0 * s, y + 26.0 * s, 22.0 * s, color);
            let who = tf("You against {} bots", &[&self.setup.ffa_bots]);
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
                t("Bots"),
                &self.setup.ffa_bots.to_string(),
                r1,
                false,
                true,
            );
            d_ffa_skill = self.ui.stepper(
                "s_ffa_skill",
                t("Difficulty"),
                t(Difficulty::ALL[self.setup.ffa_skill].label()),
                r2,
                false,
                true,
            );
            total = 1 + self.setup.ffa_bots;
        }
        let (mut d_jkills, mut d_jslay, mut d_jbonus) = (0, 0, 0);
        if mode.rules == Rules::Juggernaut {
            let y = top + 48.0 * s + 140.0 * s + 14.0 * s;
            let rows_h = 3.0 * 38.0 * s;
            ui::panel(
                Rect::new(rx, y, rw, 30.0 * s + rows_h + 10.0 * s),
                Some(t("Scoring")),
            );
            let r = |k: f32| {
                Rect::new(
                    rx + 10.0 * s,
                    y + 38.0 * s + k * 38.0 * s,
                    rw - 20.0 * s,
                    32.0 * s,
                )
            };
            let c = &self.setup;
            d_jkills = self.ui.stepper(
                "s_jkills",
                t("Kills as Juggernaut"),
                &on_off(c.jugg_kills_score),
                r(0.0),
                false,
                true,
            );
            self.ui.hint(
                "s_jkills_tip",
                r(0.0),
                t("Each kill you make while you are the Juggernaut scores a point."),
            );
            d_jslay = self.ui.stepper(
                "s_jslay",
                t("Killing the Juggernaut"),
                &on_off(c.killing_jugg_scores),
                r(1.0),
                false,
                true,
            );
            self.ui.hint(
                "s_jslay_tip",
                r(1.0),
                t("Killing the Juggernaut scores a point. One way to score always stays on."),
            );
            d_jbonus = self.ui.stepper(
                "s_jbonus",
                t("Juggernaut Bonus"),
                &on_off(c.jugg_bonus),
                r(2.0),
                false,
                c.jugg_kills_score,
            );
            self.ui.hint(
                "s_jbonus_tip",
                r(2.0),
                t("Kills made as the Juggernaut score 2 points instead of 1."),
            );
        }
        if let Some([lo, hi]) = map.recommended_players {
            let note = tf("{} players  (map suits {}-{})", &[&total, &lo, &hi]);
            let warn = total < lo || total > hi;
            let col = if warn {
                Color::new(1.0, 0.6, 0.4, 0.9)
            } else {
                Color::new(1.0, 1.0, 1.0, 0.6)
            };
            ui::text_left(&note, rx, sh - 120.0 * s, 17.0 * s, col);
        }
        // Every limit off: say so, and how to leave.
        let c = &self.setup;
        let no_clock = TIME_LIMITS[c.time] == 0.0;
        let endless = no_clock
            && !gun_game
            && match (mode.respawn, STYLES[c.elim_style]) {
                (true, _) => SCORE_LIMITS[c.score] == 0,
                (false, ElimStyle::Rounds) => false,
                (false, ElimStyle::LastManStanding) => LIVES[c.lives] == 0,
                (false, ElimStyle::Attrition) => TEAM_LIVES[c.team_lives] == 0,
            };
        if endless {
            let gold = Color::new(1.0, 0.82, 0.3, 0.95);
            ui::text_left(
                t("Endless match: no limit is on, so it never ends by itself."),
                rx,
                sh - 170.0 * s,
                17.0 * s,
                gold,
            );
            ui::text_left(
                t("Leave any time from the pause menu (P or Esc)."),
                rx,
                sh - 148.0 * s,
                17.0 * s,
                gold,
            );
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
            match (mode.respawn, STYLES[c.elim_style]) {
                (true, _) => c.score = step(c.score, d_limit, SCORE_LIMITS.len()),
                (false, ElimStyle::Rounds) => {
                    c.rounds = step(c.rounds, d_limit, ROUND_COUNTS.len())
                }
                (false, ElimStyle::LastManStanding) => {
                    c.lives = step(c.lives, d_limit, LIVES.len())
                }
                (false, ElimStyle::Attrition) => {
                    c.team_lives = step(c.team_lives, d_limit, TEAM_LIVES.len())
                }
            }
        }
        if d_style != 0 {
            c.elim_style = step(c.elim_style, d_style, STYLES.len());
            // Attrition needs teams: skip past it in free-for-all.
            if c.teams < 2 && STYLES[c.elim_style] == ElimStyle::Attrition {
                c.elim_style = step(c.elim_style, d_style, STYLES.len());
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
        if d_melee != 0 {
            c.melee_demotes = !c.melee_demotes;
        }
        if d_hill != 0 {
            c.hill_move = step(c.hill_move, d_hill, HILL_MOVES.len());
        }
        if d_suicide != 0 {
            c.suicide_demotes = !c.suicide_demotes;
        }
        // Turning off the last way to score turns the other one back on.
        if d_jkills != 0 {
            c.jugg_kills_score = !c.jugg_kills_score;
            c.killing_jugg_scores |= !c.jugg_kills_score;
        }
        if d_jslay != 0 {
            c.killing_jugg_scores = !c.killing_jugg_scores;
            c.jugg_kills_score |= !c.killing_jugg_scores;
        }
        if d_jbonus != 0 {
            c.jugg_bonus = !c.jugg_bonus;
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
        if c.teams < 2 && STYLES[c.elim_style] == ElimStyle::Attrition {
            c.elim_style = 1; // Last Man Standing
        }
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

        self.setup_scroll.end(sh);
        let sh = screen_h;

        // Back (slides left) and Start (slides right).
        let bw = 240.0 * s;
        let bh = 42.0 * s;
        let by = sh - 76.0 * s;
        if self.ui.tab(
            "s_back",
            t("Back"),
            Rect::new(50.0 * s, by, bw, bh),
            Side::Left,
            false,
        ) || ui::back_pressed()
        {
            return Some(Screen::Menu);
        }
        if self.ui.tab(
            "s_start",
            t("Start"),
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
        let (sw, screen_h) = (screen_width(), screen_height());
        // As the setup page: scrolls on a phone.
        let sh = (720.0 * s).max(screen_h);
        self.info_scroll.begin(sh);
        let title = match page {
            Info::HowTo => t("HOW TO PLAY"),
            Info::Weapons => t("WEAPON INFO"),
            Info::About => t("ABOUT"),
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
                let keys: &[&str] = if crate::platform::touch() {
                    &[
                        tn(
                            "Left stick  walk; push down to crouch, or to drop through a thin platform",
                        ),
                        tn("Jump  tap to jump, tap again in the air to double jump"),
                        tn(
                            "Fire  hold to aim at the nearest enemy you can see; drag to aim yourself",
                        ),
                        tn("Reload, Switch  the buttons above Fire"),
                        tn("Weapons  tap one in the bar at the bottom"),
                        tn("Step  use Shadowstep (the button shows while you hold one)"),
                        tn("Top left  pause and scoreboard (Back also pauses)"),
                    ]
                } else {
                    &[
                        tn(
                            "A / D  move        W or Space  jump (press again in the air to double jump)",
                        ),
                        tn("S  crouch          S + jump  drop through a thin platform"),
                        tn("Mouse  aim         Left click  fire        R  reload"),
                        tn("1 - 0, Q / E or mouse wheel  switch weapons"),
                        tn("F  use Shadowstep (when you hold one)"),
                        tn("Tab  scoreboard    P or Esc  pause"),
                        tn("F11  fullscreen on / off"),
                    ]
                };
                for k in keys {
                    line(t(k), WHITE);
                }
                line("", WHITE);
                for k in [
                    tn("Thrusters launch you high into the air. Step on the glowing pad."),
                    tn("Weapon stations deploy a random weapon every 30-45 seconds."),
                    tn(
                        "Shield stations are rarer and give +50 or +100 shield. Shield soaks up part of each hit.",
                    ),
                    tn(
                        "Powerup spots (0-4 per map) each hold one fixed powerup: first at 45 s, then 2 min after pickup:",
                    ),
                ] {
                    line(t(k), soft);
                }
                for p in &data.powerups.powerups {
                    let c = Color::from_rgba(p.color[0], p.color[1], p.color[2], 255);
                    let what = if p.duration_s > 0.0 {
                        let secs = format!("{:.0}", p.duration_s);
                        format!("   {}", tf("{}  ({} s)", &[&t(&p.name), &secs]))
                    } else if crate::platform::touch() {
                        format!("   {}", tf("{}  (one teleport, tap Step)", &[&t(&p.name)]))
                    } else {
                        format!("   {}", tf("{}  (one teleport, press F)", &[&t(&p.name)]))
                    };
                    line(&what, c);
                }
            }
            Info::Weapons => {
                let cols = [0.0, 0.26, 0.44, 0.56, 0.68, 0.8];
                let head = [
                    tn("Weapon"),
                    tn("Type"),
                    tn("Damage"),
                    tn("Rate/s"),
                    tn("Clip"),
                    tn("Range"),
                ];
                for (k, h) in head.iter().enumerate() {
                    ui::text_left(
                        t(h),
                        x + cols[k] * (r.w - 48.0 * s),
                        y,
                        size,
                        Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.9),
                    );
                }
                y += size + 10.0 * s;
                for (i, w) in data.weapons.iter().enumerate() {
                    let kind = match w.projectile.kind {
                        ProjectileKind::Hitscan => t("Instant"),
                        ProjectileKind::Bullet => t("Bullet"),
                        ProjectileKind::Grenade => t("Grenade"),
                        ProjectileKind::Rocket => t("Rocket"),
                        ProjectileKind::Beam => t("Beam"),
                        ProjectileKind::MeleeArc => t("Melee"),
                    };
                    let dmg = match &w.projectile.explosion {
                        Some(e) => tf(
                            "{} + {} blast",
                            &[&format!("{:.0}", w.damage), &format!("{:.0}", e.damage)],
                        ),
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
                        format!("{}  {}", if i == 9 { 0 } else { i + 1 }, t(&w.name)),
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
                for k in [
                    tn(
                        "Arcane Dome is a free, open-source fan project made with Rust and Macroquad.",
                    ),
                    "",
                    tn("Made by BlueAutumnX5."),
                    "",
                    tn("It is inspired by the Raze series by JuiceTin and AddisonR."),
                    tn(
                        "It is not affiliated with or endorsed by them, and it uses none of their art,",
                    ),
                    tn("sounds, code, names or map layouts."),
                    "",
                    tn("Code: MIT License. See CREDITS.md for libraries and assets."),
                ] {
                    line(t(k), soft);
                }
            }
        }
        self.info_scroll.end(sh);
        let sh = screen_h;
        let bw = 240.0 * s;
        if self.ui.tab(
            "i_back",
            t("Back"),
            Rect::new(50.0 * s, sh - 76.0 * s, bw, 42.0 * s),
            Side::Left,
            false,
        ) || ui::back_pressed()
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
        // ARCANE_SHOT_STYLE=rounds|lms|attrition, ARCANE_SHOT_ENDLESS=1 (all limits off).
        if let Ok(st) = std::env::var("ARCANE_SHOT_STYLE") {
            self.setup.elim_style = match st.as_str() {
                "lms" => 1,
                "attrition" => 2,
                _ => 0,
            };
        }
        if std::env::var_os("ARCANE_SHOT_ENDLESS").is_some() {
            (
                self.setup.score,
                self.setup.time,
                self.setup.lives,
                self.setup.team_lives,
            ) = (0, 0, 0, 0);
        }
        // ARCANE_SHOT_COLORS=3,0,2,1 picks team colours (palette indices).
        if let Ok(list) = std::env::var("ARCANE_SHOT_COLORS") {
            let picks = list.split(',').filter_map(|c| c.parse().ok());
            for (t, c) in picks.take(MAX_TEAMS).enumerate() {
                self.setup.team_colors[t] = c;
            }
        }
        match scene {
            "menu" => self.screen = Screen::Menu,
            "custom" | "setup" => self.screen = Screen::Setup,
            "howto" => self.screen = Screen::Info(Info::HowTo),
            "weapons" => self.screen = Screen::Info(Info::Weapons),
            "about" => self.screen = Screen::Info(Info::About),
            "settings" => self.screen = Screen::Settings,
            "changes" => self.screen = Screen::Changes,
            // editor[:map] opens the editor on a map.
            s if s.starts_with("editor") => {
                if let Some(id) = s.strip_prefix("editor:")
                    && let Some(i) = self.data().maps.iter().position(|m| m.id == id)
                {
                    self.setup.map = i;
                }
                self.screen = self.open_editor();
                // ARCANE_SHOT_EDITOR=help|new|validate|testplay|export|publish|import:<path>|... (see Editor::debug_show).
                if let (Screen::Editor(ed), Ok(what)) =
                    (&mut self.screen, std::env::var("ARCANE_SHOT_EDITOR"))
                {
                    ed.debug_show(&what);
                }
            }
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
                // ARCANE_SHOT_HP=1 sets the player's health (blood overlay).
                let hp = std::env::var("ARCANE_SHOT_HP")
                    .ok()
                    .and_then(|v| v.parse().ok());
                if let (Screen::Game(g), Some(hp)) = (&mut self.screen, hp) {
                    g.debug_set_health(hp);
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
/// `ARCANE_SHOT=out.png ARCANE_SHOT_SCENE=menu|setup|howto|weapons|about|settings|changes|editor[:map_id]|game[:map_id[@x,y]]|slide:<scene>|tour:<scene>,<scene>,... ARCANE_SHOT_AFTER=2`,
/// optionally `ARCANE_SHOT_MODE=<mode id> ARCANE_SHOT_TEAMS=<n> ARCANE_SHOT_BOTS=<per team> ARCANE_SHOT_COLORS=3,0,2,1
/// ARCANE_SHOT_TAB=1` (scoreboard), `ARCANE_SHOT_EDITOR=<steps>` (see `Editor::debug_show`), `ARCANE_SHOT_HP=<n>`, `ARCANE_SHOT_KILL=1`,
/// `ARCANE_SHOT_SIZE=<w>x<h>` (window size), `ARCANE_SHOT_RESIZE=1` (keep resizing for 3 s),
/// `ARCANE_SHOT_SWEEP=1` (pretend mouse down the right-hand menu buttons).
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
