# Arcane Dome

A 2D side-scrolling platform arena shooter written in Rust with
[Macroquad](https://macroquad.rs). Fast arena matches against AI bots, lots of
weapons, lots of maps, all data-driven through JSON.

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

No code changes are needed to add a weapon or a map. Schemas are in [SPEC.md](SPEC.md).

### Map variations

`examples/mapgen.rs` roughens a hand-made base map into seeded variants (uneven
ground, ragged edges, shifted platforms, asymmetric halves). Every result is
checked with the bot nav graph so all spawns and pickups stay reachable.

```bash
cargo run --release --example mapgen
```

Edit the `VARIANTS` list in that file to add variants or change seeds.

## License

Code is released under the [MIT License](LICENSE). Third-party assets keep their
own licenses, listed in [CREDITS.md](CREDITS.md).
