# Arcane Dome: Specification

A side-scrolling platform arena shooter. Original code, original art, original names.
Stack: Rust + Macroquad, native on Linux and Windows, optional WASM later.
The focus is arena matches, many weapons, many maps, all data-driven.
A campaign (defending Earth with a dome powered by three artifacts placed around
the world) is planned for much later and is out of scope for now.

## Hard rules

1. All weapons and maps are data-driven. Weapons live in `assets/data/weapons/*.json`, maps in
   `assets/data/maps/*.json`. Adding a weapon or map must never need a code change.
2. Use only original or CC0 assets (placeholder colored rectangles are fine at first).
   Never copy art, audio, names or map layouts from the Flash games Raze, Raze 2 or Raze 3.
3. Fixed timestep simulation (60 Hz) separated from rendering, so the sim is deterministic and testable.
4. Keep gameplay code separate from rendering: modules `sim`, `render`, `data`, `ai`, `app`.
5. Validate JSON on load with clear error messages that name the file and field.
6. Write unit tests for movement, collision, damage and weapon loading.
7. Work in small milestones, commit after each, keep `cargo build`, `cargo test` and
   `cargo clippy -- -D warnings` passing.

## Game feel
- Side view, tile-based maps, camera follows player with slight look-ahead toward the mouse.
- WASD or arrows: move, W/Up/Space: jump, press again in air: double jump, S: crouch.
- Mouse aims, left click fires, 1-0 select weapon slot, Q/E cycle, R reload, P pause, Tab scoreboard.
- Matches are arena fights against AI bots, not linear levels.

## Modes (Milestone 1)
- Deathmatch (free for all)
- Team Deathmatch
- Team Elimination (no respawn, last team standing)

Custom match screen: map, mode, score or time limit, number of friendly and enemy bots, bot difficulty.

## Later (NOT Milestone 1)
- Credits earned per kill with bonuses (headshot, multikill, melee) used to unlock weapons and skins
- Armor abilities and one-use gadgets: mines, teleport, cloak, heal kit, static field, ammo refill
- Classes (Medic, Sniper, Heavy, etc.) with weapon pools
- Map editor in-game
- LAN or online multiplayer

## Milestone 1 checklist
- [ ] Window, fixed-step loop, tilemap rendering, camera
- [ ] Player movement with gravity, double jump, crouch, one-way platforms
- [ ] Tile collision (AABB vs grid), tests for edge cases (corners, slopes not needed yet)
- [ ] Weapon loader from JSON, 10 weapons covering at least 5 projectile types
- [ ] Hitscan, bullet, bouncing grenade, rocket with splash, beam
- [ ] Health, damage, headshot multiplier, death, respawn at spawn points
- [ ] Pickups: health, ammo, weapon crates
- [ ] Bot AI: waypoint graph auto-built from the tilemap, chase, strafe, shoot, retreat when low health
- [ ] 3 maps loaded from JSON
- [ ] 3 modes above, HUD (health, ammo, weapon bar, score, timer), scoreboard
- [ ] Main menu and custom match screen

## Suggested layout
```
arcane-dome/
  Cargo.toml
  SPEC.md
  assets/
    data/
      weapons/        # one JSON per weapon
      maps/           # one JSON per map
      modes.json
    sprites/  audio/  tilesets/
  src/
    main.rs
    app/  sim/  render/  data/  ai/
  tests/
```

---


## Weapon JSON schema

One file per weapon in `assets/data/weapons/`.

```json
{
  "id": "arc_rifle",
  "name": "Arc Rifle",
  "category": "automatic",
  "unlock_cost": 0,
  "damage": 9.0,
  "headshot_mult": 2.0,
  "fire_rate_rps": 8.0,
  "pellets": 1,
  "spread_deg": 2.5,
  "recoil": 0.4,
  "magazine": 30,
  "reserve_ammo": 150,
  "reload_time_s": 1.6,
  "range_tiles": 28,
  "projectile": {
    "type": "bullet",
    "speed": 90.0,
    "gravity": 0.0,
    "lifetime_s": 1.2,
    "bounces": 0,
    "homing_strength": 0.0,
    "pierce": 0,
    "explosion": null
  },
  "specials": [],
  "sprite": "weapons/arc_rifle.png",
  "sounds": { "fire": "sfx/arc_fire.ogg", "reload": "sfx/reload_a.ogg" }
}
```

Field reference:

| Field | Type | Notes |
|---|---|---|
| `id` | string | unique, snake_case, must match filename |
| `category` | enum | `starter, sidearm, close_range, automatic, long_range, heavy, explosive, melee` |
| `projectile.type` | enum | `hitscan, bullet, grenade, rocket, beam, melee_arc` |
| `projectile.explosion` | object or null | `{ "radius_tiles": 3.0, "damage": 60, "falloff": "linear", "knockback": 12 }` |
| `specials` | array of enums | `lifesteal, burn, multishot, credits_bonus, crit, stun, shield_break, freeze` |
| `specials` params | optional object | e.g. `{ "type": "lifesteal", "ratio": 0.25 }` |

Example set for Milestone 1 (use these as the 10, all original names):
Peacekeeper (starter pistol), Scattergun (close range, 8 pellets), Arc Rifle (automatic),
Needle Gun (long range, high headshot mult), Rail Spike (hitscan sniper, pierce), Thumper (bouncing grenade),
Hellbarrel (rocket, splash), Sunbeam (continuous beam), Leechfang (SMG with lifesteal), Cinder Spitter (burn).

---

## Map JSON schema

One file per map in `assets/data/maps/`. The collision grid is human-editable ASCII.

```json
{
  "id": "foundry_01",
  "name": "The Foundry",
  "tile_size": 32,
  "tileset": "tilesets/industrial.png",
  "background": { "sky_color": "#1b1f2a", "parallax": ["bg/pipes_far.png", "bg/pipes_near.png"] },
  "modes": ["deathmatch", "team_deathmatch", "team_elimination"],
  "recommended_players": [4, 12],
  "legend": {
    "#": "solid",
    "=": "one_way",
    "^": "spikes",
    "~": "water",
    ".": "air"
  },
  "grid": [
    "##########################",
    "#........................#",
    "#...====........====.....#",
    "#........................#",
    "#.....##############.....#",
    "#........................#",
    "##########################"
  ],
  "spawns": [
    { "x": 3,  "y": 5, "team": 0 },
    { "x": 22, "y": 5, "team": 1 },
    { "x": 12, "y": 1, "team": null }
  ],
  "pickups": [
    { "type": "health", "x": 12, "y": 3, "respawn_s": 20 },
    { "type": "ammo",   "x": 6,  "y": 5, "respawn_s": 15 },
    { "type": "weapon", "x": 18, "y": 3, "weapon_id": "rail_spike", "respawn_s": 40 }
  ],
  "hazards": [],
  "bot_hints": [
    { "x": 12, "y": 3, "kind": "camp_spot" }
  ]
}
```

Validation rules the loader must enforce:
- All rows in `grid` have equal length, and every character exists in `legend`.
- At least 2 spawns per team and 4 free-for-all spawns.
- Every `weapon_id` exists in the weapons folder.
- Every spawn and pickup sits on an `air` tile with solid or one-way ground reachable below.
- Report errors as `maps/foundry_01.json: spawns[2] is inside a solid tile`.

---

## modes.json

```json
{
  "deathmatch":       { "teams": 1, "respawn": true,  "score_limit": 25, "time_limit_s": 600 },
  "team_deathmatch":  { "teams": 2, "respawn": true,  "score_limit": 50, "time_limit_s": 600 },
  "team_elimination": { "teams": 2, "respawn": false, "rounds": 5,       "time_limit_s": 180 }
}
```

---

## Legal and sharing checklist

- Code you write is yours. Pick a license (MIT or GPL) before making the repo public.
- Do not include any file ripped from the original SWFs, and do not use the names "Raze", "Raze 2" or "Raze 3" as the title.
- Credit every CC0 or CC-BY asset in a `CREDITS.md`.
- Mechanics (double jump, arena bots, unlockable weapons) are fine to reuse. Specific art, sounds, map layouts and exact weapon stat tables are not.
- Personal use only is the lowest-risk path. For a public release with anything resembling the originals, ask the original developers first.
