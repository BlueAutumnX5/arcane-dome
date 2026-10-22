//! Turns blocky maps into rockier ones, in place:
//!
//! - ramps where the floor steps up or down by one tile,
//! - rounded (sloped) top corners on rock blocks,
//! - sludge buildup hanging from ceilings, and some thin platforms
//!   crusted over with sludge (stand on top, but no jumping through),
//! - an interior backdrop wherever a space has a roof over it.
//!
//! Each step is checked with the bots' nav graph: every spawn, pickup and
//! object must stay reachable, otherwise that step is retried or skipped.
//!
//! cargo run --release --example terraform
//! cargo run --release --example place_objects   # re-place objects afterwards

use arcane_dome::ai::NavGraph;
use arcane_dome::data::{GameData, MapDef, TileKind, parse_map};
use arcane_dome::sim::Rng;
use std::collections::HashSet;
use std::path::Path;

type Grid = Vec<Vec<char>>;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = GameData::load(&root.join("assets")).unwrap_or_else(|e| panic!("{e}"));
    let weapon_ids: Vec<&str> = data.weapons.iter().map(|w| w.id.as_str()).collect();
    let mode_ids: Vec<&str> = data.modes.iter().map(|m| m.id.as_str()).collect();
    for (idx, map) in data.maps.iter().enumerate() {
        let path = root
            .join("assets/data/maps")
            .join(format!("{}.json", map.id));
        let text = std::fs::read_to_string(&path).unwrap();
        let mut rng = Rng::new(0xD0E5 + idx as u64 * 131);
        let keep = protected_cells(map);
        let mut grid = to_grid(map);
        let mut report = Vec::new();

        type Step = fn(&mut Grid, &HashSet<(i32, i32)>, &mut Rng);
        let steps: [(&str, Step); 4] = [
            ("ramps", step_ramps),
            ("round corners", round_corners),
            ("ceiling sludge", ceiling_sludge),
            ("sludge ledges", sludge_ledges),
        ];
        for (name, step) in steps {
            let mut applied = false;
            for _ in 0..8 {
                let mut trial = grid.clone();
                step(&mut trial, &keep, &mut rng);
                if trial != grid && still_playable(map, &trial) {
                    grid = trial;
                    applied = true;
                    break;
                }
            }
            report.push(format!(
                "{name}: {}",
                if applied { "yes" } else { "skipped" }
            ));
        }
        let interior = interior_rows(&grid);

        let new_text = rewrite(&text, &grid, &interior);
        let label = format!("maps/{}.json", map.id);
        if let Err(e) = parse_map(&label, &map.id, &new_text, &weapon_ids, &mode_ids) {
            panic!("{label} would be invalid: {e:?}");
        }
        std::fs::write(&path, new_text).unwrap();
        println!("{}: {}", map.id, report.join(", "));
    }
}

fn to_char(k: TileKind) -> char {
    match k {
        TileKind::Air => '.',
        TileKind::Solid => '#',
        TileKind::OneWay => '=',
        TileKind::Spikes => '^',
        TileKind::Water => '~',
        TileKind::SlopeUpRight => '/',
        TileKind::SlopeUpLeft => '\\',
        TileKind::Sludge => '%',
    }
}

fn to_grid(map: &MapDef) -> Grid {
    (0..map.height as i32)
        .map(|y| {
            (0..map.width as i32)
                .map(|x| to_char(map.tile(x, y)))
                .collect()
        })
        .collect()
}

fn at(g: &Grid, x: i32, y: i32) -> char {
    if x < 0 || y < 0 || y as usize >= g.len() || x as usize >= g[0].len() {
        '#'
    } else {
        g[y as usize][x as usize]
    }
}

fn inside(g: &Grid, x: i32, y: i32) -> bool {
    x > 0 && y > 0 && (y as usize) < g.len() - 1 && (x as usize) < g[0].len() - 1
}

fn set(g: &mut Grid, x: i32, y: i32, c: char) {
    if inside(g, x, y) {
        g[y as usize][x as usize] = c;
    }
}

/// Cells that must stay as they are: everything placed on the map, the
/// cell below each (its floor), and thruster launch shafts.
fn protected_cells(map: &MapDef) -> HashSet<(i32, i32)> {
    let mut cells = HashSet::new();
    fn add(cells: &mut HashSet<(i32, i32)>, x: i32, y: i32) {
        for (dx, dy) in [(0, 0), (0, 1), (0, -1)] {
            cells.insert((x + dx, y + dy));
        }
    }
    for s in &map.spawns {
        add(&mut cells, s.x, s.y);
    }
    for p in &map.pickups {
        add(&mut cells, p.x, p.y);
    }
    for o in &map.objects {
        add(&mut cells, o.x, o.y);
        if o.kind == arcane_dome::data::ObjectKind::Thruster {
            for r in 0..=(o.launch_height().ceil() as i32 + 2) {
                cells.insert((o.x, o.y - r));
            }
        }
    }
    cells
}

/// Air cell sitting on rock (not on a platform, ramp or hazard).
fn on_rock(g: &Grid, x: i32, y: i32) -> bool {
    at(g, x, y) == '.' && at(g, x, y + 1) == '#'
}

/// A one-tile step in the floor becomes a ramp.
fn step_ramps(g: &mut Grid, keep: &HashSet<(i32, i32)>, rng: &mut Rng) {
    let snapshot = g.clone();
    for y in 1..g.len() as i32 - 1 {
        for x in 1..g[0].len() as i32 - 1 {
            // A ramp needs two clear rows above it for a fighter to fit.
            let headroom = at(&snapshot, x, y - 1) == '.' && at(&snapshot, x, y - 2) == '.';
            if keep.contains(&(x, y)) || !on_rock(&snapshot, x, y) || !headroom {
                continue;
            }
            let step_right = at(&snapshot, x + 1, y) == '#' && at(&snapshot, x + 1, y - 1) == '.';
            let step_left = at(&snapshot, x - 1, y) == '#' && at(&snapshot, x - 1, y - 1) == '.';
            if step_right && !step_left && rng.chance(0.8) {
                set(g, x, y, '/');
            } else if step_left && !step_right && rng.chance(0.8) {
                set(g, x, y, '\\');
            }
        }
    }
}

/// Bevel the exposed top corners of rock blocks into slopes.
fn round_corners(g: &mut Grid, keep: &HashSet<(i32, i32)>, rng: &mut Rng) {
    let snapshot = g.clone();
    for y in 1..g.len() as i32 - 1 {
        for x in 1..g[0].len() as i32 - 1 {
            let headroom = at(&snapshot, x, y - 1) == '.' && at(&snapshot, x, y - 2) == '.';
            if at(&snapshot, x, y) != '#' || !headroom {
                continue;
            }
            // Never round the tile something stands on.
            if keep.contains(&(x, y)) || keep.contains(&(x, y - 1)) {
                continue;
            }
            let air_left = at(&snapshot, x - 1, y) == '.';
            let air_right = at(&snapshot, x + 1, y) == '.';
            // Only on blocks at least three wide, so a flat top remains.
            let wide_right = at(&snapshot, x + 1, y) == '#' && at(&snapshot, x + 2, y) == '#';
            let wide_left = at(&snapshot, x - 1, y) == '#' && at(&snapshot, x - 2, y) == '#';
            if air_left && wide_right && rng.chance(0.55) {
                set(g, x, y, '/');
            } else if air_right && wide_left && rng.chance(0.55) {
                set(g, x, y, '\\');
            }
        }
    }
}

/// Lumpy sludge buildup hanging from rock ceilings, in short runs.
fn ceiling_sludge(g: &mut Grid, keep: &HashSet<(i32, i32)>, rng: &mut Rng) {
    let snapshot = g.clone();
    let w = g[0].len() as i32;
    for y in 1..g.len() as i32 - 3 {
        let mut x = 1;
        while x < w - 1 {
            // Air hanging under rock, with room below so it still feels open.
            let hangs = |xx: i32| {
                at(&snapshot, xx, y) == '.'
                    && at(&snapshot, xx, y - 1) == '#'
                    && (1..=3).all(|k| at(&snapshot, xx, y + k) == '.')
                    && !keep.contains(&(xx, y))
            };
            if hangs(x) && rng.chance(0.06) {
                let len = 2 + rng.below(4) as i32;
                for xx in x..x + len {
                    if !hangs(xx) {
                        break;
                    }
                    set(g, xx, y, '%');
                    // Occasional thicker drip below the crust.
                    if rng.chance(0.25) && hangs_below(&snapshot, keep, xx, y + 1) {
                        set(g, xx, y + 1, '%');
                    }
                }
                x += len + 2;
            } else {
                x += 1;
            }
        }
    }
}

fn hangs_below(g: &Grid, keep: &HashSet<(i32, i32)>, x: i32, y: i32) -> bool {
    at(g, x, y) == '.' && (1..=3).all(|k| at(g, x, y + k) == '.') && !keep.contains(&(x, y))
}

/// Crust part of a thin platform with sludge: still a floor from above, but it
/// now blocks jumps from below.
fn sludge_ledges(g: &mut Grid, keep: &HashSet<(i32, i32)>, rng: &mut Rng) {
    let mut runs = Vec::new();
    for y in 1..g.len() as i32 - 1 {
        let mut x = 1;
        while x < g[0].len() as i32 - 1 {
            if at(g, x, y) == '=' {
                let start = x;
                while at(g, x, y) == '=' {
                    x += 1;
                }
                runs.push((start, x - start, y));
            } else {
                x += 1;
            }
        }
    }
    let picks = 1 + rng.below(2);
    for _ in 0..picks {
        if runs.is_empty() {
            return;
        }
        let (x, len, y) = runs.remove(rng.below(runs.len()));
        if len < 3 {
            continue;
        }
        let part = (len / 2).max(2);
        let from = x + rng.below((len - part + 1) as usize) as i32;
        for xx in from..from + part {
            if !keep.contains(&(xx, y)) {
                set(g, xx, y, '%');
            }
        }
    }
}

/// Mark covered spaces (rock or sludge overhead within a few tiles, floor
/// below) as building interior, with a window every few columns.
fn interior_rows(g: &Grid) -> Vec<String> {
    let (w, h) = (g[0].len() as i32, g.len() as i32);
    let solid = |c: char| matches!(c, '#' | '%' | '/' | '\\');
    let mut cover = vec![vec![false; w as usize]; h as usize];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if solid(at(g, x, y)) {
                continue;
            }
            let roof = (1..=5).any(|k| solid(at(g, x, y - k)) && y - k > 0);
            let floor = (1..=5).any(|k| at(g, x, y + k) != '.' && at(g, x, y + k) != '~');
            cover[y as usize][x as usize] = roof && floor;
        }
    }
    // Keep only sizeable covered regions so open sky stays open.
    let mut seen = vec![vec![false; w as usize]; h as usize];
    let mut keep = vec![vec![false; w as usize]; h as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            if !cover[y][x] || seen[y][x] {
                continue;
            }
            let mut region = Vec::new();
            let mut stack = vec![(x, y)];
            seen[y][x] = true;
            while let Some((cx, cy)) = stack.pop() {
                region.push((cx, cy));
                for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let (nx, ny) = (nx as usize, ny as usize);
                    if cover[ny][nx] && !seen[ny][nx] {
                        seen[ny][nx] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if region.len() >= 30 {
                for (cx, cy) in region {
                    keep[cy][cx] = true;
                }
            }
        }
    }
    (0..h as usize)
        .map(|y| {
            (0..w as usize)
                .map(|x| {
                    if !keep[y][x] {
                        '.'
                    } else if x % 4 == 2 && y % 3 == 1 {
                        'w'
                    } else {
                        '#'
                    }
                })
                .collect()
        })
        .collect()
}

/// Every spawn, pickup and object must stay mutually reachable.
fn still_playable(base: &MapDef, grid: &Grid) -> bool {
    let rows: Vec<String> = grid.iter().map(|r| r.iter().collect()).collect();
    let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
    let mut map = MapDef::from_rows(&refs);
    map.objects = base.objects.clone();
    let nav = NavGraph::build(&map);
    let cells: Vec<(i32, i32)> = base
        .spawns
        .iter()
        .map(|s| (s.x, s.y))
        .chain(base.pickups.iter().map(|p| (p.x, p.y)))
        .chain(base.objects.iter().map(|o| (o.x, o.y)))
        .collect();
    // Placements must still be plain air with a floor somewhere below.
    if cells.iter().any(|&(x, y)| map.tile(x, y) != TileKind::Air) {
        return false;
    }
    let nodes: Vec<Option<usize>> = cells
        .iter()
        .map(|&(x, y)| nav.nearest(macroquad::math::vec2(x as f32 + 0.5, y as f32 + 1.0)))
        .collect();
    let Some(Some(first)) = nodes.first().copied() else {
        return false;
    };
    let core = nav.mutually_reachable(first);
    nodes.iter().all(|n| n.is_some_and(|n| core[n]))
}

/// Replace the grid rows (and legend / interior) in the map file text.
fn rewrite(text: &str, grid: &Grid, interior: &[String]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("\"legend\"") {
            out.push(
                r##"  "legend": {"#": "solid", "=": "one_way", "^": "spikes", "~": "water", ".": "air", "/": "slope_up_right", "\\": "slope_up_left", "%": "sludge"},"##
                    .to_owned(),
            );
            i += 1;
        } else if t.starts_with("\"grid\"") || t.starts_with("\"interior\"") {
            let is_grid = t.starts_with("\"grid\"");
            while !lines[i].trim_start().starts_with(']') {
                i += 1;
            }
            i += 1;
            if is_grid {
                out.push("  \"grid\": [".to_owned());
                push_rows(&mut out, grid.iter().map(|r| r.iter().collect::<String>()));
                out.push("  ],".to_owned());
                out.push("  \"interior\": [".to_owned());
                push_rows(&mut out, interior.iter().cloned());
                out.push("  ],".to_owned());
            }
        } else {
            out.push(lines[i].to_owned());
            i += 1;
        }
    }
    out.join("\n") + "\n"
}

fn push_rows(out: &mut Vec<String>, rows: impl ExactSizeIterator<Item = String>) {
    let n = rows.len();
    for (k, r) in rows.enumerate() {
        let comma = if k + 1 < n { "," } else { "" };
        out.push(format!("    {}{comma}", serde_json::to_string(&r).unwrap()));
    }
}
