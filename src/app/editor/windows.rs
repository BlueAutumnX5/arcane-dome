//! The editor's pop-up windows: they dim the editor and take all input
//! until closed.

use super::*;

impl Editor {
    /// The open window: dims the editor and takes all input. `input` is
    /// false on the frame it opened.
    pub(super) fn modal_frame(&mut self, ui: &mut Ui, input: bool) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
        let esc = input && is_key_pressed(KeyCode::Escape);
        let enter = input && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter));
        let keep = match modal {
            Modal::Help => {
                let close = self.help_window(ui, input);
                (!(close || esc || input && is_key_pressed(KeyCode::F1))).then_some(Modal::Help)
            }
            Modal::NewMap(mut nm) => match self.new_map_window(ui, &mut nm, input) {
                Some(true) => {
                    self.create_map(&nm);
                    None
                }
                Some(false) => None,
                None if esc => None,
                None if enter => {
                    self.create_map(&nm);
                    None
                }
                None => Some(Modal::NewMap(nm)),
            },
            Modal::Open(list) => match self.open_window(ui, &list, input) {
                Some(Some(id)) => {
                    self.leave(Leave::Open(id));
                    None
                }
                Some(None) => None,
                None if esc => None,
                None => Some(Modal::Open(list)),
            },
            Modal::Unsaved(to) => {
                let pick = self.unsaved_window(ui, &to, input);
                let pick = pick.or(if enter {
                    Some(0)
                } else if input && is_key_pressed(KeyCode::D) {
                    Some(1)
                } else if esc {
                    Some(2)
                } else {
                    None
                });
                match pick {
                    Some(0) => {
                        self.save_draft();
                        // Only leave if the save worked.
                        if !self.doc.dirty {
                            self.go(to);
                        }
                        None
                    }
                    Some(1) => {
                        self.go(to);
                        None
                    }
                    Some(_) => None,
                    None => Some(Modal::Unsaved(to)),
                }
            }
            Modal::TestPlay(mut setup) => match self.test_window(ui, &mut setup, input) {
                Some(true) => {
                    self.start_test = Some(self.test_config(&setup));
                    None
                }
                Some(false) => None,
                None if esc => None,
                None if enter => {
                    self.start_test = Some(self.test_config(&setup));
                    None
                }
                None => Some(Modal::TestPlay(setup)),
            },
        };
        // A choice above may have opened the next window (New Map after
        // Unsaved); that one wins.
        if self.modal.is_none() {
            self.modal = keep;
        }
    }

    /// A centred window frame of `w` x `h`.
    fn window(w: f32, h: f32, title: &str) -> Rect {
        let r = Rect::new(
            (screen_width() - w) / 2.0,
            (screen_height() - h) / 2.0,
            w,
            h,
        );
        // Darker than a plain panel, so the map does not show through.
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
        ui::panel(r, Some(title));
        r
    }

    /// The instructions. Returns true when Close is clicked.
    fn help_window(&self, ui: &mut Ui, input: bool) -> bool {
        let s = ui::scale();
        let w = (920.0 * s).min(screen_width() - 40.0 * s);
        let col_w = (w - 60.0 * s) / 2.0;
        let (head, body) = (18.0 * s, 15.0 * s);
        let line_h = body + 4.0 * s;
        // Lay out both columns first to size the window.
        let half = HELP_PAGE.len().div_ceil(2);
        let columns: Vec<Vec<(&str, Vec<String>)>> = HELP_PAGE
            .chunks(half)
            .map(|c| {
                c.iter()
                    .map(|(h, b)| (*h, ui::wrap(b, col_w, body)))
                    .collect()
            })
            .collect();
        let col_h = |c: &Vec<(&str, Vec<String>)>| {
            c.iter()
                .map(|(_, l)| head + 8.0 * s + l.len() as f32 * line_h + 12.0 * s)
                .sum::<f32>()
        };
        let content = columns.iter().map(col_h).fold(0.0, f32::max);
        let h = (content + 110.0 * s).min(screen_height() - 20.0 * s);
        let r = Self::window(w, h, "Map Editor Instructions");
        for (k, col) in columns.iter().enumerate() {
            let x = r.x + 20.0 * s + k as f32 * (col_w + 20.0 * s);
            let mut y = r.y + 52.0 * s;
            for (heading, lines) in col {
                ui::text_left(heading, x, y + head * 0.8, head, ACCENT);
                y += head + 8.0 * s;
                for line in lines {
                    ui::text_left(
                        line,
                        x,
                        y + body * 0.8,
                        body,
                        Color::new(1.0, 1.0, 1.0, 0.85),
                    );
                    y += line_h;
                }
                y += 12.0 * s;
            }
        }
        let b = Rect::new(
            r.x + r.w - 200.0 * s,
            r.y + r.h - 42.0 * s,
            180.0 * s,
            28.0 * s,
        );
        ui.button("ed_help_close", "Close (Esc)", b, false, input)
    }

    /// The New Map window. Returns Some(true) for Create, Some(false) for
    /// Cancel.
    fn new_map_window(&self, ui: &mut Ui, nm: &mut NewMap, input: bool) -> Option<bool> {
        let s = ui::scale();
        let r = Self::window(480.0 * s, 244.0 * s, "New Map");
        let x = r.x + 20.0 * s;
        let w = r.w - 40.0 * s;
        if input {
            while let Some(c) = get_char_pressed() {
                let ok = c.is_alphanumeric() || c == ' ' || c == '-' || c == '\'';
                if ok && nm.name.chars().count() < NAME_MAX {
                    nm.name.push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                nm.name.pop();
            }
        }
        // Name box with a blinking caret.
        let size = 16.0 * s;
        ui::text_left(
            "Name",
            x,
            r.y + 62.0 * s,
            size,
            Color::new(1.0, 1.0, 1.0, 0.75),
        );
        let field = Rect::new(x + 90.0 * s, r.y + 44.0 * s, w - 90.0 * s, 26.0 * s);
        draw_rectangle(
            field.x,
            field.y,
            field.w,
            field.h,
            Color::new(1.0, 1.0, 1.0, 0.08),
        );
        draw_rectangle_lines(field.x, field.y, field.w, field.h, 1.5, ACCENT);
        let shown = if nm.name.is_empty() && self.time % 1.0 > 0.5 {
            "Type a name".to_owned()
        } else {
            nm.name.clone()
        };
        let text_col = if nm.name.is_empty() {
            Color::new(1.0, 1.0, 1.0, 0.35)
        } else {
            WHITE
        };
        ui::text_left(
            &shown,
            field.x + 8.0 * s,
            field.y + field.h * 0.7,
            size,
            text_col,
        );
        if !nm.name.is_empty() && self.time % 1.0 < 0.5 {
            let tw = measure_text(&nm.name, None, size as u16, 1.0).width;
            let cx = field.x + 10.0 * s + tw;
            draw_line(
                cx,
                field.y + 5.0 * s,
                cx,
                field.y + field.h - 5.0 * s,
                1.5,
                WHITE,
            );
        }
        let row = |n: f32| Rect::new(x, r.y + (84.0 + n * 32.0) * s, w, 28.0 * s);
        let d = ui.stepper(
            "ed_new_w",
            "Width",
            &format!("{} tiles", NEW_WIDTHS[nm.width]),
            row(0.0),
            false,
            input,
        );
        nm.width = (nm.width as i32 + d).clamp(0, NEW_WIDTHS.len() as i32 - 1) as usize;
        let d = ui.stepper(
            "ed_new_h",
            "Height",
            &format!("{} tiles", NEW_HEIGHTS[nm.height]),
            row(1.0),
            false,
            input,
        );
        nm.height = (nm.height as i32 + d).clamp(0, NEW_HEIGHTS.len() as i32 - 1) as usize;
        let name = if nm.name.trim().is_empty() {
            "New Map"
        } else {
            nm.name.trim()
        };
        ui::text_left(
            &format!("Saves as assets/editor/{}.json", self.new_id(name)),
            x,
            r.y + 170.0 * s,
            14.0 * s,
            Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.8),
        );
        let bw = (w - 10.0 * s) / 2.0;
        let by = r.y + r.h - 46.0 * s;
        let create = Rect::new(x, by, bw, 30.0 * s);
        let cancel = Rect::new(x + bw + 10.0 * s, by, bw, 30.0 * s);
        if ui.button("ed_new_ok", "Create (Enter)", create, false, input) {
            return Some(true);
        }
        ui.hint(
            "ed_new_ok",
            create,
            "Make the map: a flat floor plus the spawns every map needs. Nothing is saved until Save Draft.",
        );
        if ui.button("ed_new_cancel", "Cancel (Esc)", cancel, false, input) {
            return Some(false);
        }
        None
    }

    fn create_map(&mut self, nm: &NewMap) {
        let name = if nm.name.trim().is_empty() {
            "New Map"
        } else {
            nm.name.trim()
        };
        let id = self.new_id(name);
        let doc = Doc::blank(
            &id,
            name,
            NEW_WIDTHS[nm.width] as f32,
            NEW_HEIGHTS[nm.height] as f32,
        );
        self.replace(doc);
        self.note = Some((
            format!("New map {name}: Save Draft (Ctrl+S) to keep it"),
            0.0,
        ));
    }

    /// The Open Map window. Returns Some(Some(id)) for a pick, Some(None)
    /// for Cancel.
    fn open_window(&self, ui: &mut Ui, list: &[MapEntry], input: bool) -> Option<Option<String>> {
        let s = ui::scale();
        let row_h = 30.0 * s;
        let most = (((screen_height() - 200.0 * s) / row_h) as usize).max(3);
        let shown = &list[..list.len().min(most)];
        let h = 110.0 * s + shown.len() as f32 * row_h;
        let r = Self::window(440.0 * s, h, "Open Map");
        let x = r.x + 20.0 * s;
        let w = r.w - 40.0 * s;
        let mut pick = None;
        for (i, e) in shown.iter().enumerate() {
            let b = Rect::new(x, r.y + 44.0 * s + i as f32 * row_h, w, row_h - 4.0 * s);
            let label = if e.draft {
                format!("{}  (draft)", e.name)
            } else {
                e.name.clone()
            };
            let id = format!("ed_open{i}");
            let current = e.id == self.doc.id();
            if ui.button(&id, &label, b, current, input) {
                pick = Some(Some(e.id.clone()));
            }
            let tip = match (e.draft, self.data.maps.iter().any(|m| m.id == e.id)) {
                (true, true) => "Opens your saved draft of this map.",
                (true, false) => "A map you made. It lives only as a draft so far.",
                _ => "Opens the map as the game plays it.",
            };
            if input {
                ui.hint(&id, b, tip);
            }
        }
        let b = Rect::new(
            r.x + r.w - 200.0 * s,
            r.y + r.h - 44.0 * s,
            180.0 * s,
            28.0 * s,
        );
        if ui.button("ed_open_cancel", "Cancel (Esc)", b, false, input) {
            pick = Some(None);
        }
        pick
    }

    /// Unsaved edits: 0 Save, 1 Discard, 2 Cancel.
    fn unsaved_window(&self, ui: &mut Ui, to: &Leave, input: bool) -> Option<usize> {
        let s = ui::scale();
        let r = Self::window(560.0 * s, 170.0 * s, "Unsaved Changes");
        let what = match to {
            Leave::Menu => "leaving the editor",
            Leave::NewMap => "starting a new map",
            Leave::Open(_) => "opening another map",
        };
        ui::text_center(
            &format!("Save a draft of this map before {what}?"),
            r.x + r.w / 2.0,
            r.y + 78.0 * s,
            17.0 * s,
            WHITE,
        );
        let bw = (r.w - 60.0 * s) / 3.0;
        let by = r.y + r.h - 50.0 * s;
        let labels = ["Save Draft (Enter)", "Discard (D)", "Cancel (Esc)"];
        let tips = [
            "Save a draft, then go on.",
            "Throw away the edits since the last save, then go on.",
            "Stay here and keep editing.",
        ];
        let mut pick = None;
        for (i, label) in labels.iter().enumerate() {
            let b = Rect::new(
                r.x + 20.0 * s + i as f32 * (bw + 10.0 * s),
                by,
                bw,
                30.0 * s,
            );
            let id = format!("ed_unsaved{i}");
            if ui.button(&id, label, b, false, input) {
                pick = Some(i);
            }
            if input {
                ui.hint(&id, b, tips[i]);
            }
        }
        pick
    }

    /// The modes this map lists, as indices into the game's modes.
    fn test_modes(&self) -> Vec<usize> {
        let Some(p) = &self.preview else {
            return Vec::new();
        };
        (0..self.data.modes.len())
            .filter(|&m| p.map.modes.contains(&self.data.modes[m].id))
            .collect()
    }

    /// Team counts the map and the chosen mode allow (0 = no teams).
    fn test_team_counts(&self, setup: &TestSetup) -> Vec<usize> {
        let modes = self.test_modes();
        match (&self.preview, modes.get(setup.mode)) {
            (Some(p), Some(&m)) => super::super::allowed_team_counts(&p.map, &self.data.modes[m]),
            _ => vec![0],
        }
    }

    /// The match the Test Play window describes.
    pub(super) fn test_config(&self, setup: &TestSetup) -> MatchConfig {
        let modes = self.test_modes();
        let mode = modes.get(setup.mode).map_or_else(
            || "deathmatch".to_owned(),
            |&m| self.data.modes[m].id.clone(),
        );
        let teams = self
            .test_team_counts(setup)
            .get(setup.teams)
            .copied()
            .unwrap_or(0);
        let skill = Difficulty::ALL[setup.skill];
        // Teams: everyone gets `bots`, the player's team one fewer (the
        // player fills that seat).
        let mut team_bots = [setup.bots; crate::sim::MAX_TEAMS];
        team_bots[0] = setup.bots.saturating_sub(1);
        MatchConfig {
            mode_id: mode,
            teams: teams as u8,
            bots: setup.bots,
            difficulty: skill,
            team_bots,
            team_difficulty: [skill; crate::sim::MAX_TEAMS],
            seed: (self.time * 1000.0) as u64 + 7,
            ..Default::default()
        }
    }

    /// The Test Play window. Returns Some(true) for Start, Some(false) for
    /// Cancel.
    fn test_window(&self, ui: &mut Ui, setup: &mut TestSetup, input: bool) -> Option<bool> {
        let s = ui::scale();
        let r = Self::window(460.0 * s, 250.0 * s, "Test Play");
        let x = r.x + 20.0 * s;
        let w = r.w - 40.0 * s;
        let row = |n: f32| Rect::new(x, r.y + (46.0 + n * 34.0) * s, w, 30.0 * s);
        let modes = self.test_modes();
        if !modes.is_empty() {
            let name = self.data.modes[modes[setup.mode.min(modes.len() - 1)]].display_name();
            let d = ui.stepper("ed_test_mode", "Match Type", &name, row(0.0), false, input);
            if d != 0 {
                setup.mode = step(setup.mode, d, modes.len());
                setup.teams = 0;
            }
        }
        let counts = self.test_team_counts(setup);
        setup.teams = setup.teams.min(counts.len() - 1);
        let teams = counts[setup.teams];
        let label = if teams >= 2 {
            teams.to_string()
        } else {
            "Off".into()
        };
        let d = ui.stepper(
            "ed_test_teams",
            "Teams",
            &label,
            row(1.0),
            false,
            counts.len() > 1,
        );
        setup.teams = step(setup.teams, d, counts.len());
        let (what, most) = if teams >= 2 {
            ("Bots per Team", 4)
        } else {
            ("Bots", 7)
        };
        setup.bots = setup.bots.clamp(1, most);
        let d = ui.stepper(
            "ed_test_bots",
            what,
            &setup.bots.to_string(),
            row(2.0),
            false,
            input,
        );
        setup.bots = (setup.bots as i32 + d).clamp(1, most as i32) as u32;
        let d = ui.stepper(
            "ed_test_skill",
            "Bot Skill",
            Difficulty::ALL[setup.skill].label(),
            row(3.0),
            false,
            input,
        );
        setup.skill = step(setup.skill, d, Difficulty::ALL.len());
        let bw = (w - 10.0 * s) / 2.0;
        let by = r.y + r.h - 44.0 * s;
        let start = Rect::new(x, by, bw, 30.0 * s);
        let cancel = Rect::new(x + bw + 10.0 * s, by, bw, 30.0 * s);
        if ui.button("ed_test_start", "Start (Enter)", start, false, input) {
            return Some(true);
        }
        if input {
            ui.hint(
                "ed_test_start",
                start,
                "Play the map as it is now. Pause (Esc) and pick Back to Editor to return; nothing is lost.",
            );
        }
        if ui.button("ed_test_cancel", "Cancel (Esc)", cancel, false, input) {
            return Some(false);
        }
        None
    }
}
