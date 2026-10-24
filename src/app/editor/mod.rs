//! The in-game map editor.

#[allow(dead_code)]
mod doc;

use super::ui::{self, ACCENT};
use crate::data::{GameData, MapDef, ObjectKind, PickupKind, parse_map};
use crate::render::{GameCamera, MapArt, TEAM_COLORS, draw_sky};
use doc::{Doc, Layer};
use macroquad::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

/// Grid step that points snap to, in tiles.
const SNAP: f32 = 0.5;
const MIN_PPT: f32 = 6.0;
const MAX_PPT: f32 = 96.0;
/// Keyboard pan speed, in screen pixels per second.
const PAN_SPEED: f32 = 900.0;
/// Zoom (pixels per tile) from which markers get their names.
const LABEL_PPT: f32 = 26.0;
/// How long a note like "Saved draft" stays up, in seconds.
const NOTE_S: f32 = 3.0;

pub enum Outcome {
    Continue,
    Menu,
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
}

impl Editor {
    /// Open a map for editing: its draft if there is one, else the map the
    /// game plays.
    pub fn open(data: Arc<GameData>, assets: PathBuf, id: &str) -> Result<Self, String> {
        let draft = draft_path(&assets, id);
        let path = if draft.exists() {
            draft
        } else {
            assets.join("data/maps").join(format!("{id}.json"))
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let doc = Doc::from_json(&text)?;
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
        };
        ed.rebuild();
        ed.fit();
        Ok(ed)
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

    /// Show the whole map.
    fn fit(&mut self) {
        let size = self.doc.size();
        let screen = vec2(screen_width(), screen_height() - self.bar_height());
        self.cam.ppt = (screen.x / (size.x + 2.0))
            .min(screen.y / (size.y + 2.0))
            .clamp(MIN_PPT, MAX_PPT);
        self.cam.pos = size * 0.5 - vec2(0.0, self.bar_height() * 0.5 / self.cam.ppt);
    }

    fn bar_height(&self) -> f32 {
        34.0 * ui::scale()
    }

    pub fn frame(&mut self, dt: f32) -> Outcome {
        self.time += dt;
        if let Some((_, age)) = &mut self.note {
            *age += dt;
            if *age > NOTE_S {
                self.note = None;
            }
        }
        let mouse: Vec2 = mouse_position().into();
        if is_key_pressed(KeyCode::Escape) {
            return Outcome::Menu;
        }
        self.camera_controls(mouse, dt);
        if is_key_pressed(KeyCode::G) {
            self.grid = !self.grid;
        }
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if ctrl && is_key_pressed(KeyCode::S) {
            self.save_draft();
        }
        if ctrl && is_key_pressed(KeyCode::Z) && self.doc.undo()
            || ctrl && is_key_pressed(KeyCode::Y) && self.doc.redo()
        {
            self.rebuild();
        }
        self.last_mouse = mouse;
        self.draw(mouse);
        Outcome::Continue
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
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
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
        set_camera(&self.cam.camera2d());
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
        set_default_camera();
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
        let w = snap(self.cam.screen_to_world(mouse));
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
            "Right-drag or WASD: move   Wheel: zoom   F: fit   G: grid   Ctrl+Z/Y: undo/redo   Ctrl+S: save draft   Esc: back",
            screen_width() * 0.5,
            screen_height() - 12.0 * s,
            14.0 * s,
            Color::new(1.0, 1.0, 1.0, 0.55),
        );
    }
}

fn object_name(k: ObjectKind) -> &'static str {
    match k {
        ObjectKind::Thruster => "Thruster",
        ObjectKind::WeaponStation => "Weapon Station",
        ObjectKind::ShieldStation => "Shield Station",
        ObjectKind::PowerupSpot => "Powerup",
    }
}

fn draft_path(assets: &std::path::Path, id: &str) -> PathBuf {
    assets.join("editor").join(format!("{id}.json"))
}

fn snap(p: Vec2) -> Vec2 {
    (p / SNAP).round() * SNAP
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
