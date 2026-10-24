//! Maps: freeform polygon terrain, a visual theme, props, and placements.
//!
//! Two formats load:
//! - **Shape maps** (current): `size`, `shapes` (polygons), `theme`, `decor`.
//!   Placement `x`/`y` are feet positions in world units (1 unit = 1 tile).
//! - **Grid maps** (legacy / tests): ASCII `grid` + `legend`, converted to
//!   polygons on load. Placement `x`/`y` are tile cells, converted to feet
//!   positions at the bottom-centre of the cell.

use super::{DataError, is_snake_case, parse_hex_color, parse_json};
use crate::geo::{Material, Shape, ShapeKind, Terrain};
use macroquad::math::{Vec2, vec2};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

/// Damage per second of legacy spike tiles.
const SPIKE_DPS: f32 = 45.0;

/// Legacy grid tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileKind {
    Air,
    Solid,
    OneWay,
    Spikes,
    Water,
    SlopeUpRight,
    SlopeUpLeft,
    Sludge,
}

/// Purely visual layer behind the play area, for building interiors (legacy grids).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    #[default]
    None,
    Wall,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnDef {
    pub x: f32,
    pub y: f32,
    /// `None` = free-for-all spawn.
    #[serde(default)]
    pub team: Option<u8>,
    /// Different owners in matches with a given number of teams, say
    /// `{"3": 2}` (team 2's in 3-team matches) or `{"3": null}` (nobody's).
    #[serde(default)]
    pub by_teams: ByTeams,
}

impl SpawnDef {
    pub fn feet(&self) -> Vec2 {
        vec2(self.x, self.y)
    }

    /// The team this spawn belongs to in a match with `teams` teams
    /// (0 = free-for-all, which uses `team` as is).
    pub fn team_for(&self, teams: usize) -> Option<u8> {
        match teams.checked_sub(2).and_then(|k| self.by_teams.0.get(k)) {
            Some(Some(owner)) => *owner,
            _ => self.team,
        }
    }
}

/// Per team count (2, 3, 4) overrides of a spawn's team: outer `None` =
/// no override, `Some(None)` = nobody's spawn at that count.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ByTeams(pub [Option<Option<u8>>; 3]);

impl<'de> Deserialize<'de> for ByTeams {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = BTreeMap::<String, Option<u8>>::deserialize(d)?;
        let mut out = ByTeams::default();
        for (count, team) in raw {
            let k = match count.as_str() {
                "2" => 0,
                "3" => 1,
                "4" => 2,
                other => {
                    return Err(serde::de::Error::custom(format!(
                        "by_teams keys are team counts \"2\", \"3\" or \"4\" (got \"{other}\")"
                    )));
                }
            };
            out.0[k] = Some(team);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickupKind {
    Health,
    Ammo,
    Weapon,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PickupDef {
    #[serde(rename = "type")]
    pub kind: PickupKind,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub weapon_id: Option<String>,
    #[serde(default = "default_respawn")]
    pub respawn_s: f32,
    /// Health restored (health pickups only). Defaults to 50.
    #[serde(default)]
    pub amount: Option<f32>,
}

impl PickupDef {
    pub fn feet(&self) -> Vec2 {
        vec2(self.x, self.y)
    }
}

fn default_respawn() -> f32 {
    20.0
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotHint {
    pub x: f32,
    pub y: f32,
    pub kind: String,
}

/// Placeable map gadgets. Kept separate from pickups so a level editor can
/// drop them anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    /// Launch pad that throws fighters straight up.
    Thruster,
    /// Small pod that deploys a random weapon every so often.
    WeaponStation,
    /// Small pod that deploys shield every so often (rarer than weapons).
    ShieldStation,
    /// Fixed spot where a powerup appears.
    PowerupSpot,
}

impl ObjectKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Thruster => "thruster",
            Self::WeaponStation => "weapon_station",
            Self::ShieldStation => "shield_station",
            Self::PowerupSpot => "powerup_spot",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapObject {
    #[serde(rename = "type")]
    pub kind: ObjectKind,
    pub x: f32,
    pub y: f32,
    /// Thruster: how high it launches, in tiles. Defaults to 8.
    #[serde(default)]
    pub height_tiles: Option<f32>,
    /// Stations: random wait between deployments.
    #[serde(default)]
    pub min_delay_s: Option<f32>,
    #[serde(default)]
    pub max_delay_s: Option<f32>,
    /// Shield station: shield points given (50 small, 100 big). Defaults to 50.
    #[serde(default)]
    pub shield: Option<f32>,
    /// Powerup spot: the powerup id it always offers (random if left out).
    #[serde(default)]
    pub powerup: Option<String>,
}

impl MapObject {
    pub fn feet(&self) -> Vec2 {
        vec2(self.x, self.y)
    }

    pub fn launch_height(&self) -> f32 {
        self.height_tiles.unwrap_or(8.0)
    }

    pub fn delay_range(&self) -> (f32, f32) {
        let (lo, hi): (f32, f32) = match self.kind {
            ObjectKind::ShieldStation => (60.0, 90.0),
            _ => (30.0, 45.0),
        };
        let min = self.min_delay_s.unwrap_or(lo);
        (min, self.max_delay_s.unwrap_or(hi.max(min)))
    }

    pub fn shield_amount(&self) -> f32 {
        self.shield.unwrap_or(50.0)
    }
}

/// Far background style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skyline {
    #[default]
    City,
    Mountains,
    Spires,
    Clouds,
    Girders,
    None,
}

/// Colours and backdrop of a map.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub sky_top: [u8; 3],
    pub sky_bottom: [u8; 3],
    pub far: [u8; 3],
    pub skyline: Skyline,
    pub sun: Option<Vec2>,
    pub sun_color: [u8; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    sky_top: String,
    sky_bottom: String,
    far: String,
    #[serde(default)]
    skyline: Skyline,
    /// Sun position as a fraction of the screen (0..1).
    #[serde(default)]
    sun: Option<[f32; 2]>,
    #[serde(default)]
    sun_color: Option<String>,
}

/// Decorative props, drawn but never collided with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorKind {
    /// Interior back wall panel.
    Wall,
    Window,
    Girder,
    Pipe,
    Lamp,
    Tree,
    Crystal,
    Chain,
    Banner,
    Tank,
    Fence,
    Vines,
    Cactus,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decor {
    #[serde(rename = "type")]
    pub kind: DecorKind,
    pub x: f32,
    pub y: f32,
    #[serde(default = "one")]
    pub w: f32,
    #[serde(default = "one")]
    pub h: f32,
    #[serde(default)]
    pub flip: bool,
    /// Draw in front of fighters instead of behind.
    #[serde(default)]
    pub front: bool,
    #[serde(default)]
    pub color: Option<String>,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShapeFile {
    kind: ShapeKind,
    #[serde(default)]
    material: Option<Material>,
    points: Vec<[f32; 2]>,
    #[serde(default)]
    dps: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapFile {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_tile_size")]
    tile_size: u32,
    #[serde(default)]
    #[allow(dead_code)]
    tileset: Option<String>,
    #[serde(default)]
    background: Option<BackgroundFile>,
    #[serde(default)]
    theme: Option<ThemeFile>,
    modes: Vec<String>,
    #[serde(default)]
    recommended_players: Option<[u32; 2]>,
    /// Team counts this map is fair for, say [2, 4] for a four-corner map.
    /// Default: 2 up to the number of teams that have their own spawns.
    #[serde(default)]
    team_counts: Option<Vec<u8>>,
    // Shape maps.
    #[serde(default)]
    size: Option<[f32; 2]>,
    #[serde(default)]
    shapes: Vec<ShapeFile>,
    #[serde(default)]
    decor: Vec<Decor>,
    // Grid maps.
    #[serde(default)]
    legend: BTreeMap<String, TileKind>,
    #[serde(default)]
    grid: Vec<String>,
    #[serde(default)]
    interior: Vec<String>,
    spawns: Vec<SpawnDef>,
    #[serde(default)]
    pickups: Vec<PickupDef>,
    #[serde(default)]
    objects: Vec<MapObject>,
    /// Reserved for scripted hazards.
    #[serde(default)]
    #[allow(dead_code)]
    hazards: Vec<serde_json::Value>,
    #[serde(default)]
    bot_hints: Vec<BotHint>,
}

fn default_tile_size() -> u32 {
    32
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackgroundFile {
    #[serde(default = "default_sky")]
    sky_color: String,
    #[serde(default)]
    #[allow(dead_code)]
    parallax: Vec<String>,
}

fn default_sky() -> String {
    "#1b1f2a".into()
}

/// A validated map. World units are tiles; y grows downward.
#[derive(Debug, Clone)]
pub struct MapDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Screen pixels per world unit at 720p.
    pub tile_size: u32,
    pub theme: Theme,
    pub modes: Vec<String>,
    pub recommended_players: Option<[u32; 2]>,
    /// Team counts this map supports in team modes (sorted, 2 to 4).
    pub team_counts: Vec<u8>,
    pub width: f32,
    pub height: f32,
    pub terrain: Terrain,
    pub decor: Vec<Decor>,
    backdrop: Vec<Backdrop>,
    backdrop_w: usize,
    pub spawns: Vec<SpawnDef>,
    pub pickups: Vec<PickupDef>,
    pub objects: Vec<MapObject>,
    pub bot_hints: Vec<BotHint>,
}

impl MapDef {
    /// Legacy interior cell (grid maps only).
    pub fn backdrop(&self, x: i32, y: i32) -> Backdrop {
        if x < 0 || y < 0 || self.backdrop_w == 0 || x as usize >= self.backdrop_w {
            return Backdrop::None;
        }
        self.backdrop
            .get(y as usize * self.backdrop_w + x as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn has_backdrop(&self) -> bool {
        self.backdrop.iter().any(|b| *b != Backdrop::None)
    }

    /// Build a bare map from ASCII rows (`#` rock, `=` hatch platform,
    /// `^` spikes, `~` water, `%` sludge, `/` `\` ramps, else air).
    /// Handy for tests and tools; no placements.
    pub fn from_rows(rows: &[&str]) -> Self {
        let tiles: Vec<Vec<TileKind>> = rows
            .iter()
            .map(|r| {
                r.chars()
                    .map(|c| match c {
                        '#' => TileKind::Solid,
                        '=' => TileKind::OneWay,
                        '^' => TileKind::Spikes,
                        '~' => TileKind::Water,
                        '%' => TileKind::Sludge,
                        '/' => TileKind::SlopeUpRight,
                        '\\' => TileKind::SlopeUpLeft,
                        _ => TileKind::Air,
                    })
                    .collect()
            })
            .collect();
        let width = tiles.iter().map(|r| r.len()).max().unwrap_or(0) as f32;
        let height = tiles.len() as f32;
        Self::bare(shapes_from_tiles(&tiles), width, height)
    }

    /// Map from polygons only (tests and tools).
    pub fn from_shapes(shapes: Vec<Shape>, width: f32, height: f32) -> Self {
        Self::bare(shapes, width, height)
    }

    fn bare(shapes: Vec<Shape>, width: f32, height: f32) -> Self {
        Self {
            id: "test".into(),
            name: "Test".into(),
            description: String::new(),
            tile_size: 32,
            theme: legacy_theme([27, 31, 42]),
            modes: Vec::new(),
            recommended_players: None,
            team_counts: vec![2],
            width,
            height,
            terrain: Terrain::new(shapes, width, height),
            decor: Vec::new(),
            backdrop: Vec::new(),
            backdrop_w: 0,
            spawns: Vec::new(),
            pickups: Vec::new(),
            objects: Vec::new(),
            bot_hints: Vec::new(),
        }
    }

    /// The height a body of half-width `half` actually rests at near `feet`.
    /// Like the physics, it stands on the highest of its left, centre and
    /// right feet rays, so on a slope it sits a little above the centre.
    pub fn rest_height(&self, feet: Vec2, half: f32) -> Option<f32> {
        let from = feet.y - 0.5;
        let inset = half - 0.06;
        [-inset, 0.0, inset]
            .into_iter()
            .filter_map(|dx| {
                self.terrain
                    .cast(vec2(feet.x + dx, from), vec2(0.0, 1.0), 1.2, |e| {
                        e.walkable()
                    })
                    .map(|(t, _)| from + t)
            })
            .reduce(f32::min)
    }

    /// Where a body standing near `feet` would rest: the first floor at or
    /// below `feet.y - lift`, if within `max` below.
    pub fn ground_below(&self, feet: Vec2, lift: f32, max: f32) -> Option<f32> {
        let from = feet - vec2(0.0, lift);
        self.terrain
            .cast(from, vec2(0.0, 1.0), lift + max, |e| e.walkable())
            .map(|(t, _)| from.y + t)
    }
}

fn legacy_theme(sky: [u8; 3]) -> Theme {
    let k = |c: [u8; 3], f: f32| {
        [
            (c[0] as f32 * f) as u8,
            (c[1] as f32 * f) as u8,
            (c[2] as f32 * f) as u8,
        ]
    };
    Theme {
        sky_top: sky,
        sky_bottom: k(sky, 0.8),
        far: k(sky, 0.7),
        skyline: Skyline::City,
        sun: None,
        sun_color: [255, 220, 150],
    }
}

/// Convert grid tiles to polygons: merged rectangles of rock and sludge,
/// triangles for ramps, thin hatches for platforms, and hazard/water regions.
pub fn shapes_from_tiles(tiles: &[Vec<TileKind>]) -> Vec<Shape> {
    let h = tiles.len();
    let w = tiles.iter().map(|r| r.len()).max().unwrap_or(0);
    let at = |x: usize, y: usize| {
        tiles
            .get(y)
            .and_then(|r| r.get(x))
            .copied()
            .unwrap_or(TileKind::Air)
    };
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        vec![vec2(x0, y0), vec2(x1, y0), vec2(x1, y1), vec2(x0, y1)]
    };
    let mut shapes = Vec::new();

    // Horizontal runs per row, then stack equal runs into taller rectangles.
    for (kind, shape_kind, material, top, bottom) in [
        (TileKind::Solid, ShapeKind::Solid, Material::Rock, 0.0, 1.0),
        (
            TileKind::Sludge,
            ShapeKind::Solid,
            Material::Sludge,
            0.0,
            1.0,
        ),
        (
            TileKind::OneWay,
            ShapeKind::Hatch,
            Material::Wood,
            0.0,
            0.25,
        ),
        (
            TileKind::Spikes,
            ShapeKind::Hazard,
            Material::Spikes,
            0.4,
            1.0,
        ),
        (TileKind::Water, ShapeKind::Water, Material::Water, 0.0, 1.0),
    ] {
        let mut open: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
        let mut done: Vec<(usize, usize, usize, usize)> = Vec::new();
        for y in 0..h {
            let mut runs = Vec::new();
            let mut x = 0;
            while x < w {
                if at(x, y) == kind {
                    let s = x;
                    while x < w && at(x, y) == kind {
                        x += 1;
                    }
                    runs.push((s, x));
                } else {
                    x += 1;
                }
            }
            let mut next = HashMap::new();
            for run in runs {
                // Platforms and spikes stay one row tall; rock and water stack.
                let stack = matches!(kind, TileKind::Solid | TileKind::Sludge | TileKind::Water);
                match open.remove(&run) {
                    Some((y0, _)) if stack => {
                        next.insert(run, (y0, y));
                    }
                    Some(prev) => {
                        done.push((run.0, run.1, prev.0, prev.1));
                        next.insert(run, (y, y));
                    }
                    None => {
                        next.insert(run, (y, y));
                    }
                }
            }
            for (run, (y0, y1)) in open.drain() {
                done.push((run.0, run.1, y0, y1));
            }
            open = next;
        }
        for (run, (y0, y1)) in open.drain() {
            done.push((run.0, run.1, y0, y1));
        }
        for (x0, x1, y0, y1) in done {
            shapes.push(Shape {
                kind: shape_kind,
                material,
                points: rect(x0 as f32, y0 as f32 + top, x1 as f32, y1 as f32 + bottom),
                dps: if shape_kind == ShapeKind::Hazard {
                    SPIKE_DPS
                } else {
                    0.0
                },
            });
        }
    }
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let pts = match at(x, y) {
                TileKind::SlopeUpRight => vec![
                    vec2(fx, fy + 1.0),
                    vec2(fx + 1.0, fy),
                    vec2(fx + 1.0, fy + 1.0),
                ],
                TileKind::SlopeUpLeft => {
                    vec![vec2(fx, fy), vec2(fx + 1.0, fy + 1.0), vec2(fx, fy + 1.0)]
                }
                _ => continue,
            };
            shapes.push(Shape {
                kind: ShapeKind::Solid,
                material: Material::Rock,
                points: pts,
                dps: 0.0,
            });
        }
    }
    shapes
}

/// Parse and validate one map file against the known weapon and mode ids.
pub fn parse_map(
    label: &str,
    stem: &str,
    text: &str,
    weapon_ids: &[&str],
    mode_ids: &[&str],
) -> Result<MapDef, Vec<DataError>> {
    let mut file: MapFile = parse_json(label, text).map_err(|e| vec![e])?;
    let mut errs: Vec<String> = Vec::new();

    if !is_snake_case(&file.id) {
        errs.push(format!("id: \"{}\" must be snake_case", file.id));
    }
    if file.id != stem {
        errs.push(format!(
            "id: \"{}\" must match the file name \"{stem}\"",
            file.id
        ));
    }
    if file.tile_size == 0 {
        errs.push("tile_size: must be greater than 0".into());
    }
    for (i, m) in file.modes.iter().enumerate() {
        if !mode_ids.contains(&m.as_str()) {
            errs.push(format!(
                "modes[{i}]: unknown mode \"{m}\" (not in modes.json)"
            ));
        }
    }
    if file.modes.is_empty() {
        errs.push("modes: must list at least one mode".into());
    }

    let color = |field: &str, s: &str, errs: &mut Vec<String>| {
        parse_hex_color(s).unwrap_or_else(|| {
            errs.push(format!("{field}: \"{s}\" is not a #rrggbb color"));
            [0, 0, 0]
        })
    };
    let theme = match &file.theme {
        Some(t) => Theme {
            sky_top: color("theme.sky_top", &t.sky_top, &mut errs),
            sky_bottom: color("theme.sky_bottom", &t.sky_bottom, &mut errs),
            far: color("theme.far", &t.far, &mut errs),
            skyline: t.skyline,
            sun: t.sun.map(|[x, y]| vec2(x, y)),
            sun_color: match &t.sun_color {
                Some(c) => color("theme.sun_color", c, &mut errs),
                None => [255, 220, 150],
            },
        },
        None => {
            let sky = file
                .background
                .as_ref()
                .map_or_else(default_sky, |b| b.sky_color.clone());
            legacy_theme(color("background.sky_color", &sky, &mut errs))
        }
    };
    for (i, d) in file.decor.iter().enumerate() {
        if let Some(c) = &d.color {
            color(&format!("decor[{i}].color"), c, &mut errs);
        }
    }

    let legacy = !file.grid.is_empty();
    let (shapes, width, height, backdrop, backdrop_w) = if legacy {
        if !file.shapes.is_empty() {
            errs.push("shapes: use either grid or shapes, not both".into());
        }
        match legacy_grid(&file, &mut errs) {
            Some(v) => v,
            None => return Err(to_errors(label, errs)),
        }
    } else {
        let Some([w, h]) = file.size else {
            errs.push("size: required for shape maps, e.g. [64, 36]".into());
            return Err(to_errors(label, errs));
        };
        if w < 8.0 || h < 8.0 {
            errs.push(format!("size: must be at least 8 x 8 (got {w} x {h})"));
        }
        let mut shapes = Vec::new();
        for (i, s) in file.shapes.iter().enumerate() {
            if s.points.len() < 3 {
                errs.push(format!("shapes[{i}].points: needs at least 3 points"));
                continue;
            }
            let material = s.material.unwrap_or(match s.kind {
                ShapeKind::Water => Material::Water,
                ShapeKind::Hazard => Material::Spikes,
                _ => Material::Rock,
            });
            shapes.push(Shape {
                kind: s.kind,
                material,
                points: s.points.iter().map(|&[x, y]| vec2(x, y)).collect(),
                dps: s.dps.unwrap_or(if s.kind == ShapeKind::Hazard {
                    SPIKE_DPS
                } else {
                    0.0
                }),
            });
        }
        (shapes, w, h, Vec::new(), 0)
    };
    if !errs.is_empty() {
        return Err(to_errors(label, errs));
    }

    // Grid maps place things by tile cell; move them to feet positions.
    if legacy {
        for s in &mut file.spawns {
            s.x += 0.5;
            s.y += 1.0;
        }
        for p in &mut file.pickups {
            p.x += 0.5;
            p.y += 1.0;
        }
        for o in &mut file.objects {
            o.x += 0.5;
            o.y += 1.0;
        }
        for b in &mut file.bot_hints {
            b.x += 0.5;
            b.y += 1.0;
        }
    }

    let file_team_counts = file.team_counts.clone();
    let mut map = MapDef {
        id: file.id,
        name: file.name,
        description: file.description,
        tile_size: file.tile_size,
        theme,
        modes: file.modes,
        recommended_players: file.recommended_players,
        team_counts: Vec::new(),
        width,
        height,
        terrain: Terrain::new(shapes, width, height),
        decor: file.decor,
        backdrop,
        backdrop_w,
        spawns: file.spawns,
        pickups: file.pickups,
        objects: file.objects,
        bot_hints: file.bot_hints,
    };

    // Spawns.
    // Teams 0 and 1 need their own spawns. Teams 2 and 3 may have some; if
    // not, they borrow the free-for-all spawns in 3 and 4 team matches.
    let mut counts = [0usize; 4];
    let mut ffa = 0usize;
    for (i, s) in map.spawns.iter().enumerate() {
        let what = format!("spawns[{i}]");
        check_placement(&map, &what, s.feet(), 3.0, &mut errs);
        match s.team {
            None => ffa += 1,
            Some(t @ 0..=3) => counts[t as usize] += 1,
            Some(t) => errs.push(format!("{what}.team: must be 0 to 3 or null (got {t})")),
        }
    }
    for (team, n) in counts.iter().enumerate() {
        if *n < 2 && (team < 2 || *n > 0) {
            errs.push(format!(
                "spawns: needs at least 2 spawns for team {team}, found {n}"
            ));
        }
    }
    if ffa < 4 {
        errs.push(format!(
            "spawns: needs at least 4 free-for-all spawns (team null), found {ffa}"
        ));
    }
    // Team counts: in a match with n teams, every team below n needs at
    // least 2 spawns (after the spawns' `by_teams` overrides).
    let fair_for = |n: usize| {
        (0..n).all(|t| {
            map.spawns
                .iter()
                .filter(|s| s.team_for(n) == Some(t as u8))
                .count()
                >= 2
        })
    };
    for (i, s) in map.spawns.iter().enumerate() {
        for n in 2..=4 {
            if let Some(Some(t)) = s.by_teams.0[n - 2]
                && t as usize >= n
            {
                errs.push(format!(
                    "spawns[{i}].by_teams: team {t} does not exist in a {n} team match"
                ));
            }
        }
    }
    let with_spawns = (2..=4).filter(|&n| fair_for(n)).max().unwrap_or(2) as u8;
    map.team_counts = match &file_team_counts {
        Some(list) => {
            let mut list = list.clone();
            list.sort_unstable();
            list.dedup();
            for &n in &list {
                if !(2..=4).contains(&n) {
                    errs.push(format!("team_counts: {n} is not 2, 3 or 4"));
                } else if !fair_for(n as usize) {
                    errs.push(format!(
                        "team_counts: {n} teams needs 2 spawns each for teams 0 to {}",
                        n - 1
                    ));
                }
            }
            if list.is_empty() {
                errs.push("team_counts: list at least one team count".into());
            }
            list
        }
        None => (2..=with_spawns)
            .filter(|&n| fair_for(n as usize))
            .collect(),
    };

    // Pickups.
    for (i, p) in map.pickups.iter().enumerate() {
        let what = format!("pickups[{i}]");
        check_placement(&map, &what, p.feet(), 3.0, &mut errs);
        match (&p.kind, &p.weapon_id) {
            (PickupKind::Weapon, None) => {
                errs.push(format!("{what}.weapon_id: required for weapon pickups"))
            }
            (PickupKind::Weapon, Some(id)) if !weapon_ids.contains(&id.as_str()) => errs.push(
                format!("{what}.weapon_id: \"{id}\" does not exist in weapons/"),
            ),
            (PickupKind::Health | PickupKind::Ammo, Some(_)) => errs.push(format!(
                "{what}.weapon_id: only weapon pickups take a weapon_id"
            )),
            _ => {}
        }
        if p.respawn_s < 0.0 {
            errs.push(format!("{what}.respawn_s: must be 0 or more"));
        }
    }

    // Objects.
    for (i, o) in map.objects.iter().enumerate() {
        let what = format!("objects[{i}]");
        let max_drop = if o.kind == ObjectKind::Thruster {
            0.3
        } else {
            3.0
        };
        check_placement(&map, &what, o.feet(), max_drop, &mut errs);
        let only = |field: &str, ok: bool, errs: &mut Vec<String>| {
            if !ok {
                errs.push(format!(
                    "{what}.{field}: not used by {} objects",
                    o.kind.name()
                ));
            }
        };
        let station = matches!(
            o.kind,
            ObjectKind::WeaponStation | ObjectKind::ShieldStation
        );
        only(
            "height_tiles",
            o.height_tiles.is_none() || o.kind == ObjectKind::Thruster,
            &mut errs,
        );
        only("min_delay_s", o.min_delay_s.is_none() || station, &mut errs);
        only("max_delay_s", o.max_delay_s.is_none() || station, &mut errs);
        only(
            "shield",
            o.shield.is_none() || o.kind == ObjectKind::ShieldStation,
            &mut errs,
        );
        only(
            "powerup",
            o.powerup.is_none() || o.kind == ObjectKind::PowerupSpot,
            &mut errs,
        );
        if o.kind == ObjectKind::Thruster {
            let h = o.launch_height();
            if !(2.0..=40.0).contains(&h) {
                errs.push(format!(
                    "{what}.height_tiles: must be between 2 and 40 (got {h})"
                ));
            }
        }
        if station {
            let (lo, hi) = o.delay_range();
            if lo <= 0.0 || hi < lo {
                errs.push(format!(
                    "{what}: min_delay_s must be > 0 and max_delay_s >= min_delay_s (got {lo}, {hi})"
                ));
            }
        }
        if o.kind == ObjectKind::ShieldStation && !(1.0..=100.0).contains(&o.shield_amount()) {
            errs.push(format!(
                "{what}.shield: must be between 1 and 100 (got {})",
                o.shield_amount()
            ));
        }
    }

    if errs.is_empty() {
        Ok(map)
    } else {
        Err(to_errors(label, errs))
    }
}

type Legacy = (Vec<Shape>, f32, f32, Vec<Backdrop>, usize);

fn legacy_grid(file: &MapFile, errs: &mut Vec<String>) -> Option<Legacy> {
    let mut legend: HashMap<char, TileKind> = HashMap::new();
    for (key, kind) in &file.legend {
        let mut chars = key.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => {
                legend.insert(c, *kind);
            }
            _ => errs.push(format!(
                "legend: key \"{key}\" must be exactly one character"
            )),
        }
    }
    let width = file.grid.first().map_or(0, |r| r.chars().count());
    let mut tiles = Vec::new();
    for (y, row) in file.grid.iter().enumerate() {
        let len = row.chars().count();
        if len != width {
            errs.push(format!(
                "grid[{y}]: has {len} columns, expected {width} (same as grid[0])"
            ));
            continue;
        }
        let mut r = Vec::new();
        for (x, c) in row.chars().enumerate() {
            match legend.get(&c) {
                Some(k) => r.push(*k),
                None => {
                    errs.push(format!(
                        "grid[{y}] column {x}: character '{c}' is not in legend"
                    ));
                    r.push(TileKind::Air);
                }
            }
        }
        tiles.push(r);
    }
    let mut backdrop = Vec::new();
    if !file.interior.is_empty() {
        if file.interior.len() != file.grid.len() {
            errs.push(format!(
                "interior: has {} rows, expected {} (same as grid)",
                file.interior.len(),
                file.grid.len()
            ));
        }
        for (y, row) in file.interior.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                backdrop.push(match c {
                    '.' => Backdrop::None,
                    '#' => Backdrop::Wall,
                    'w' => Backdrop::Window,
                    other => {
                        errs.push(format!(
                            "interior[{y}] column {x}: '{other}' must be '.', '#' or 'w'"
                        ));
                        Backdrop::None
                    }
                });
            }
        }
    }
    if !errs.is_empty() {
        return None;
    }
    Some((
        shapes_from_tiles(&tiles),
        width as f32,
        file.grid.len() as f32,
        backdrop,
        width,
    ))
}

/// A placement must have room for a body and floor not far below.
fn check_placement(map: &MapDef, what: &str, feet: Vec2, max_drop: f32, errs: &mut Vec<String>) {
    if feet.x < 0.0 || feet.y < 0.0 || feet.x > map.width || feet.y > map.height {
        errs.push(format!(
            "{what}: ({}, {}) is outside the {} x {} map",
            feet.x, feet.y, map.width, map.height
        ));
        return;
    }
    // Check the body where it would really stand (slopes hold it a bit higher).
    let rest = map
        .rest_height(feet, 0.35)
        .filter(|r| (r - feet.y).abs() < 0.5)
        .unwrap_or(feet.y);
    let lifted = vec2(feet.x, rest - 0.08);
    let body_min = vec2(lifted.x - 0.3, lifted.y - 1.7);
    let body_max = vec2(lifted.x + 0.3, lifted.y);
    if map.terrain.box_blocked(body_min, body_max) {
        errs.push(format!("{what}: is inside solid terrain"));
        return;
    }
    match map.ground_below(feet, 0.2, max_drop) {
        Some(_) => {}
        None => errs.push(format!(
            "{what}: has no floor within {max_drop} tiles below"
        )),
    }
}

fn to_errors(label: &str, msgs: Vec<String>) -> Vec<DataError> {
    msgs.into_iter().map(|m| DataError::new(label, m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_json(grid: &[&str], spawns: &str, pickups: &str) -> String {
        format!(
            r##"{{
              "id": "test_map", "name": "Test",
              "modes": ["deathmatch"],
              "legend": {{ "#": "solid", "=": "one_way", "^": "spikes", "~": "water", ".": "air" }},
              "grid": {grid},
              "spawns": {spawns},
              "pickups": {pickups}
            }}"##,
            grid = serde_json::to_string(grid).unwrap()
        )
    }

    const GRID: &[&str] = &[
        "##########",
        "#........#",
        "#........#",
        "#..====..#",
        "#........#",
        "#........#",
        "##########",
    ];
    const SPAWNS: &str = r#"[
        {"x":1,"y":5,"team":0},{"x":2,"y":5,"team":0},
        {"x":7,"y":5,"team":1},{"x":8,"y":5,"team":1},
        {"x":3,"y":5},{"x":4,"y":5},{"x":5,"y":5},{"x":4,"y":2}
    ]"#;

    fn parse(text: &str) -> Result<MapDef, Vec<DataError>> {
        parse_map(
            "maps/test_map.json",
            "test_map",
            text,
            &["rail_spike"],
            &["deathmatch"],
        )
    }

    #[test]
    fn grid_map_parses_into_polygons() {
        let pickups = r#"[{"type":"weapon","x":3,"y":2,"weapon_id":"rail_spike","respawn_s":40}]"#;
        let map = parse(&grid_json(GRID, SPAWNS, pickups)).unwrap();
        assert_eq!((map.width, map.height), (10.0, 7.0));
        assert!(map.terrain.point_solid(vec2(0.5, 0.5)));
        assert!(!map.terrain.point_solid(vec2(4.5, 4.5)));
        // Cell (1, 5) becomes feet at the bottom-centre of the cell.
        assert_eq!(map.spawns[0].feet(), vec2(1.5, 6.0));
        assert_eq!(
            map.ground_below(vec2(4.5, 2.5), 0.2, 3.0),
            Some(3.0),
            "standing on the platform"
        );
    }

    #[test]
    fn spawn_inside_solid_is_reported() {
        let spawns = SPAWNS.replace(r#"{"x":5,"y":5}"#, r#"{"x":5,"y":6}"#);
        let err = parse(&grid_json(GRID, &spawns, "[]")).unwrap_err();
        assert_eq!(
            err[0].to_string(),
            "maps/test_map.json: spawns[6]: is inside solid terrain"
        );
    }

    #[test]
    fn uneven_rows_and_unknown_chars() {
        let mut grid = GRID.to_vec();
        grid[1] = "#.......#";
        grid[2] = "#...x....#";
        let err = parse(&grid_json(&grid, SPAWNS, "[]")).unwrap_err();
        let msgs: Vec<String> = err.iter().map(|e| e.message.clone()).collect();
        assert!(msgs[0].starts_with("grid[1]: has 9 columns"), "{msgs:?}");
        assert!(msgs[1].contains("'x' is not in legend"), "{msgs:?}");
    }

    #[test]
    fn unknown_weapon_and_spawn_counts() {
        let spawns = r#"[{"x":1,"y":5,"team":0},{"x":7,"y":5,"team":1},{"x":3,"y":5}]"#;
        let pickups = r#"[{"type":"weapon","x":3,"y":2,"weapon_id":"nope"}]"#;
        let err = parse(&grid_json(GRID, spawns, pickups)).unwrap_err();
        let all = err
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains("needs at least 2 spawns for team 0, found 1"),
            "{all}"
        );
        assert!(all.contains("at least 4 free-for-all spawns"), "{all}");
        assert!(
            all.contains("pickups[0].weapon_id: \"nope\" does not exist"),
            "{all}"
        );
    }

    #[test]
    fn extra_team_spawns_are_checked() {
        // Team 2 with a single spawn, and a team number that does not exist.
        let spawns = r#"[{"x":1,"y":5,"team":0},{"x":2,"y":5,"team":0},
            {"x":7,"y":5,"team":1},{"x":8,"y":5,"team":1},
            {"x":3,"y":5,"team":2},{"x":4,"y":5,"team":4},
            {"x":3,"y":5},{"x":4,"y":5},{"x":5,"y":5},{"x":6,"y":5}]"#;
        let err = parse(&grid_json(GRID, spawns, "[]")).unwrap_err();
        let all = err
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains("needs at least 2 spawns for team 2, found 1"),
            "{all}"
        );
        assert!(
            all.contains("spawns[5].team: must be 0 to 3 or null (got 4)"),
            "{all}"
        );
        assert!(
            !all.contains("team 3"),
            "team 3 has no spawns, which is fine: {all}"
        );
    }

    #[test]
    fn team_counts_default_and_explicit() {
        let four = r#"[{"x":1,"y":5,"team":0},{"x":2,"y":5,"team":0},
            {"x":7,"y":5,"team":1},{"x":8,"y":5,"team":1},
            {"x":3,"y":5,"team":2},{"x":4,"y":5,"team":2},
            {"x":5,"y":5,"team":3},{"x":6,"y":5,"team":3},
            {"x":3,"y":5},{"x":4,"y":5},{"x":5,"y":5},{"x":6,"y":5}]"#;
        // Default: 2 up to the teams that have spawns.
        assert_eq!(
            parse(&grid_json(GRID, SPAWNS, "[]")).unwrap().team_counts,
            vec![2]
        );
        assert_eq!(
            parse(&grid_json(GRID, four, "[]")).unwrap().team_counts,
            vec![2, 3, 4]
        );
        // Explicit list, sorted.
        let text = grid_json(GRID, four, "[]").replacen('{', r#"{ "team_counts": [4, 2],"#, 1);
        assert_eq!(parse(&text).unwrap().team_counts, vec![2, 4]);
        // Asking for more teams than have spawns, or a silly count.
        let text = grid_json(GRID, SPAWNS, "[]").replacen('{', r#"{ "team_counts": [2, 3, 5],"#, 1);
        let all = parse(&text)
            .unwrap_err()
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains("team_counts: 3 teams needs 2 spawns each for teams 0 to 2"),
            "{all}"
        );
        assert!(all.contains("team_counts: 5 is not 2, 3 or 4"), "{all}");
    }

    #[test]
    fn spawns_can_change_teams_with_the_team_count() {
        // Four corners for 2 and 4 teams; for 3 teams, team 2 moves from its
        // corner to two free spawns in the middle.
        let spawns = r#"[{"x":1,"y":5,"team":0},{"x":2,"y":5,"team":0},
            {"x":7,"y":5,"team":1},{"x":8,"y":5,"team":1},
            {"x":1,"y":2,"team":2,"by_teams":{"3":null}},{"x":2,"y":2,"team":2,"by_teams":{"3":null}},
            {"x":7,"y":2,"team":3},{"x":8,"y":2,"team":3},
            {"x":4,"y":5,"by_teams":{"3":2}},{"x":5,"y":5,"by_teams":{"3":2}},
            {"x":3,"y":5},{"x":6,"y":5}]"#;
        let map = parse(&grid_json(GRID, spawns, "[]")).unwrap();
        assert_eq!(map.team_counts, vec![2, 3, 4]);
        let owners = |n| map.spawns.iter().map(|s| s.team_for(n)).collect::<Vec<_>>();
        assert_eq!(owners(3)[4], None);
        assert_eq!(owners(3)[8], Some(2));
        assert_eq!(owners(4)[4], Some(2));
        assert_eq!(owners(4)[8], None);
        assert_eq!(owners(0)[8], None, "free-for-all keeps it a free spawn");
        // Without the middle spawns, 3 teams would lack team 2 spawns.
        let broken = spawns.replace(r#"{"3":2}"#, "{}");
        assert_eq!(
            parse(&grid_json(GRID, &broken, "[]")).unwrap().team_counts,
            vec![2, 4]
        );
        // An owner that does not exist at that count.
        let bad = spawns.replace(
            r#"{"x":4,"y":5,"by_teams":{"3":2}}"#,
            r#"{"x":4,"y":5,"by_teams":{"3":3}}"#,
        );
        let all = parse(&grid_json(GRID, &bad, "[]"))
            .unwrap_err()
            .iter()
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains("spawns[8].by_teams: team 3 does not exist in a 3 team match"),
            "{all}"
        );
    }

    #[test]
    fn shape_map_parses() {
        let text = r##"{
          "id": "test_map", "name": "Test", "modes": ["deathmatch"], "size": [20, 10],
          "theme": { "sky_top": "#102030", "sky_bottom": "#203040", "far": "#101010", "skyline": "mountains" },
          "shapes": [
            { "kind": "solid", "material": "rock", "points": [[0,8],[20,8],[20,10],[0,10]] },
            { "kind": "solid", "material": "metal", "points": [[6,8],[10,6],[14,8]] }
          ],
          "spawns": [
            {"x":1.5,"y":8,"team":0},{"x":2.5,"y":8,"team":0},{"x":17.5,"y":8,"team":1},{"x":18.5,"y":8,"team":1},
            {"x":4,"y":8},{"x":16,"y":8},{"x":10,"y":6},{"x":3,"y":8}
          ]
        }"##;
        let map = parse(text).unwrap();
        assert_eq!(map.theme.skyline, Skyline::Mountains);
        assert_eq!(
            map.ground_below(vec2(10.0, 5.0), 0.2, 3.0),
            Some(6.0),
            "top of the hill"
        );
        let g = map.ground_below(vec2(8.0, 5.0), 0.2, 3.0).unwrap();
        assert!((g - 7.0).abs() < 1e-4, "halfway up the slope: {g}");
    }
}
