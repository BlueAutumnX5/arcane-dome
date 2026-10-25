>  [!CAUTION]
> # VERY VERY VERY VERY EARLY development!1!1!1! Do not expect playable visuals and gameplay yet!

# Arcane Dome

A 2D side-scrolling platform arena shooter written in Rust with
[Macroquad](https://macroquad.rs). Fast arena matches against AI bots, with
weapons, maps and modes all defined in JSON.

> Status: early development. See [SPEC.md](SPEC.md) for the design and milestones.

## Whats already in the game

- **3 maps**:
  - Foundry Ascent: a steel mill around a lava pit, with a climbable tower and
    a thruster shaft. 2 to 4 teams.
  - Frostline Relay: a frozen mountain relay station built for 3 and 4 teams,
    with cliffs, a valley, an ice island and a relay shaft to the tower roof.
  - Redstone Arches: a desert canyon built for 2 teams, under a giant stone
    arch you can walk across.
- **2 modes**, Deathmatch and Elimination, each played free-for-all or with
  2 to 4 teams (Blue, Red, Green, Yellow; each team picks its colour).
  Elimination comes in three styles: Rounds, Last Man Standing (everyone has
  their own lives) and Attrition (each team shares a pool of lives). Every
  limit can be turned off, even all of them for an endless match. Every
  match and round starts with a 3 second countdown.
- **10 weapons**: Pistol, Assault Rifle, Leech SMG (heals you as you hit),
  Shotgun, Flamethrower, Marksman Rifle, Sniper Rifle, Rocket Launcher,
  Grenade Launcher and Laser.
- **Bots** with a view cone, reaction time and three difficulties (Easy,
  Normal, Hard), set per team.
- **Map gadgets**: thrusters (stand on the pad and press jump), weapon
  stations, shield stations, and powerup spots (Divine Shield, Rapid Fire,
  Critical Strike, Shadowstep).
- **Map editor (preview)**: draw terrain, place spawns, pickups and gadgets,
  undo and redo, save drafts, open any map or start a new one. Every button
  explains itself when you rest the mouse on it, and Instructions (F1) covers
  the rest. Playing your own maps comes next.
- **Settings**: fullscreen, menu slides, blood effects and an FPS counter,
  remembered between runs. View Changes on the main menu shows what is new.
- Placeholder art: shapes and colours drawn in code. No sound yet.

## Controls

| Action | Keys |
|---|---|
| Move | A / D (or arrow keys) |
| Jump, double jump | W, Space or Up (press again in the air) |
| Crouch | Hold S |
| Drop through a hatch | Press S while standing on it |
| Aim and fire | Mouse, left button |
| Reload | R |
| Switch weapon | 1 to 0, Q / E, or the mouse wheel |
| Use Shadowstep | F |
| Scoreboard | Hold Tab |
| Pause | P or Esc |
| Fullscreen | F11 |

## Inspiration and credits

Arcane Dome is a fan-made, original game **inspired by the _Raze_ Flash game series
by JuiceTin and AddisonR**. Thank you for the countless hours of fun.

This project is **not affiliated with or endorsed by** the original authors. It
contains no art, audio, code, names or map layouts from the original games. All
code and content here are original or used under open licenses listed in
[CREDITS.md](CREDITS.md).

## Building

Requires a stable Rust toolchain.

```bash
cargo run --release
```

```bash
cargo test
```

On Linux, Macroquad needs the usual system libraries: ALSA, X11, Xi and an
OpenGL driver (for example `alsa-lib libx11 libxi mesa` on Arch).

## Adding content

- Weapons: one JSON file per weapon in `assets/data/weapons/`
- Maps: one JSON file per map in `assets/data/maps/`
- Powerups: `assets/data/powerups.json`

No code changes are needed to add a weapon or a map. Schemas are in [SPEC.md](SPEC.md).

### Maps

Maps are hand-designed out of freeform polygons (`shapes`). Each shape has a
`kind`:

- `solid`: blocks bodies and bullets from every side
- `hatch`: a thin floor you can stand on and drop through, and that thruster
  riders fly up through from below
- `water`: slows you down and lets you swim
- `hazard`: hurts anything inside it (`dps`), for lava, spikes or a bottomless void

A `material` (rock, metal, wood, sand, moss, sludge, lava...) picks the look.
All positions are in tiles. Spawns, pickups and objects are given by their
**feet** position (the point where a fighter would stand).

Objects are thrusters (`height_tiles`), weapon stations, shield stations
(`shield` 50 or 100) and powerup spots (`powerup`: always the same powerup).
Purely visual `decor` (walls, windows, girders, chains, lamps, trees...) and a
`theme` (sky colours, sun, skyline) set the mood.

Spawns belong to a team (0 to 3; the player is on team 0) or to free-for-all, and can change
owner with the number of teams (`by_teams`). A map
lists which team counts it is fair for with `team_counts`, for example `[2, 4]`.

Check that every spawn, pickup and object can be reached by bots:

```bash
cargo run --release --example map_check
```

```bash
cargo run --release --example map_check -- foundry_ascent nav.json
```

The second form checks one map and also writes its navigation graph as JSON.
Unfinished maps live in `drafts/maps/` and are not loaded by the game.

To watch bots play a map and report anyone stuck or falling into hazards (add
`MODE=elimination TEAMS=4` for team matches, `VERBOSE=1` for details):

```bash
cargo run --release --example bot_watch -- foundry_ascent 180 4
```

## Roadmap

Milestone 1 is done (v0.1.0). Milestone 2:

1. A map editor, so anyone can build maps (preview in v0.1.2; checking,
   test-playing and publishing maps come next)
2. Animations and textures
3. Sound
4. Possibly Capture the Flag and King of the Hill

## License

Code is released under the [MIT License](LICENSE). Third-party assets keep their
own licenses, listed in [CREDITS.md](CREDITS.md).
