//! Places map objects (thrusters, weapon/shield stations, the powerup spot)
//! on every map, using the bots' nav graph so everything stays reachable.
//!
//! cargo run --release --example place_objects
//!
//! Run it after `mapgen`, since regenerated variants start without objects.
//! Thrusters go where a tall shaft leads to a ledge too high to jump to.
//! The powerup spot goes on a ledge you must jump or thrust up to, far from
//! the spawns. Stations spread out across the map.

use arcane_dome::ai::{EdgeKind, NavGraph};
use arcane_dome::data::{GameData, MapDef, MapObject, ObjectKind, TileKind};
use arcane_dome::sim::Rng;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = GameData::load(&root.join("assets")).unwrap_or_else(|e| panic!("{e}"));
    for (idx, base) in data.maps.iter().enumerate() {
        let mut map = base.clone();
        map.objects.clear();
        let mut rng = Rng::new(idx as u64 * 7919 + 17);
        let objects = place(&mut map, idx, &mut rng);
        let path = root
            .join("assets/data/maps")
            .join(format!("{}.json", map.id));
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, with_objects(&text, &objects)).unwrap();
        let kinds: Vec<&str> = objects.iter().map(|o| o.kind.name()).collect();
        println!("{}: {}", map.id, kinds.join(", "));
    }
}

fn object(kind: ObjectKind, (x, y): (i32, i32)) -> MapObject {
    MapObject {
        kind,
        x,
        y,
        height_tiles: None,
        min_delay_s: None,
        max_delay_s: None,
        shield: None,
    }
}

fn dist(a: (i32, i32), b: (i32, i32)) -> f32 {
    (((a.0 - b.0).pow(2) + (a.1 - b.1).pow(2)) as f32).sqrt()
}

/// Cells where an object may sit: plain air with headroom (no water).
fn placeable(map: &MapDef, (x, y): (i32, i32)) -> bool {
    map.tile(x, y) == TileKind::Air && map.tile(x, y - 1) == TileKind::Air
}

fn place(map: &mut MapDef, _idx: usize, rng: &mut Rng) -> Vec<MapObject> {
    let mut taken: HashSet<(i32, i32)> = map.spawns.iter().map(|s| (s.x, s.y)).collect();
    taken.extend(map.pickups.iter().map(|p| (p.x, p.y)));
    let w = map.width as i32;
    let mut objects = Vec::new();

    // Thrusters: one per half, where a shaft leads up to a ledge 5+ tiles higher.
    let nav = NavGraph::build(map);
    let clear =
        |m: &MapDef, x: i32, y: i32| !matches!(m.tile(x, y), TileKind::Solid | TileKind::Spikes);
    let mut pads: Vec<((i32, i32), f32, f32)> = Vec::new();
    for &(x, y) in &nav.nodes {
        if map.tile(x, y + 1) != TileKind::Solid
            || !placeable(map, (x, y))
            || taken.contains(&(x, y))
        {
            continue;
        }
        for rise in 5..=11 {
            let ty = y - rise;
            let column = (ty - 1..y).all(|r| clear(map, x, r));
            if !column {
                break;
            }
            let lands = (-1..=1).any(|dx| nav.nodes.contains(&(x + dx, ty)));
            if lands {
                let score = -((rise as f32) - 7.0).abs() + rng.range(0.0, 1.5);
                pads.push(((x, y), rise as f32 + 1.5, score));
                break;
            }
        }
    }
    for left in [true, false] {
        let best = pads
            .iter()
            .filter(|(c, _, _)| (c.0 < w / 2) == left)
            .filter(|(c, _, _)| {
                objects
                    .iter()
                    .all(|o: &MapObject| dist(*c, (o.x, o.y)) > 10.0)
            })
            .max_by(|a, b| a.2.total_cmp(&b.2));
        if let Some(&(cell, height, _)) = best {
            let mut o = object(ObjectKind::Thruster, cell);
            o.height_tiles = Some(height);
            taken.insert(cell);
            objects.push(o);
        }
    }

    // Rebuild the graph with the thrusters in, and keep to the part every spawn shares.
    map.objects = objects.clone();
    let nav = NavGraph::build(map);
    let spawn_nodes: Vec<usize> = map
        .spawns
        .iter()
        .filter_map(|s| nav.nearest(macroquad::math::vec2(s.x as f32 + 0.5, s.y as f32 + 1.0)))
        .collect();
    let core = nav.mutually_reachable(spawn_nodes[0]);
    let cost = nav.costs_from(&spawn_nodes);
    let mut incoming: Vec<Vec<EdgeKind>> = vec![Vec::new(); nav.nodes.len()];
    for edges in &nav.edges {
        for e in edges {
            incoming[e.to].push(e.kind);
        }
    }
    let usable: Vec<usize> = (0..nav.nodes.len())
        .filter(|&n| core[n] && placeable(map, nav.nodes[n]) && !taken.contains(&nav.nodes[n]))
        .collect();

    // Powerup spot: a ledge you have to jump or thrust up to, far from spawns, high up.
    let h = map.height as f32;
    let score = |n: usize| {
        let (x, y) = nav.nodes[n];
        let climbs = incoming[n]
            .iter()
            .filter(|k| matches!(k, EdgeKind::Jump | EdgeKind::Thrust))
            .count() as f32;
        let thrust = incoming[n].contains(&EdgeKind::Thrust);
        cost[n] + (h - y as f32) * 0.4 - (x - w / 2).abs() as f32 * 0.15
            + if thrust { 4.0 } else { 0.0 }
            - climbs * 0.3
    };
    // No walking in: you must jump, thrust, or drop down onto it.
    let ledge = |n: &usize| {
        !incoming[*n].is_empty()
            && !incoming[*n].contains(&EdgeKind::Walk)
            && incoming[*n]
                .iter()
                .any(|k| matches!(k, EdgeKind::Jump | EdgeKind::Thrust))
    };
    let spot = usable
        .iter()
        .copied()
        .filter(ledge)
        .max_by(|&a, &b| score(a).total_cmp(&score(b)))
        .or_else(|| {
            usable
                .iter()
                .copied()
                .max_by(|&a, &b| score(a).total_cmp(&score(b)))
        });
    if let Some(n) = spot {
        taken.insert(nav.nodes[n]);
        objects.push(object(ObjectKind::PowerupSpot, nav.nodes[n]));
    }

    // Stations, spread far apart. Shield stations are rarer than weapon
    // stations: 1-4 small and 0-2 big per map, with weapon stations
    // always outnumbering them.
    let mut avoid: Vec<(i32, i32)> = taken.iter().copied().collect();
    let pick = |rng: &mut Rng, avoid: &mut Vec<(i32, i32)>| -> Option<(i32, i32)> {
        let best = (0..60)
            .map(|_| nav.nodes[usable[rng.below(usable.len())]])
            .filter(|c| !avoid.contains(c))
            .max_by(|a, b| {
                let gap =
                    |c: &(i32, i32)| avoid.iter().map(|o| dist(*c, *o)).fold(f32::MAX, f32::min);
                gap(a).total_cmp(&gap(b))
            })?;
        avoid.push(best);
        Some(best)
    };
    let small = 1 + rng.below(4);
    let big = rng.below(3);
    let weapons = small + big + 1 + rng.below(2);
    for _ in 0..weapons {
        if let Some(c) = pick(rng, &mut avoid) {
            objects.push(object(ObjectKind::WeaponStation, c));
        }
    }
    for _ in 0..small {
        if let Some(c) = pick(rng, &mut avoid) {
            objects.push(object(ObjectKind::ShieldStation, c));
        }
    }
    for _ in 0..big {
        if let Some(c) = pick(rng, &mut avoid) {
            let mut o = object(ObjectKind::ShieldStation, c);
            o.shield = Some(100.0);
            o.min_delay_s = Some(90.0);
            o.max_delay_s = Some(120.0);
            objects.push(o);
        }
    }
    objects
}

fn object_json(o: &MapObject) -> String {
    let mut s = format!(r#"{{"type":"{}","x":{},"y":{}"#, o.kind.name(), o.x, o.y);
    if let Some(h) = o.height_tiles {
        write!(s, r#","height_tiles":{h}"#).unwrap();
    }
    if let Some(v) = o.min_delay_s {
        write!(s, r#","min_delay_s":{v}"#).unwrap();
    }
    if let Some(v) = o.max_delay_s {
        write!(s, r#","max_delay_s":{v}"#).unwrap();
    }
    if let Some(v) = o.shield {
        write!(s, r#","shield":{v}"#).unwrap();
    }
    s.push('}');
    s
}

/// Replace (or add) the `"objects"` block just before `"bot_hints"`.
fn with_objects(text: &str, objects: &[MapObject]) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if let Some(start) = lines
        .iter()
        .position(|l| l.trim_start().starts_with("\"objects\""))
    {
        let end = (start..lines.len())
            .find(|&i| lines[i].trim_start().starts_with(']'))
            .unwrap_or(start);
        lines.drain(start..=end);
    }
    let at = lines
        .iter()
        .position(|l| l.trim_start().starts_with("\"bot_hints\""))
        .expect("map file has a bot_hints line");
    let mut block = vec!["  \"objects\": [".to_owned()];
    for (i, o) in objects.iter().enumerate() {
        let comma = if i + 1 < objects.len() { "," } else { "" };
        block.push(format!("    {}{comma}", object_json(o)));
    }
    block.push("  ],".to_owned());
    let mut out: Vec<String> = lines[..at].iter().map(|s| s.to_string()).collect();
    out.extend(block);
    out.extend(lines[at..].iter().map(|s| s.to_string()));
    out.join("\n") + "\n"
}
