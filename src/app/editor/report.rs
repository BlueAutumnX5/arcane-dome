//! Validate: the map checks' results in a panel at the bottom of the
//! editor. Clicking a problem flies the camera to it.

use super::*;
use crate::ai::check_map;

/// Rows of problems the panel shows before "+N more".
const REPORT_ROWS: usize = 5;
/// How long the ring marking a problem's spot pulses, in seconds.
const FLASH_S: f32 = 2.0;
/// Zoom (pixels per tile) the camera uses at least when it flies to a
/// problem.
const FLY_PPT: f32 = 32.0;

/// One line of the Validate panel.
pub(super) struct Issue {
    text: String,
    /// Where it is; loader errors have no place.
    at: Option<Vec2>,
    warning: bool,
}

/// The last Validate's results.
pub(super) struct Report {
    issues: Vec<Issue>,
    /// The map has changed since; the list may be out of date.
    stale: bool,
}

impl Report {
    /// Nothing blocks publishing (warnings are fine) and the map is
    /// unchanged since.
    pub(super) fn passed(&self) -> bool {
        !self.stale && self.issues.iter().all(|i| i.warning)
    }

    pub(super) fn mark_stale(&mut self) {
        self.stale = true;
    }
}

impl Editor {
    /// Run every check: the map loader's (live in the top bar too), then
    /// reachability and spawn safety.
    pub(super) fn validate(&mut self) {
        let issues: Vec<Issue> = if !self.problems.is_empty() {
            self.problems
                .iter()
                .map(|p| Issue {
                    text: p.clone(),
                    at: None,
                    warning: false,
                })
                .collect()
        } else if let Some(p) = &self.preview {
            check_map(&p.map)
                .into_iter()
                .map(|p| Issue {
                    text: p.message,
                    at: Some(p.at),
                    warning: p.warning,
                })
                .collect()
        } else {
            Vec::new()
        };
        let errors = issues.iter().filter(|i| !i.warning).count();
        let msg = match (errors, issues.len()) {
            (0, 0) => "Validate: no problems found".to_owned(),
            (0, n) => format!("Validate: {n} warning(s), nothing blocking"),
            (e, _) => format!("Validate: {e} error(s) to fix"),
        };
        self.note = Some((msg, 0.0));
        self.report = Some(Report {
            issues,
            stale: false,
        });
    }

    /// The panel's rect, if there is a report to show.
    pub(super) fn report_rect(&self) -> Option<Rect> {
        let report = self.report.as_ref()?;
        let s = ui::scale();
        let rows =
            report.issues.len().min(REPORT_ROWS) + usize::from(report.issues.len() > REPORT_ROWS);
        let h = (34.0 + rows as f32 * 24.0 + 8.0) * s;
        let left = 320.0 * s;
        let free = screen_width() - left - 250.0 * s;
        let w = free.min(640.0 * s);
        let x = left + (free - w) / 2.0;
        Some(Rect::new(x, screen_height() - 30.0 * s - h, w, h))
    }

    /// Draw the Validate panel; clicking a problem flies to it, the x closes
    /// the panel.
    pub(super) fn report_panel(&mut self, ui: &mut Ui, live: bool) {
        let (Some(r), Some(report)) = (self.report_rect(), self.report.as_ref()) else {
            return;
        };
        let s = ui::scale();
        ui::panel(r, None);
        let errors = report.issues.iter().filter(|i| !i.warning).count();
        let warnings = report.issues.len() - errors;
        let (title, color) = if report.stale {
            (
                "Map changed since: Validate (V) again".to_owned(),
                Color::new(1.0, 0.8, 0.35, 1.0),
            )
        } else if errors > 0 {
            (
                format!("{errors} error(s), {warnings} warning(s): fix the errors to publish"),
                Color::new(1.0, 0.45, 0.4, 1.0),
            )
        } else if warnings > 0 {
            (
                format!("{warnings} warning(s), nothing blocking: ready to publish"),
                Color::new(1.0, 0.8, 0.35, 1.0),
            )
        } else {
            (
                "No problems found: ready to publish".to_owned(),
                Color::new(0.45, 0.95, 0.5, 1.0),
            )
        };
        ui::text_left(&title, r.x + 12.0 * s, r.y + 22.0 * s, 16.0 * s, color);
        let close = Rect::new(r.x + r.w - 116.0 * s, r.y + 6.0 * s, 110.0 * s, 22.0 * s);
        if ui.button("ed_report_close", "Close (Esc)", close, false, live) {
            self.report = None;
            return;
        }
        if live {
            ui.hint("ed_report_close", close, "Close the Validate results.");
        }
        let fade = if report.stale { 0.45 } else { 1.0 };
        let mut fly = None;
        for (i, issue) in report.issues.iter().take(REPORT_ROWS).enumerate() {
            let row = Rect::new(
                r.x + 8.0 * s,
                r.y + (34.0 + i as f32 * 24.0) * s,
                r.w - 16.0 * s,
                22.0 * s,
            );
            let hot = live && issue.at.is_some() && row.contains(mouse_position().into());
            if hot {
                draw_rectangle(row.x, row.y, row.w, row.h, Color::new(1.0, 1.0, 1.0, 0.1));
            }
            let mark = if issue.warning {
                Color::new(1.0, 0.8, 0.35, fade)
            } else {
                Color::new(1.0, 0.45, 0.4, fade)
            };
            draw_rectangle(row.x + 4.0 * s, row.y + 6.0 * s, 10.0 * s, 10.0 * s, mark);
            let place = issue
                .at
                .map_or(String::new(), |p| format!("  ({:.1}, {:.1})", p.x, p.y));
            ui::text_left(
                &format!("{}{place}", issue.text),
                row.x + 22.0 * s,
                row.y + row.h * 0.72,
                14.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.9 * fade),
            );
            if hot {
                ui.hint(
                    &format!("ed_issue{i}"),
                    row,
                    "Click to fly the camera to this spot.",
                );
                if is_mouse_button_pressed(MouseButton::Left) {
                    fly = issue.at;
                }
            }
        }
        if report.issues.len() > REPORT_ROWS {
            let y = r.y + (34.0 + REPORT_ROWS as f32 * 24.0) * s;
            ui::text_left(
                &format!(
                    "+{} more: fix these first, then Validate again",
                    report.issues.len() - REPORT_ROWS
                ),
                r.x + 30.0 * s,
                y + 16.0 * s,
                14.0 * s,
                Color::new(1.0, 1.0, 1.0, 0.6),
            );
        }
        if let Some(at) = fly {
            self.cam.ppt = self.cam.ppt.max(FLY_PPT);
            self.cam.pos = at - vec2(0.0, 1.0);
            self.flash = Some((at, 0.0));
        }
    }

    /// A ring pulsing at the problem the camera flew to (world space).
    pub(super) fn draw_flash(&self) {
        let Some((at, age)) = self.flash else {
            return;
        };
        let k = age / FLASH_S;
        let r = 0.8 + 1.2 * (age * 6.0).sin().abs();
        let c = Color::new(1.0, 0.45, 0.4, 1.0 - k);
        draw_circle_lines(at.x, at.y - 0.9, r, 3.0 / self.cam.ppt, c);
    }

    /// Age the flash ring; it goes away after FLASH_S.
    pub(super) fn tick_flash(&mut self, dt: f32) {
        if let Some((_, age)) = &mut self.flash {
            *age += dt;
            if *age > FLASH_S {
                self.flash = None;
            }
        }
    }
}
