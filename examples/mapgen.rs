//! Map variation generator.
//!
//! Takes a hand-made base map and roughens it with a seed: uneven ground,
//! ragged rock edges, hanging chunks, shifted and broken platforms, and
//! independent changes per side so the halves no longer mirror each other.
//! Every candidate is checked with the bots' nav graph so all spawns and
//! pickups stay reachable, then validated by the normal map loader.
//!
//! cargo run --example mapgen                 # regenerate all variants
//! cargo run --example mapgen -- <variant_id> # just one

use arcane_dome::ai::NavGraph;
use arcane_dome::data::{GameData, MapDef, PickupKind, TileKind, parse_map};
use arcane_dome::sim::Rng;
use std::collections::{HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::Path;

struct Variant {
    base: &'static str,
    id: &'static str,
    name: &'static str,
    sky: &'static str,
    seed: u64,
    /// 0..1, how hard to roughen.
    rough: f32,
}

const VARIANTS: &[Variant] = &[
    Variant {
        base: "skyforge_yard",
        id: "skyforge_slagheap",
        name: "Skyforge Slagheap",
        sky: "#2a1f1d",
        seed: 11,
        rough: 0.6,
    },
    Variant {
        base: "skyforge_yard",
        id: "skyforge_collapse",
        name: "Skyforge Collapse",
        sky: "#1a2128",
        seed: 23,
        rough: 0.9,
    },
    Variant {
        base: "glass_cistern",
        id: "cracked_cistern",
        name: "Cracked Cistern",
        sky: "#132420",
        seed: 37,
        rough: 0.6,
    },
    Variant {
        base: "glass_cistern",
        id: "cistern_overgrowth",
        name: "Cistern Overgrowth",
        sky: "#18281a",
        seed: 41,
        rough: 0.9,
    },
    Variant {
        base: "lantern_spire",
        id: "fallen_spire",
        name: "Fallen Spire",
        sky: "#2b1c26",
        seed: 53,
        rough: 0.6,
    },
    Variant {
        base: "lantern_spire",
        id: "spire_roots",
        name: "Spire Roots",
        sky: "#1e1a30",
        seed: 67,
        rough: 0.9,
    },
];

type Grid = Vec<Vec<char>>;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = GameData::load(&root.join("assets")).unwrap_or_else(|e| panic!("{e}"));
    let only = std::env::args().nth(1);
    for v in VARIANTS
        .iter()
        .filter(|v| only.as_deref().is_none_or(|o| o == v.id))
    {
        let base = data
            .map(v.base)
            .unwrap_or_else(|| panic!("no base map {}", v.base));
        let mut done = false;
        for attempt in 0..400 {
            if let Some(json) = generate(base, &data, v, v.seed * 1000 + attempt) {
                let path = root.join("assets/data/maps").join(format!("{}.json", v.id));
                std::fs::write(&path, json).unwrap();
                println!("{} <- {} (attempt {attempt})", v.id, v.base);
                done = true;
                break;
            }
        }
        assert!(done, "could not generate {}", v.id);
    }
}

fn to_char(k: TileKind) -> char {
    match k {
        TileKind::Air => '.',
        TileKind::Solid => '#',
        TileKind::OneWay => '=',
        TileKind::Spikes => '^',
        TileKind::Water => '~',
    }
}

fn generate(base: &MapDef, data: &GameData, v: &Variant, seed: u64) -> Option<String> {
    let mut rng = Rng::new(seed);
    let (w, h) = (base.width, base.height);
    let mut g: Grid = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| to_char(base.tile(x as i32, y as i32)))
                .collect()
        })
        .collect();

    shuffle_platforms(&mut g, &mut rng, v.rough);
    ragged_edges(&mut g, &mut rng, v.rough);
    uneven_ground(&mut g, &mut rng, v.rough);
    add_chunks(&mut g, &mut rng, v.rough);
    hanging_rocks(&mut g, &mut rng, v.rough);
    if rng.chance(0.5 * v.rough) {
        spike_patch(&mut g, &mut rng);
    }

    let rows: Vec<String> = g.iter().map(|r| r.iter().collect()).collect();
    let row_refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
    let map = MapDef::from_rows(&row_refs);
    let nav = NavGraph::build(&map);
    let core = largest_core(&nav, &mut rng)?;
    // Most of the standable space must be one connected arena.
    if core.len() * 10 < nav.nodes.len() * 7 {
        return None;
    }
    let cells: Vec<(i32, i32)> = core.iter().map(|&n| nav.nodes[n]).collect();

    let mut used: HashSet<(i32, i32)> = HashSet::new();
    let third = w as i32 / 3;
    let team0 = spread(&cells, |c| c.0 < third, 2, 5.0, &mut used, &mut rng)?;
    let team1 = spread(
        &cells,
        |c| c.0 >= w as i32 - third,
        2,
        5.0,
        &mut used,
        &mut rng,
    )?;
    let ffa = spread(&cells, |_| true, 6, 7.0, &mut used, &mut rng)?;

    let mut pickups = Vec::new();
    for p in &base.pickups {
        let (x, y) = nearest_free(&cells, (p.x, p.y), &used)?;
        used.insert((x, y));
        let mut s = format!(
            r#"{{"type":"{}","x":{x},"y":{y}"#,
            match p.kind {
                PickupKind::Health => "health",
                PickupKind::Ammo => "ammo",
                PickupKind::Weapon => "weapon",
            }
        );
        if let Some(id) = &p.weapon_id {
            write!(s, r#","weapon_id":"{id}""#).unwrap();
        }
        write!(s, r#","respawn_s":{}}}"#, p.respawn_s).unwrap();
        pickups.push(s);
    }

    let mut spawns = Vec::new();
    for (x, y) in team0 {
        spawns.push(format!(r#"{{"x":{x},"y":{y},"team":0}}"#));
    }
    for (x, y) in team1 {
        spawns.push(format!(r#"{{"x":{x},"y":{y},"team":1}}"#));
    }
    for (x, y) in ffa {
        spawns.push(format!(r#"{{"x":{x},"y":{y},"team":null}}"#));
    }

    let json = render_json(base, v, &rows, &spawns, &pickups);
    let weapon_ids: Vec<&str> = data.weapons.iter().map(|w| w.id.as_str()).collect();
    let mode_ids: Vec<&str> = data.modes.iter().map(|m| m.id.as_str()).collect();
    let label = format!("maps/{}.json", v.id);
    parse_map(&label, v.id, &json, &weapon_ids, &mode_ids).ok()?;
    Some(json)
}

fn inside(g: &Grid, x: i32, y: i32) -> bool {
    x > 0 && y > 0 && (y as usize) < g.len() - 1 && (x as usize) < g[0].len() - 1
}

fn at(g: &Grid, x: i32, y: i32) -> char {
    if x < 0 || y < 0 || y as usize >= g.len() || x as usize >= g[0].len() {
        '#'
    } else {
        g[y as usize][x as usize]
    }
}

fn set(g: &mut Grid, x: i32, y: i32, c: char) {
    if inside(g, x, y) {
        g[y as usize][x as usize] = c;
    }
}

fn irange(rng: &mut Rng, lo: i32, hi: i32) -> i32 {
    lo + rng.below((hi - lo + 1) as usize) as i32
}

/// Move, resize and sometimes split every one-way platform.
fn shuffle_platforms(g: &mut Grid, rng: &mut Rng, rough: f32) {
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
    for &(x, len, y) in &runs {
        for i in 0..len {
            set(g, x + i, y, '.');
        }
    }
    let shift = (2.0 + 2.0 * rough) as i32;
    for (x, len, y) in runs {
        let nx = x + irange(rng, -shift, shift);
        let ny = y + if rng.chance(0.4 * rough) {
            irange(rng, -1, 1)
        } else {
            0
        };
        let nlen = (len + irange(rng, -2, 2)).max(2);
        let split = nlen >= 6 && rng.chance(0.5 * rough);
        let gap_at = if split { irange(rng, 2, nlen - 3) } else { -1 };
        let gap_len = irange(rng, 1, 2);
        for i in 0..nlen {
            if split && i >= gap_at && i < gap_at + gap_len {
                continue;
            }
            if at(g, nx + i, ny) == '.' {
                set(g, nx + i, ny, '=');
            }
        }
    }
}

/// Erode and grow the surface of solid areas so edges are no longer straight.
fn ragged_edges(g: &mut Grid, rng: &mut Rng, rough: f32) {
    let snapshot = g.clone();
    for y in 1..g.len() as i32 - 1 {
        for x in 1..g[0].len() as i32 - 1 {
            let c = at(&snapshot, x, y);
            let air_n = [(0, -1), (1, 0), (-1, 0), (0, 1)]
                .iter()
                .filter(|(dx, dy)| at(&snapshot, x + dx, y + dy) == '.')
                .count();
            if c == '#' && air_n > 0 && rng.chance(0.12 * rough) {
                set(g, x, y, '.');
            } else if c == '.' && air_n < 4 && rng.chance(0.07 * rough) {
                let touching_solid = [(0, 1), (1, 0), (-1, 0)]
                    .iter()
                    .any(|(dx, dy)| at(&snapshot, x + dx, y + dy) == '#');
                if touching_solid {
                    set(g, x, y, '#');
                }
            }
        }
    }
}

/// Raise the floor into a gentle random-walk landscape.
fn uneven_ground(g: &mut Grid, rng: &mut Rng, rough: f32) {
    let h = g.len() as i32;
    let max = (1.0 + 2.0 * rough) as i32;
    let mut height = 0i32;
    for x in 1..g[0].len() as i32 - 1 {
        if rng.chance(0.35) {
            height = (height + irange(rng, -1, 1)).clamp(0, max);
        }
        // Only build on open floor, never over water or spikes.
        let floor_open = (1..=height + 2).all(|k| at(g, x, h - 1 - k) == '.');
        if !floor_open {
            continue;
        }
        for k in 1..=height {
            set(g, x, h - 1 - k, '#');
        }
    }
}

/// Scatter small solid blobs and short ledges through open space.
fn add_chunks(g: &mut Grid, rng: &mut Rng, rough: f32) {
    let (w, h) = (g[0].len() as i32, g.len() as i32);
    let count = ((w * h) as f32 / 220.0 * rough) as i32 + 1;
    for _ in 0..count {
        let x = irange(rng, 2, w - 4);
        let y = irange(rng, 3, h - 5);
        let (cw, ch, c) = if rng.chance(0.5) {
            (irange(rng, 1, 3), irange(rng, 1, 2), '#')
        } else {
            (irange(rng, 2, 4), 1, '=')
        };
        // Leave breathing room so chunks do not seal corridors.
        let clear = (x - 1..=x + cw).all(|xx| (y - 2..=y + ch + 1).all(|yy| at(g, xx, yy) == '.'));
        if clear {
            for dx in 0..cw {
                for dy in 0..ch {
                    set(g, x + dx, y + dy, c);
                }
            }
        }
    }
}

/// Rock teeth hanging from the ceiling.
fn hanging_rocks(g: &mut Grid, rng: &mut Rng, rough: f32) {
    for x in 2..g[0].len() as i32 - 2 {
        if at(g, x, 1) == '.' && rng.chance(0.18 * rough) {
            let len = irange(rng, 1, 2);
            for k in 0..len {
                if at(g, x, 1 + k + 1) == '.' {
                    set(g, x, 1 + k, '#');
                }
            }
        }
    }
}

fn spike_patch(g: &mut Grid, rng: &mut Rng) {
    let w = g[0].len() as i32;
    for _ in 0..20 {
        let x = irange(rng, 4, w - 7);
        let len = irange(rng, 2, 3);
        // Find the floor surface in this column span.
        let y = (1..g.len() as i32 - 1)
            .rev()
            .find(|&y| at(g, x, y) == '.' && at(g, x, y + 1) == '#');
        let Some(y) = y else { continue };
        let flat = (x..x + len).all(|xx| at(g, xx, y) == '.' && at(g, xx, y + 1) == '#');
        if flat {
            for xx in x..x + len {
                set(g, xx, y, '^');
            }
            return;
        }
    }
}

/// The biggest set of nodes that can all reach each other.
fn largest_core(nav: &NavGraph, rng: &mut Rng) -> Option<Vec<usize>> {
    let n = nav.nodes.len();
    if n == 0 {
        return None;
    }
    let mut back: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (a, edges) in nav.edges.iter().enumerate() {
        for e in edges {
            back[e.to].push(a);
        }
    }
    let reach = |start: usize, adj: &dyn Fn(usize) -> Vec<usize>| {
        let mut seen = vec![false; n];
        let mut q = VecDeque::from([start]);
        seen[start] = true;
        while let Some(a) = q.pop_front() {
            for b in adj(a) {
                if !seen[b] {
                    seen[b] = true;
                    q.push_back(b);
                }
            }
        }
        seen
    };
    let mut best: Vec<usize> = Vec::new();
    for _ in 0..6 {
        let s = rng.below(n);
        let fwd = reach(s, &|a| nav.edges[a].iter().map(|e| e.to).collect());
        let bwd = reach(s, &|a| back[a].clone());
        let core: Vec<usize> = (0..n).filter(|&i| fwd[i] && bwd[i]).collect();
        if core.len() > best.len() {
            best = core;
        }
    }
    Some(best)
}

/// Pick `count` cells matching `filter`, at least `min_gap` apart.
fn spread(
    cells: &[(i32, i32)],
    filter: impl Fn(&(i32, i32)) -> bool,
    count: usize,
    min_gap: f32,
    used: &mut HashSet<(i32, i32)>,
    rng: &mut Rng,
) -> Option<Vec<(i32, i32)>> {
    let pool: Vec<(i32, i32)> = cells
        .iter()
        .copied()
        .filter(|c| filter(c) && !used.contains(c))
        .collect();
    let mut picked: Vec<(i32, i32)> = Vec::new();
    for _ in 0..400 {
        if picked.len() == count {
            break;
        }
        let c = *pool.get(rng.below(pool.len()))?;
        let far = picked.iter().chain(used.iter()).all(|p| {
            let (dx, dy) = ((p.0 - c.0) as f32, (p.1 - c.1) as f32);
            (dx * dx + dy * dy).sqrt() >= min_gap
        });
        if far {
            picked.push(c);
        }
    }
    if picked.len() < count {
        return None;
    }
    used.extend(picked.iter().copied());
    Some(picked)
}

fn nearest_free(
    cells: &[(i32, i32)],
    to: (i32, i32),
    used: &HashSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    cells
        .iter()
        .copied()
        .filter(|c| !used.contains(c))
        .min_by_key(|c| (c.0 - to.0).pow(2) + (c.1 - to.1).pow(2))
}

fn render_json(
    base: &MapDef,
    v: &Variant,
    rows: &[String],
    spawns: &[String],
    pickups: &[String],
) -> String {
    let modes: Vec<String> = base.modes.iter().map(|m| format!("\"{m}\"")).collect();
    let rec = base
        .recommended_players
        .map_or("null".to_owned(), |[a, b]| format!("[{a}, {b}]"));
    let mut s = String::new();
    writeln!(s, "{{").unwrap();
    writeln!(s, "  \"id\": \"{}\",", v.id).unwrap();
    writeln!(s, "  \"name\": \"{}\",", v.name).unwrap();
    writeln!(s, "  \"tile_size\": {},", base.tile_size).unwrap();
    writeln!(
        s,
        "  \"background\": {{\"sky_color\": \"{}\", \"parallax\": []}},",
        v.sky
    )
    .unwrap();
    writeln!(s, "  \"modes\": [{}],", modes.join(", ")).unwrap();
    writeln!(s, "  \"recommended_players\": {rec},").unwrap();
    writeln!(
        s,
        "  \"legend\": {{\"#\": \"solid\", \"=\": \"one_way\", \"^\": \"spikes\", \"~\": \"water\", \".\": \"air\"}},"
    )
    .unwrap();
    writeln!(s, "  \"grid\": [").unwrap();
    for (i, r) in rows.iter().enumerate() {
        let comma = if i + 1 < rows.len() { "," } else { "" };
        writeln!(s, "    \"{r}\"{comma}").unwrap();
    }
    writeln!(s, "  ],").unwrap();
    for (key, items, last) in [("spawns", spawns, false), ("pickups", pickups, false)] {
        writeln!(s, "  \"{key}\": [").unwrap();
        for (i, it) in items.iter().enumerate() {
            let comma = if i + 1 < items.len() { "," } else { "" };
            writeln!(s, "    {it}{comma}").unwrap();
        }
        writeln!(s, "  ]{}", if last { "" } else { "," }).unwrap();
    }
    writeln!(s, "  \"bot_hints\": []").unwrap();
    writeln!(s, "}}").unwrap();
    s
}
