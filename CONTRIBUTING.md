# Contributing to Skirmish II

Welcome — Skirmish II is built in the open, and the fastest way to help is
to implement one of the rules that is still missing. The
[Roadmap](docs/ROADMAP.md) lists what exists, what is missing, and a
shortlist of good first contributions. Every missing mechanic has a rule
ID in the [Mechanics Rulebook](docs/MECHANICS-RULEBOOK.md) — pick one and
build exactly that.

## Repository layout

- `sim/` — the deterministic simulation (pure Rust, no rendering, no I/O)
- `web-preview/` — the browser client: landing page, the fullscreen play
  page (`web-preview/play/`), the Canvas renderer (`app.js`)
- `docs/` — the design documentation set (rulebook, gap analysis, specs)
- `tools/sprite-pipeline/` — the procedural sprite art generator
- `ARCHITECTURE.md` — the technical contract of the simulation

## Build & test

Requires Rust stable (1.99+) with the `wasm32-unknown-unknown` target,
plus Node.js for the client checks.

```sh
rustup target add wasm32-unknown-unknown

# Simulation tests — the determinism harness must stay green
cargo test

# Headless replay: 600 ticks from seed 42 (same hash on every machine)
cargo run --example replay -- 42 600

# Browser preview: build the sim to WebAssembly, then serve statically
./web-preview/build.sh
cd web-preview && python3 -m http.server 8080
# open http://localhost:8080/ and http://localhost:8080/play/

# Client checks
node --check web-preview/app.js
node --test web-preview/play/minimap.test.mjs
```

## House rules (please read — these are not negotiable)

### 1. Determinism is the architecture

The sim is built for lockstep multiplayer: same seed + same commands ⇒
bit-identical state on every machine, forever. That means:

- **Fixed-point math only** inside the sim (`fixed` crate, I16F16). No
  floats in simulation state or logic, ever.
- **The seeded RNG lives inside the state**, and its call count and call
  order must be identical on every run — draw random numbers only inside
  fixed, id-ordered iteration, never from rendering or timing code.
- **Any new state goes into the state hash.** If you add a field, a queue,
  or a subsystem, extend the hash serialization to cover it, and add or
  update a replay/determinism test that exercises it.
- Entities iterate in id order; no hash-map iteration order may influence
  the simulation. No wall clock, no threads, no system randomness in `sim/`.

### 2. Clean room — hard rule

Skirmish II is an original implementation inspired by a classic game:

- Features and numbers are researched **only from public documentation**
  (community wikis, published papers, public references).
- **All code and all art are original to this project.** Never copy game
  assets, sprites, sounds, code, text, or UI artwork from any existing
  game — not even "temporarily".
- Layouts and mechanics *behave* like the reference because behavior is
  the design target; the artwork expressing them is always ours.
- If a value cannot be verified from public sources, mark it
  **UNVERIFIED** in the docs/code — never guess a number into the game.

### 3. Documentation

- Project documentation is written **in English**.
- If your change flips a rulebook status (MISSING/DIFFERS → MATCHES),
  update the rule's status line in
  [docs/MECHANICS-RULEBOOK.md](docs/MECHANICS-RULEBOOK.md) in the same PR.
- Art goes through the sprite pipeline — see
  [docs/SPRITE-PIPELINE.md](docs/SPRITE-PIPELINE.md). One command
  (`python3 tools/sprite-pipeline/render.py`) regenerates every sheet;
  commit the generator change and the regenerated sheets together.

## Pull request flow

1. Branch off `main` (e.g. `eco-09-gatherer-retarget`).
2. Implement the rule(s); keep the change focused on them.
3. Run the full checks: `cargo test` green, `node --check` on any client
   file you touched, and the preview builds (`./web-preview/build.sh`).
4. Open the PR with a description that names the **rule IDs you
   implemented** (e.g. "Implements ECO-09 — gatherers retarget to a
   same-type neighbor") and says how you tested determinism.
5. Contributions are made under the project's license once it is
   published by the maintainer.

Not sure where to start? The "Good first contributions" table in
[docs/ROADMAP.md](docs/ROADMAP.md) is maintained exactly for that.
