//! Map inspection tool: checks that every spawn, pickup and object on a map
//! can be reached by walking, jumping, falling or thrusting, and optionally
//! dumps the bot navigation graph as JSON for drawing an overview.
//!
//! cargo run --release --example map_check                 # all maps
//! cargo run --release --example map_check -- <map_id> [nav.json]

use arcane_dome::ai::{EdgeKind, NavGraph};
use arcane_dome::data::GameData;
use macroquad::math::Vec2;
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = GameData::load(&root.join("assets")).unwrap_or_else(|e| panic!("{e}"));
    let mut args = std::env::args().skip(1);
    let only = args.next();
    let dump = args.next();
    let mut failed = false;

    for map in data
        .maps
        .iter()
        .filter(|m| only.as_deref().is_none_or(|o| o == m.id))
    {
        let nav = NavGraph::build(map);
        let mut spots: Vec<(String, Vec2)> = Vec::new();
        for (i, s) in map.spawns.iter().enumerate() {
            let team = s.team.map_or("ffa".to_owned(), |t| format!("team {t}"));
            spots.push((format!("spawn {i} ({team})"), s.feet()));
        }
        for (i, p) in map.pickups.iter().enumerate() {
            spots.push((format!("pickup {i} ({:?})", p.kind), p.feet()));
        }
        for (i, o) in map.objects.iter().enumerate() {
            spots.push((format!("object {i} ({})", o.kind.name()), o.feet()));
        }

        let nodes: Vec<Option<usize>> = spots.iter().map(|(_, p)| nav.nearest(*p)).collect();
        let Some(Some(start)) = nodes.first().copied() else {
            println!("{}: no nav nodes at all", map.id);
            failed = true;
            continue;
        };
        let core = nav.mutually_reachable(start);
        let core_size = core.iter().filter(|c| **c).count();
        let edges: usize = nav.edges.iter().map(|e| e.len()).sum();
        println!(
            "{}: {} nodes, {} links, {} in the main connected area",
            map.id,
            nav.nodes.len(),
            edges,
            core_size
        );
        for ((name, p), n) in spots.iter().zip(&nodes) {
            let ok = n.is_some_and(|n| core[n]);
            let snapped = n.map_or(f32::INFINITY, |n| nav.pos(n).distance(*p));
            if !ok || snapped > 1.5 {
                failed = true;
                println!(
                    "  UNREACHABLE {name} at ({:.1}, {:.1}) (nearest node {:.1} away)",
                    p.x, p.y, snapped
                );
            }
        }

        if let Some(path) = &dump {
            let mut s = String::from("{\"nodes\":[");
            for (i, p) in nav.nodes.iter().enumerate() {
                let sep = if i > 0 { "," } else { "" };
                write!(s, "{sep}[{:.2},{:.2},{}]", p.x, p.y, core[i] as u8).unwrap();
            }
            s.push_str("],\"edges\":[");
            let mut first = true;
            for (a, list) in nav.edges.iter().enumerate() {
                for e in list {
                    let kind = match e.kind {
                        EdgeKind::Walk => "w",
                        EdgeKind::Jump => "j",
                        EdgeKind::Fall => "f",
                        EdgeKind::Drop => "d",
                        EdgeKind::Thrust => "t",
                    };
                    let sep = if first { "" } else { "," };
                    first = false;
                    write!(s, "{sep}[{a},{},\"{kind}\"]", e.to).unwrap();
                }
            }
            s.push_str("]}");
            std::fs::write(path, s).unwrap();
        }
    }
    if failed {
        std::process::exit(1);
    }
}
