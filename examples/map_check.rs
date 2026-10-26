//! Map inspection tool: runs the map checks the editor's Validate button uses
//! (every spawn, pickup and object reachable by bots, no spawn in lava or
//! water), and optionally dumps the bot navigation graph as JSON for drawing
//! an overview.
//!
//! cargo run --release --example map_check                 # all maps
//! cargo run --release --example map_check -- <map_id> [nav.json]

use arcane_dome::ai::{EdgeKind, NavGraph, check_map};
use arcane_dome::data::GameData;
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
        let edges: usize = nav.edges.iter().map(|e| e.len()).sum();
        println!("{}: {} nodes, {} links", map.id, nav.nodes.len(), edges);
        for p in check_map(map) {
            failed |= !p.warning;
            let what = if p.warning { "warning" } else { "PROBLEM" };
            println!("  {what}: {} at ({:.1}, {:.1})", p.message, p.at.x, p.at.y);
        }

        if let Some(path) = &dump {
            let start = map.spawns.first().and_then(|s| nav.nearest(s.feet()));
            let core = start.map_or(vec![false; nav.nodes.len()], |n| nav.mutually_reachable(n));
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
