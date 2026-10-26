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
- Deathmatch: most kills wins
- Elimination, in three styles:
  - Rounds: one life per round; the last side standing takes the round, most rounds wins
  - Last Man Standing: everyone has their own lives; out of lives, out (spectating)
  - Attrition (teams only): each team shares a pool of lives; when it is empty nobody
    respawns, and the team fights on until its last fighter falls
- Both play free-for-all or in 2 to 4 teams (Team Deathmatch, Team Elimination). Each team
  picks a colour: Blue, Red, Green or Yellow.

Custom match screen: map, mode, score or time limit, number of teams, bots and difficulty per
team (or bot count and difficulty in free-for-all). Every match and every elimination round
starts with a 3 second countdown ("3, 2, 1, GO"); fighters can aim but not move or shoot.

## Later (NOT Milestone 1)
- Credits earned per kill with bonuses (headshot, multikill, melee) used to unlock weapons and skins
- Armor abilities and one-use gadgets: mines, teleport, cloak, heal kit, static field, ammo refill
- Classes (Medic, Sniper, Heavy, etc.) with weapon pools
- LAN or online multiplayer

## Milestone 1 checklist (done, v0.1.0)
- [x] Window, fixed-step loop, terrain rendering, camera (freeform polygon terrain instead of a tilemap)
- [x] Player movement with gravity, double jump, crouch, one-way platforms (hatches: stand on them, drop through with S, fly up through them)
- [x] Terrain collision (polygon ray probes), tests for edge cases (corners, slopes, hatches)
- [x] Weapon loader from JSON, 10 weapons covering at least 5 projectile types
- [x] Hitscan, bullet, bouncing grenade, rocket with splash, beam
- [x] Health, damage, headshot multiplier, death, respawn at spawn points
- [x] Pickups: health, ammo, weapon crates
- [x] Bot AI: waypoint graph auto-built from the terrain, chase, strafe, shoot, retreat when low health
- [x] 3 maps loaded from JSON (Foundry Ascent, Frostline Relay, Redstone Arches)
- [x] 3 modes above, HUD (health, ammo, weapon bar, score, timer), scoreboard
- [x] Main menu and custom match screen

Beyond the plan, Milestone 1 also gained: thrusters, weapon and shield stations, four
powerups, 2 to 4 teams with pickable colours, free-for-all Elimination, a start countdown,
a death screen, and bot-behaviour tools (`map_check`, `bot_watch`).

## Milestone 2 (next)
- [ ] Map editor in-game, so anyone can build maps (v1 below; hand-placed bot jump links in v2)
- [ ] Animations for fighters and weapons
- [ ] Textures for terrain and decor
- [ ] Sound: CC0 packs and generated effects, credited in CREDITS.md
- [ ] Maybe: Capture the Flag and King of the Hill

### Map editor v1 (Milestone 2, step 1) (done)
An in-game screen ("Map Editor" on the main menu). It reuses the map renderer,
physics and nav graph, and writes the same map JSON the game loads.

- **Shapes**: draw polygons point by point on a grid (snap 0.5 tile, hold Shift
  for free placement). Kinds: solid, hatch, water, lava. Each shape has a
  material. Select, move, delete; drag single points.
- **Things**: place spawns (team Any/Blue/Red/Green/Yellow, plus per-team-count
  overrides), pickups (health, ammo, weapon), thrusters (height), weapon and
  shield stations, powerup spots.
- **Map settings**: name, size, supported team counts, sky preset.
- **Undo/redo** for every edit (Ctrl+Z / Ctrl+Y).
- **Validate**: runs the normal map checks plus the nav check (every spawn can
  reach every other) and the headroom check, and lists problems; clicking one
  jumps the camera to it.
- **Test-play**: starts a quick match with bots on the map being edited;
  leaving returns to the editor with nothing lost.
- **Saving**: work in progress is saved to `assets/editor/<id>.json` at any
  time, even if broken. "Publish" copies it to `assets/data/maps/<id>.json`
  only when Validate passes, because one broken file in `maps/` stops the game
  from loading. Published maps show up in the setup map list.
- **Sharing**: Export writes `<id>.json` to "Arcane Dome Maps" in Documents
  (or `ARCANE_DOME_MAPS`); Import reads from there or from a file dropped on
  the window. An id clash asks: replace your draft, or import as a copy.
- **v2 (later)**: decor, themes and sky editing, hand-placed bot jump links.

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

The Milestone 1 set, with plain display names for now (file ids in brackets):
Pistol (peacekeeper, starter), Shotgun (scattergun, 8 pellets), Assault Rifle (arc_rifle, automatic),
Marksman Rifle (needle_gun, high headshot mult), Sniper Rifle (rail_spike, hitscan, pierce),
Grenade Launcher (thumper, bouncing grenade), Rocket Launcher (hellbarrel, splash), Laser (sunbeam,
continuous beam), Leech SMG (leechfang, lifesteal), Flamethrower (cinder_spitter, burn).
`short_name` (optional) is the label on the weapon bar.

---

## Map JSON schema

One file per map in `assets/data/maps/`. Terrain is a list of freeform polygons
in tile units (y grows downward). Spawns, pickups and objects give the **feet**
position of whoever stands there.

```json
{
  "id": "foundry_ascent",
  "name": "Foundry Ascent",
  "description": "One or two sentences for the setup screen.",
  "tile_size": 32,
  "size": [64, 36],
  "theme": { "sky_top": "#150f12", "sky_bottom": "#5c2f1c", "far": "#2c1d1a",
             "skyline": "girders", "sun": [0.5, 0.95], "sun_color": "#ff8a3c" },
  "modes": ["deathmatch", "elimination"],
  "recommended_players": [4, 12],
  "shapes": [
    { "kind": "solid",  "material": "rock",  "points": [[0, 32], [23, 32], [25.5, 34.2], [25.5, 36], [0, 36]] },
    { "kind": "hatch",  "material": "metal", "points": [[9.3, 26], [10.7, 26], [10.7, 26.25], [9.3, 26.25]] },
    { "kind": "hazard", "material": "lava",  "points": [[25, 34], [39, 34], [39, 36], [25, 36]], "dps": 60 }
  ],
  "decor": [
    { "type": "window", "x": 5, "y": 30, "w": 1.6, "h": 1.2, "color": "#ff9a4a", "front": false, "flip": false }
  ],
  "spawns": [
    { "x": 6, "y": 32, "team": 0 },
    { "x": 20, "y": 32, "team": null }
  ],
  "pickups": [
    { "type": "health", "x": 2, "y": 23, "respawn_s": 25 },
    { "type": "weapon", "x": 26, "y": 18, "weapon_id": "thumper", "respawn_s": 30 }
  ],
  "objects": [
    { "type": "thruster", "x": 10, "y": 32, "height_tiles": 24.5 },
    { "type": "weapon_station", "x": 14, "y": 26 },
    { "type": "shield_station", "x": 62, "y": 12, "shield": 100, "min_delay_s": 90, "max_delay_s": 120 },
    { "type": "powerup_spot", "x": 32, "y": 8.5, "powerup": "critical_strike" }
  ]
}
```

- Shape kinds: `solid` (blocks bodies and bullets from all sides), `hatch` (a
  floor only from above; drop through it, thrusters fly up through it), `water`,
  `hazard` (`dps` per second inside).
- Materials: rock, metal, wood, ice, sand, moss, sludge, glass, lava, spikes,
  water, void. Sludge is for sewer-themed maps only.
- Skylines: city, mountains, spires, clouds, girders, none.
- Decor types: wall, window, girder, pipe, lamp, tree, crystal, chain, banner,
  tank, fence, vines, cactus. `front: true` draws over the fighters.
- Powerup spots: 0 to 4 per map (1 is the norm), each always spawning the same
  powerup. Shield stations: 1 to 4 small (50) and 0 to 2 big (100).
- Spawn `team`: team 0 to 3 (the player is on team 0), or null for free-for-all. `by_teams`
  gives a spawn another owner in matches with a given team count, e.g. `{"3": 2}` or `{"3": null}`. Teams 0 and 1
  need at least 2 spawns each; teams 2 and 3 are optional but need 2 if present.
- `team_counts` (optional), for example `[2, 4]`: the team counts the map is fair for.
  Default: 2 up to the number of teams with their own spawns. A four-corner map is fair
  for 2 or 4 teams but not 3 (two teams would share a side), so it lists `[2, 4]`.

Validation rules the loader enforces:
- The map is at least 8 x 8 and every polygon has at least 3 points.
- At least 2 spawns for teams 0 and 1 (and for teams 2 and 3 if they have any), and 4
  free-for-all spawns. Every count in `team_counts` has spawns for all its teams.
- Every `weapon_id` and `powerup` exists.
- Every spawn, pickup and object has room for a body and a floor close below it.
- Errors name the file and the field, for example
  `maps/foundry_ascent.json: spawns[2]: is inside solid terrain`.

Run `cargo run --release --example map_check` to confirm bots can reach every
spawn, pickup and object. The old ASCII `grid` + `legend` format still loads
(used by tests) but new maps should use shapes.

---

## modes.json

```json
{
  "deathmatch":  { "teams": 4, "respawn": true,  "score_limit": 25, "team_score_limit": 50, "time_limit_s": 600 },
  "elimination": { "teams": 4, "respawn": false, "rounds": 5, "time_limit_s": 180, "lives": 3, "team_lives": 15 }
}
```

`teams`: the most teams the mode can be played with (1 = free-for-all only). Teams are a
match setting, not a separate mode: "Teams: Off" plays free-for-all, 2 to 4 plays Team
Deathmatch or Team Elimination. The setup offers the team counts the mode and the map both
support. `team_score_limit` is the default score limit when played in teams. `lives` and
`team_lives` are the default lives for Last Man Standing and Attrition.

Every limit (score, time, lives) can be turned off in the match setup. With all of them
off the match is endless, and the setup says so; it is left from the pause menu.


