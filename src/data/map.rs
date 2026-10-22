use super::{DataError, is_snake_case, parse_hex_color, parse_json};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileKind {
    Air,
    Solid,
    /// Platform you can jump up through and stand on.
    OneWay,
    Spikes,
    Water,
    /// Ramp whose floor rises toward the right (`/`).
    SlopeUpRight,
    /// Ramp whose floor rises toward the left (`\`).
    SlopeUpLeft,
    /// Crusty buildup. Blocks like rock; looks like sludge.
    Sludge,
}

impl TileKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::Solid => "solid",
            Self::OneWay => "one_way",
            Self::Spikes => "spikes",
            Self::Water => "water",
            Self::SlopeUpRight => "slope_up_right",
            Self::SlopeUpLeft => "slope_up_left",
            Self::Sludge => "sludge",
        }
    }

    /// Fully blocking tiles (rock and sludge).
    pub fn is_solid(self) -> bool {
        matches!(self, Self::Solid | Self::Sludge)
    }

    pub fn is_slope(self) -> bool {
        matches!(self, Self::SlopeUpRight | Self::SlopeUpLeft)
    }

    /// Something a fighter can stand on top of.
    pub fn is_floor(self) -> bool {
        self.is_solid() || self.is_slope() || self == Self::OneWay
    }

    /// For a slope tile at row `cy`, the floor height at local x `u` (0..1).
    pub fn slope_surface(self, cy: i32, u: f32) -> Option<f32> {
        let u = u.clamp(0.0, 1.0);
        match self {
            Self::SlopeUpRight => Some(cy as f32 + 1.0 - u),
            Self::SlopeUpLeft => Some(cy as f32 + u),
            _ => None,
        }
    }
}

/// Purely visual layer behind the play area, for building interiors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    #[default]
    None,
    /// Interior wall (`#` in the `interior` rows).
    Wall,
    /// Interior wall with a window (`w`).
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnDef {
    pub x: i32,
    pub y: i32,
    /// `None` = free-for-all spawn.
    #[serde(default)]
    pub team: Option<u8>,
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
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub weapon_id: Option<String>,
    #[serde(default = "default_respawn")]
    pub respawn_s: f32,
    /// Health restored (health pickups only). Defaults to 50.
    #[serde(default)]
    pub amount: Option<f32>,
}

fn default_respawn() -> f32 {
    20.0
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotHint {
    pub x: i32,
    pub y: i32,
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
    /// Fixed spot where a random powerup appears.
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
    pub x: i32,
    pub y: i32,
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
    background: BackgroundFile,
    modes: Vec<String>,
    #[serde(default)]
    recommended_players: Option<[u32; 2]>,
    legend: BTreeMap<String, TileKind>,
    grid: Vec<String>,
    /// Optional backdrop rows, same size as `grid`: `#` interior wall,
    /// `w` window, `.` open sky.
    #[serde(default)]
    interior: Vec<String>,
    spawns: Vec<SpawnDef>,
    #[serde(default)]
    pickups: Vec<PickupDef>,
    /// Reserved for scripted hazards (not in Milestone 1).
    #[serde(default)]
    #[allow(dead_code)]
    hazards: Vec<serde_json::Value>,
    #[serde(default)]
    objects: Vec<MapObject>,
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

impl Default for BackgroundFile {
    fn default() -> Self {
        Self {
            sky_color: default_sky(),
            parallax: Vec::new(),
        }
    }
}

fn default_sky() -> String {
    "#1b1f2a".into()
}

/// A validated map. Coordinates are in tiles, y grows downward.
#[derive(Debug, Clone, PartialEq)]
pub struct MapDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tile_size: u32,
    pub sky_color: [u8; 3],
    pub modes: Vec<String>,
    pub recommended_players: Option<[u32; 2]>,
    pub width: usize,
    pub height: usize,
    tiles: Vec<TileKind>,
    backdrop: Vec<Backdrop>,
    pub spawns: Vec<SpawnDef>,
    pub pickups: Vec<PickupDef>,
    pub objects: Vec<MapObject>,
    pub bot_hints: Vec<BotHint>,
}

impl MapDef {
    /// Tile at a grid cell. Everything outside the grid counts as solid,
    /// so nothing can ever leave the map.
    pub fn tile(&self, x: i32, y: i32) -> TileKind {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            TileKind::Solid
        } else {
            self.tiles[y as usize * self.width + x as usize]
        }
    }

    pub fn backdrop(&self, x: i32, y: i32) -> Backdrop {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return Backdrop::None;
        }
        self.backdrop
            .get(y as usize * self.width + x as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn has_backdrop(&self) -> bool {
        self.backdrop.iter().any(|b| *b != Backdrop::None)
    }

    /// Build a bare map from ASCII rows with the default legend
    /// (`#` solid, `=` one-way, `^` spikes, `~` water, anything else air).
    /// Handy for tests and tools; no spawns or pickups.
    pub fn from_rows(rows: &[&str]) -> Self {
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut tiles = Vec::with_capacity(width * rows.len());
        for row in rows {
            let mut chars = row.chars();
            for _ in 0..width {
                tiles.push(match chars.next() {
                    Some('#') => TileKind::Solid,
                    Some('/') => TileKind::SlopeUpRight,
                    Some('\\') => TileKind::SlopeUpLeft,
                    Some('%') => TileKind::Sludge,
                    Some('=') => TileKind::OneWay,
                    Some('^') => TileKind::Spikes,
                    Some('~') => TileKind::Water,
                    _ => TileKind::Air,
                });
            }
        }
        Self {
            id: "test".into(),
            name: "Test".into(),
            description: String::new(),
            tile_size: 32,
            sky_color: [27, 31, 42],
            modes: Vec::new(),
            recommended_players: None,
            width,
            height: rows.len(),
            tiles,
            backdrop: Vec::new(),
            spawns: Vec::new(),
            pickups: Vec::new(),
            objects: Vec::new(),
            bot_hints: Vec::new(),
        }
    }

    /// Scan downward from (x, y+1) for something to stand on.
    fn has_ground_below(&self, x: i32, y: i32) -> bool {
        let mut yy = y + 1;
        loop {
            match self.tile(x, yy) {
                TileKind::Air | TileKind::Water => yy += 1,
                k if k.is_floor() => return true,
                _ => return false,
            }
        }
    }
}

/// Parse and validate one map file against the known weapon and mode ids.
pub fn parse_map(
    label: &str,
    stem: &str,
    text: &str,
    weapon_ids: &[&str],
    mode_ids: &[&str],
) -> Result<MapDef, Vec<DataError>> {
    let file: MapFile = parse_json(label, text).map_err(|e| vec![e])?;
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
    let sky_color = parse_hex_color(&file.background.sky_color).unwrap_or_else(|| {
        errs.push(format!(
            "background.sky_color: \"{}\" is not a #rrggbb color",
            file.background.sky_color
        ));
        [0, 0, 0]
    });
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

    // Legend and grid shape. Stop here if the grid itself is broken.
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
    if file.grid.is_empty() {
        errs.push("grid: must have at least one row".into());
    }
    let width = file.grid.first().map_or(0, |r| r.chars().count());
    let mut tiles = Vec::with_capacity(width * file.grid.len());
    for (y, row) in file.grid.iter().enumerate() {
        let len = row.chars().count();
        if len != width {
            errs.push(format!(
                "grid[{y}]: has {len} columns, expected {width} (same as grid[0])"
            ));
            continue;
        }
        for (x, c) in row.chars().enumerate() {
            match legend.get(&c) {
                Some(k) => tiles.push(*k),
                None => {
                    errs.push(format!(
                        "grid[{y}] column {x}: character '{c}' is not in legend"
                    ));
                    tiles.push(TileKind::Air);
                }
            }
        }
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
            if row.chars().count() != width {
                errs.push(format!(
                    "interior[{y}]: has {} columns, expected {width}",
                    row.chars().count()
                ));
                continue;
            }
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
        return Err(to_errors(label, errs));
    }

    let map = MapDef {
        id: file.id,
        name: file.name,
        description: file.description,
        tile_size: file.tile_size,
        sky_color,
        modes: file.modes,
        recommended_players: file.recommended_players,
        width,
        height: file.grid.len(),
        tiles,
        backdrop,
        spawns: file.spawns,
        pickups: file.pickups,
        objects: file.objects,
        bot_hints: file.bot_hints,
    };

    // Spawns.
    let mut counts = [0usize; 2];
    let mut ffa = 0usize;
    for (i, s) in map.spawns.iter().enumerate() {
        let what = format!("spawns[{i}]");
        check_placement(&map, &what, s.x, s.y, true, &mut errs);
        match s.team {
            None => ffa += 1,
            Some(t @ 0..=1) => counts[t as usize] += 1,
            Some(t) => errs.push(format!("{what}.team: must be 0, 1 or null (got {t})")),
        }
    }
    for (team, n) in counts.iter().enumerate() {
        if *n < 2 {
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

    // Pickups.
    for (i, p) in map.pickups.iter().enumerate() {
        let what = format!("pickups[{i}]");
        check_placement(&map, &what, p.x, p.y, false, &mut errs);
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
        check_placement(&map, &what, o.x, o.y, true, &mut errs);
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
            if !(2.0..=30.0).contains(&h) {
                errs.push(format!(
                    "{what}.height_tiles: must be between 2 and 30 (got {h})"
                ));
            }
            if !map.tile(o.x, o.y + 1).is_solid() {
                errs.push(format!(
                    "{what}: thrusters must sit directly on a solid tile"
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

    for (i, h) in map.bot_hints.iter().enumerate() {
        if map.tile(h.x, h.y).is_solid() {
            errs.push(format!("bot_hints[{i}]: is inside a solid tile"));
        }
    }

    if errs.is_empty() {
        Ok(map)
    } else {
        Err(to_errors(label, errs))
    }
}

fn check_placement(
    map: &MapDef,
    what: &str,
    x: i32,
    y: i32,
    needs_headroom: bool,
    errs: &mut Vec<String>,
) {
    if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height {
        errs.push(format!(
            "{what}: ({x}, {y}) is outside the {}x{} grid",
            map.width, map.height
        ));
        return;
    }
    let kind = map.tile(x, y);
    if kind != TileKind::Air {
        errs.push(format!("{what}: is inside a {} tile", kind.name()));
        return;
    }
    // Fighters are just under 2 tiles tall.
    if needs_headroom && map.tile(x, y - 1).is_solid() {
        errs.push(format!("{what}: has no headroom (tile above is solid)"));
    }
    if !map.has_ground_below(x, y) {
        errs.push(format!("{what}: has no solid or one-way ground below"));
    }
}

fn to_errors(label: &str, msgs: Vec<String>) -> Vec<DataError> {
    msgs.into_iter().map(|m| DataError::new(label, m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_json(grid: &[&str], spawns: &str, pickups: &str) -> String {
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
    fn valid_map_parses() {
        let pickups = r#"[{"type":"weapon","x":3,"y":2,"weapon_id":"rail_spike","respawn_s":40}]"#;
        let map = parse(&map_json(GRID, SPAWNS, pickups)).unwrap();
        assert_eq!((map.width, map.height), (10, 7));
        assert_eq!(map.tile(3, 3), TileKind::OneWay);
        assert_eq!(map.tile(-1, 0), TileKind::Solid);
    }

    #[test]
    fn spawn_inside_solid_is_reported_like_spec() {
        let spawns = SPAWNS.replace(r#"{"x":5,"y":5}"#, r#"{"x":5,"y":6}"#);
        let err = parse(&map_json(GRID, &spawns, "[]")).unwrap_err();
        assert_eq!(
            err[0].to_string(),
            "maps/test_map.json: spawns[6]: is inside a solid tile"
        );
    }

    #[test]
    fn uneven_rows_and_unknown_chars() {
        let mut grid = GRID.to_vec();
        grid[1] = "#.......#";
        grid[2] = "#...x....#";
        let err = parse(&map_json(&grid, SPAWNS, "[]")).unwrap_err();
        let msgs: Vec<String> = err.iter().map(|e| e.message.clone()).collect();
        assert!(msgs[0].starts_with("grid[1]: has 9 columns"), "{msgs:?}");
        assert!(msgs[1].contains("'x' is not in legend"), "{msgs:?}");
    }

    #[test]
    fn unknown_weapon_and_spawn_counts() {
        let spawns = r#"[{"x":1,"y":5,"team":0},{"x":7,"y":5,"team":1},{"x":3,"y":5}]"#;
        let pickups = r#"[{"type":"weapon","x":3,"y":2,"weapon_id":"nope"}]"#;
        let err = parse(&map_json(GRID, spawns, pickups)).unwrap_err();
        let all = err
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all.contains("needs at least 2 spawns for team 0, found 1"),
            "{all}"
        );
        assert!(
            all.contains("needs at least 2 spawns for team 1, found 1"),
            "{all}"
        );
        assert!(all.contains("at least 4 free-for-all spawns"), "{all}");
        assert!(
            all.contains("pickups[0].weapon_id: \"nope\" does not exist"),
            "{all}"
        );
    }

    #[test]
    fn spawn_over_spikes_has_no_ground() {
        let mut grid = GRID.to_vec();
        grid[5] = "#^^^^^^^^#";
        let err = parse(&map_json(&grid, SPAWNS, "[]")).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.message == "spawns[0]: is inside a spikes tile")
        );
    }
}
