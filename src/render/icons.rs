//! Weapon pictures for the HUD, drawn from plain shapes so no image files
//! are needed. Each faces right inside a box about twice as wide as tall.

use crate::data::Icon;
use macroquad::prelude::*;

/// Draws in a box `r`, with coordinates from 0 to 1 across and down.
struct Pen {
    r: Rect,
    c: Color,
    dark: Color,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> Vec2 {
        vec2(self.r.x + x * self.r.w, self.r.y + y * self.r.h)
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) {
        let p = self.at(x, y);
        draw_rectangle(p.x, p.y, w * self.r.w, h * self.r.h, self.c);
    }

    /// A four-cornered piece (a slanted grip or magazine).
    fn quad(&self, pts: [(f32, f32); 4]) {
        let [a, b, c, d] = pts.map(|(x, y)| self.at(x, y));
        draw_triangle(a, b, c, self.c);
        draw_triangle(a, c, d, self.c);
    }

    fn tri(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
        draw_triangle(
            self.at(a.0, a.1),
            self.at(b.0, b.1),
            self.at(c.0, c.1),
            self.c,
        );
    }

    fn circle(&self, x: f32, y: f32, r: f32) {
        let p = self.at(x, y);
        draw_circle(p.x, p.y, r * self.r.h, self.c);
    }

    /// A thin dark line for detail (a seam, a slot).
    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        let (a, b) = (self.at(a.0, a.1), self.at(b.0, b.1));
        draw_line(a.x, a.y, b.x, b.y, (self.r.h * 0.05).max(1.0), self.dark);
    }
}

pub fn draw_weapon_icon(icon: Icon, r: Rect, c: Color) {
    let dark = Color::new(c.r * 0.35, c.g * 0.35, c.b * 0.35, c.a);
    let p = Pen { r, c, dark };
    match icon {
        Icon::Pistol => {
            p.rect(0.28, 0.24, 0.5, 0.17);
            p.rect(0.78, 0.27, 0.06, 0.08);
            p.quad([(0.3, 0.41), (0.44, 0.41), (0.4, 0.82), (0.25, 0.82)]);
            p.quad([(0.44, 0.41), (0.54, 0.41), (0.54, 0.52), (0.44, 0.56)]);
            p.line((0.3, 0.3), (0.76, 0.3));
        }
        Icon::Smg => {
            p.rect(0.02, 0.34, 0.16, 0.08);
            p.rect(0.18, 0.26, 0.52, 0.2);
            p.rect(0.7, 0.31, 0.18, 0.08);
            p.quad([(0.32, 0.46), (0.42, 0.46), (0.38, 0.8), (0.28, 0.8)]);
            p.rect(0.5, 0.46, 0.09, 0.4);
            p.line((0.2, 0.33), (0.68, 0.33));
        }
        Icon::Rifle => {
            p.quad([(0.0, 0.32), (0.2, 0.3), (0.2, 0.52), (0.02, 0.6)]);
            p.rect(0.2, 0.28, 0.48, 0.2);
            p.rect(0.68, 0.33, 0.3, 0.07);
            p.rect(0.38, 0.18, 0.16, 0.1);
            p.quad([(0.3, 0.48), (0.38, 0.48), (0.35, 0.76), (0.27, 0.76)]);
            p.quad([(0.48, 0.48), (0.57, 0.48), (0.62, 0.84), (0.53, 0.86)]);
            p.line((0.22, 0.36), (0.66, 0.36));
        }
        Icon::Shotgun => {
            p.quad([(0.0, 0.36), (0.18, 0.32), (0.18, 0.5), (0.02, 0.62)]);
            p.rect(0.18, 0.3, 0.2, 0.2);
            p.rect(0.38, 0.3, 0.6, 0.09);
            p.rect(0.5, 0.39, 0.24, 0.11);
            p.quad([(0.24, 0.5), (0.32, 0.5), (0.28, 0.72), (0.2, 0.72)]);
            p.line((0.53, 0.44), (0.71, 0.44));
        }
        Icon::Flamer => {
            p.circle(0.3, 0.62, 0.22);
            p.rect(0.08, 0.3, 0.5, 0.18);
            p.rect(0.58, 0.34, 0.26, 0.1);
            p.tri((0.84, 0.3), (0.98, 0.39), (0.84, 0.48));
            p.quad([(0.42, 0.48), (0.5, 0.48), (0.48, 0.74), (0.4, 0.74)]);
            p.line((0.2, 0.62), (0.4, 0.62));
        }
        Icon::Marksman => {
            p.quad([(0.0, 0.34), (0.2, 0.32), (0.2, 0.52), (0.02, 0.58)]);
            p.rect(0.2, 0.3, 0.42, 0.18);
            p.rect(0.62, 0.35, 0.36, 0.06);
            p.rect(0.3, 0.16, 0.26, 0.09);
            p.rect(0.36, 0.25, 0.04, 0.05);
            p.rect(0.48, 0.48, 0.08, 0.26);
            p.quad([(0.28, 0.48), (0.36, 0.48), (0.33, 0.74), (0.25, 0.74)]);
        }
        Icon::Sniper => {
            p.quad([(0.0, 0.36), (0.18, 0.32), (0.18, 0.5), (0.0, 0.56)]);
            p.rect(0.18, 0.32, 0.36, 0.16);
            p.rect(0.54, 0.37, 0.44, 0.05);
            p.rect(0.24, 0.14, 0.3, 0.11);
            p.circle(0.24, 0.195, 0.07);
            p.rect(0.36, 0.25, 0.05, 0.07);
            p.quad([(0.26, 0.48), (0.33, 0.48), (0.3, 0.72), (0.22, 0.72)]);
            p.line((0.78, 0.42), (0.7, 0.78));
            p.line((0.78, 0.42), (0.86, 0.78));
        }
        Icon::Rocket => {
            p.rect(0.04, 0.24, 0.8, 0.26);
            p.quad([(0.84, 0.22), (0.98, 0.16), (0.98, 0.58), (0.84, 0.52)]);
            p.rect(0.0, 0.28, 0.06, 0.18);
            p.rect(0.36, 0.12, 0.14, 0.12);
            p.quad([(0.3, 0.5), (0.4, 0.5), (0.37, 0.8), (0.27, 0.8)]);
            p.rect(0.5, 0.5, 0.08, 0.16);
            p.line((0.1, 0.37), (0.8, 0.37));
        }
        Icon::Grenade => {
            p.quad([(0.0, 0.34), (0.18, 0.3), (0.18, 0.5), (0.02, 0.58)]);
            p.rect(0.18, 0.28, 0.22, 0.22);
            p.circle(0.5, 0.42, 0.24);
            p.rect(0.6, 0.27, 0.36, 0.17);
            p.quad([(0.24, 0.5), (0.32, 0.5), (0.29, 0.76), (0.21, 0.76)]);
            p.line((0.42, 0.42), (0.58, 0.42));
        }
        Icon::Laser => {
            p.quad([(0.04, 0.3), (0.62, 0.24), (0.62, 0.5), (0.1, 0.5)]);
            p.rect(0.62, 0.3, 0.22, 0.12);
            p.circle(0.9, 0.36, 0.1);
            p.quad([(0.24, 0.5), (0.34, 0.5), (0.3, 0.78), (0.2, 0.78)]);
            p.rect(0.4, 0.16, 0.16, 0.08);
            p.line((0.12, 0.38), (0.6, 0.36));
        }
        Icon::Baton => {
            p.rect(0.06, 0.4, 0.24, 0.14);
            p.rect(0.28, 0.34, 0.05, 0.26);
            p.rect(0.33, 0.42, 0.56, 0.1);
            p.circle(0.9, 0.47, 0.08);
            // A spark along the shaft.
            p.line((0.4, 0.38), (0.5, 0.3));
            p.line((0.5, 0.3), (0.58, 0.38));
            p.line((0.58, 0.38), (0.68, 0.3));
            p.line((0.68, 0.3), (0.76, 0.38));
        }
        Icon::Hail => {
            // A bulky body with a round water tank on its back.
            p.circle(0.16, 0.36, 0.2);
            p.quad([(0.0, 0.5), (0.14, 0.46), (0.14, 0.62), (0.02, 0.7)]);
            p.rect(0.16, 0.24, 0.5, 0.26);
            p.rect(0.66, 0.3, 0.2, 0.13);
            p.circle(0.9, 0.365, 0.08);
            p.quad([(0.3, 0.5), (0.39, 0.5), (0.36, 0.78), (0.27, 0.78)]);
            p.rect(0.48, 0.5, 0.1, 0.18);
            p.line((0.1, 0.36), (0.22, 0.36));
            p.line((0.22, 0.32), (0.64, 0.32));
        }
    }
}
