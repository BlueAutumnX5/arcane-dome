//! Drawing the world with placeholder shapes. Reads the sim, never changes it.

mod effects;
mod gadgets;
mod hud;
mod terrain;

pub use terrain::{MapArt, draw_sky};

pub use effects::Effects;
pub use hud::{draw_crosshair, draw_hud, draw_scoreboard};

use crate::data::{Category, MapDef, PickupKind, ProjectileKind};
use crate::sim::{Fighter, HEAD_HEIGHT, MAX_HEALTH, MAX_SHIELD, MAX_TEAMS, World};
use macroquad::prelude::*;

/// The team palette (Blue, Red, Green, Yellow); a team's colour is picked in
/// the match setup.
pub const TEAM_COLORS: [Color; MAX_TEAMS] = [
    Color::new(0.33, 0.63, 1.0, 1.0),
    Color::new(1.0, 0.38, 0.32, 1.0),
    Color::new(0.42, 0.86, 0.42, 1.0),
    Color::new(1.0, 0.8, 0.28, 1.0),
];
pub const BLUE: Color = TEAM_COLORS[0];
pub const RED: Color = TEAM_COLORS[1];
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
        Some(_) => TEAM_COLORS[f.tint.unwrap_or(0) as usize % MAX_TEAMS],
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

/// Switch to a camera. Macroquad draws everything queued so far when the
/// camera changes, and that leaves miniquad noting the font texture as
/// bound. If new text then outgrows the font texture, it is deleted and
/// remade under the same id, miniquad skips binding it, and its pixels
/// never arrive: every letter draws as a black block. Clearing the note
/// after each switch prevents that.
pub fn use_camera(camera: &dyn Camera) {
    set_camera(camera);
    resync_texture_bindings();
}

/// Back to plain screen pixels (see `use_camera`).
pub fn use_screen() {
    set_default_camera();
    resync_texture_bindings();
}

/// Unbind every texture through miniquad, so its note of what is bound
/// matches OpenGL again (also needed after `Texture2D::grab_screen`, which
/// binds behind its back).
pub fn resync_texture_bindings() {
    unsafe { get_internal_gl().quad_context.commit_frame() };
}

/// Follows a point with look-ahead toward the mouse, clamped to the map.
/// Share of the mouse's distance from the screen centre the camera shifts
/// toward it.
pub const LOOK_FOLLOW: f32 = 0.7;

/// How far, in tiles, the camera may look ahead with a weapon of this range:
/// long guns see further, like a scope.
pub fn look_reach(range_tiles: f32) -> f32 {
    (range_tiles / 4.0).clamp(6.0, 14.0)
}

pub struct GameCamera {
    pub pos: Vec2,
    /// Screen pixels per world tile.
    pub ppt: f32,
}

impl GameCamera {
    pub fn new(pos: Vec2) -> Self {
        Self { pos, ppt: 32.0 }
    }

    /// Follow `focus`, shifted toward the mouse by up to `reach` tiles (see
    /// `look_reach`).
    pub fn update(&mut self, world: &World, focus: Vec2, mouse: Vec2, reach: f32, dt: f32) {
        let screen = vec2(screen_width(), screen_height());
        self.ppt = world.map.tile_size as f32 * (screen.y / 720.0).max(0.5);
        let look = ((mouse - screen * 0.5) / self.ppt * LOOK_FOLLOW).clamp_length_max(reach);
        let desired = focus + look;
        let t = 1.0 - (-8.0 * dt).exp();
        self.pos = self.pos.lerp(desired, t);
        let half = screen / self.ppt * 0.5;
        let size = vec2(world.map.width, world.map.height);
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

/// A picture of the whole map (sky, terrain and decor, no fighters), drawn
/// with the same art as the game. Call [`MapThumb::draw`] every frame it is
/// shown: a render target drawn only once can come out blank.
pub struct MapThumb {
    art: MapArt,
    target: RenderTarget,
}

impl MapThumb {
    pub fn new(map: &MapDef, width: u32) -> Self {
        let height = (width as f32 * map.height / map.width).round() as u32;
        let target = render_target(width, height);
        target.texture.set_filter(FilterMode::Linear);
        Self {
            art: MapArt::new(map),
            target,
        }
    }

    /// Redraw the picture and return it.
    pub fn draw(&self, map: &MapDef) -> &Texture2D {
        let tex = &self.target.texture;
        let (w, h) = (tex.width(), tex.height());
        let ppt = w / map.width;
        use_camera(&Camera2D {
            target: vec2(w, h) * 0.5,
            zoom: vec2(2.0 / w, 2.0 / h),
            render_target: Some(self.target.clone()),
            ..Default::default()
        });
        clear_background(BLACK);
        terrain::draw_sky_sized(map, w, h, map.width * 0.5, ppt, 0.0);
        use_camera(&Camera2D {
            target: vec2(map.width, map.height) * 0.5,
            zoom: vec2(2.0 * ppt / w, 2.0 * ppt / h),
            render_target: Some(self.target.clone()),
            ..Default::default()
        });
        self.art.draw_back(
            map,
            Vec2::splat(-1.0),
            vec2(map.width, map.height) + 1.0,
            0.0,
        );
        self.art.draw_front(map, 0.0);
        use_screen();
        tex
    }
}

fn shade(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

pub fn draw_world(
    world: &World,
    cam: &GameCamera,
    effects: &Effects,
    player: Option<usize>,
    art: &MapArt,
) {
    let map = &world.map;
    terrain::draw_sky(map, cam.pos.x, cam.ppt, world.time);
    use_camera(&cam.camera2d());

    let half = vec2(screen_width(), screen_height()) / cam.ppt * 0.5;
    let (view_min, view_max) = (cam.pos - half - Vec2::ONE, cam.pos + half + Vec2::ONE);
    art.draw_back(map, view_min, view_max, world.time);
    effects.draw_marks();

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

    // Water and front props drawn over everything inside them.
    art.draw_front(map, world.time);

    use_screen();
    draw_labels(world, cam, player);
    gadgets::draw_object_labels(world, cam);
}

/// Nameplate size at 720p, in pixels; long names widen it.
const PLATE_W: f32 = 104.0;
const PLATE_H: f32 = 12.0;

/// A fighter's nameplate, its bottom centre at `at`: the bar is its health,
/// filled in its team colour, with the name on it. The player's own plate
/// is solid with a bright frame; everyone else's is see-through. Shield
/// shows as a pale strip along the top. (Room is left below for an
/// abilities bar.)
fn draw_plate(f: &Fighter, at: Vec2, own: bool, s: f32) {
    let size = if own { 13.0 } else { 12.0 } * s;
    let text_w = measure_text(&f.name, None, size as u16, 1.0).width;
    let w = (PLATE_W * s).max(text_w + 16.0 * s);
    let h = PLATE_H * s;
    let r = Rect::new(at.x - w / 2.0, at.y - h, w, h);
    let c = fighter_color(f);
    let a = if own { 1.0 } else { 0.6 };
    let shade = |k: f32, alpha: f32| Color::new(c.r * k, c.g * k, c.b * k, alpha);
    let health = (f.health / MAX_HEALTH).clamp(0.0, 1.0);
    // The lost part stays as a dark band of the same colour.
    draw_rectangle(r.x, r.y, r.w, r.h, shade(0.22, 0.8 * a));
    draw_rectangle(r.x, r.y, r.w * health, r.h, shade(0.78, a));
    draw_rectangle(
        r.x,
        r.y,
        r.w * health,
        r.h * 0.45,
        Color::new(1.0, 1.0, 1.0, 0.13 * a),
    );
    if f.shield > 0.0 {
        let k = (f.shield / MAX_SHIELD).clamp(0.0, 1.0);
        draw_rectangle(
            r.x,
            r.y,
            r.w * k,
            2.5 * s,
            Color::new(0.85, 0.96, 1.0, 0.95 * a),
        );
    }
    if own {
        let m = 2.0 * s;
        draw_rectangle_lines(
            r.x - m,
            r.y - m,
            r.w + 2.0 * m,
            r.h + 2.0 * m,
            2.0 * s,
            Color::new(0.92, 0.96, 1.0, 0.95),
        );
    }
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color::new(0.0, 0.0, 0.0, 0.5 * a));
    // Name, drawn twice a pixel apart to read as bold.
    let x = at.x - text_w / 2.0;
    let y = r.y + h * 0.76;
    let ink = Color::new(1.0, 1.0, 1.0, if own { 1.0 } else { 0.9 });
    draw_text(
        &f.name,
        x + 1.0,
        y + 1.5,
        size,
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    draw_text(&f.name, x, y, size, ink);
    draw_text(&f.name, x + 0.8, y, size, ink);
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

/// Blinks per second of the spawn-protection overlay in its last second.
const PROTECT_BLINK_HZ: f32 = 8.0;

fn draw_fighter(world: &World, f: &Fighter, is_player: bool) {
    let b = f.aabb();
    let col = fighter_color(f);
    let size = b.max - b.min;
    draw_rectangle(b.min.x, b.min.y, size.x, size.y, col);
    draw_rectangle(b.min.x, b.min.y, size.x, HEAD_HEIGHT, shade(col, 0.7));
    // Spawn protection: a turquoise overlay that blinks fast in its last second.
    let blink_off = f.spawn_protect < 1.0 && (world.time * PROTECT_BLINK_HZ).fract() < 0.5;
    if f.spawn_protect > 0.0 && !blink_off {
        let t = Color::new(0.25, 0.95, 0.85, 0.5);
        draw_rectangle(b.min.x, b.min.y, size.x, size.y, t);
        draw_rectangle_lines(
            b.min.x - 0.06,
            b.min.y - 0.06,
            size.x + 0.12,
            size.y + 0.12,
            0.07,
            Color::new(0.4, 1.0, 0.92, 0.9),
        );
    }
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

/// Nameplates over fighters, weapon names over crates.
fn draw_labels(world: &World, cam: &GameCamera, player: Option<usize>) {
    let s = (screen_height() / 720.0).max(0.6);
    for f in world.fighters.iter().filter(|f| f.alive) {
        let top = cam.world_to_screen(vec2(f.feet.x, f.aabb().min.y));
        draw_plate(f, top - vec2(0.0, 8.0 * s), Some(f.id) == player, s);
    }
    for p in world.pickups.iter().filter(|p| p.active) {
        if let Some(w) = p.weapon {
            let name = &world.data.weapons[w].name;
            let at = cam.world_to_screen(p.pos + vec2(0.0, -0.55));
            // Keep world labels out of the weapon bar strip.
            if at.y > screen_height() - 64.0 * s {
                continue;
            }
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
