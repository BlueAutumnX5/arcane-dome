//! Drawing the world with placeholder shapes. Reads the sim, never changes it.

mod effects;
mod gadgets;
mod hud;

pub use effects::Effects;
pub use hud::{draw_crosshair, draw_hud, draw_scoreboard};

use crate::data::{Category, PickupKind, ProjectileKind, TileKind};
use crate::sim::{Fighter, HEAD_HEIGHT, World};
use macroquad::prelude::*;

pub const BLUE: Color = Color::new(0.33, 0.63, 1.0, 1.0);
pub const RED: Color = Color::new(1.0, 0.38, 0.32, 1.0);
const FFA_COLORS: [Color; 8] = [
    Color::new(0.33, 0.63, 1.0, 1.0),
    Color::new(1.0, 0.38, 0.32, 1.0),
    Color::new(0.45, 0.85, 0.45, 1.0),
    Color::new(1.0, 0.8, 0.3, 1.0),
    Color::new(0.75, 0.5, 1.0, 1.0),
    Color::new(0.3, 0.9, 0.9, 1.0),
    Color::new(1.0, 0.55, 0.8, 1.0),
    Color::new(0.85, 0.85, 0.85, 1.0),
];

pub fn fighter_color(f: &Fighter) -> Color {
    match f.team {
        Some(0) => BLUE,
        Some(_) => RED,
        None => FFA_COLORS[f.id % FFA_COLORS.len()],
    }
}

pub fn category_color(c: Category) -> Color {
    match c {
        Category::Starter | Category::Sidearm => Color::new(0.7, 0.7, 0.75, 1.0),
        Category::CloseRange => Color::new(1.0, 0.6, 0.25, 1.0),
        Category::Automatic => Color::new(0.4, 0.8, 1.0, 1.0),
        Category::LongRange => Color::new(0.6, 0.9, 0.5, 1.0),
        Category::Heavy => Color::new(1.0, 0.85, 0.3, 1.0),
        Category::Explosive => Color::new(1.0, 0.45, 0.4, 1.0),
        Category::Melee => Color::new(0.85, 0.6, 1.0, 1.0),
    }
}

/// Follows a point with look-ahead toward the mouse, clamped to the map.
pub struct GameCamera {
    pub pos: Vec2,
    /// Screen pixels per world tile.
    pub ppt: f32,
}

impl GameCamera {
    pub fn new(pos: Vec2) -> Self {
        Self { pos, ppt: 32.0 }
    }

    pub fn update(&mut self, world: &World, focus: Vec2, mouse: Vec2, dt: f32) {
        let screen = vec2(screen_width(), screen_height());
        self.ppt = world.map.tile_size as f32 * (screen.y / 720.0).max(0.5);
        let look = ((mouse - screen * 0.5) / self.ppt * 0.35).clamp_length_max(6.0);
        let desired = focus + look;
        let t = 1.0 - (-8.0 * dt).exp();
        self.pos = self.pos.lerp(desired, t);
        let half = screen / self.ppt * 0.5;
        let size = vec2(world.map.width as f32, world.map.height as f32);
        for axis in 0..2 {
            self.pos[axis] = if size[axis] <= half[axis] * 2.0 {
                size[axis] * 0.5
            } else {
                self.pos[axis].clamp(half[axis], size[axis] - half[axis])
            };
        }
    }

    pub fn snap(&mut self, focus: Vec2) {
        self.pos = focus;
    }

    pub fn camera2d(&self) -> Camera2D {
        Camera2D {
            target: self.pos,
            zoom: vec2(
                2.0 * self.ppt / screen_width(),
                2.0 * self.ppt / screen_height(),
            ),
            ..Default::default()
        }
    }

    pub fn world_to_screen(&self, p: Vec2) -> Vec2 {
        (p - self.pos) * self.ppt + vec2(screen_width(), screen_height()) * 0.5
    }

    pub fn screen_to_world(&self, s: Vec2) -> Vec2 {
        (s - vec2(screen_width(), screen_height()) * 0.5) / self.ppt + self.pos
    }
}

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

fn shade(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// Far-away silhouettes that scroll slower than the world.
fn draw_backdrop(world: &World, cam: &GameCamera) {
    let sky = rgb(world.map.sky_color);
    clear_background(sky);
    let far = shade(sky, 0.7);
    let sw = screen_width();
    let sh = screen_height();
    let parallax = 0.3;
    let spacing = 140.0 * cam.ppt / 32.0;
    let offset = (cam.pos.x * cam.ppt * parallax) % spacing;
    let mut seed = world
        .map
        .id
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let count = (sw / spacing) as i32 + 3;
    let first = (cam.pos.x * cam.ppt * parallax / spacing) as i32;
    for k in -1..count {
        let idx = (first + k) as u32;
        seed = idx.wrapping_mul(2654435761) ^ seed.rotate_left(5);
        let h = sh * (0.25 + (seed % 1000) as f32 / 1000.0 * 0.45);
        let w = spacing * (0.45 + (seed / 1000 % 100) as f32 / 100.0 * 0.35);
        let x = k as f32 * spacing - offset;
        draw_rectangle(x, sh - h, w, h, far);
        draw_rectangle(x + w * 0.2, sh - h - 18.0, w * 0.15, 18.0, far);
    }
}

pub fn draw_world(world: &World, cam: &GameCamera, effects: &Effects, player: Option<usize>) {
    draw_backdrop(world, cam);
    set_camera(&cam.camera2d());

    let half = vec2(screen_width(), screen_height()) / cam.ppt * 0.5;
    let x0 = (cam.pos.x - half.x).floor() as i32 - 1;
    let x1 = (cam.pos.x + half.x).ceil() as i32 + 1;
    let y0 = (cam.pos.y - half.y).floor() as i32 - 1;
    let y1 = (cam.pos.y + half.y).ceil() as i32 + 1;
    let map = &world.map;

    let solid = Color::from_rgba(52, 58, 78, 255);
    let solid_top = Color::from_rgba(96, 104, 132, 255);
    let one_way = Color::from_rgba(170, 140, 90, 255);
    let spikes = Color::from_rgba(210, 70, 70, 255);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
                continue;
            }
            let (fx, fy) = (x as f32, y as f32);
            match map.tile(x, y) {
                TileKind::Solid => {
                    draw_rectangle(fx, fy, 1.0, 1.0, solid);
                    if map.tile(x, y - 1) != TileKind::Solid && y > 0 {
                        draw_rectangle(fx, fy, 1.0, 0.12, solid_top);
                    }
                }
                TileKind::OneWay => {
                    draw_rectangle(fx, fy, 1.0, 0.22, one_way);
                    draw_rectangle(fx, fy + 0.22, 1.0, 0.06, shade(one_way, 0.6));
                }
                TileKind::Spikes => {
                    for k in 0..3 {
                        let bx = fx + k as f32 / 3.0;
                        draw_triangle(
                            vec2(bx, fy + 1.0),
                            vec2(bx + 1.0 / 3.0, fy + 1.0),
                            vec2(bx + 1.0 / 6.0, fy + 0.35),
                            spikes,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    draw_pickups(world);
    gadgets::draw_objects(world);
    for p in &world.projectiles {
        let def = &world.data.weapons[p.weapon];
        match p.kind {
            ProjectileKind::Grenade => {
                draw_circle(p.pos.x, p.pos.y, 0.17, Color::from_rgba(120, 200, 90, 255));
                draw_circle(p.pos.x, p.pos.y, 0.07, Color::from_rgba(230, 255, 200, 255));
            }
            ProjectileKind::Rocket => {
                let back = p.pos - p.vel.normalize_or_zero() * 0.6;
                draw_line(
                    back.x,
                    back.y,
                    p.pos.x,
                    p.pos.y,
                    0.14,
                    Color::from_rgba(255, 170, 60, 160),
                );
                draw_circle(p.pos.x, p.pos.y, 0.15, Color::from_rgba(255, 120, 40, 255));
            }
            _ => {
                let has_burn = def
                    .specials
                    .iter()
                    .any(|s| matches!(s, crate::data::Special::Burn { .. }));
                let color = if has_burn {
                    Color::from_rgba(255, 140, 40, 255)
                } else {
                    Color::from_rgba(255, 235, 160, 255)
                };
                let back = p.pos - p.vel * 0.012;
                draw_line(
                    back.x,
                    back.y,
                    p.pos.x,
                    p.pos.y,
                    0.07,
                    with_alpha(color, 0.6),
                );
                draw_circle(p.pos.x, p.pos.y, 0.07, color);
            }
        }
    }

    for f in world.fighters.iter().filter(|f| f.alive) {
        draw_fighter(world, f, player == Some(f.id));
        gadgets::draw_shield_hint(f);
        gadgets::draw_powerup_glow(world, f);
    }

    effects.draw_world();

    // Water drawn over everything inside it.
    let water = Color::from_rgba(50, 120, 210, 110);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if map.tile(x, y) == TileKind::Water && x >= 0 && y >= 0 {
                draw_rectangle(x as f32, y as f32, 1.0, 1.0, water);
                if map.tile(x, y - 1) != TileKind::Water {
                    draw_rectangle(
                        x as f32,
                        y as f32,
                        1.0,
                        0.06,
                        Color::from_rgba(160, 210, 255, 160),
                    );
                }
            }
        }
    }

    set_default_camera();
    draw_labels(world, cam, player);
    gadgets::draw_object_labels(world, cam);
}

fn draw_pickups(world: &World) {
    let bob = (world.time * 3.0).sin() * 0.08;
    for p in &world.pickups {
        let c = p.pos + vec2(0.0, bob - 0.1);
        if !p.active {
            draw_rectangle_lines(
                c.x - 0.3,
                c.y - 0.3,
                0.6,
                0.6,
                0.03,
                Color::new(1.0, 1.0, 1.0, 0.15),
            );
            continue;
        }
        match p.def.kind {
            PickupKind::Health => {
                let g = Color::from_rgba(80, 220, 110, 255);
                draw_rectangle(
                    c.x - 0.32,
                    c.y - 0.32,
                    0.64,
                    0.64,
                    Color::from_rgba(240, 250, 240, 255),
                );
                draw_rectangle(c.x - 0.08, c.y - 0.26, 0.16, 0.52, g);
                draw_rectangle(c.x - 0.26, c.y - 0.08, 0.52, 0.16, g);
            }
            PickupKind::Ammo => {
                let y = Color::from_rgba(230, 190, 60, 255);
                draw_rectangle(c.x - 0.32, c.y - 0.22, 0.64, 0.44, y);
                draw_rectangle(c.x - 0.32, c.y - 0.05, 0.64, 0.1, shade(y, 0.6));
            }
            PickupKind::Weapon => {
                let col = p
                    .weapon
                    .map(|w| category_color(world.data.weapons[w].category))
                    .unwrap_or(WHITE);
                draw_rectangle(
                    c.x - 0.4,
                    c.y - 0.3,
                    0.8,
                    0.6,
                    Color::from_rgba(60, 50, 40, 255),
                );
                draw_rectangle_lines(c.x - 0.4, c.y - 0.3, 0.8, 0.6, 0.08, col);
                draw_rectangle(c.x - 0.25, c.y - 0.05, 0.5, 0.1, col);
            }
        }
    }
}

fn draw_fighter(world: &World, f: &Fighter, is_player: bool) {
    let b = f.aabb();
    let mut col = fighter_color(f);
    if f.spawn_protect > 0.0 && (world.time * 12.0).sin() > 0.0 {
        col.a = 0.45;
    }
    let size = b.max - b.min;
    draw_rectangle(b.min.x, b.min.y, size.x, size.y, col);
    draw_rectangle(b.min.x, b.min.y, size.x, HEAD_HEIGHT, shade(col, 0.7));
    // Visor on the facing side.
    let vx = if f.facing > 0.0 {
        b.max.x - 0.28
    } else {
        b.min.x + 0.04
    };
    draw_rectangle(
        vx,
        b.min.y + 0.14,
        0.24,
        0.12,
        Color::from_rgba(230, 245, 255, 230),
    );
    if is_player {
        draw_rectangle_lines(b.min.x, b.min.y, size.x, size.y, 0.06, WHITE);
    }
    if f.burn.is_some() {
        draw_rectangle_lines(
            b.min.x - 0.04,
            b.min.y - 0.04,
            size.x + 0.08,
            size.y + 0.08,
            0.06,
            ORANGE,
        );
    }
    if f.freeze > 0.0 || f.stun > 0.0 {
        draw_rectangle_lines(
            b.min.x - 0.04,
            b.min.y - 0.04,
            size.x + 0.08,
            size.y + 0.08,
            0.06,
            SKYBLUE,
        );
    }
    // Weapon.
    let eye = f.eye();
    let def = &world.data.weapons[f.current];
    let len = match def.category {
        Category::LongRange | Category::Heavy => 1.0,
        Category::Starter | Category::Sidearm => 0.55,
        _ => 0.8,
    };
    let tip = eye + f.aim * len;
    draw_line(
        eye.x,
        eye.y,
        tip.x,
        tip.y,
        0.16,
        Color::from_rgba(40, 42, 50, 255),
    );
    let mid = eye + f.aim * len * 0.6;
    draw_line(
        mid.x,
        mid.y,
        tip.x,
        tip.y,
        0.1,
        category_color(def.category),
    );
}

/// Names and health bars above other fighters, weapon names over crates.
fn draw_labels(world: &World, cam: &GameCamera, player: Option<usize>) {
    let s = (screen_height() / 720.0).max(0.6);
    for f in world
        .fighters
        .iter()
        .filter(|f| f.alive && Some(f.id) != player)
    {
        let top = cam.world_to_screen(vec2(f.feet.x, f.aabb().min.y));
        let w = 34.0 * s;
        let y = top.y - 10.0 * s;
        draw_rectangle(
            top.x - w / 2.0,
            y,
            w,
            4.0 * s,
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        draw_rectangle(
            top.x - w / 2.0,
            y,
            w * (f.health / 100.0).clamp(0.0, 1.0),
            4.0 * s,
            fighter_color(f),
        );
        let size = (14.0 * s) as u16;
        let dims = measure_text(&f.name, None, size, 1.0);
        draw_text(
            &f.name,
            top.x - dims.width / 2.0,
            y - 4.0 * s,
            size as f32,
            Color::new(1.0, 1.0, 1.0, 0.85),
        );
    }
    for p in world.pickups.iter().filter(|p| p.active) {
        if let Some(w) = p.weapon {
            let name = &world.data.weapons[w].name;
            let at = cam.world_to_screen(p.pos + vec2(0.0, -0.55));
            let size = (13.0 * s) as u16;
            let dims = measure_text(name, None, size, 1.0);
            draw_text(
                name,
                at.x - dims.width / 2.0,
                at.y,
                size as f32,
                Color::new(1.0, 1.0, 1.0, 0.7),
            );
        }
    }
}
