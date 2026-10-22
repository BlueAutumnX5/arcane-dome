use super::{DataError, is_snake_case, parse_json};
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
}

impl TileKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::Solid => "solid",
            Self::OneWay => "one_way",
            Self::Spikes => "spikes",
            Self::Water => "water",
        }
    }
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapFile {
    id: String,
    name: String,
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
    spawns: Vec<SpawnDef>,
    #[serde(default)]
    pickups: Vec<PickupDef>,
    /// Reserved for scripted hazards (not in Milestone 1).
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
    pub tile_size: u32,
    pub sky_color: [u8; 3],
    pub modes: Vec<String>,
    pub recommended_players: Option<[u32; 2]>,
    pub width: usize,
    pub height: usize,
    tiles: Vec<TileKind>,
    pub spawns: Vec<SpawnDef>,
    pub pickups: Vec<PickupDef>,
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
            tile_size: 32,
            sky_color: [27, 31, 42],
            modes: Vec::new(),
            recommended_players: None,
            width,
            height: rows.len(),
            tiles,
            spawns: Vec::new(),
            pickups: Vec::new(),
            bot_hints: Vec::new(),
        }
    }

    /// Scan downward from (x, y+1) for something to stand on.
    fn has_ground_below(&self, x: i32, y: i32) -> bool {
        let mut yy = y + 1;
        loop {
            match self.tile(x, yy) {
                TileKind::Air | TileKind::Water => yy += 1,
                TileKind::Solid | TileKind::OneWay => return true,
                TileKind::Spikes => return false,
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
    if !errs.is_empty() {
        return Err(to_errors(label, errs));
    }

    let map = MapDef {
        id: file.id,
        name: file.name,
        tile_size: file.tile_size,
        sky_color,
        modes: file.modes,
        recommended_players: file.recommended_players,
        width,
        height: file.grid.len(),
        tiles,
        spawns: file.spawns,
        pickups: file.pickups,
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

    for (i, h) in map.bot_hints.iter().enumerate() {
        if map.tile(h.x, h.y) == TileKind::Solid {
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
    if needs_headroom && map.tile(x, y - 1) == TileKind::Solid {
        errs.push(format!("{what}: has no headroom (tile above is solid)"));
    }
    if !map.has_ground_below(x, y) {
        errs.push(format!("{what}: has no solid or one-way ground below"));
    }
}

fn to_errors(label: &str, msgs: Vec<String>) -> Vec<DataError> {
    msgs.into_iter().map(|m| DataError::new(label, m)).collect()
}

fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
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
