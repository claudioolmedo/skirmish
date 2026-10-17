# Skirmish II

**Skirmish II** is a from-scratch browser real-time strategy game inspired by
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
- **Adaptive quality**: one build probes CPU/memory/display at load and
  scales visuals — sprite tiers, resolution cap, cosmetic density —
  never the simulation. The governor shipped in the web client ahead
  of the rest of M4; see ARCHITECTURE.md ("Adaptive quality governor").

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical contract.

## Roadmap

| Milestone | Scope |
|-----------|-------|
| **M0** ✅ | Repo scaffolding; `sim` crate with fixed-point math, seeded RNG, tick loop; determinism harness (per-tick state hash, headless replay, CI) |
| **M1** ✅ | Unit movement: A\* on a coarse grid + steering (separation + arrival); `Move` commands; debug preview page |
| **M2** ✅ | Economy loop: resources + stockpile, resource nodes, villagers that gather/carry/deposit, Town Center that trains villagers |
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

## Debug preview (M2)

A throwaway view of the simulation in 2D isometric (AoE-style): flat
programmer-art sprites on a fixed-angle isometric projection — no real
3D — showing the grid, walls, resource nodes, the Town Center,
villagers working the economy loop, the live stockpile, and the live
state hash. Programmer art only; the production renderer is M4.

```sh
./web-preview/build.sh        # builds sim → web-preview/pkg/ (wasm-pack)
cd web-preview && python3 -m http.server 8080
# open http://localhost:8080/ — click a resource node to send all
# villagers to gather it, click open ground to move them there, and
# use the button to train more villagers (50 food)
```

## Documentation & contributing

Skirmish II is built in the open — and what is still missing is written
down, rule by rule, so anyone can pick it up:

- [docs/ROADMAP.md](docs/ROADMAP.md) — what exists, **what's missing**,
  the most-wanted list, and good first contributions
- [docs/MECHANICS-RULEBOOK.md](docs/MECHANICS-RULEBOOK.md) — the complete
  behavioral rulebook (213 rules, each with a status)
- [docs/GAP-ANALYSIS.md](docs/GAP-ANALYSIS.md) — the parity inventory
  against the code
- [docs/OPENING-SPEC.md](docs/OPENING-SPEC.md) — the standard start,
  minute by minute
- [docs/MECHANICS-STUDY.md](docs/MECHANICS-STUDY.md) — the numbers
- [docs/OPENAGE-RESEARCH.md](docs/OPENAGE-RESEARCH.md) — engine lessons
- [docs/SPRITE-PIPELINE.md](docs/SPRITE-PIPELINE.md) — how the original
  art is produced
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to build, test, and submit

## License

To be decided.
