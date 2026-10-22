>  [!CAUTION]
> # VERY VERY VERY VERY EARLY development!1!1!1! Do not expect playable visuals and gameplay yet!

# Arcane Dome

A 2D side-scrolling platform arena shooter written in Rust with
[Macroquad](https://macroquad.rs). Fast arena matches against AI bots, lots of
weapons, lots of maps, all data-driven through JSON, for now.

> Status: early development. See [SPEC.md](SPEC.md) for the design and milestones.

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

Check that every spawn, pickup and object can be reached by bots:

```bash
cargo run --release --example map_check
```

```bash
cargo run --release --example map_check -- foundry_ascent nav.json
```

The second form checks one map and also writes its navigation graph as JSON.
Unfinished maps live in `drafts/maps/` and are not loaded by the game.

## License

Code is released under the [MIT License](LICENSE). Third-party assets keep their
own licenses, listed in [CREDITS.md](CREDITS.md).
