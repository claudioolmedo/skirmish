# Skirmish — ARCHITECTURE

> **Source of truth.** Every agent (human or AI) working on this codebase must
> follow this document. If a change conflicts with it, the change is wrong —
> update this file deliberately first, with a clear reason, never silently.
> Reviewed at each milestone.

## What this is

Skirmish is a from-scratch browser RTS inspired by Age of Empires II,
built in Rust and compiled to WebAssembly. Clean room: every line of code,
every sprite, and every UI layout is original. Tuning targets come only from
public, documented numbers (community wikis). Never the commercial game's
binary, assets, or interface.

## The locked decisions

These were locked at M0 and do not change without an explicit, recorded
decision by the project owner.

1. **Headless deterministic sim crate.** All game rules live in `sim/`, a
   pure-Rust crate with no rendering, no input handling, no networking, no
   wall-clock time, and no threads. The outside world talks to it only
   through a thin `wasm-bindgen` ABI: *commands in, state out*. The renderer
   never computes rules. This is what makes adaptive visual quality and
   future lockstep multiplayer possible.
2. **Fixed-point math, always.** The sim uses the `fixed` crate (`I16F16`:
   ±32768 range, 1/65536 precision — plenty for game coordinates) everywhere.
   **Zero floating point in sim code** — not even `f32`, because
   transcendental functions differ across libm implementations. Floats are
   allowed *only* in renderer code (M4+) and at the wasm-bindgen boundary
   (converted to fixed-point immediately on entry), which never feed back
   into the sim.
3. **The tick count is the only clock.** Fixed 60 Hz ticks. No
   `Instant::now()`, no frame delta, no `std::time` in sim code. On the web
   the 60 Hz tick loop is decoupled from `requestAnimationFrame` via an
   accumulator, with a max-ticks-per-frame guard (spiral of death). On a slow
   machine the game slows down *visually*, never *logically*.
4. **One seeded RNG, inside the state.** A hand-rolled PCG32 lives in the
   sim state; every random draw goes through it. Never the `rand` ecosystem
   (implementations may change output between releases). The RNG words are
   part of the per-tick state hash.
5. **No hash maps in sim.** Ordered iteration only: `Vec` sorted by id,
   `BTreeMap` where a map is truly needed. `HashMap` iteration order is
   nondeterministic across platforms — it is the classic desync factory.
6. **Per-tick state hash from day one.** Every tick feeds a canonical byte
   stream (tick counter, RNG words, all entity fields as raw bits, in fixed
   order) into a hand-rolled, platform-independent FNV-1a 64. Any
   nondeterminism anywhere changes the hash. CI compares native and WASM
   hash streams (WASM-twin fixtures).
7. **Match = (seed, settings, command stream).** Commands are serializable
   data applied in FIFO order at tick start. Replays and lockstep-readiness
   come free from this shape. No networking is wired until multiplayer is
   actually built — but commands are *designed* as data from the start.

## Crate layout

```text
skirmish/
├── Cargo.toml            # workspace root
├── ARCHITECTURE.md       # this file
├── README.md
├── .github/workflows/ci.yml
├── web-preview/          # throwaway debug slice (NOT the M4 renderer):
│   │                     # top-down Canvas 2D view of sim state; M1 click-
│   │                     # to-move, M2 adds nodes/gather/train + HUD
│   ├── index.html        # labeled debug page
│   ├── app.js            # dependency-free WASM loader + renderer
│   └── build.sh          # wasm-pack build of sim → web-preview/pkg/
└── sim/                  # the deterministic core
    ├── Cargo.toml        # pinned deps: fixed, wasm-bindgen
    ├── src/
    │   ├── lib.rs        # Sim: tick loop, commands, state hash
    │   ├── rng.rs        # hand-rolled PCG32 (in sim state)
    │   ├── hash.rs       # hand-rolled FNV-1a 64 (canonical hashing)
    │   ├── entity.rs     # entities, move orders, villager work state
    │   ├── econ.rs       # M2: resources, nodes, stockpile, econ numbers
    │   ├── grid.rs       # M1: coarse grid + deterministic A*
    │   ├── steer.rs      # M1: separation + arrival (integer sqrt)
    │   └── wasm.rs       # thin wasm-bindgen ABI (commands in, state out)
    ├── tests/
    │   ├── determinism.rs# the harness: identical runs, seed divergence,
    │   │                 # replay equality, golden-hash tripwire
    │   ├── movement.rs   # M1: A* routing, arrival, separation, determinism
    │   └── economy.rs    # M2: gather cycle, depletion, training, replay
    └── examples/
        └── replay.rs     # headless runner: N ticks from a seed, print hashes
```

Future crates plug in beside `sim/` (never inside it):

| Milestone | New home            | Contents |
|-----------|---------------------|----------|
| M0 ✅     | `sim` (harness)     | fixed-point math, seeded RNG, tick loop, determinism harness |
| M1 ✅     | `sim` (entities)    | A* + steering, command queue, real `Command` set |
| M2 ✅     | `sim` (entities)    | economy: workers, resources, buildings |
| M3        | `sim` (entities)    | combat, projectiles, first original empire |
| M4        | `web/`              | Canvas 2D renderer, input shell, adaptive-quality governor |
| M5        | `sim` + `web/`      | feel tuning, scripted AI, playtest loop |

`web/` (M4) reads sim state and sends commands. It never writes sim rules.

## M1 — movement (shipped 2026-10-14)

The movement model, and the decisions that keep it deterministic:

- **Commands.** `Command::Move { entity, x, y }` — plain serializable data,
  applied FIFO at tick start. The A* path is recomputed *immediately* when
  the command is applied (not lazily at arrival), so a match is still fully
  described by (seed, command stream). A later `Move` replaces the order;
  unreachable/blocked targets clear it — the entity holds position.
- **Grid.** Fixed 200×200 world, 50×50 coarse cells (4 units each).
  `world_to_cell` is a pure fixed-point mapping; out-of-bounds counts as
  blocked. Obstacles are compile-time wall constants (vertical wall with a
  3-cell gap + one horizontal wall) — identical for every seed, and hashed
  so the golden tripwire pins the layout.
- **A*.** Integer costs (10 orthogonal / 14 diagonal), octile heuristic,
  fixed neighbor order (E, W, N, S, NE, SE, NW, SW), no corner cutting, and
  a `BinaryHeap` with a *total* deterministic ordering `(f, h, cell)`.
  Waypoints are cell centers ending exactly on the commanded target point.
- **Steering.** Per tick, per ordered entity: advance past reached
  waypoints (1.0-unit reach radius), desired velocity toward the current
  waypoint with linear arrival slowdown inside 4.0 units of the final leg,
  plus separation (2.0-unit radius, overlap-proportional push, 0.75 force
  scale) summed over the id-ordered entity list. Combined velocity is
  clamped to 1.0 unit/tick. All distances use a hand-rolled integer square
  root on raw fixed-point bits — no floats, no trig, ever.
- **WASM boundary.** `push_move(entity, x, y)` accepts f64 *only* at the
  JS boundary and converts to fixed-point on entry. New read-only
  accessors: `entity_count`, `positions`, `entity_path`, grid layout,
  `obstacles`. The sim never sees a float.
- **Debug preview.** `web-preview/` is an explicitly throwaway top-down
  Canvas 2D slice for watching M1 movement (grid, walls, entities, paths,
  live state-hash HUD). It is NOT the M4 renderer: no isometric camera,
  no art, no input shell. It must keep working when served statically.

## M2 — economy loop (shipped 2026-10-16)

The AoE2-style economy: villagers gather from nodes, haul to the Town
Center, deposit, and repeat; the Town Center trains more villagers.

- **Starting layout (fixed, no RNG).** Entity 0 is the Town Center at
  cell (23, 23) — world (-6, -6), just west of the wall gap. Three
  villagers start at fixed offsets. Stockpile starts at the standard
  200 food / 200 wood / 100 gold / 200 stone. The RNG is still in state
  and hashed (seeds diverge from tick one); M2 systems simply don't draw
  from it yet.
- **Nodes.** 20 nodes at fixed cells (`econ::NODE_SPECS`): trees (100
  wood), berry bushes (125 food, close to the TC), gold mines (800),
  stone mines (350). Amounts drain bit-exactly as villagers gather —
  every bit taken from a node lands in a carry, and every deposit moves
  the whole carry — and a node at zero is inert and rejects new gather
  orders.
- **Gather rates** (per-tick fixed-point constants, tuned toward public
  wiki per-second values): wood 0.390/s (426 bits/tick), berries
  0.310/s (339), gold 0.380/s (415), stone 0.360/s (393). Carry
  capacity 10; villager cost 50 food; train time 1500 ticks (25 s);
  queue cap 15, cost deducted on enqueue, rejected orders deduct
  nothing.
- **Villager work state machine** (`entity::Task`): `Idle → ToNode →
  Gathering → Returning → (deposit) → ToNode …`. The persistent order
  is the assignment (`task_node`); the phases are mechanism. Transitions
  trigger on *distance* (gather range 2.5, deposit range 3.0), not only
  on move-order completion, so separation pressure near a node or the
  TC can never stall the cycle. If the node dies mid-task: haul the
  carry home, deposit, then go idle. Ordered onto a different resource
  while carrying: deposit the current carry first, then head out.
  A plain `Move` cancels the task — and the villager **keeps** its
  carry, un-deposited, until a later work cycle delivers it (deliberate
  choice: carried resources are never silently created or destroyed).
- **Buildings don't move or block.** The Town Center ignores `Move` and
  `Gather`, and is excluded from the steering snapshot: separation
  neither pushes it nor pushes villagers away from it, so a hauler can
  always reach its exact position to complete a drop-off.
- **Steering revision (supersedes the M1 note above).** Separation is
  now *displacement*-based — half the overlap per tick — and applies
  also to entities with no move order (mutual shove). Two bugs found in
  M2 testing forced this: (1) a standing villager exactly on a
  traveler's path cancelled its velocity in perfect equilibrium
  (head-on deadlock); (2) the old 0.75 force spring, with stateless
  velocity and 1-unit steps, oscillated with period 2 when units
  converged on one point. Displacement at 0.5×overlap is critically
  damped for a pair: they separate to exactly the radius and stop.
- **Hash.** Extended to cover the stockpile, node amounts, the training
  queue/progress, entity kinds, and villager work state (task phase,
  assignment, carry). Golden hash updated deliberately (see
  `tests/determinism.rs`).
- **WASM boundary.** New commands `push_gather(entity, node)` and
  `push_train_villager()`; new accessors `stockpile()` (+ per-resource
  getters), `nodes()` (flat `[id, x, y, kind, amount_left]`),
  `entity_kinds()`, `villager_count()`, `train_queue_len()`.
- **Debug preview.** Updated to M2: resource/villager HUD, nodes drawn
  by kind (depleted ones vanish), TC as a square, click a node to send
  all villagers gathering, click ground to move, and a "Train villager
  (50 food)" button.

## The sim may

- Use fixed-point arithmetic (`I16F16`: ±32768 range, 1/65536 precision —
  plenty for game coordinates) for all positions, velocities, timers.
- Draw randomness only from the in-state PCG32.
- Iterate `Vec`s in id order and `BTreeMap`s.
- Grow new *deterministic* systems, each covered by a determinism test.

## The sim must NEVER

- Use `f32`/`f64`, `HashMap`/`HashSet` iteration, threads, atomics, or time.
- Read the environment: no env vars, no files, no network, no `getrandom`
  without an explicit seed path (note: `getrandom` with JS/WASM backend is
  nondeterministic — never use it in sim).
- Depend on a crate whose output could change between versions for anything
  that affects state. (`fixed` is pinned exactly; arithmetic is integer math.)
- Copy mechanics *implementations* from anywhere — behavior targets come
  from public documented numbers only.

## Determinism tests are mandatory

Every new sim feature ships with a test proving it does not break
determinism: same seed + same commands → identical per-tick hashes. The
golden-hash test in `tests/determinism.rs` is a tripwire — if it fails after
an intentional change, update the expected hashes deliberately, never blindly.

## Clean-room rules (legal safety)

- All code, art, names, and UI layouts are original, created from scratch.
- No reference to, decompilation of, or extraction from the commercial
  game's binary or assets — ever. (Precedent: studios are actively issuing
  takedowns for AI-assisted recreations.)
- Gameplay *feel* targets come from public community documentation
  (gather rates, unit stats on wikis), never from the binary.
- "Inspired by the mechanics" is fine. Similar-looking art or UI is not —
  trade dress matters from the first sketch.
