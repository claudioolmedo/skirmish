# Skirmish

**Skirmish** is a from-scratch browser real-time strategy game inspired by
Age of Empires II, built in Rust and compiled to WebAssembly. Same familiar
feel and mechanics — zero relearning for RTS players — with all-original
code, art, and interface. One build adapts its visual quality to any machine,
from old laptops to gaming PCs, while the simulation stays identical for
everyone.

Send a link, play with friends. Like chess, but with catapults.

## Clean room

This project is a clean-room implementation:

- Every line of code, every sprite, every name, and every UI layout is
  created from scratch for this project.
- Gameplay tuning targets come only from **public, documented numbers**
  (community wikis: gather rates, unit stats, pacing).
- Never the commercial game's binary, assets, interface, or extracted data —
  no decompilation, no ripping, no exceptions.

## Tech overview

- **Headless deterministic simulation** (`sim/`): pure Rust, fixed-point math
  (`fixed` crate, zero floats), a hand-rolled seeded RNG inside the state,
  fixed 60 Hz ticks, and a per-tick platform-independent state hash. No
  rendering, no wall clock, no threads, no hash maps — by construction, the
  same seed always replays bit-identically, including on WebAssembly.
- **Thin WASM ABI**: commands in, state out. The renderer (M4) will only
  ever read state through this boundary.
- **Adaptive quality** (M4): one build probes CPU/memory/GPU at load and
  scales visuals — never the simulation.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical contract.

## Roadmap

| Milestone | Scope |
|-----------|-------|
| **M0** ✅ | Repo scaffolding; `sim` crate with fixed-point math, seeded RNG, tick loop; determinism harness (per-tick state hash, headless replay, CI) |
| M1 | Unit movement: A\* on grid + steering/separation; command queue |
| M2 | Economy loop: workers, resource nodes, gather/carry/drop-off, buildings |
| M3 | Combat: attack-move, targeting, HP/death, projectiles; first original empire |
| M4 | Renderer (Canvas 2D) + input shell + adaptive-quality governor |
| M5 | Feel tuning + scripted opponent AI; playtest loop |

Multiplayer, lobby, and ads are explicitly out of the prototype — they come
after the game feels right.

## Building

Requires Rust stable (1.99+) with the `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown

cargo test                    # determinism harness + unit tests
cargo run --example replay -- 42 600   # headless: 600 ticks from seed 42
cargo build --target wasm32-unknown-unknown   # WASM build of the sim
```

## License

To be decided.
