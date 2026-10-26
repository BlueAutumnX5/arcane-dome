//! Map checks beyond what the loader enforces: can bots get everywhere, and
//! is every spawn out of lava and water? Used by the map editor's Validate
//! button and the `map_check` tool.

use super::NavGraph;
use crate::data::MapDef;
use crate::geo::ShapeKind;
use crate::sim::{STAND_SIZE, TEAM_NAMES};
use macroquad::math::{Vec2, vec2};

/// How far, in tiles, a thing may be from the nearest nav node and still
/// count as on the route map.
const NODE_REACH: f32 = 1.5;

/// One thing wrong with a map.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub message: String,
    /// Where it is, for jumping the camera there.
    pub at: Vec2,
    /// Worth a look, but the map still plays (errors block publishing).
    pub warning: bool,
}

impl Problem {
    fn error(message: String, at: Vec2) -> Self {
        Self {
            message,
            at,
            warning: false,
        }
    }

    fn warning(message: String, at: Vec2) -> Self {
        Self {
            message,
            at,
            warning: true,
        }
    }
}

/// Every spawn, pickup, object, zone and flag, with a name for messages.
fn spots(map: &MapDef) -> Vec<(String, Vec2)> {
    let mut spots = Vec::new();
    for (i, s) in map.spawns.iter().enumerate() {
        let team = s.team.map_or("free-for-all".to_owned(), |t| {
            TEAM_NAMES[t as usize % TEAM_NAMES.len()].to_owned()
        });
        spots.push((format!("Spawn {} ({team})", i + 1), s.feet()));
    }
    for (i, p) in map.pickups.iter().enumerate() {
        spots.push((format!("Pickup {} ({:?})", i + 1, p.kind), p.feet()));
    }
    for (i, o) in map.objects.iter().enumerate() {
        spots.push((format!("{} {}", o.kind.name(), i + 1), o.feet()));
    }
    for (i, z) in map.zones.iter().enumerate() {
        spots.push((format!("Zone {}", i + 1), z.feet()));
    }
    for (i, f) in map.flags.iter().enumerate() {
        spots.push((format!("Flag {}", i + 1), f.feet()));
    }
    spots
}

/// All problems with a map that loads: reachability for bots, then spawns
/// in hazards or water. Errors come before warnings.
pub fn check_map(map: &MapDef) -> Vec<Problem> {
    let mut out = reachability(map, &NavGraph::build(map));
    out.extend(spawn_safety(map));
    out.sort_by_key(|p| p.warning);
    out
}

/// Things bots cannot walk, jump, fall or thrust to (and back from).
pub fn reachability(map: &MapDef, nav: &NavGraph) -> Vec<Problem> {
    let spots = spots(map);
    let nodes: Vec<Option<usize>> = spots.iter().map(|(_, p)| nav.nearest(*p)).collect();
    let Some(Some(start)) = nodes.first().copied() else {
        let at = vec2(map.width, map.height) * 0.5;
        return vec![Problem::error(
            "Bots find nowhere to stand: add floors under the spawns".into(),
            at,
        )];
    };
    let core = nav.mutually_reachable(start);
    spots
        .iter()
        .zip(&nodes)
        .filter_map(|((name, p), n)| {
            let on_route = n.is_some_and(|n| core[n]);
            let near = n.is_some_and(|n| nav.pos(n).distance(*p) <= NODE_REACH);
            (!on_route || !near).then(|| {
                Problem::error(
                    format!("{name} can't be reached by bots from Spawn 1 (or back)"),
                    *p,
                )
            })
        })
        .collect()
}

/// Spawns standing in a hazard (error) or in water (warning). Room to
/// stand and a floor below are already checked by the map loader.
pub fn spawn_safety(map: &MapDef) -> Vec<Problem> {
    let half = STAND_SIZE.x / 2.0;
    let mut out = Vec::new();
    for (i, s) in map.spawns.iter().enumerate() {
        let feet = s.feet();
        let name = format!("Spawn {}", i + 1);
        // The standing body where the physics would rest it (on a slope, a
        // little above the feet point), lifted a hair off the floor.
        let rest = map
            .rest_height(feet, half)
            .map_or(feet.y, |y| y.min(feet.y));
        let min = vec2(feet.x - half, rest - STAND_SIZE.y);
        let max = vec2(feet.x + half, rest - 0.05);
        if map.terrain.hazard_dps(min, max) > 0.0 {
            out.push(Problem::error(format!("{name} stands in a hazard"), feet));
        } else if map.terrain.box_touches(min, max, ShapeKind::Water) {
            out.push(Problem::warning(format!("{name} stands in water"), feet));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, parse_map};
    use std::path::Path;

    /// The loader wants 2 spawns per team as well; these stand on the floor.
    const TEAMS: &str = r#"{"x": 3, "y": 16, "team": 0}, {"x": 7, "y": 16, "team": 0},
                           {"x": 33, "y": 16, "team": 1}, {"x": 37, "y": 16, "team": 1}"#;
    /// Four free-for-all spawns on the floor (Spawns 1 to 4).
    const FOUR: &str =
        r#"{"x": 5, "y": 16}, {"x": 15, "y": 16}, {"x": 25, "y": 16}, {"x": 35, "y": 16}"#;

    /// A 40 x 20 box with a floor at y 16, the spawns given, the team
    /// spawns, and any `extra` shapes (each starting with a comma).
    fn map(spawns: &str, extra: &str) -> MapDef {
        let text = format!(
            r#"{{"id": "t", "name": "T", "size": [40, 20], "modes": ["deathmatch"],
                "shapes": [{{"kind": "solid", "material": "rock",
                             "points": [[0, 16], [40, 16], [40, 20], [0, 20]]}}
                           {extra}],
                "spawns": [{spawns}, {TEAMS}]}}"#
        );
        parse_map("t.json", "t", &text, &[], &["deathmatch"]).unwrap_or_else(|e| panic!("{e:?}"))
    }

    #[test]
    fn the_shipped_maps_have_no_errors() {
        let data = GameData::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
            .expect("game data loads");
        for m in &data.maps {
            let errors: Vec<_> = check_map(m).into_iter().filter(|p| !p.warning).collect();
            assert!(errors.is_empty(), "{}: {errors:?}", m.id);
        }
    }

    #[test]
    fn a_flat_arena_is_fine() {
        assert_eq!(check_map(&map(FOUR, "")), vec![]);
    }

    #[test]
    fn a_walled_off_spawn_is_unreachable() {
        // A wall from the top of the map down to the floor at x 20 to 21.
        let wall = r#", {"kind": "solid", "material": "rock",
                         "points": [[20, 0], [21, 0], [21, 16], [20, 16]]}"#;
        let problems = check_map(&map(FOUR, wall));
        assert!(
            problems
                .iter()
                .any(|p| !p.warning && p.message.contains("can't be reached")),
            "{problems:?}"
        );
    }

    #[test]
    fn spawns_in_lava_or_water_are_reported() {
        // Lava around Spawn 2, water around Spawn 3.
        let extra = r#", {"kind": "hazard", "material": "lava",
                          "points": [[13, 15], [17, 15], [17, 16], [13, 16]]},
                        {"kind": "water", "material": "water",
                          "points": [[23, 15], [27, 15], [27, 16], [23, 16]]}"#;
        let problems = spawn_safety(&map(FOUR, extra));
        assert_eq!(
            problems
                .iter()
                .map(|p| (p.message.as_str(), p.warning))
                .collect::<Vec<_>>(),
            vec![
                ("Spawn 2 stands in a hazard", false),
                ("Spawn 3 stands in water", true)
            ]
        );
    }
}
