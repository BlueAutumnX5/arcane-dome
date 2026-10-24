//! Watches bot-only matches and reports trouble spots: bots touching a hazard
//! (lava, spikes, the void) and bots standing stuck in one place.
//!
//! cargo run --release --example bot_watch -- <map_id> [seconds] [matches]

use arcane_dome::ai::BotSet;
use arcane_dome::data::GameData;
use arcane_dome::sim::physics::{Aabb, hazard_dps};
use arcane_dome::sim::{Difficulty, Event, Input, MatchConfig, Phase, TICK_RATE, World};
use macroquad::math::Vec2;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// A rounded tile position.
type Spot = (i32, i32);

/// Seconds without leaving a small area before a bot counts as stuck.
const STUCK_S: f32 = 8.0;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = Arc::new(GameData::load(&root.join("assets")).unwrap_or_else(|e| panic!("{e}")));
    let mut args = std::env::args().skip(1);
    let map = args.next().unwrap_or_else(|| "foundry_ascent".into());
    let seconds: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(180);
    let matches: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);

    let mut hazards: BTreeMap<(i32, i32), u32> = BTreeMap::new();
    let mut stuck: BTreeMap<(i32, i32), u32> = BTreeMap::new();
    let mut origins: BTreeMap<(i32, i32, bool), u32> = BTreeMap::new();
    // Repeated take-offs from the same spot: a bot going round in circles.
    let mut loops: BTreeMap<(i32, i32), u32> = BTreeMap::new();
    // Planned jumps: (take-off spot, landing spot) -> (made it, missed).
    let mut jumps: BTreeMap<(Spot, Spot), (u32, u32)> = BTreeMap::new();
    let (mut kills, mut thrusts, mut hazard_deaths) = (0, 0, 0);
    // Behaviour, per difficulty: ticks alive, standing still, crouched, in a fight.
    let mut behaviour: BTreeMap<String, [u64; 5]> = BTreeMap::new();
    let mut doing: BTreeMap<(String, &str), u64> = BTreeMap::new();
    // Pauses of a second or more outside fights: (count, total seconds).
    let mut pauses: BTreeMap<String, (u32, f32)> = BTreeMap::new();

    for seed in 0..matches {
        for difficulty in [Difficulty::Easy, Difficulty::Hard] {
            // MODE=elimination TEAMS=4 watches team matches (2 bots a team).
            let mode = std::env::var("MODE").unwrap_or_else(|_| "deathmatch".into());
            let teams: u8 = std::env::var("TEAMS")
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or(0);
            let cfg = MatchConfig {
                map_id: map.clone(),
                mode_id: mode,
                teams,
                team_bots: [2; 4],
                team_difficulty: [difficulty; 4],
                bots: 7,
                difficulty,
                include_player: false,
                seed: seed * 31 + 5,
                ..Default::default()
            };
            let mut w = World::new(Arc::clone(&data), cfg).unwrap();
            let mut bots = BotSet::new(&w);
            let mut inputs = vec![Input::default(); w.fighters.len()];
            // Per fighter: (anchor position, seconds spent near it, already reported).
            let mut still: Vec<(Vec2, f32, bool)> =
                w.fighters.iter().map(|f| (f.feet, 0.0, false)).collect();
            let mut was_hurt = vec![false; w.fighters.len()];
            // Last place each fighter stood, and when it was last hit.
            let mut last_ground: Vec<Vec2> = w.fighters.iter().map(|f| f.feet).collect();
            let mut last_hp: Vec<f32> = w.fighters.iter().map(|f| f.health).collect();
            let mut hit_ago = vec![99.0f32; w.fighters.len()];
            let mut air_t = vec![0.0f32; w.fighters.len()];
            let mut trail: Vec<Vec<String>> = vec![Vec::new(); w.fighters.len()];
            let mut takeoffs: Vec<Vec<(f32, (i32, i32))>> = vec![Vec::new(); w.fighters.len()];
            let mut was_ground = vec![true; w.fighters.len()];
            let mut planned: Vec<Option<((i32, i32), Vec2)>> = vec![None; w.fighters.len()];
            let mut buried = vec![false; w.fighters.len()];
            let mut left_vel = vec![Vec2::ZERO; w.fighters.len()];
            // Seconds standing still right now, outside a fight.
            let mut calm = vec![0.0f32; w.fighters.len()];
            for _ in 0..seconds * TICK_RATE {
                bots.think(&w, &mut inputs);
                w.step(&inputs);
                for e in &w.events {
                    if matches!(e, Event::Thrust { .. }) {
                        thrusts += 1;
                    }
                }
                for (i, f) in w.fighters.iter().enumerate() {
                    if !f.alive {
                        if was_hurt[i] {
                            hazard_deaths += 1;
                        }
                        was_hurt[i] = false;
                        still[i] = (f.feet, 0.0, false);
                        continue;
                    }
                    let key = (f.feet.x.round() as i32, f.feet.y.round() as i32);
                    let b = behaviour.entry(format!("{difficulty:?}")).or_default();
                    b[0] += 1;
                    if f.on_ground && f.vel.x.abs() < 0.3 {
                        b[1] += 1;
                    }
                    if f.crouching {
                        b[2] += 1;
                    }
                    let about = bots.describe(&w, i);
                    let fighting = about.contains("visible true");
                    if !about.starts_with("idle 0.00") {
                        *doing
                            .entry((format!("{difficulty:?}"), "idle-flag"))
                            .or_default() += 1;
                        if f.vel.x.abs() > 0.3 || !f.on_ground {
                            *doing
                                .entry((format!("{difficulty:?}"), "idle-but-moving"))
                                .or_default() += 1;
                        }
                    }
                    for d in ["fight", "chase", "roam"] {
                        if about.contains(&format!("doing {d} ")) {
                            *doing.entry((format!("{difficulty:?}"), d)).or_default() += 1;
                        }
                    }
                    if fighting && about.contains("goal Retreat") {
                        b[4] += 1;
                    }
                    if fighting {
                        b[3] += 1;
                    }
                    // Real pauses: still for a second or more, not fighting.
                    if f.on_ground && f.vel.x.abs() < 0.3 && !fighting {
                        calm[i] += 1.0 / TICK_RATE as f32;
                        if (calm[i] - 1.0).abs() < 0.5 / TICK_RATE as f32 {
                            pauses.entry(format!("{difficulty:?}")).or_default().0 += 1;
                        }
                    } else {
                        if calm[i] >= 1.0 {
                            pauses.entry(format!("{difficulty:?}")).or_default().1 += calm[i];
                        }
                        calm[i] = 0.0;
                    }
                    if was_ground[i] && !f.on_ground && f.vel.y < -5.0 {
                        let now = w.time;
                        let list = &mut takeoffs[i];
                        list.retain(|(t, _)| now - t < 25.0);
                        let again = list.iter().filter(|(_, k)| *k == key).count();
                        list.push((now, key));
                        if again == 3 {
                            *loops.entry(key).or_default() += 1;
                            if std::env::var("VERBOSE").is_ok() {
                                println!(
                                    "  LOOP: #{i} took off from {key:?} 4 times in 25s | {}",
                                    bots.describe(&w, i)
                                );
                                if std::env::var("LOOP_TRAIL")
                                    .is_ok_and(|v| v == format!("{},{}", key.0, key.1))
                                {
                                    for l in trail[i]
                                        .iter()
                                        .skip(trail[i].len().saturating_sub(240))
                                        .step_by(3)
                                    {
                                        println!("      {l}");
                                    }
                                }
                            }
                        }
                    }
                    // Planned jumps: remember the target at take-off, judge on landing.
                    if !f.on_ground
                        && planned[i].is_none()
                        && let Some(t) = bots.landing_target(i)
                    {
                        planned[i] = Some((key, t));
                    }
                    if f.on_ground
                        && !was_ground[i]
                        && let Some((from, t)) = planned[i].take()
                    {
                        let made = (f.feet.x - t.x).abs() < 1.5 && (f.feet.y - t.y).abs() < 0.4;
                        let to = (t.x.round() as i32, t.y.round() as i32);
                        let e = jumps.entry((from, to)).or_default();
                        if made { e.0 += 1 } else { e.1 += 1 }
                    }
                    was_ground[i] = f.on_ground;
                    let inp = &inputs[i];
                    trail[i].push(format!(
                        "({:.2},{:.2}) v({:.1},{:.1}) g{} m{:.2} j{} c{} | {}",
                        f.feet.x,
                        f.feet.y,
                        f.vel.x,
                        f.vel.y,
                        f.on_ground as u8,
                        inp.move_x,
                        inp.jump as u8,
                        inp.crouch as u8,
                        bots.describe(&w, i)
                    ));
                    if trail[i].len() > 600 {
                        trail[i].remove(0);
                    }
                    // Deeper than the feet inset: really inside terrain.
                    let b = Aabb::from_feet(f.feet, f.size());
                    let inside = w
                        .map
                        .terrain
                        .box_blocked(b.min + Vec2::splat(0.1), b.max - Vec2::splat(0.1));
                    if inside && !buried[i] {
                        println!("  BURIED: #{i} at ({:.2},{:.2})", f.feet.x, f.feet.y);
                        if std::env::var("VERBOSE").is_ok() {
                            for l in trail[i].iter().skip(trail[i].len().saturating_sub(15)) {
                                println!("      {l}");
                            }
                        }
                    }
                    buried[i] = inside;
                    let hurt = hazard_dps(&w.map, f.feet, f.size()) > 0.0;
                    if hurt && !was_hurt[i] {
                        *hazards.entry(key).or_default() += 1;
                        let g = last_ground[i];
                        let k = (g.x.round() as i32, g.y.round() as i32, hit_ago[i] < 1.0);
                        *origins.entry(k).or_default() += 1;
                        if std::env::var("VERBOSE").is_ok() {
                            println!(
                                "  fell: from ({:.1},{:.1}) to ({:.1},{:.1}) air {:.2}s, took off at vel ({:.1},{:.1}), now vel ({:.1},{:.1}) {:?}",
                                g.x,
                                g.y,
                                f.feet.x,
                                f.feet.y,
                                air_t[i],
                                left_vel[i].x,
                                left_vel[i].y,
                                f.vel.x,
                                f.vel.y,
                                difficulty
                            );
                            for l in trail[i].iter().skip(trail[i].len().saturating_sub(20)) {
                                println!("        {l}");
                            }
                        }
                    }
                    if f.on_ground {
                        air_t[i] = 0.0;
                    } else {
                        if air_t[i] == 0.0 {
                            left_vel[i] = f.vel;
                        }
                        air_t[i] += 1.0 / TICK_RATE as f32;
                    }
                    if f.on_ground && !hurt {
                        last_ground[i] = f.feet;
                    }
                    hit_ago[i] = if f.health < last_hp[i] && !hurt {
                        0.0
                    } else {
                        hit_ago[i] + 1.0 / TICK_RATE as f32
                    };
                    last_hp[i] = f.health;
                    was_hurt[i] = hurt;
                    let s = &mut still[i];
                    // A deliberate pause is not being stuck.
                    let pausing = !bots.describe(&w, i).starts_with("idle 0.00")
                        && !bots.describe(&w, i).starts_with("idle -");
                    if f.feet.distance(s.0) > 2.5 || pausing {
                        *s = (f.feet, 0.0, false);
                    } else {
                        s.1 += 1.0 / TICK_RATE as f32;
                        if s.1 > STUCK_S && !s.2 {
                            s.2 = true;
                            *stuck.entry(key).or_default() += 1;
                            if std::env::var("VERBOSE").is_ok() {
                                let inp = &inputs[i];
                                println!(
                                    "  stuck: at ({:.2},{:.2}) vel ({:.1},{:.1}) ground {} move {:.2} jump {} crouch {} {:?}",
                                    f.feet.x,
                                    f.feet.y,
                                    f.vel.x,
                                    f.vel.y,
                                    f.on_ground,
                                    inp.move_x,
                                    inp.jump,
                                    inp.crouch,
                                    difficulty
                                );
                                for o in &w.fighters {
                                    if o.id != i && o.feet.distance(f.feet) < 3.0 {
                                        println!(
                                            "    near: #{} alive {} feet ({:.2},{:.2}) facing {} | {}",
                                            o.id,
                                            o.alive,
                                            o.feet.x,
                                            o.feet.y,
                                            o.facing,
                                            bots.describe(&w, o.id)
                                        );
                                    }
                                }
                                for l in trail[i].iter().skip(trail[i].len().saturating_sub(25)) {
                                    println!("      {l}");
                                }
                            }
                        }
                    }
                }
                if matches!(w.phase, Phase::MatchOver { .. }) {
                    break;
                }
            }
            kills += w.fighters.iter().map(|f| f.kills).sum::<u32>();
            if w.mode.is_team() {
                let per: Vec<String> = (0..w.num_sides())
                    .map(|t| format!("{} {}", w.side_name(t), w.scores[t]))
                    .collect();
                println!(
                    "  {difficulty:?} seed {seed}: {} | round wins {:?} | {:?}",
                    per.join(", "),
                    w.round_wins,
                    w.phase
                );
            }
        }
    }
    println!("{map}: {kills} kills, {thrusts} thruster launches, {hazard_deaths} hazard deaths");
    println!("hazard touches (x, y): count");
    for (k, n) in &hazards {
        println!("  {k:?}: {n}");
    }
    println!("hazard entered from last ground (x, y, hit within 1s): count");
    for (k, n) in &origins {
        println!("  {k:?}: {n}");
    }
    for ((d, what), n) in &doing {
        let all = behaviour.get(d).map_or(1, |b| b[0]);
        println!(
            "{d}: {what} {:.1}% of the time",
            *n as f64 * 100.0 / all as f64
        );
    }
    for (d, (n, secs)) in &pauses {
        let bot_minutes = behaviour
            .get(d)
            .map_or(1.0, |b| b[0] as f32 / TICK_RATE as f32 / 60.0);
        println!(
            "{d}: {:.1} real pauses per bot-minute, {:.1}s long on average",
            *n as f32 / bot_minutes,
            secs / (*n).max(1) as f32
        );
    }
    for (d, b) in &behaviour {
        let pct = |n: u64| n as f64 * 100.0 / b[0].max(1) as f64;
        println!(
            "{d}: standing still {:.1}%, crouched {:.1}%, fighting {:.1}% (backing off {:.1}%)",
            pct(b[1]),
            pct(b[2]),
            pct(b[3]),
            pct(b[4])
        );
    }
    println!("planned jumps that missed most (from -> to: made / missed):");
    let mut worst: Vec<_> = jumps
        .iter()
        .filter(|(_, (m, x))| *x >= 3 && *x * 3 >= *m)
        .collect();
    worst.sort_by_key(|(_, (m, x))| std::cmp::Reverse(*x * 100 / (*m + *x)));
    for ((from, to), (m, x)) in worst.iter().take(12) {
        println!("  {from:?} -> {to:?}: {m} / {x}");
    }
    let (all_m, all_x) = jumps.values().fold((0, 0), |a, (m, x)| (a.0 + m, a.1 + x));
    println!("  all planned jumps: {all_m} made, {all_x} missed");
    println!("loops: 4+ take-offs from one spot in 25s (x, y): count");
    for (k, n) in &loops {
        println!("  {k:?}: {n}");
    }
    println!("stuck > {STUCK_S}s (x, y): count");
    for (k, n) in &stuck {
        println!("  {k:?}: {n}");
    }
}
