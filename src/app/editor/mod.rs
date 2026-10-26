//! The in-game map editor.

#[allow(dead_code)]
mod doc;
mod files;
mod report;
mod windows;

use super::session::Session;
use super::ui::{self, ACCENT, Ui};
use crate::data::{GameData, MapDef, ObjectKind, PickupKind, parse_map};
use crate::render::{GameCamera, MapArt, TEAM_COLORS, draw_sky, use_camera, use_screen};
use crate::sim::{Difficulty, MatchConfig};
use doc::{Doc, Layer};
use macroquad::prelude::*;
use report::Report;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

/// Grid step that points snap to, in tiles.
const SNAP: f32 = 0.5;
const MIN_PPT: f32 = 6.0;
const MAX_PPT: f32 = 96.0;
/// Keyboard pan speed, in screen pixels per second.
const PAN_SPEED: f32 = 900.0;
/// Mouse help along the bottom, per tool (keys are on the buttons).
const MOUSE_HINT: [&str; 3] = [
    "Left-click: select   Drag: move   Right-drag: pan   Wheel: zoom",
    "Left-click: add a corner   Click the first corner: close the shape   Right-drag: pan   Wheel: zoom",
    "Left-click: place (drops onto the floor below)   Right-drag: pan   Wheel: zoom",
];
/// What each tool button does, shown when the mouse rests on it.
const TOOL_TIPS: [&str; 10] = [
    "Click a shape, spawn, pickup or gadget to select it, then drag to move it. On a selected shape, drag the corner squares to reshape it.",
    "Click to place corners. Click the first corner again, or press Enter, to close the shape. Pick its kind and material in the panel at the bottom left.",
    "Where fighters appear. Set a team in the panel, or leave it on Any for free-for-all. A map needs 4 free-for-all spawns and 2 per team.",
    "A health pack. It comes back 25 seconds after someone takes it.",
    "An ammo box. It comes back 15 seconds after someone takes it.",
    "A weapon lying on the floor. Pick which weapon in the panel.",
    "A glowing pad that launches fighters high into the air. Set how high in the panel.",
    "Deploys a random weapon every 30 to 45 seconds.",
    "Gives +50 or +100 shield. Shield soaks up part of each hit.",
    "Holds one powerup: first at 45 seconds, then 2 minutes after each pickup. Pick one in the panel, or leave it on Random.",
];
/// The instructions page: a heading and a paragraph per section, in two
/// columns.
const HELP_PAGE: [(&str, &str); 8] = [
    (
        "Getting around",
        "Right-drag or middle-drag the map to pan; WASD and the arrow keys pan too. The mouse wheel zooms toward the cursor. Fit View (F) shows the whole map again.",
    ),
    (
        "Tools (left)",
        "Pick a tool with its button or its number key. Select (1) picks and moves things, Draw Shape (2) adds terrain, and 3 to 0 put down spawns, pickups and gadgets. They drop onto the floor below the cursor.",
    ),
    (
        "Shapes",
        "Solid is ground and walls. Hatch is a thin floor you can jump up through. Water lets you swim. Hazard hurts anyone touching it. Select a shape to change its kind and material, or Delete (Del) it.",
    ),
    (
        "Settings panel",
        "The panel at the bottom left shows the settings of what you selected: a spawn's team, which weapon a pickup is, a thruster's height, a shield station's strength, a powerup spot's powerup. Click < or > to change them.",
    ),
    (
        "Snap Grid and Show Grid",
        "With Snap Grid (Shift) on, corners and things land on half-tile steps; switch it off to place freely. Show Grid (G) hides or shows the lines; bright lines are every 5 tiles.",
    ),
    (
        "Checking the map",
        "The top bar always says whether the game can load the map: Map OK in green, or the first problem in red. Validate (V) checks more: can bots reach every spawn, pickup and gadget, and is any spawn in lava or water? Click a problem to fly to it. Test Play (T) plays the map against bots right away.",
    ),
    (
        "Saving and sharing",
        "Save Draft (Ctrl+S) keeps your work in assets/editor/ without touching the game's maps. Publish (Ctrl+P) adds a map that passes Validate to Quick Match. Export Map (Ctrl+E) and Import Map (Ctrl+I) share maps through the Arcane Dome Maps folder in Documents, and you can drop a map file onto the editor too.",
    ),
    (
        "Undo",
        "Undo (Ctrl+Z) and Redo (Ctrl+Y) step through your last 200 edits. A whole drag counts as one edit.",
    ),
];
/// Sizes a new map can have, in tiles.
const NEW_WIDTHS: [u32; 8] = [48, 56, 64, 72, 80, 96, 112, 128];
const NEW_HEIGHTS: [u32; 8] = [28, 32, 36, 40, 44, 48, 56, 64];
/// Longest name a new map can have.
const NAME_MAX: usize = 24;

/// Mouse travel, in pixels, before a click becomes a drag.
const DRAG_START_PX: f32 = 4.0;
/// How close, in pixels, a click must be to grab a shape's corner.
const HANDLE_PX: f32 = 8.0;
/// Zoom (pixels per tile) from which markers get their names.
const LABEL_PPT: f32 = 26.0;
/// How long a note like "Saved draft" stays up, in seconds.
const NOTE_S: f32 = 3.0;

pub enum Outcome {
    Continue,
    Menu,
    /// Start a test match on the map being edited (see `test_session`).
    TestPlay(MatchConfig),
    /// A map was published: the game data, reloaded with it, and its id.
    Published(Arc<GameData>, String),
}

/// Shape kinds, and the material a new shape of that kind starts with.
const KINDS: [(&str, &str); 4] = [
    ("solid", "rock"),
    ("hatch", "wood"),
    ("water", "water"),
    ("hazard", "lava"),
];
const MATERIALS: [&str; 12] = [
    "rock", "metal", "wood", "ice", "sand", "moss", "sludge", "glass", "lava", "spikes", "water",
    "void",
];
/// Things the place tools put down: name, list, and starting fields.
const PLACE: [(&str, Layer, &str); 8] = [
    ("Spawn", Layer::Spawns, "{}"),
    (
        "Health",
        Layer::Pickups,
        r#"{"type": "health", "respawn_s": 25}"#,
    ),
    (
        "Ammo",
        Layer::Pickups,
        r#"{"type": "ammo", "respawn_s": 15}"#,
    ),
    (
        "Weapon",
        Layer::Pickups,
        r#"{"type": "weapon", "respawn_s": 25}"#,
    ),
    (
        "Thruster",
        Layer::Objects,
        r#"{"type": "thruster", "height_tiles": 8}"#,
    ),
    (
        "Weapon Station",
        Layer::Objects,
        r#"{"type": "weapon_station"}"#,
    ),
    (
        "Shield Station",
        Layer::Objects,
        r#"{"type": "shield_station"}"#,
    ),
    ("Powerup", Layer::Objects, r#"{"type": "powerup_spot"}"#),
];
/// How far below the cursor a placed thing looks for a floor to stand on.
const FLOOR_REACH: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Select,
    /// Click corners; click the first corner again or press Enter to finish.
    Draw,
    /// Index into PLACE.
    Place(usize),
}

/// Everything the right-hand buttons do; each has a key too.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    New,
    Open,
    Save,
    Settings,
    Import,
    Export,
    Undo,
    Redo,
    Delete,
    Finish,
    PopCorner,
    Snap,
    Grid,
    Fit,
    Validate,
    TestPlay,
    Publish,
    Help,
    Back,
}

/// The right-hand buttons, in groups.
const ACTIONS: [&[Action]; 6] = [
    &[
        Action::New,
        Action::Open,
        Action::Save,
        Action::Settings,
        Action::Import,
        Action::Export,
    ],
    &[Action::Undo, Action::Redo, Action::Delete],
    &[Action::Finish, Action::PopCorner],
    &[Action::Snap, Action::Grid, Action::Fit],
    &[Action::Validate, Action::TestPlay, Action::Publish],
    &[Action::Help, Action::Back],
];

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::New => "New Map (Ctrl+N)",
            Action::Open => "Open Map (Ctrl+O)",
            Action::Save => "Save Draft (Ctrl+S)",
            Action::Settings => "Map Settings (M)",
            Action::Import => "Import Map (Ctrl+I)",
            Action::Export => "Export Map (Ctrl+E)",
            Action::Undo => "Undo (Ctrl+Z)",
            Action::Redo => "Redo (Ctrl+Y)",
            Action::Delete => "Delete (Del)",
            Action::Finish => "Finish Shape (Enter)",
            Action::PopCorner => "Remove Corner (Backspace)",
            Action::Snap => "Snap Grid (Shift)",
            Action::Grid => "Show Grid (G)",
            Action::Fit => "Fit View (F)",
            Action::Validate => "Validate (V)",
            Action::TestPlay => "Test Play (T)",
            Action::Publish => "Publish (Ctrl+P)",
            Action::Help => "Instructions (F1)",
            Action::Back => "Back to Menu (Esc)",
        }
    }

    fn tip(self) -> &'static str {
        match self {
            Action::New => "Start a new, empty map with a floor and the spawns it needs.",
            Action::Open => "Open one of the game's maps, or a draft you saved.",
            Action::Save => {
                "Save your work as a draft in assets/editor/. The game's own maps are not touched."
            }
            Action::Settings => {
                "The map's name, description and size, and which team counts and match types it offers."
            }
            Action::Import => {
                "Open a map file someone shared with you (from the Arcane Dome Maps folder in Documents). You can also drop a map file onto the editor."
            }
            Action::Export => {
                "Save a copy of this map to the Arcane Dome Maps folder in Documents, to share it or keep it safe."
            }
            Action::Undo => "Take back the last edit.",
            Action::Redo => "Put back the edit you just took back.",
            Action::Delete => {
                "Remove what is selected. With a corner selected, removes just that corner."
            }
            Action::Finish => "Close the shape you are drawing (it needs 3 corners or more).",
            Action::PopCorner => "Take back the last corner of the shape you are drawing.",
            Action::Snap => {
                "When on, corners and things land on half-tile steps. Switch off to place freely."
            }
            Action::Grid => "Show or hide the grid lines. Bright lines are every 5 tiles.",
            Action::Fit => "Zoom and pan so the whole map fits on screen.",
            Action::Validate => {
                "Check the map: can bots reach every spawn, pickup and gadget, and is any spawn in lava or water? Click a problem to fly to it."
            }
            Action::TestPlay => {
                "Play this map against bots right now, even before saving. Leave the match to come back here."
            }
            Action::Publish => {
                "Add this map to Quick Match. It must pass Validate first, and is only written if the game can load it."
            }
            Action::Help => "How the editor works.",
            Action::Back => {
                "Leave the editor. Esc first cancels a shape you are drawing, then the tool."
            }
        }
    }

    /// Was this action's key pressed this frame? (Esc is handled apart: it
    /// steps back one level at a time.)
    fn pressed(self, tool: Tool) -> bool {
        let ctrl = held(KeyCode::LeftControl) || held(KeyCode::RightControl);
        let shift = held(KeyCode::LeftShift) || held(KeyCode::RightShift);
        let key = is_key_pressed;
        match self {
            Action::New => ctrl && key(KeyCode::N),
            Action::Open => ctrl && key(KeyCode::O),
            Action::Save => ctrl && key(KeyCode::S),
            Action::Settings => !ctrl && key(KeyCode::M),
            Action::Import => ctrl && key(KeyCode::I),
            Action::Export => ctrl && key(KeyCode::E),
            Action::Undo => ctrl && !shift && key(KeyCode::Z),
            Action::Redo => ctrl && (key(KeyCode::Y) || shift && key(KeyCode::Z)),
            Action::Delete => {
                key(KeyCode::Delete) || tool == Tool::Select && key(KeyCode::Backspace)
            }
            Action::Finish => tool == Tool::Draw && (key(KeyCode::Enter) || key(KeyCode::KpEnter)),
            Action::PopCorner => tool == Tool::Draw && key(KeyCode::Backspace),
            Action::Snap => !ctrl && (key(KeyCode::LeftShift) || key(KeyCode::RightShift)),
            Action::Grid => !ctrl && key(KeyCode::G),
            Action::Fit => !ctrl && key(KeyCode::F),
            Action::Validate => !ctrl && key(KeyCode::V),
            Action::TestPlay => !ctrl && key(KeyCode::T),
            Action::Publish => ctrl && key(KeyCode::P),
            Action::Help => key(KeyCode::F1),
            Action::Back => false,
        }
    }
}

/// A window over the editor that takes all input until it closes.
enum Modal {
    Help,
    NewMap(NewMap),
    Open(Vec<MapEntry>),
    /// Leaving would lose edits: save, discard or stay.
    Unsaved(Leave),
    TestPlay(TestSetup),
    Settings(MapSettings),
    /// Maps in the share folder to import.
    Import(Vec<files::SharedMap>),
    /// An imported map has the id of a map or draft already here.
    ImportClash(Doc),
    /// Publishing would replace one of the game's maps: go ahead?
    ConfirmPublish,
    /// A message with OK, and an Open Folder button when `folder` is set.
    Info {
        title: String,
        text: String,
        folder: Option<PathBuf>,
    },
}

/// Longest description a map can have.
const DESCRIPTION_MAX: usize = 140;
/// Map size limits, in tiles, and the step the size changes by.
const MAP_SIZE_MIN: Vec2 = vec2(24.0, 16.0);
const MAP_SIZE_MAX: Vec2 = vec2(200.0, 120.0);
const MAP_SIZE_STEP: f32 = 4.0;

/// The Map Settings window's choices, applied as one edit.
struct MapSettings {
    name: String,
    description: String,
    size: Vec2,
    /// Offers 2, 3 and 4 team matches.
    teams: [bool; 3],
    /// Every match type in the game: (id, display name, offered).
    modes: Vec<(String, String, bool)>,
    /// Which text box takes typing: 0 name, 1 description.
    focus: usize,
}

impl MapSettings {
    fn from_doc(doc: &Doc, data: &GameData) -> Self {
        let list = |key: &str| -> Vec<Value> {
            doc.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let counts = list("team_counts");
        let modes = list("modes");
        Self {
            name: doc.name().to_owned(),
            description: doc
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            size: doc.size(),
            teams: [2, 3, 4].map(|n| counts.contains(&json!(n))),
            modes: data
                .modes
                .iter()
                .map(|m| (m.id.clone(), m.display_name(), modes.contains(&json!(m.id))))
                .collect(),
            focus: 0,
        }
    }
}

/// The Test Play window's choices.
struct TestSetup {
    /// Index into the modes this map lists.
    mode: usize,
    /// Index into the team counts this map and mode allow (0 = no teams).
    teams: usize,
    /// Bots in free-for-all, or per team.
    bots: u32,
    /// Index into `Difficulty::ALL`.
    skill: usize,
}

impl Default for TestSetup {
    fn default() -> Self {
        Self {
            mode: 0,
            teams: 0,
            bots: 3,
            skill: 1,
        }
    }
}

/// Where the editor goes once unsaved edits are dealt with.
#[derive(Clone, PartialEq, Debug)]
enum Leave {
    Menu,
    NewMap,
    Open(String),
    /// A map file to import, from the Import window or dropped on the editor.
    Import(PathBuf),
}

/// The New Map window's choices.
struct NewMap {
    name: String,
    /// Indices into NEW_WIDTHS and NEW_HEIGHTS.
    width: usize,
    height: usize,
}

/// A map the Open window offers.
struct MapEntry {
    id: String,
    name: String,
    /// There is a saved draft (it opens instead of the game's version).
    draft: bool,
}

/// What is selected.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Sel {
    Thing(Layer, usize),
    Shape(usize),
    /// One corner of a shape.
    Point(usize, usize),
}

/// A left-button drag of the selection.
struct Drag {
    /// Where the mouse grabbed it, in world space.
    grab: Vec2,
    /// The selection's points when grabbed.
    before: Vec<Vec2>,
    /// Moved far enough to count as an edit (a plain click only selects).
    moved: bool,
}

/// The map as the game sees it, rebuilt after every edit.
struct Preview {
    map: MapDef,
    art: MapArt,
}

pub struct Editor {
    data: Arc<GameData>,
    doc: Doc,
    /// The assets folder: drafts live in `editor/`, finished maps in
    /// `data/maps/`.
    assets: PathBuf,
    /// A short message under the top bar ("Saved draft"), and how long it
    /// has been shown.
    note: Option<(String, f32)>,
    /// Last version of the map that loaded; drawn while the current one has
    /// errors, so the screen never goes blank mid-edit.
    preview: Option<Preview>,
    /// What the game's map loader says is wrong with the current version.
    problems: Vec<String>,
    cam: GameCamera,
    /// Mouse position in the previous frame, for drag panning.
    last_mouse: Vec2,
    grid: bool,
    time: f32,
    sel: Option<Sel>,
    drag: Option<Drag>,
    tool: Tool,
    /// Corners of the shape being drawn.
    pending: Vec<Vec2>,
    /// Kind (index into KINDS) and material for the next drawn shape.
    new_kind: usize,
    new_material: usize,
    /// Points land on the grid.
    snap: bool,
    modal: Option<Modal>,
    /// Set when the editor should close at the end of the frame.
    exit: bool,
    /// Window size last frame; the view refits when it changes.
    window: Vec2,
    /// The last Validate's results, shown in a panel until closed.
    report: Option<Report>,
    /// A test match to start at the end of the frame.
    start_test: Option<MatchConfig>,
    /// Just published: the reloaded game data, for the app.
    published: Option<Arc<GameData>>,
    /// A ring marking the problem the camera flew to, and its age.
    flash: Option<(Vec2, f32)>,
}

impl Editor {
    /// Open a map for editing: its draft if there is one, else the map the
    /// game plays.
    pub fn open(data: Arc<GameData>, assets: PathBuf, id: &str) -> Result<Self, String> {
        let doc = read_doc(&assets, id)?;
        let mut ed = Self {
            data,
            doc,
            assets,
            note: None,
            preview: None,
            problems: Vec::new(),
            cam: GameCamera::new(Vec2::ZERO),
            last_mouse: mouse_position().into(),
            grid: true,
            time: 0.0,
            sel: None,
            drag: None,
            tool: Tool::Select,
            pending: Vec::new(),
            new_kind: 0,
            new_material: 0,
            snap: true,
            modal: None,
            exit: false,
            window: vec2(screen_width(), screen_height()),
            report: None,
            flash: None,
            start_test: None,
            published: None,
        };
        ed.rebuild();
        ed.fit();
        ed.note = Some((
            "Instructions (F1) explains how the editor works".into(),
            0.0,
        ));
        Ok(ed)
    }

    /// Swap in another map and start fresh on it.
    fn replace(&mut self, doc: Doc) {
        self.doc = doc;
        self.preview = None;
        self.sel = None;
        self.drag = None;
        self.tool = Tool::Select;
        self.pending.clear();
        self.report = None;
        self.flash = None;
        self.rebuild();
        self.fit();
    }

    /// Screenshot hook: open a window (help, new, open or unsaved), pick a
    /// tool (draw), run Validate (validate; broken adds a problem first),
    /// test-play (test opens the window, testplay starts a match), export,
    /// importlist, import:<path> or publish. Comma-separate several.
    pub fn debug_show(&mut self, what: &str) {
        // Several steps, comma separated, run in order.
        if what.contains(',') {
            for step in what.split(',') {
                self.debug_show(step);
            }
            return;
        }
        match what {
            "help" => self.modal = Some(Modal::Help),
            "new" => self.go(Leave::NewMap),
            "open" => self.modal = Some(Modal::Open(self.map_list())),
            "unsaved" => self.modal = Some(Modal::Unsaved(Leave::Menu)),
            "draw" => self.set_tool(Tool::Draw),
            "validate" => self.validate(),
            "test" => self.modal = Some(Modal::TestPlay(TestSetup::default())),
            "export" => self.export(),
            "settings" => self.act(Action::Settings),
            "importlist" => self.act(Action::Import),
            "publish" => self.try_publish(),
            w if w.starts_with("import:") => self.leave(Leave::Import(PathBuf::from(&w[7..]))),
            "testplay" => self.start_test = Some(self.test_config(&TestSetup::default())),
            // A ledge high in the sky with a health pack bots can't reach.
            "broken" => {
                self.doc.checkpoint();
                let ledge = [
                    vec2(2.0, 3.0),
                    vec2(6.0, 3.0),
                    vec2(6.0, 4.0),
                    vec2(2.0, 4.0),
                ];
                self.doc.add_shape("solid", "rock", &ledge);
                let fields = json!({"type": "health", "respawn_s": 25});
                self.doc.add_thing(Layer::Pickups, vec2(4.0, 3.0), fields);
                self.rebuild();
                self.validate();
            }
            _ => {}
        }
    }

    /// Re-run the game's map loader on the document.
    fn rebuild(&mut self) {
        let weapons: Vec<&str> = self.data.weapons.iter().map(|w| w.id.as_str()).collect();
        let modes: Vec<&str> = self.data.modes.iter().map(|m| m.id.as_str()).collect();
        let id = self.doc.id().to_owned();
        match parse_map(
            &format!("{id}.json"),
            &id,
            &self.doc.to_json(),
            &weapons,
            &modes,
        ) {
            Ok(map) => {
                self.problems.clear();
                self.preview = Some(Preview {
                    art: MapArt::new(&map),
                    map,
                });
            }
            Err(errs) => self.problems = errs.into_iter().map(|e| e.message).collect(),
        }
        if let Some(r) = &mut self.report {
            r.mark_stale();
        }
    }

    /// Save the map as it is, problems and all, to its draft file.
    fn save_draft(&mut self) {
        let path = draft_path(&self.assets, self.doc.id());
        let result = std::fs::create_dir_all(path.parent().expect("draft folder"))
            .and_then(|_| std::fs::write(&path, self.doc.to_json()));
        let msg = match result {
            Ok(()) => {
                self.doc.dirty = false;
                format!("Saved draft to {}", path.display())
            }
            Err(e) => format!("Could not save {}: {e}", path.display()),
        };
        self.note = Some((msg, 0.0));
    }

    /// Save a copy to the share folder and say where.
    fn export(&mut self) {
        let dir = files::share_dir();
        match files::export(&dir, self.doc.id(), &self.doc.to_json()) {
            Ok(path) => {
                let mut text = format!(
                    "Saved a copy to {}. Send this file to share the map: Import Map (Ctrl+I) in another copy of the game opens it.",
                    path.display()
                );
                if !self.problems.is_empty() {
                    text.push_str(
                        " Note: the map still has problems to fix before it can be played.",
                    );
                }
                self.modal = Some(Modal::Info {
                    title: "Map Exported".into(),
                    text,
                    folder: Some(dir),
                });
            }
            Err(e) => self.info("Export Failed", e),
        }
    }

    /// Publish after a clean Validate; ask first if it replaces one of the
    /// game's maps.
    fn try_publish(&mut self) {
        self.validate();
        if !self.report.as_ref().is_some_and(|r| r.passed()) {
            self.say("Publish needs a clean Validate: fix the errors below first");
        } else if self.data.maps.iter().any(|m| m.id == self.doc.id()) {
            self.modal = Some(Modal::ConfirmPublish);
        } else {
            self.publish();
        }
    }

    /// Write the map into the game's map list (undone if the game could not
    /// load it), then drop the draft, which now matches.
    fn publish(&mut self) {
        match files::publish(&self.assets, self.doc.id(), &self.doc.to_json()) {
            Ok(data) => {
                let _ = std::fs::remove_file(draft_path(&self.assets, self.doc.id()));
                self.doc.dirty = false;
                let data = Arc::new(data);
                self.data = data.clone();
                self.published = Some(data);
                self.say(format!(
                    "Published {}: it is in Quick Match now",
                    self.doc.name()
                ));
            }
            Err(e) => self.info("Not Published", e),
        }
    }

    /// Read a shared map file. If a map or draft here has its id, ask what
    /// to do; otherwise open it.
    fn import(&mut self, path: &std::path::Path) {
        let doc = std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read {}: {e}", path.display()))
            .and_then(|t| Doc::from_json(&t));
        let mut doc = match doc {
            Ok(doc) => doc,
            Err(e) => return self.info("Can't Import", e),
        };
        // Ids name files: keep them plain.
        let id = slug(if doc.id().is_empty() {
            doc.name()
        } else {
            doc.id()
        });
        if id != doc.id() {
            doc.set("id", json!(id));
        }
        let taken =
            self.data.maps.iter().any(|m| m.id == id) || draft_path(&self.assets, &id).exists();
        if taken {
            self.modal = Some(Modal::ImportClash(doc));
        } else {
            self.accept_import(doc);
        }
    }

    /// Open an imported map as unsaved work.
    fn accept_import(&mut self, mut doc: Doc) {
        doc.dirty = true;
        let name = doc.name().to_owned();
        self.replace(doc);
        self.say(format!("Imported {name}: Save Draft (Ctrl+S) to keep it"));
    }

    /// Import under a fresh id, with "(copy)" after the name.
    fn import_copy(&mut self, mut doc: Doc) {
        let name = format!("{} (copy)", doc.name());
        let id = self.new_id(&name);
        doc.set("id", json!(id));
        doc.set("name", json!(name));
        self.accept_import(doc);
    }

    /// A message window with OK.
    fn info(&mut self, title: &str, text: impl Into<String>) {
        self.modal = Some(Modal::Info {
            title: title.into(),
            text: text.into(),
            folder: None,
        });
    }

    /// A match on the map as it is now (even unsaved): the game data gets a
    /// copy of it in place of any version with the same id.
    pub fn test_session(&self, cfg: MatchConfig) -> Result<Session, String> {
        let preview = self
            .preview
            .as_ref()
            .ok_or("The map has problems: fix them to test-play")?;
        let mut data = (*self.data).clone();
        data.maps.retain(|m| m.id != preview.map.id);
        data.maps.push(preview.map.clone());
        let cfg = MatchConfig {
            map_id: preview.map.id.clone(),
            ..cfg
        };
        Session::new(Arc::new(data), cfg)
    }

    /// Show a short message under the top bar.
    pub fn say(&mut self, msg: impl Into<String>) {
        self.note = Some((msg.into(), 0.0));
    }

    /// Show the whole map, clear of the top bar and the toolbar.
    fn fit(&mut self) {
        let size = self.doc.size();
        let s = ui::scale();
        let (left, right) = (200.0 * s, 240.0 * s);
        let top = self.bar_height();
        let bottom = 24.0 * s;
        let room = vec2(
            screen_width() - left - right,
            screen_height() - top - bottom,
        );
        self.cam.ppt = (room.x / (size.x + 1.0))
            .min(room.y / (size.y + 1.0))
            .clamp(MIN_PPT, MAX_PPT);
        // Centre the map in the free area, not the whole screen.
        self.cam.pos = size * 0.5 - vec2(left - right, top - bottom) * 0.5 / self.cam.ppt;
    }

    fn bar_height(&self) -> f32 {
        34.0 * ui::scale()
    }

    pub fn frame(&mut self, dt: f32, ui: &mut Ui) -> Outcome {
        self.time += dt;
        self.tick_flash(dt);
        if let Some((_, age)) = &mut self.note {
            *age += dt;
            if *age > NOTE_S {
                self.note = None;
            }
        }
        let window = vec2(screen_width(), screen_height());
        if window != self.window {
            self.window = window;
            self.fit();
        }
        let mouse: Vec2 = mouse_position().into();
        // Read every frame: macroquad forgets dropped files at frame end.
        let dropped = get_dropped_files();
        // A window opened this frame waits for the next one before it takes
        // input, so the click that opened it does not land in it.
        let live = self.modal.is_none();
        if live {
            if is_key_pressed(KeyCode::Escape) {
                // Step back one level: the shape being drawn, then the tool,
                // then the Validate panel, then the editor.
                if !self.pending.is_empty() {
                    self.pending.clear();
                } else if self.tool != Tool::Select {
                    self.tool = Tool::Select;
                } else if self.report.is_some() {
                    self.report = None;
                } else {
                    self.leave(Leave::Menu);
                }
            }
            // A map file dropped on the window imports it.
            if let Some(path) = dropped.iter().filter_map(|f| f.path.clone()).find(|p| {
                p.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("json"))
            }) {
                self.leave(Leave::Import(path));
            } else if !dropped.is_empty() {
                self.say("Only map files (.json) can be dropped here");
            }
            self.tool_keys();
            for a in ACTIONS.into_iter().flatten() {
                if self.modal.is_none() && a.pressed(self.tool) && self.enabled(*a) {
                    self.act(*a);
                }
            }
            self.camera_controls(mouse, dt);
            let over_ui = self.over_ui(mouse);
            match self.tool {
                Tool::Select => {
                    if !over_ui || self.drag.is_some() {
                        self.select_and_drag(mouse);
                    }
                }
                Tool::Draw => self.draw_tool(mouse, over_ui),
                Tool::Place(k) => {
                    if !over_ui && is_mouse_button_pressed(MouseButton::Left) {
                        self.place(k, mouse);
                    }
                }
            }
        }
        self.last_mouse = mouse;
        self.draw(mouse);
        let was_open = !live;
        let live = live && self.modal.is_none();
        self.toolbar(ui, live);
        self.actions(ui, live);
        if live {
            self.properties(ui);
        }
        self.report_panel(ui, live);
        if self.modal.is_some() {
            self.modal_frame(ui, was_open);
        }
        // Typed letters pile up until read; only the name box reads them.
        while get_char_pressed().is_some() {}
        if std::mem::take(&mut self.exit) {
            return Outcome::Menu;
        }
        if let Some(cfg) = self.start_test.take() {
            return Outcome::TestPlay(cfg);
        }
        if let Some(data) = self.published.take() {
            return Outcome::Published(data, self.doc.id().to_owned());
        }
        Outcome::Continue
    }

    fn enabled(&self, a: Action) -> bool {
        match a {
            Action::TestPlay | Action::Publish => {
                self.preview.is_some() && self.problems.is_empty()
            }
            Action::Undo => self.doc.can_undo(),
            Action::Redo => self.doc.can_redo(),
            Action::Delete => self.tool == Tool::Select && self.sel.is_some(),
            Action::Finish => self.tool == Tool::Draw && self.pending.len() >= 3,
            Action::PopCorner => self.tool == Tool::Draw && !self.pending.is_empty(),
            _ => true,
        }
    }

    fn act(&mut self, a: Action) {
        match a {
            Action::New => self.leave(Leave::NewMap),
            Action::Open => self.modal = Some(Modal::Open(self.map_list())),
            Action::Save => self.save_draft(),
            Action::Settings => {
                self.modal = Some(Modal::Settings(MapSettings::from_doc(
                    &self.doc, &self.data,
                )))
            }
            Action::Import => {
                let dir = files::share_dir();
                self.modal = Some(Modal::Import(files::list_shared(&dir)));
            }
            Action::Export => self.export(),
            Action::Publish => self.try_publish(),
            Action::Undo | Action::Redo => {
                let done = if a == Action::Undo {
                    self.doc.undo()
                } else {
                    self.doc.redo()
                };
                if done {
                    // Indices may point past the end after undo.
                    self.sel = None;
                    self.drag = None;
                    self.rebuild();
                }
            }
            Action::Delete => self.delete_selected(),
            Action::Finish => self.finish_shape(),
            Action::PopCorner => {
                self.pending.pop();
            }
            Action::Snap => {
                self.snap = !self.snap;
                let msg = if self.snap {
                    "Snap Grid on: points land on half tiles"
                } else {
                    "Snap Grid off: points go exactly where you click"
                };
                self.note = Some((msg.into(), 0.0));
            }
            Action::Grid => self.grid = !self.grid,
            Action::Fit => self.fit(),
            Action::Validate => self.validate(),
            Action::TestPlay => self.modal = Some(Modal::TestPlay(TestSetup::default())),
            Action::Help => self.modal = Some(Modal::Help),
            Action::Back => self.leave(Leave::Menu),
        }
    }

    /// Go somewhere else, asking first if there are unsaved edits.
    fn leave(&mut self, to: Leave) {
        if self.doc.dirty {
            self.modal = Some(Modal::Unsaved(to));
        } else {
            self.go(to);
        }
    }

    fn go(&mut self, to: Leave) {
        match to {
            Leave::Menu => self.exit = true,
            Leave::NewMap => {
                self.modal = Some(Modal::NewMap(NewMap {
                    name: String::new(),
                    width: 2,
                    height: 2,
                }))
            }
            Leave::Open(id) => match read_doc(&self.assets, &id) {
                Ok(doc) => self.replace(doc),
                Err(e) => self.note = Some((e, 0.0)),
            },
            Leave::Import(path) => self.import(&path),
        }
    }

    /// The game's maps, then drafts of maps the game does not have yet.
    fn map_list(&self) -> Vec<MapEntry> {
        let mut list: Vec<MapEntry> = self
            .data
            .maps
            .iter()
            .map(|m| MapEntry {
                id: m.id.clone(),
                name: m.name.clone(),
                draft: draft_path(&self.assets, &m.id).exists(),
            })
            .collect();
        let mut drafts: Vec<MapEntry> = std::fs::read_dir(self.assets.join("editor"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                let id = path.file_stem()?.to_str()?.to_owned();
                if path.extension()? != "json" || self.data.maps.iter().any(|m| m.id == id) {
                    return None;
                }
                let text = std::fs::read_to_string(&path).ok()?;
                let v: Value = serde_json::from_str(&text).ok()?;
                let name = v
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_owned();
                Some(MapEntry {
                    id,
                    name,
                    draft: true,
                })
            })
            .collect();
        drafts.sort_by(|a, b| a.name.cmp(&b.name));
        list.extend(drafts);
        list
    }

    /// An id for a new map from its name, not used by any map or draft.
    fn new_id(&self, name: &str) -> String {
        let base = slug(name);
        let taken = |id: &str| {
            self.data.maps.iter().any(|m| m.id == id) || draft_path(&self.assets, id).exists()
        };
        let mut id = base.clone();
        let mut n = 2;
        while taken(&id) {
            id = format!("{base}_{n}");
            n += 1;
        }
        id
    }

    /// Number keys pick tools: 1 select, 2 draw, 3 to 0 the place tools.
    fn tool_keys(&mut self) {
        const KEYS: [KeyCode; 10] = [
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
        for (i, key) in KEYS.into_iter().enumerate() {
            if is_key_pressed(key) {
                self.set_tool(tool_at(i));
            }
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.pending.clear();
        self.drag = None;
        if tool != Tool::Select {
            self.sel = None;
        }
    }

    /// The screen rects of the tool buttons (left).
    fn toolbar_rects(&self) -> Vec<Rect> {
        let s = ui::scale();
        let (w, h, gap) = (180.0 * s, 26.0 * s, 4.0 * s);
        let top = self.bar_height() + 10.0 * s;
        (0..PLACE.len() + 2)
            .map(|i| Rect::new(10.0 * s, top + i as f32 * (h + gap), w, h))
            .collect()
    }

    /// The action buttons (right), in groups.
    fn action_rects(&self) -> Vec<(Action, Rect)> {
        let s = ui::scale();
        let (w, h, gap) = (220.0 * s, 24.0 * s, 3.0 * s);
        let x = screen_width() - 10.0 * s - w;
        let mut y = self.bar_height() + 10.0 * s;
        let mut out = Vec::new();
        for group in ACTIONS {
            for &a in group {
                out.push((a, Rect::new(x, y, w, h)));
                y += h + gap;
            }
            y += 8.0 * s;
        }
        out
    }

    fn panel_rect(&self, rows: usize) -> Rect {
        let s = ui::scale();
        let h = (rows as f32 * 30.0 + 40.0) * s;
        Rect::new(10.0 * s, screen_height() - 30.0 * s - h, 300.0 * s, h)
    }

    fn over_ui(&self, mouse: Vec2) -> bool {
        mouse.y <= self.bar_height()
            || self.toolbar_rects().iter().any(|r| r.contains(mouse))
            || self.action_rects().iter().any(|(_, r)| r.contains(mouse))
            || {
                let rows = self.property_rows();
                rows > 0 && self.panel_rect(rows).contains(mouse)
            }
            || self.report_rect().is_some_and(|r| r.contains(mouse))
    }

    fn toolbar(&mut self, ui: &mut Ui, live: bool) {
        for (i, r) in self.toolbar_rects().into_iter().enumerate() {
            let tool = tool_at(i);
            let key = (i + 1) % 10;
            let label = format!("{} ({key})", tool_name(tool));
            let id = format!("ed_tool{i}");
            if ui.button(&id, &label, r, self.tool == tool, live) {
                self.set_tool(tool);
            }
            if live {
                ui.hint(&id, r, TOOL_TIPS[i]);
            }
        }
    }

    fn actions(&mut self, ui: &mut Ui, live: bool) {
        for (a, r) in self.action_rects() {
            let id = format!("ed_{a:?}");
            let enabled = live && self.enabled(a);
            let clicked = match a {
                Action::Snap => ui.toggle(&id, a.label(), r, self.snap, live),
                Action::Grid => ui.toggle(&id, a.label(), r, self.grid, live),
                _ => ui.button(&id, a.label(), r, false, enabled),
            };
            if live {
                ui.hint(&id, r, a.tip());
            }
            if clicked && self.modal.is_none() {
                self.act(a);
            }
        }
    }

    /// Draw tool: left click adds a corner, clicking the first corner or
    /// Enter closes the shape, Backspace takes back the last corner.
    fn draw_tool(&mut self, mouse: Vec2, over_ui: bool) {
        let world = self.cam.screen_to_world(mouse);
        let at = self.snapped(world);
        if !over_ui && is_mouse_button_pressed(MouseButton::Left) {
            let closes = self.pending.len() >= 3
                && self.pending[0].distance(world) * self.cam.ppt <= HANDLE_PX;
            if closes {
                self.finish_shape();
            } else if self.pending.last() != Some(&at) {
                self.pending.push(at);
            }
        }
    }

    /// `p` on the grid, if Snap Grid is on.
    fn snapped(&self, p: Vec2) -> Vec2 {
        if self.snap { snap(p) } else { p }
    }

    fn finish_shape(&mut self) {
        if self.pending.len() < 3 {
            self.note = Some(("A shape needs at least 3 corners".into(), 0.0));
            return;
        }
        let pts = std::mem::take(&mut self.pending);
        self.doc.checkpoint();
        let i = self
            .doc
            .add_shape(KINDS[self.new_kind].0, MATERIALS[self.new_material], &pts);
        self.rebuild();
        self.tool = Tool::Select;
        self.sel = Some(Sel::Shape(i));
    }

    /// Put down a new thing at the cursor, standing on the floor below it.
    fn place(&mut self, k: usize, mouse: Vec2) {
        let (_, layer, fields) = PLACE[k];
        let mut fields: Value = serde_json::from_str(fields).expect("PLACE fields are JSON");
        if fields.get("type").and_then(Value::as_str) == Some("weapon") {
            let w = self
                .data
                .weapons
                .iter()
                .find(|w| w.category != crate::data::Category::Starter);
            fields["weapon_id"] = json!(w.map_or("", |w| w.id.as_str()));
        }
        let at = self.snapped(self.cam.screen_to_world(mouse));
        let at = self.floor_below(at);
        self.doc.checkpoint();
        let i = self.doc.add_thing(layer, at, fields);
        self.sel = Some(Sel::Thing(layer, i));
        self.rebuild();
    }

    /// The floor under `p` (within reach), or `p` itself if there is none.
    fn floor_below(&self, p: Vec2) -> Vec2 {
        let Some(preview) = &self.preview else {
            return p;
        };
        preview
            .map
            .terrain
            .cast(p - vec2(0.0, 0.01), vec2(0.0, 1.0), FLOOR_REACH, |e| {
                e.walkable()
            })
            .map_or(p, |(t, _)| vec2(p.x, p.y - 0.01 + t))
    }

    fn select_and_drag(&mut self, mouse: Vec2) {
        let world = self.cam.screen_to_world(mouse);
        if is_mouse_button_pressed(MouseButton::Left) && mouse.y > self.bar_height() {
            self.sel = self.hit(world);
            self.drag = self.sel.map(|sel| Drag {
                grab: world,
                before: self.sel_points(sel),
                moved: false,
            });
        }
        if is_mouse_button_released(MouseButton::Left) {
            if self.drag.take().is_some_and(|d| d.moved) {
                self.rebuild();
            }
            return;
        }
        let (Some(sel), Some(mut drag)) = (self.sel, self.drag.take()) else {
            return;
        };
        let delta = world - drag.grab;
        if !drag.moved && delta.length() * self.cam.ppt >= DRAG_START_PX {
            self.doc.checkpoint();
            drag.moved = true;
        }
        if drag.moved {
            // Snap the first point to the grid and move the rest with it, so
            // a shape keeps its form.
            let anchor = self.snapped(drag.before[0] + delta);
            let d = anchor - drag.before[0];
            let moved: Vec<Vec2> = drag.before.iter().map(|&p| p + d).collect();
            match sel {
                Sel::Thing(layer, i) => self.doc.set_thing_pos(layer, i, moved[0]),
                Sel::Shape(i) => self.doc.set_shape_points(i, &moved),
                Sel::Point(i, j) => {
                    let mut pts = self.doc.shape_points(i);
                    pts[j] = moved[0];
                    self.doc.set_shape_points(i, &pts);
                }
            }
        }
        self.drag = Some(drag);
    }

    /// The points a selection moves.
    fn sel_points(&self, sel: Sel) -> Vec<Vec2> {
        match sel {
            Sel::Thing(layer, i) => vec![self.doc.thing_pos(layer, i)],
            Sel::Shape(i) => self.doc.shape_points(i),
            Sel::Point(i, j) => vec![self.doc.shape_points(i)[j]],
        }
    }

    /// What is under the cursor: a corner of the selected shape first, then
    /// markers, then shapes (the one drawn last wins).
    fn hit(&self, world: Vec2) -> Option<Sel> {
        if let Some(Sel::Shape(i) | Sel::Point(i, _)) = self.sel {
            let reach = HANDLE_PX / self.cam.ppt;
            let pts = self.doc.shape_points(i);
            if let Some(j) = pts.iter().position(|p| p.distance(world) <= reach) {
                return Some(Sel::Point(i, j));
            }
        }
        for layer in Layer::ALL.into_iter().rev() {
            for i in (0..self.doc.count(layer)).rev() {
                if marker_rect(layer, self.doc.thing_pos(layer, i)).contains(world) {
                    return Some(Sel::Thing(layer, i));
                }
            }
        }
        (0..self.doc.shape_count())
            .rev()
            .find(|&i| inside(&self.doc.shape_points(i), world))
            .map(Sel::Shape)
    }

    fn delete_selected(&mut self) {
        let Some(sel) = self.sel else {
            return;
        };
        match sel {
            Sel::Thing(layer, i) => {
                self.doc.checkpoint();
                self.doc.remove_thing(layer, i);
            }
            Sel::Shape(i) => {
                self.doc.checkpoint();
                self.doc.remove_shape(i);
            }
            Sel::Point(i, j) => {
                let mut pts = self.doc.shape_points(i);
                if pts.len() <= 3 {
                    self.note = Some(("A shape needs at least 3 corners".into(), 0.0));
                    return;
                }
                self.doc.checkpoint();
                pts.remove(j);
                self.doc.set_shape_points(i, &pts);
            }
        }
        self.sel = None;
        self.drag = None;
        self.rebuild();
    }

    /// How many rows the properties panel has right now (0 hides it).
    fn property_rows(&self) -> usize {
        match (self.tool, self.sel) {
            (Tool::Draw, _) => 2,
            (Tool::Select, Some(Sel::Shape(_) | Sel::Point(..))) => 2,
            (Tool::Select, Some(Sel::Thing(layer, i))) => {
                thing_props(self.doc.thing(layer, i), layer).len()
            }
            _ => 0,
        }
    }

    /// Settings for the selected shape or thing, or for the next shape drawn.
    fn properties(&mut self, ui: &mut Ui) {
        let rows = self.property_rows();
        if rows == 0 {
            return;
        }
        let s = ui::scale();
        let r = self.panel_rect(rows);
        ui::panel(r, None);
        let row = |n: usize| {
            Rect::new(
                r.x + 8.0 * s,
                r.y + (34.0 + n as f32 * 30.0) * s,
                r.w - 16.0 * s,
                26.0 * s,
            )
        };
        let title_y = r.y + 22.0 * s;
        match (self.tool, self.sel) {
            (Tool::Draw, _) => {
                ui::text_left("New shape", r.x + 12.0 * s, title_y, 16.0 * s, ACCENT);
                let d = ui.stepper(
                    "ed_kind",
                    "Kind",
                    KINDS[self.new_kind].0,
                    row(0),
                    false,
                    true,
                );
                if d != 0 {
                    self.new_kind = step(self.new_kind, d, KINDS.len());
                    let m = KINDS[self.new_kind].1;
                    self.new_material = MATERIALS.iter().position(|&x| x == m).unwrap_or(0);
                }
                let d = ui.stepper(
                    "ed_mat",
                    "Material",
                    MATERIALS[self.new_material],
                    row(1),
                    false,
                    true,
                );
                if d != 0 {
                    self.new_material = step(self.new_material, d, MATERIALS.len());
                }
            }
            (_, Some(Sel::Shape(i) | Sel::Point(i, _))) => {
                ui::text_left(
                    &format!("Shape {}", i + 1),
                    r.x + 12.0 * s,
                    title_y,
                    16.0 * s,
                    ACCENT,
                );
                let shape = self.doc.shape(i).clone();
                let kind = shape
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("solid")
                    .to_owned();
                let mat = shape
                    .get("material")
                    .and_then(Value::as_str)
                    .unwrap_or("rock")
                    .to_owned();
                let d = ui.stepper("ed_kind", "Kind", &kind, row(0), false, true);
                if d != 0 {
                    let k = KINDS.iter().position(|(n, _)| *n == kind).unwrap_or(0);
                    let k = step(k, d, KINDS.len());
                    self.doc.checkpoint();
                    self.doc.set_shape_field(i, "kind", Some(json!(KINDS[k].0)));
                    self.rebuild();
                }
                let d = ui.stepper("ed_mat", "Material", &mat, row(1), false, true);
                if d != 0 {
                    let m = MATERIALS.iter().position(|n| *n == mat).unwrap_or(0);
                    let m = step(m, d, MATERIALS.len());
                    self.doc.checkpoint();
                    self.doc
                        .set_shape_field(i, "material", Some(json!(MATERIALS[m])));
                    self.rebuild();
                }
            }
            (_, Some(Sel::Thing(layer, i))) => {
                let t = self.doc.thing(layer, i).clone();
                let title = match layer {
                    Layer::Spawns => "Spawn".to_owned(),
                    Layer::Pickups => tool_name(Tool::Place(match pickup_kind(&t) {
                        Some(PickupKind::Health) => 1,
                        Some(PickupKind::Ammo) => 2,
                        _ => 3,
                    }))
                    .to_owned(),
                    Layer::Objects => object_kind(&t).map_or("?", object_name).to_owned(),
                };
                ui::text_left(&title, r.x + 12.0 * s, title_y, 16.0 * s, ACCENT);
                for (n, prop) in thing_props(&t, layer).into_iter().enumerate() {
                    let value = prop.show(&t, &self.data);
                    let d = ui.stepper(
                        &format!("ed_prop{n}"),
                        prop.label(),
                        &value,
                        row(n),
                        false,
                        true,
                    );
                    if d != 0 {
                        let (field, v) = prop.step(&t, d, &self.data);
                        self.doc.checkpoint();
                        self.doc.set_thing_field(layer, i, field, v);
                        self.rebuild();
                    }
                }
            }
            _ => {}
        }
    }

    fn camera_controls(&mut self, mouse: Vec2, dt: f32) {
        if is_mouse_button_down(MouseButton::Right) || is_mouse_button_down(MouseButton::Middle) {
            self.cam.pos -= (mouse - self.last_mouse) / self.cam.ppt;
        }
        let mut pan = Vec2::ZERO;
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            pan.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            pan.x += 1.0;
        }
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            pan.y -= 1.0;
        }
        let ctrl = held(KeyCode::LeftControl) || held(KeyCode::RightControl);
        if is_key_down(KeyCode::S) && !ctrl || is_key_down(KeyCode::Down) {
            pan.y += 1.0;
        }
        self.cam.pos += pan * PAN_SPEED * dt / self.cam.ppt;
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            // Zoom around the cursor: the point under it stays put.
            let before = self.cam.screen_to_world(mouse);
            let k = if wheel > 0.0 { 1.15 } else { 1.0 / 1.15 };
            self.cam.ppt = (self.cam.ppt * k).clamp(MIN_PPT, MAX_PPT);
            self.cam.pos += before - self.cam.screen_to_world(mouse);
        }
        if is_key_pressed(KeyCode::F) {
            self.fit();
        }
        let size = self.doc.size();
        self.cam.pos = self.cam.pos.clamp(Vec2::ZERO, size);
    }

    fn draw(&self, mouse: Vec2) {
        clear_background(Color::new(0.05, 0.06, 0.08, 1.0));
        let size = self.doc.size();
        if let Some(p) = &self.preview {
            draw_sky(&p.map, self.cam.pos.x, self.cam.ppt, self.time);
        }
        use_camera(&self.cam.camera2d());
        // Dim everything outside the map.
        let out = Color::new(0.0, 0.0, 0.0, 0.55);
        let big = 1000.0;
        draw_rectangle(-big, -big, size.x + 2.0 * big, big, out);
        draw_rectangle(-big, size.y, size.x + 2.0 * big, big, out);
        draw_rectangle(-big, 0.0, big, size.y, out);
        draw_rectangle(size.x, 0.0, big, size.y, out);
        if let Some(p) = &self.preview {
            let lo = self.cam.screen_to_world(Vec2::ZERO);
            let hi = self
                .cam
                .screen_to_world(vec2(screen_width(), screen_height()));
            p.art.draw_back(&p.map, lo, hi, self.time);
            p.art.draw_front(&p.map, self.time);
        }
        if self.grid {
            self.draw_grid(size);
        }
        draw_rectangle_lines(0.0, 0.0, size.x, size.y, 0.08, ACCENT);
        self.draw_things();
        self.draw_selection();
        self.draw_pending(mouse);
        self.draw_flash();
        use_screen();
        self.draw_labels();
        self.draw_bar(mouse);
    }

    fn draw_grid(&self, size: Vec2) {
        // Thin lines every tile, brighter every 5; skip the fine lines when
        // zoomed far out.
        let w = 1.0 / self.cam.ppt;
        let fine = self.cam.ppt >= 14.0;
        for x in 0..=size.x as i32 {
            let major = x % 5 == 0;
            if major || fine {
                let a = if major { 0.22 } else { 0.08 };
                draw_line(
                    x as f32,
                    0.0,
                    x as f32,
                    size.y,
                    w,
                    Color::new(1.0, 1.0, 1.0, a),
                );
            }
        }
        for y in 0..=size.y as i32 {
            let major = y % 5 == 0;
            if major || fine {
                let a = if major { 0.22 } else { 0.08 };
                draw_line(
                    0.0,
                    y as f32,
                    size.x,
                    y as f32,
                    w,
                    Color::new(1.0, 1.0, 1.0, a),
                );
            }
        }
    }

    /// Spawns, pickups and gadgets as plain markers, in world space.
    fn draw_things(&self) {
        for i in 0..self.doc.count(Layer::Spawns) {
            let p = self.doc.thing_pos(Layer::Spawns, i);
            let color = spawn_team(self.doc.thing(Layer::Spawns, i))
                .map_or(WHITE, |t| TEAM_COLORS[t as usize % TEAM_COLORS.len()]);
            draw_rectangle(p.x - 0.35, p.y - 1.8, 0.7, 1.8, with_alpha(color, 0.35));
            draw_rectangle_lines(p.x - 0.35, p.y - 1.8, 0.7, 1.8, 0.08, color);
            draw_circle(p.x, p.y - 2.2, 0.3, color);
        }
        for i in 0..self.doc.count(Layer::Pickups) {
            let p = self.doc.thing_pos(Layer::Pickups, i);
            let color = match pickup_kind(self.doc.thing(Layer::Pickups, i)) {
                Some(PickupKind::Health) => Color::new(0.4, 0.95, 0.45, 1.0),
                Some(PickupKind::Ammo) => Color::new(1.0, 0.85, 0.35, 1.0),
                _ => Color::new(0.8, 0.6, 1.0, 1.0),
            };
            draw_rectangle(p.x - 0.4, p.y - 0.8, 0.8, 0.8, with_alpha(color, 0.5));
            draw_rectangle_lines(p.x - 0.4, p.y - 0.8, 0.8, 0.8, 0.07, color);
        }
        for i in 0..self.doc.count(Layer::Objects) {
            let t = self.doc.thing(Layer::Objects, i);
            let p = self.doc.thing_pos(Layer::Objects, i);
            match object_kind(t) {
                Some(ObjectKind::Thruster) => {
                    let h = t
                        .get("height_tiles")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(8.0) as f32;
                    let c = Color::new(1.0, 0.55, 0.2, 1.0);
                    draw_rectangle(p.x - 0.8, p.y - 0.3, 1.6, 0.3, c);
                    draw_line(p.x, p.y - 0.3, p.x, p.y - h, 0.06, with_alpha(c, 0.6));
                    draw_triangle(
                        vec2(p.x, p.y - h - 0.4),
                        vec2(p.x - 0.35, p.y - h + 0.1),
                        vec2(p.x + 0.35, p.y - h + 0.1),
                        c,
                    );
                }
                Some(ObjectKind::PowerupSpot) => {
                    draw_circle_lines(p.x, p.y - 0.8, 0.6, 0.08, Color::new(1.0, 0.9, 0.3, 1.0));
                }
                _ => {
                    let c = Color::new(0.4, 0.85, 1.0, 1.0);
                    draw_rectangle(p.x - 0.5, p.y - 1.0, 1.0, 1.0, with_alpha(c, 0.35));
                    draw_rectangle_lines(p.x - 0.5, p.y - 1.0, 1.0, 1.0, 0.07, c);
                }
            }
        }
    }

    /// The shape being drawn, with a line to where the next corner would go.
    fn draw_pending(&self, mouse: Vec2) {
        if self.tool != Tool::Draw {
            return;
        }
        let px = 1.0 / self.cam.ppt;
        let at = self.snapped(self.cam.screen_to_world(mouse));
        let color = Color::new(1.0, 0.9, 0.35, 1.0);
        for w in self.pending.windows(2) {
            draw_line(w[0].x, w[0].y, w[1].x, w[1].y, 2.0 * px, color);
        }
        if let Some(last) = self.pending.last() {
            draw_line(last.x, last.y, at.x, at.y, 1.5 * px, with_alpha(color, 0.6));
        }
        for p in &self.pending {
            draw_circle(p.x, p.y, 4.0 * px, color);
        }
        draw_circle_lines(at.x, at.y, 5.0 * px, 1.5 * px, color);
    }

    fn draw_selection(&self) {
        let Some(sel) = self.sel else {
            return;
        };
        let px = 1.0 / self.cam.ppt;
        match sel {
            Sel::Thing(layer, i) => {
                let r = marker_rect(layer, self.doc.thing_pos(layer, i));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0 * px, ACCENT);
            }
            Sel::Shape(i) | Sel::Point(i, _) => {
                let pts = self.doc.shape_points(i);
                for (k, a) in pts.iter().enumerate() {
                    let b = pts[(k + 1) % pts.len()];
                    draw_line(a.x, a.y, b.x, b.y, 2.0 * px, ACCENT);
                }
                let h = 4.0 * px;
                for (k, p) in pts.iter().enumerate() {
                    let picked = sel == Sel::Point(i, k);
                    let fill = if picked {
                        WHITE
                    } else {
                        Color::new(0.05, 0.1, 0.12, 1.0)
                    };
                    draw_rectangle(p.x - h, p.y - h, 2.0 * h, 2.0 * h, fill);
                    draw_rectangle_lines(p.x - h, p.y - h, 2.0 * h, 2.0 * h, 1.5 * px, ACCENT);
                }
            }
        }
    }

    /// Short names over the markers, in screen space so text stays sharp.
    fn draw_labels(&self) {
        if self.cam.ppt < LABEL_PPT {
            return;
        }
        let size = 13.0 * ui::scale();
        let color = Color::new(1.0, 1.0, 1.0, 0.8);
        for layer in Layer::ALL {
            for i in 0..self.doc.count(layer) {
                let t = self.doc.thing(layer, i);
                let label = match layer {
                    Layer::Spawns => match spawn_team(t) {
                        Some(n) => crate::sim::TEAM_NAMES[n as usize % 4].to_owned(),
                        None => "Spawn".into(),
                    },
                    Layer::Pickups => match pickup_kind(t) {
                        Some(PickupKind::Health) => "Health".into(),
                        Some(PickupKind::Ammo) => "Ammo".into(),
                        _ => t.get("weapon_id").and_then(|v| v.as_str()).map_or(
                            "Weapon".into(),
                            |id| {
                                self.data
                                    .weapons
                                    .iter()
                                    .find(|w| w.id == id)
                                    .map_or(id.to_owned(), |w| w.short_name())
                            },
                        ),
                    },
                    Layer::Objects => object_kind(t).map_or("?", object_name).to_owned(),
                };
                let lift = match layer {
                    Layer::Spawns => 2.7,
                    Layer::Pickups => 1.1,
                    Layer::Objects => 1.4,
                };
                let s = self
                    .cam
                    .world_to_screen(self.doc.thing_pos(layer, i) - vec2(0.0, lift));
                ui::text_center(&label, s.x, s.y, size, color);
            }
        }
    }

    /// Top bar: map name, cursor position, problems and key help.
    fn draw_bar(&self, mouse: Vec2) {
        let s = ui::scale();
        let h = self.bar_height();
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            h,
            Color::new(0.03, 0.07, 0.09, 0.92),
        );
        let name = self
            .preview
            .as_ref()
            .map_or_else(|| self.doc.id().to_owned(), |p| p.map.name.clone());
        let dirty = if self.doc.dirty { " *" } else { "" };
        let size = 18.0 * s;
        ui::text_left(
            &format!("Editing: {name}{dirty}"),
            14.0 * s,
            h * 0.68,
            size,
            WHITE,
        );
        let w = self.snapped(self.cam.screen_to_world(mouse));
        ui::text_center(
            &format!("x {:.1}  y {:.1}", w.x, w.y),
            screen_width() * 0.5,
            h * 0.68,
            size,
            Color::new(1.0, 1.0, 1.0, 0.7),
        );
        let (msg, color) = if self.problems.is_empty() {
            ("Map OK".to_owned(), Color::new(0.45, 0.95, 0.5, 1.0))
        } else {
            (
                format!("{} problem(s): {}", self.problems.len(), self.problems[0]),
                Color::new(1.0, 0.45, 0.4, 1.0),
            )
        };
        ui::text_right(&msg, screen_width() - 14.0 * s, h * 0.68, size, color);
        if let Some((msg, _)) = &self.note {
            ui::text_center(msg, screen_width() * 0.5, h + 24.0 * s, 16.0 * s, ACCENT);
        }
        ui::text_center(
            MOUSE_HINT[match self.tool {
                Tool::Select => 0,
                Tool::Draw => 1,
                Tool::Place(_) => 2,
            }],
            screen_width() * 0.5,
            screen_height() - 12.0 * s,
            14.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.55),
        );
    }
}

fn tool_at(i: usize) -> Tool {
    match i {
        0 => Tool::Select,
        1 => Tool::Draw,
        n => Tool::Place(n - 2),
    }
}

fn tool_name(t: Tool) -> &'static str {
    match t {
        Tool::Select => "Select",
        Tool::Draw => "Draw Shape",
        Tool::Place(k) => PLACE[k].0,
    }
}

/// Move an index by `d`, wrapping around `n`.
fn step(i: usize, d: i32, n: usize) -> usize {
    (i as i32 + d).rem_euclid(n as i32) as usize
}

/// An editable setting of a placed thing.
#[derive(Clone, Copy)]
enum Prop {
    Team,
    Weapon,
    Height,
    Shield,
    Powerup,
}

fn thing_props(t: &Value, layer: Layer) -> Vec<Prop> {
    match layer {
        Layer::Spawns => vec![Prop::Team],
        Layer::Pickups if pickup_kind(t) == Some(PickupKind::Weapon) => vec![Prop::Weapon],
        Layer::Pickups => vec![],
        Layer::Objects => match object_kind(t) {
            Some(ObjectKind::Thruster) => vec![Prop::Height],
            Some(ObjectKind::ShieldStation) => vec![Prop::Shield],
            Some(ObjectKind::PowerupSpot) => vec![Prop::Powerup],
            _ => vec![],
        },
    }
}

impl Prop {
    fn label(self) -> &'static str {
        match self {
            Prop::Team => "Team",
            Prop::Weapon => "Weapon",
            Prop::Height => "Height",
            Prop::Shield => "Shield",
            Prop::Powerup => "Powerup",
        }
    }

    fn show(self, t: &Value, data: &GameData) -> String {
        match self {
            Prop::Team => spawn_team(t).map_or("Any".into(), |n| {
                crate::sim::TEAM_NAMES[n as usize % 4].to_owned()
            }),
            Prop::Weapon => {
                let id = t.get("weapon_id").and_then(Value::as_str).unwrap_or("");
                data.weapons
                    .iter()
                    .find(|w| w.id == id)
                    .map_or(id.to_owned(), |w| w.short_name())
            }
            Prop::Height => format!(
                "{} tiles",
                t.get("height_tiles").and_then(Value::as_f64).unwrap_or(8.0)
            ),
            Prop::Shield => format!(
                "{}",
                t.get("shield").and_then(Value::as_f64).unwrap_or(50.0)
            ),
            Prop::Powerup => t
                .get("powerup")
                .and_then(Value::as_str)
                .and_then(|id| data.powerups.powerups.iter().find(|p| p.id == id))
                .map_or("Random".into(), |p| p.name.clone()),
        }
    }

    /// The field and its new value after one step left (-1) or right (+1).
    fn step(self, t: &Value, d: i32, data: &GameData) -> (&'static str, Option<Value>) {
        match self {
            Prop::Team => {
                // Any, then teams 0 to 3.
                let now = spawn_team(t).map_or(0, |n| n as usize + 1);
                let next = step(now, d, 5);
                ("team", (next > 0).then(|| json!(next - 1)))
            }
            Prop::Weapon => {
                let ids: Vec<&str> = data
                    .weapons
                    .iter()
                    .filter(|w| w.category != crate::data::Category::Starter)
                    .map(|w| w.id.as_str())
                    .collect();
                let id = t.get("weapon_id").and_then(Value::as_str).unwrap_or("");
                let now = ids.iter().position(|&x| x == id).unwrap_or(0);
                ("weapon_id", Some(json!(ids[step(now, d, ids.len())])))
            }
            Prop::Height => {
                let h = t.get("height_tiles").and_then(Value::as_f64).unwrap_or(8.0);
                ("height_tiles", Some(json!((h + d as f64).clamp(2.0, 40.0))))
            }
            Prop::Shield => {
                let now = t.get("shield").and_then(Value::as_f64).unwrap_or(50.0);
                ("shield", Some(json!(if now >= 100.0 { 50 } else { 100 })))
            }
            Prop::Powerup => {
                // Random, then each powerup.
                let ids: Vec<&str> = data
                    .powerups
                    .powerups
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect();
                let id = t.get("powerup").and_then(Value::as_str);
                let now = id
                    .and_then(|id| ids.iter().position(|&x| x == id))
                    .map_or(0, |n| n + 1);
                let next = step(now, d, ids.len() + 1);
                ("powerup", (next > 0).then(|| json!(ids[next - 1])))
            }
        }
    }
}

/// The clickable area of a marker whose feet are at `p`.
fn marker_rect(layer: Layer, p: Vec2) -> Rect {
    match layer {
        Layer::Spawns => Rect::new(p.x - 0.45, p.y - 2.55, 0.9, 2.55),
        Layer::Pickups => Rect::new(p.x - 0.45, p.y - 0.85, 0.9, 0.85),
        Layer::Objects => Rect::new(p.x - 0.85, p.y - 1.1, 1.7, 1.4),
    }
}

/// Is `p` inside the polygon (even-odd rule)?
fn inside(poly: &[Vec2], p: Vec2) -> bool {
    let mut odd = false;
    for (k, a) in poly.iter().enumerate() {
        let b = poly[(k + 1) % poly.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}

fn object_name(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Thruster => "Thruster",
        ObjectKind::WeaponStation => "Weapon Station",
        ObjectKind::ShieldStation => "Shield Station",
        ObjectKind::PowerupSpot => "Powerup",
    }
}

/// A file-name-safe id from a map name: lowercase letters and digits
/// joined by underscores ("new_map" if nothing is left).
fn slug(name: &str) -> String {
    let lower: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let id = lower
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if id.is_empty() { "new_map".into() } else { id }
}

/// A map's draft if there is one, else the version the game plays.
fn read_doc(assets: &std::path::Path, id: &str) -> Result<Doc, String> {
    let draft = draft_path(assets, id);
    let path = if draft.exists() {
        draft
    } else {
        assets.join("data/maps").join(format!("{id}.json"))
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    Doc::from_json(&text)
}

fn draft_path(assets: &std::path::Path, id: &str) -> PathBuf {
    assets.join("editor").join(format!("{id}.json"))
}

fn snap(p: Vec2) -> Vec2 {
    (p / SNAP).round() * SNAP
}

/// Down now, or pressed this frame: a quick tap of Ctrl+O can press and
/// release Ctrl before the frame that sees the O.
fn held(k: KeyCode) -> bool {
    is_key_down(k) || is_key_pressed(k)
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

fn spawn_team(t: &serde_json::Value) -> Option<u8> {
    t.get("team").and_then(|v| v.as_u64()).map(|n| n as u8)
}

fn pickup_kind(t: &serde_json::Value) -> Option<PickupKind> {
    serde_json::from_value(t.get("type")?.clone()).ok()
}

fn object_kind(t: &serde_json::Value) -> Option<ObjectKind> {
    serde_json::from_value(t.get("type")?.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_land_inside_shapes_of_any_form() {
        // An L shape: the notch must not count as inside.
        let l = [
            vec2(0.0, 0.0),
            vec2(4.0, 0.0),
            vec2(4.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 4.0),
            vec2(0.0, 4.0),
        ];
        assert!(inside(&l, vec2(0.5, 3.0)));
        assert!(inside(&l, vec2(3.0, 0.5)));
        assert!(!inside(&l, vec2(3.0, 3.0)));
        assert!(!inside(&l, vec2(-1.0, 0.5)));
    }

    #[test]
    fn map_names_become_safe_ids() {
        assert_eq!(slug("  Sky Harbor 2 "), "sky_harbor_2");
        assert_eq!(slug("Gyro's Lab -- Mk.II"), "gyro_s_lab_mk_ii");
        assert_eq!(slug("Café"), "caf");
        assert_eq!(slug("!!!"), "new_map");
    }

    #[test]
    fn snapping_rounds_to_half_tiles() {
        assert_eq!(snap(vec2(3.26, 7.74)), vec2(3.5, 7.5));
        assert_eq!(snap(vec2(-0.2, 0.24)), vec2(0.0, 0.0));
    }
}
