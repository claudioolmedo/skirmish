# Skirmish II — ARCHITECTURE

> **Source of truth.** Every agent (human or AI) working on this codebase must
> follow this document. If a change conflicts with it, the change is wrong —
> update this file deliberately first, with a clear reason, never silently.
> Reviewed at each milestone.

## What this is

Skirmish II is a from-scratch browser RTS inspired by Age of Empires II,
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
    │   ├── build.rs      # M2.5: buildings, placement, population numbers
    │   ├── classes.rs    # M3: armor-class ids (membership bitmasks)
    │   ├── units.rs      # M3: the unit table (data: stats/costs/speeds)
    │   ├── combat.rs     # M3: damage formula, combat phase, projectiles
    │   ├── grid.rs       # M1: coarse grid + deterministic A*
    │   ├── steer.rs      # M1: separation + arrival (integer sqrt)
    │   └── wasm.rs       # thin wasm-bindgen ABI (commands in, state out)
    ├── tests/
    │   ├── determinism.rs# the harness: identical runs, seed divergence,
    │   │                 # replay equality, golden-hash tripwire
    │   ├── movement.rs   # M1: A* routing, arrival, separation, determinism
    │   ├── economy.rs    # M2: gather cycle, depletion, training, replay
    │   ├── construction.rs # M2.5: houses, farms, population, placement
    │   └── combat.rs     # M3: animals, hunting, production, replays
    └── examples/
        └── replay.rs     # headless runner: N ticks from a seed, print hashes
```

Future crates plug in beside `sim/` (never inside it):

| Milestone | New home            | Contents |
|-----------|---------------------|----------|
| M0 ✅     | `sim` (harness)     | fixed-point math, seeded RNG, tick loop, determinism harness |
| M1 ✅     | `sim` (entities)    | A* + steering, command queue, real `Command` set |
| M2 ✅     | `sim` (entities)    | economy: workers, resources, buildings |
| M3 ✅     | `sim` (entities)    | combat core, projectiles, animals, Barracks (first empire is M6) |
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
  fixed neighbor order (E, W, N, S, NE, SE, NW, SW), no corner cutting
  (M7 relaxes this past tree cells only — see the M7 section), and
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

## M2.5 — construction (shipped 2026-10-16)

Villagers raise buildings; the Town Center's output is capped by
population room. Numbers are from the public mechanics study (house
25 W / 1500 t / +5 room; farm 60 W / 900 t / 175 food; TC +5 room).

- **Placement is sim-validated.** `Command::Build { entity, kind, x, y }`
  names a villager, a kind, and a world point; the footprint is the
  2×2 cells anchored at the point's cell. The sim rejects the order
  whole (no deduction) when the footprint leaves the grid, overlaps a
  wall, a standing building, a live node, or the Town Center cell,
  when no ring cell around the footprint is reachable, or when wood is
  short. Validation lives in one function shared by the command path
  and the read-only `can_build` query, so UI affordances can't drift
  from the rules — lockstep multiplayer inherits the checks as-is.
  Ordering onto an *incomplete* building of the same kind at the same
  anchor resumes it instead of placing a copy (and charges nothing).
- **Blocking.** Houses mark their footprint in the grid's new dynamic
  layer (`Grid::dynamic`), which A* respects and the hash covers.
  Farms don't block: the farmer stands on the field to work it.
- **Construction flow.** The builder travels to a ring cell
  (`Task::ToBuild`), then builds in place (`Task::Building`); progress
  accrues only while a villager is in that phase, so an abandoned site
  just waits. On completion a House starts counting toward the cap; a
  Farm becomes an ordinary food node (175, farm gather rate) that its
  builders start working unprompted, depletes like any node, and then
  retires — the ground is placeable again.
- **Population.** Capacity = Town Center (+5) + completed Houses (+5).
  Enqueueing a villager is unchanged (food + queue cap), but training
  *progress* stalls while living villagers are at capacity — the
  reference game's "housed" state — and resumes by itself when a house
  completes. `train_block_reason` tells the client which of ok /
  no-food / queue-full / housed applies.
- **Hash.** Extended to cover the dynamic grid layer, the buildings
  list (kind, anchor, progress, completion, farm node link), and the
  villagers' construction assignment. Golden hash updated deliberately
  (see `tests/determinism.rs`).
- **WASM boundary.** New commands `push_build_house` /
  `push_build_farm` (same would-it-be-accepted return pattern as
  training); new accessors `buildings()` (flat, with progress and
  completion), `population()`, `population_cap()`,
  `train_block_reason()`; farm nodes appear in `nodes()` with kind 4.
- **Play page.** The game moved out of the landing page to a dedicated
  fullscreen page (`web-preview/play/`, served at `/play`) with the
  start overlay, pause menu, villager selection, build menu with
  sim-mirrored placement ghost, and population HUD. The landing page
  links to it; ad slots stay on the landing page only.

## M3 — combat core (shipped 2026-10-16)

Units fight, animals live and die, and the village gains its first
military. All numbers come from the mechanics study / rulebook
(MECHANICS-STUDY.md, MECHANICS-RULEBOOK.md §§8–10) and the opening
spec; anything the sources don't pin down is marked UNVERIFIED in
`units.rs` and never silently invented.

- **Phase order (SIM-07).** commands → movement → **combat** →
  economy. The combat phase runs, in fixed order: sheep capture,
  animal behavior, target acquisition, attack machines (ascending
  attacker id), projectile impacts (spawn order), and the death
  sweep (ascending id). Carcass decay is economy, before work.
- **The unit table is data** (`units.rs::spec`): HP, attack rows
  (armor class + value), melee/pierce armor, class membership
  bitmask (`classes.rs`), range, reload, attack delay, accuracy,
  projectile speed/scatter, speed, LoS, collision radius, carcass
  yield/decay, cost, train time. Eight kinds: Villager, Town Center
  (stats for display — buildings take no damage yet), Scout Cavalry,
  Militia, Archer, Sheep, Boar, Deer.
- **Damage (CMB-01).** Per attack row whose class the target belongs
  to: `max(0, attack − armor of that class)`; melee/pierce rows use
  the armor *values*, all other classes are flat bonuses; the total
  clamps to a minimum of 1 when any row applies. Elevation factor is
  1 (flat map). Worked anchors: boar hits Scout for 7 + 3 (cavalry
  bonus) = 10; archer hits Militia for 4 − 1 (pierce armor) = 3.
- **Attack machines (CMB-04/05/06).** An Attack order (or an
  acquired target) chases; in range, the first strike lands after
  the table's attack delay, later strikes every reload ticks
  measured from the landing tick; a melee strike that finds its
  target out of range at the landing tick whiffs (reload still
  applies). Re-tasking aborts a pending strike.
- **Per-unit speeds.** Steering takes the table speed per entity
  (the old uniform speed is gone). Scale: study tiles/s × 1.25
  units/tick, anchored so the villager keeps its shipped 1.0
  units/tick pace — every unit-to-unit ratio is the study's.
  Ranges/LoS are tiles × 4 (one cell = one tile). *(Scale corrected
  in 2026-10: the × 1.25 prototype scale was ratio-only and made
  travel ≈12× too fast once M7 fixed the tile at 4 world units —
  see "Pace correction" after the M7 section. Speeds are now the
  absolute study values: tiles/s × 4 ÷ 60.)*
- **MOV-04.** A melee unit in range of its target with an attack
  engaged is exempt from separation steering, both as the pushed
  and the pusher's problem: it holds its ground mid-swing.
- **Death is a tombstone (deliberate deviation from CMB-14's
  letter).** HP is fractional; an entity dies when HP < 1 (CMB-02),
  in the death sweep at the end of the combat phase. The entity
  stays in the Vec flagged `dead` — out of play in every observable
  way (no motion, work, targeting, population, rendering), its carry
  lost, its pop slot freed the same tick — but ids are never reused
  and indices never shift, which keeps the client's index==id
  invariant and `spawn id = entities.len()` true forever.
- **Gaia animals.** Entities with `owner = Gaia`. Sheep are
  captured by proximity (any living player unit within 1 tile flips
  ownership; a majority of one — no second player exists yet) and
  then take move orders like units. Boars hold a post; damage
  aggros them (a villager's hit locks the aggro, any two hits lock
  it, otherwise they give up when the chase runs long or the target
  leaves the post's area), they chase at their faster chase speed,
  and on disengaging they walk home and heal — but only once no
  player unit is nearby, so a boar never resets mid-brawl. Deer
  flee when hurt. Wolves are out of scope (no source-verified need
  yet).
- **Carcasses (ECO-12).** A dead animal becomes a food node at its
  death position (new NodeKinds: hunt carcass — boar/deer;
  herd carcass — sheep) carrying the meat the animal held. Carcass
  nodes decay at a per-node rate (0.40 food/s hunt, 0.25 herd) from
  the moment of death. Villagers gather them at the hunt/shepherd
  rates with the node's carry capacity: **35 from a hunt carcass**,
  10 elsewhere (ECO-03). The villager who lands the killing blow
  starts harvesting unprompted.
- **Auto-acquisition (narrow M3 scope of CMB-08/09).** Idle Militia
  and Archers acquire the closest *boar* inside their LoS — never
  sheep or deer (herdables are never auto-targeted) — and idle
  military retaliate against whoever hit them. The Scout defaults
  to Stand Ground and never acquires. Full stances are M4/M5.
- **Projectiles (PRJ-01..06).** Arrows are sim entities: spawn point,
  impact point, spawn/impact ticks (flight = distance ÷ speed,
  integer), the weapon row, the intended target. Accuracy is rolled
  once per shot at release from the seeded RNG — the sim's first RNG
  consumer — in ascending attacker order (SIM-04/PRJ-03): a hit aims
  at the target's release-time position; a miss scatters uniformly.
  At impact, the intended target takes full damage if it is within
  its radius (+ slack) of the point; otherwise the first
  non-friendly unit there takes a 50% mishit. No homing, no
  re-rolls: moving targets dodge geometrically.
- **Barracks.** 175 W, 3000 ticks, **3×3 footprint** (the study's;
  placement/blocking machinery generalized from 2×2). Trains
  Militia (60 F / 20 G — the M3 brief's figure; the study's table
  lists the reference game's later-patch 50 F — recorded divergence,
  one-line change in `units.rs`) and Archer (25 W / 45 G) with the
  Town Center's queue/pop rules: enqueue needs queue room + funds;
  progress stalls while housed and resumes by itself.
- **Standard start (OPENING-SPEC §2).** 3 villagers + Scout Cavalry
  (which occupies a population slot: the start is 4/5) + fixed Gaia:
  4 sheep ≈4–5 tiles out, two pairs farther, 2 boars (≈16–20 tiles,
  split directions), a 3-deer herd. Placement table in `lib.rs`
  (`ANIMAL_SPECS`).
- **Hash.** Extended to cover node decay, building production
  queues, entity HP/owner/death/combat/animal state, and in-flight
  projectiles. Golden hash updated deliberately (see
  `tests/determinism.rs`).

## Adaptive quality governor (client, shipped 2026-10-16)

One build serves every machine: the **simulation is bit-identical for
every player; only the visuals scale**. A low-memory Chromebook and a
32 GB MacBook run — and will play — the same game; the MacBook just
gets a richer picture. The governor lives in
`web-preview/play/quality.js` (pure logic, Node-tested in
`quality.test.mjs`) with the wiring in `web-preview/app.js`. It never
touches the sim, its tick rate, or any command.

- **Probe at load.** `deviceMemory`, `hardwareConcurrency`,
  `devicePixelRatio`, screen size and touch are scored into an
  initial level, with hard caps on top of the score (≤1 GB RAM →
  LOW; ≤2 GB, ≤2 cores, or a phone-class touch screen → at most
  MEDIUM). Unknown signals (Safari/Firefox expose no
  `deviceMemory`) score mid — a wrong guess is corrected by the
  runtime monitor below.
- **Levels.** HIGH / MEDIUM / LOW control exactly three things:
  (a) the sprite **direction tier** whose sheets load — 32 / 16 / 8
  directions, fetched lazily from `assets-original/tier32/`,
  `tier16/`, or the folder root, so a client never downloads a tier
  it won't use (tier sheets are palette-optimized; measured sets:
  LOW ≈205 KB, MEDIUM ≈191 KB, HIGH ≈389 KB for the four villager
  sheets); (b) the **canvas DPR cap** — 2 / 1.5 / 1, the biggest
  fill-rate lever on weak GPUs; (c) **cosmetic draw passes** —
  shadows, ground grid strokes, path lines, projectile arc/trails,
  unit id labels — shed by LOW only. Information is never shed:
  units, HP bars on damaged units, selection rings, amount/progress
  bars and the minimap render identically at every level.
- **Runtime adaptation.** While a match is playing, a rolling FPS
  monitor judges 1.5 s windows: sustained <45 fps for ~6 s steps the
  level down one notch (announced once via a subtle toast);
  sustained ≥58 fps for ~12 s may step it back up — never above the
  probe's initial level. A 15 s cooldown after every change, plus
  the asymmetric thresholds (hysteresis), keep the level from
  oscillating.
- **Manual override.** Pause menu: Auto / High / Medium / Low,
  persisted in `localStorage` (`skirmish2.quality`); the current
  level shows in the top bar. `?quality=low|medium|high|auto` on the
  URL wins for the session without persisting — a support tool.
- **Interaction with the frame loop.** The loop's max-ticks guard
  already bounds sim catch-up per rendered frame (5 ticks, backlog
  dropped): on a slow machine the game renders fewer frames but the
  simulation rules never change. The governor shrinks the cost of
  each rendered frame so the guard rarely has to bite; it adds no
  timing behavior of its own.

## M5 — scripted AI opponent + Conquest (shipped 2026-10-16)

The sandbox became a match. The sim now carries **two player seats**:
stockpiles, population/pop-cap, building ownership, production queues
and deposits are all per-player (owner 0 = human, owner 2 = opponent,
owner 1 = Gaia, unchanged). Player 0's sandbox start is byte-identical
to before; the per-player plumbing is covered by the state hash, as is
the match mode and the AI's wave timer.

- **The opponent** (`sim/src/ai.rs`) is a scripted player *inside* the
  sim that plays only through the same `Command` queue the human uses
  — Train, Gather, Build, Attack, rally — on a fixed decision cadence.
  It reads live sim state only (no RNG calls at all in v1), so an AI
  match replays bit-identically like any other. Its script: villagers
  to an economy target with deficit-balanced resource assignment
  (food first, sheep hunted when its own food runs out), houses ahead
  of the pop cap, one Barracks, alternating Militia/Archer production,
  a rally staged toward the enemy, attack waves at an army threshold,
  and a small home-defense reaction. **Easy/Normal** are parameter
  profiles (cadence, targets, wave size/cooldown) — the sim has no
  difficulty concept of its own, only the match mode tag. **Sandbox**
  spawns no opponent and runs no AI: the pre-M5 game, preserved.
- **Buildings became combat state.** They carry owner + HP (study §5
  values), take ordinary formula damage (archers pierce their high
  pierce armor poorly; melee grinds), and can be ordered attacked
  (`AttackBuilding`) as well as auto-acquired never — a Town Center is
  only ever attacked on an explicit order or as a wave's fallback.
  Destruction frees the footprint, zeroes farm nodes, and retargets
  or idles everyone who was working on it.
- **Conquest (VIC-01).** A player with no buildings (under
  construction counts) and no population-counting units is
  eliminated; the AI also resigns when it has no TC and no villagers
  (AI-05). `match_state()` reports ongoing / victory / defeat from
  the human's seat; human elimination is checked first, so a
  same-tick double elimination is a defeat. The state is a pure
  function of hashed state, and the sim keeps ticking harmlessly
  after the decision.
- **Client.** The start overlay chooses Sandbox / AI Easy / AI Normal
  (default Normal); the enemy renders in red (minimap dots, TC
  pennant, unit cloth, a palette-swapped red villager sheet generated
  client-side from the same sprite pipeline output), the enemy TC
  and buildings are selectable for information only, and a
  victory/defeat overlay closes the match with Restart (same mode)
  and Back to menu. The HUD still shows player 0 only.

## M6 — ages, technologies & the economy buildings (shipped 2026-10-16)

The first age rung and the technology substrate. Each player seat now
also carries an **age** (Dark → Feudal) and a **researched-technology
bitset** (`players: [PlayerState; 2]`), both hashed.

- **The tech substrate** (`sim/src/tech.rs`). Technologies are a data
  table — id, host building, required age, cost, research ticks —
  and their effects are *patches folded into stat lookups*, never
  code branches in consumers: `Sim::stats_for(owner, kind)` returns
  the base row plus the owner's age layer (the Scout Cavalry's
  Feudal jump, CLS-04) plus the owner's researched patches (Loom's
  villager +15 HP / +1/+2 armor / +0.1 speed; Fletching's archer
  +1 attack & +1 range; Scale Mail and Padded Archer Armor's +1/+1
  armor). Gaia always reads base rows. Combat, movement speed, spawn
  HP and the ABI's HP readout all consume the effective row, so a
  patch lands everywhere at once. Man-at-Arms is the one *sweep*:
  on completion the owner's Militia change kind in id order (HP
  kept, max rises — CLS-03) and future production spawns the tier.
- **Queues generalized.** Both production queue types now hold
  `ProdEntry` — `Unit(kind)` or `Tech(id)` — in one FIFO per
  building, TC included: Feudal Age and Loom research *in the Town
  Center's queue*, behind or ahead of villagers exactly as ordered.
  Costs are charged at enqueue; progress accrues on the head only;
  a Unit head stalls while its owner is housed, a Tech head never
  does. There is still no cancel, so TEC-05's refund half stays
  open. Research block reasons mirror the training pattern
  (Ok / NoFunds / QueueFull / AlreadyDone / Locked).
- **Drop-off routing.** The Returning phase re-resolves the nearest
  valid drop-off *for the carried resource* every tick — Town Center
  (everything), Lumber Camp (wood), Mill (food), Mining Camp
  (gold + stone) — so a camp completing mid-haul reroutes the trip.
  Arrival at a camp is measured to its footprint edge; hauls steer
  to a point pushed just outside the footprint, because the camp's
  center cell is inside its own blocked footprint and a path *to the
  center* does not exist (the TC never had this problem — it never
  marks the grid). `ForceDeposit` flips a loaded villager to
  Returning with its gather/construction assignment kept — the kept
  assignment *is* the resume marker. A builder finishing a camp
  deposits a carried load there on the spot when the camp accepts
  it (OPENING-SPEC Bucket B).
- **Gates live in `validate_build`**, so the command path, the ABI
  verdict and the client's ghost all agree: Farms require a
  completed Mill; the Feudal buildings require the Feudal Age; the
  Archery Range and Stable additionally require a completed
  Barracks. Production moved with the gates: Archers train at the
  Range, Scout Cavalry at the Stable, the Barracks keeps Militia.
- **The AI learned the layer lightly**: a Mill by its berries and a
  Lumber Camp by its woodline (its Feudal pair), Loom once its
  economy rolls, the age-up when the food allows, then an Archery
  Range for its archers. Still only through the Command queue.
- **Perf fix found by the milestone's own replay test.** An AI match
  past ~30k ticks fell off a cliff (a 10k-tick chunk went from ~6 s
  to ~138 s in debug): any unit whose destination became
  unreachable — typically a gatherer whose node a new building had
  enclosed — re-ran a failing full-grid A* *every tick*, forever.
  `set_move_order` now reports failure, and the per-tick callers
  (gather, haul, build-approach, combat chase, building approach,
  boar homecoming) take a terminal transition instead — idle, or
  the order completes — matching what the player would conclude:
  the target cannot be reached. Reachability only changes when
  buildings complete or fall, and those events re-task the affected
  units through other paths. (The same audit caught Man-at-Arms
  missing from the auto-acquisition set; it is included now.)
  The fix exposed a second latent issue the retry loop had been
  masking: the AI's own buildings can plug the map's wall gap, and
  its wave targeted the nearest human units — two of which stand
  in that gap — whether or not a path to them exists. Wave target
  selection now walks candidates nearest-first and takes the first
  *reachable* one (Town Center and building fallbacks unchanged).
  Defense targeting got the same reachability filter for the
  mirror-image reason: an unreachable intruder at the edge of the
  defense radius (the human's forward Scout, drifted inside it)
  re-absorbed every defender on every decision, and the `acted`
  bookkeeping then starved the wave step entirely — on Easy the
  AI never attacked at all. The same entombment could strand a
  sheep inside a completed building's footprint, beyond hunting;
  placement now refuses to cover a standing unit (farms exempt),
  and completion displaces anything that moved onto the footprint
  while the building rose to the nearest open cell. A per-tick
  grid-integrity pass closes the last door: separation alone can
  push a unit into a blocked cell (one Scout ended a match
  entombed in the wall column itself), so no living unit ends a
  tick inside one — it is displaced the same way. The golden
  scenario never triggers any of this; its hash is unchanged.
  Three AI lessons shipped with the same work: the build step
  may pull a villager off gathering (in the deposit → resume
  steady state nobody is ever idle at a decision boundary, and
  construction starved); the anchor search now looks 512
  candidates deep (a grown base crowds the inner rings); and
  military production pauses while the script saves for the
  Feudal Age once a minimal guard stands — otherwise the army's
  food draw keeps the stockpile under the age's cost forever.
- **Hash changes (deliberate).** Per-player age + tech bitset; queue
  entries hash as typed codes (unit = kind tag, tech = 100 + tech
  tag); each entity's current drop-off target (`task_dropoff`) is
  hashed. The golden hash moved to `09e22978bb8876db` with M6.
- **Recorded simplifications.** Building HP/armor do not step up
  with age yet (BLD-11); "nearest drop-off" is straight-line
  distance, not path distance; under-construction buildings carry
  full HP (BLD-05, from M5).

## M7 — the world: random maps, terrain, fog (shipped 2026-10-17)

- **One scale, faithfully translated.** The rulebook's 2-player map
  (Tiny, 120×120 tiles) is the world: 1 sim cell = 1 tile = 4 world
  units, so the world spans [-240, 240) and `GRID_DIM` is 120. Every
  client dimension comes from the ABI (`world_half`, `grid_width`,
  `cell_size`), so nothing downstream hardcodes the size.
- **The generator (`sim/src/mapgen.rs`).** A deterministic Arabia
  generator runs inside match creation from the match seed, on its
  own derived sub-seed stream (the in-state RNG's draw sequence is
  untouched by however many candidates generation tried). Start B's
  package is start A's rotated 180° through the map center — equal
  counts *and* mirrored distances, exact fairness by construction.
  Heights are blurred integer noise, symmetrized; ponds are mirrored
  pairs, clear of starts; starts and package nodes are flattened to
  the start's level so openings play identically. Generation
  **validates** (land flood-fill: starts connected, every package
  cell on the shared landmass, no water near a start, no cliff-like
  height breaks) and regenerates with the next sub-seed on failure —
  bounded at 64 attempts, then a fixed template that validates by
  construction. Never a hang; same seed ⇒ byte-identical map.
- **Terrain & elevation are grid layers.** `Grid` carries `terrain`
  (land/water), `height` (0–4), `node_block` (standing trees,
  MAP-04) and `dynamic` (buildings) as byte layers; `is_blocked` is
  their union for the land domain. The old compile-time wall layout
  is deleted. Water is *terrain support only*: impassable to land
  units in pathing and placement, rendered, shoreline-validated —
  ships/docks/fishing are trimmed (rulebook MAP-08 stays DIFFERS).
  Elevation feeds exactly one gameplay rule: CMB-01's binary factor
  (×1.25 / ×0.75 / ×1 on the armor-subtracted sum, clamped after), with a
  projectile carrying its shooter's elevation from release (PRJ-10).
  No movement effect — AoE2 elevation does not slow movement.
- **Fog of war is sim state (VIS-01, with the deviation recorded in
  the rulebook).** Per player slot, a byte grid (0 unexplored /
  1 explored / 2 visible), hash-covered, rebuilt from scratch every
  15 ticks and at creation: demote visible→explored, then stamp
  integer sight discs (pure radius — VIS-05, no occlusion) from
  every living unit (effective-table LoS, capped at 20 tiles,
  VIS-04) and standing building. Cost is bounded by sources × disc
  area per interval, independent of map area — no per-tick scans.
  The ABI serves the local player's grid; the client renders
  strictly from it (shroud black, explored dimmed with last-known
  enemy buildings, units/projectiles only where visible — minimap
  included, VIS-10). Fog never feeds back into truth (VIS-09): it
  gates knowledge consumers only.
- **The AI plays under its own fog.** Its combat targeting picks
  only currently-visible units and explored buildings; its Scout
  rides to the expected enemy start (the mirror of its own TC —
  the one fact Arabia's symmetry gives every player) before the
  first wave can aim; its economy assigns nodes by Town-Center
  proximity instead of node-id blocks, so it works any generated
  map. Explicit Attack orders persist through fog for both players
  equally — the same privilege as a human's click. One behavior
  the generated maps forced into the open: the AI must *capture*
  sheep, not just hunt owned ones — on the arena its work routes
  crossed the herds by luck, but on a real map the berries can run
  out first, and an AI with full wood and gold starved at the
  Feudal gate. Its food fallback now sends a hunter to the
  nearest reachable Gaia sheep (the approach captures it), the
  one click a player makes without thinking.
- **UNVERIFIED values adopted this milestone** (flagged in code and
  rulebook VIS-03): building sight radii — Town Center 6 tiles,
  other buildings 4. The unit LoS column is the study's; Town
  Watch/Patrol are not implemented.
- **Hash changes (deliberate).** The four grid layers and both fog
  grids are hashed (chunk-fed — the FNV writer takes 8 bytes per
  multiply so the 120×120 layers cost like the old 50×50 byte
  loop); projectiles hash their release elevation. The golden hash
  moved with M7 (see `tests/determinism.rs` for the trail).
- **Scale traps the long matches caught (recorded so they stay
  caught).** All sim distance comparisons go through
  `steer::vec_len` (i64 bit math): a squared fixed-point distance
  overflows I16F16 beyond ~181 units, which at arena scale never
  happened and at map scale silently corrupts "nearest" queries.
  Deposit steering falls back to the building's proven approach
  point when the geometric steer point cannot be pathed to — a
  camp hugging its woodline (the normal M7 placement) can have
  the push-out point land inside the trees, and a hauler that
  simply stops with a full load is the failure mode. A third trap
  is path *traversal*, not path *finding*: a felled tree's freed
  cell is ringed by trunks, so A* admits a diagonal step past
  trees (water and buildings stay hard corners) — but a unit
  steering continuously between waypoints cuts that corner by a
  hair, clips the flanking trunk, and the integrity teleport
  plus task re-issue looped forever at the gap's mouth.
  Movement therefore slides axis-separated against tree cells
  (the standard grid mover); the M6 teleport still guards every
  other blocked layer. All three were found by the M7
  long-match tests, not by inspection.

## Pace correction (2026-10-18, post-M7 fix)

- **Root cause.** The unit table kept the M1/M2 prototype pace:
  speeds were the study's tiles/second values scaled ×1.25 off a
  villager anchored at 1.0 world-units/tick, preserving only the
  unit-to-unit RATIOS; the units.rs header said the absolute
  tile-scale pass was "a later tuning pass". It never happened,
  and M7 then fixed 1 tile = 4 world units — so travel ran at
  prototype speed on a real-scale map. Measured just before the
  fix (seed 42, villager ordered 10 tiles): arrival in 61 ticks
  ≈ 1.02 s (≈9.8 tiles/s effective; cruise step exactly 1.0
  units/tick = 15 tiles/s) against the reference 0.8 tiles/s —
  travel ≈12× too fast. The game clock, train times, gather
  rates and rates of fire were always correct game-time values;
  only the movement scale was wrong.
- **The fix.** Every movement speed is now the absolute study
  value converted at the tile scale: `units/tick = tiles/s × 4
  world-units-per-tile ÷ 60 ticks/s` (I16F16 bits =
  round(tiles/s × 65536 / 15); per-row rounding ≤ 0.011%). Rows:
  Villager 0.8, Scout 1.20 (Feudal 1.55 via `stats_for`), Militia
  0.90, Archer 0.96, Man-at-Arms 0.96, Sheep 0.7, Boar 0.8 roaming
  / 1.0 chasing, Deer 1.0 (still UNVERIFIED — kept at its
  documented ratio to the anchor). Loom's +0.1 tiles/s (TEC-06)
  converts in the same formula (0.8 → 0.90 exactly in bits).
  Projectile speed (0.5 units/tick) is an independent visual
  constant and deliberately unchanged. Regression-pinned by
  `sim/tests/pace.rs`: sustained cruise within 1.5% of the table
  value for Villager/Scout/Militia, and a 10-tile villager leg in
  ≈12.5 s of game time (measured 760 ticks ≈ 12.7 s).
- **Consequence the correction exposed (fixed with it).**
  Separation is a per-tick *displacement* calibrated when a
  villager's whole speed was 1.0 units/tick; at 0.053 units/tick
  two neighbors flanking a unit's heading sum to a push that can
  cancel its order exactly — a stable fixed point (three
  villagers wedged in a seed-9 forest pocket froze bit-identical
  for 300k+ ticks; the scripted-siege match stalled the same
  way). Steering now caps the separation term at half the unit's
  own speed, so an order always keeps at least half its
  authority; the orderless shove is uncapped (a standing unit
  must stay displaceable at up to its full speed — the M2
  anti-wall rule). Also exposed, in tests only: long build-ups
  that re-issued a Gather order against a by-then-depleted node
  (silently ignored by design) had to re-bind a live node — at
  the prototype pace the re-gather always came while the first
  node still stood.
- **Hashes.** The golden hash (`tests/determinism.rs`) does NOT
  move: its scenario issues no commands and nothing moves in 360
  ticks, so speeds never enter it. Every live-match replay hash
  changes, as expected — movement feeds everything — and those
  are pinned by their own bit-identity tests.

## M8 — lockstep multiplayer (shipped 2026-10-18)

Networking changed no simulation behavior. The sim gained exactly
three additive things (flagged in the milestone report): a fourth
match mode, one command form, and per-seat wrappers over existing
helpers. Everything else is client code (`web-preview/play/netplay.js`,
pure and Node-tested; `app.js` integrates it) plus a deliberately
dumb relay.

- **Model.** Bettner & Terrano lockstep, exactly what the sim was
  built for: clients exchange ONLY commands. Both sides construct
  the identical sim from a shared seed — `MatchMode::TwoPlayer`
  (tag 3): the mirrored start spawns and Conquest applies, but no
  script drives the second seat; every command for both players
  arrives from outside. (Seed, mode, command stream) is the match.
- **Turn structure.** 1 turn = 3 ticks (50 ms), 20 turns/s. Each
  client assembles one bundle per turn; a turn executes only when
  BOTH bundles for it have arrived (an empty bundle is a
  heartbeat), then both bundles apply in slot order (slot 0 first,
  each in issue order) and the sim ticks 3 times. If a bundle is
  late, execution waits — the slowest machine sets the pace
  (NET-03); sim time is never dropped.
- **Input delay.** Commands are stamped for the turn `delay` turns
  in the future — 4 turns (200 ms, the reference turnaround) at
  match start; the first `delay` turns are empty by construction on
  both sides. The delay adapts UP only: when more than ~35% of the
  last ~200 turns executed late, the host proposes +1 (cap 10) and
  both sides apply it to their sending schedules. It never
  decreases mid-match.
- **Hash cadence & desync.** After the last tick of every turn,
  each side sends its state hash. A mismatch stops both clients on
  the spot: the sim holds, an overlay names the turn, and the
  recorded replay is offered for download. A desync is never
  "repaired" — it is a bug report, not a state to patch over.
- **Commands on the wire.** A small semantic vocabulary (move,
  gather, attack, attack-building, build, train, research, rally,
  force-deposit) with JSON numbers for coordinates — JSON's
  shortest-round-trip formatting reproduces the exact same double
  on the far side, and the sim's fixed-point conversion is
  deterministic. Town Center orders carry no Town Center id: the
  applier resolves the ISSUER's Town Center from its own sim,
  identically on both clients. Acceptance at execution is the
  sim's own re-validation, which agrees on both sides by
  construction. Identical duplicates within one bundle collapse
  (NET-07), bundle size is capped, and every inbound message is
  structurally validated before it can reach the sim.
- **Handshake.** Over the room socket: `hello` both ways (presence
  is client-side; the relay knows nothing), the host sends `start`
  with a seed and its construction `state_hash()`; the guest
  builds the same sim and compares — a mismatch aborts with a
  version error before a single tick (build compatibility is
  checked by construction hash, not by a version string). Guest
  acks, host sends `go`, both anchor a local pacing clock. Roles
  come from how you entered: the link creator is slot 0
  (owner 0), the link opener is slot 1 (owner 2). A third opener
  is told the room is full.
- **Lobby & regions.** The region list is fixed and identical for
  everyone (decision 2026-10-18): `wnam`, `enam`, `sam`, `weur`,
  `eeur`, `apac`, `oc`. Ping is measured honestly: for each region
  the client opens a fresh probe room — a Durable Object PLACED
  under that region's location hint — and times an echo round
  trip, reporting the second (warm) one. The nearest region is
  pre-highlighted; the pick stays the player's, so friends choose
  the same location. The match room itself is created under the
  chosen region's hint, so match traffic lands in the picked
  region too.
- **Open-games directory (M9, heartbeat + locks in M10).** The play
  page opens on a Lobby Browser — the list of games waiting for a
  second player — so the relay gained exactly one directory: a
  singleton `Lobby` Durable Object. Each match room reports its
  own socket count to it on every open/close (`{room, region,
  players, locked, ts}`; the region is the location hint the room
  was created under, persisted in the room's storage on its first
  request). `GET /lobbies` serves
  `{open: [{room, region, players: 1, createdAt, locked}],
  inProgress}` and prunes entries with no report for 120 s. Probe
  rooms never register. The directory stores nothing else — room
  ids are the random ids hosts already share publicly by link;
  there are no accounts, names, IPs or per-player records, and
  nothing about a match that has started beyond its count in
  `inProgress`. The client polls it every ~5 s while the browser
  is open and renders the traditional table (auto title "1v1
  Arabia — <region>", players 1/2, the player's measured ping to
  the region); joining a row is the same guest flow as opening
  its link. M10 fixed the directory's one live-found flaw: a room
  only reported on open/close, so a host simply WAITING aged out
  of the list after 120 s. A waiting room now re-reports itself
  on a Durable Object alarm every ~45 s for as long as it holds
  at least one socket — comfortably inside the prune window —
  and the chain stops when the room empties.
- **Room model (M10).** Rooms are created **Public** (listed,
  anyone may join) or **Private**, following the reference game's
  browser: a private room is NOT hidden — it lists with
  `locked: true` and a 🔒 in the table — but joining one, from
  the list or by its invite link, requires the password the host
  set at create time. The host's create connection carries
  `?private=1`; the room persists only that flag (first sight
  wins) and reports it with its count. **The password itself
  never reaches the relay as data**: it lives only in the host
  client's memory for the session, and the check runs over the
  room pipe — the guest sends `{type:'pw'}`, the host's client
  compares and answers `pw-ok` / `pw-no` (asking first with
  `pw-req` when a guest arrives). Until `pw-ok`, the guest does
  not count as present on either client: no slot presence, no
  ready, no start. This is honestly a **courtesy lock** — it
  keeps honest players honest and mirrors the reference UX; it
  is not account-grade security (there are no accounts), and a
  modified client could bypass the gate-keeping on its own side
  — it cannot mint a match the host's client refuses to start.
  The room screen itself is the calm DE shape: the two seats
  (Host — Blue / Guest — Red) with presence and ready states,
  ONE prominent **Start** button per player that toggles their
  ready flag (`{type:'ready'}` over the pipe), and the invite
  link demoted to one quiet line under the seats. **Start rule:**
  the host's client initiates the M8 start handshake by itself
  the moment BOTH players are ready; un-readying before that
  keeps the gate closed, and a guest leaving resets both flags.
  Ready/password traffic is lobby traffic — it never enters the
  sim or the hashed state (NET-10).
- **Loading phase & load order (M10).** The heavy game loads
  AFTER commitment, never before: `/play` opens as a light lobby
  (browser, region pings — WebSocket probes only — room
  creation/joining all work with the .wasm never requested and
  no sprite tier fetched; the sim module is a dynamic `import()`
  behind `ensureGameLoaded()`). When the start rule fires
  (online) or the player taps Play solo, both clients enter a
  dedicated **loading screen** where the real load happens —
  WASM fetch + init, artwork, world construction from the seed,
  construction-hash verification — with each seat's state shown
  ("Blue — loaded ✓ / Red — loading…"). The loaded signal IS the
  M8 handshake, extended not replaced: the guest's `start-ack`
  now means "built and verified", the host marks itself loaded
  when its own build verifies, and the host sends `go` only
  after BOTH loaded signals are in. **Cancellation:** a
  disconnect during loading, an explicit `{type:'cancel'}` from
  a client whose own load failed, or the loaded signal not
  arriving within **90 s** (`LOAD_TIMEOUT_MS` — the same
  patience as the rejoin window, for slow mobile networks)
  cancels the match for everyone: every remaining client shows
  "Match cancelled — a player left or failed to load." and
  returns cleanly to the Lobby Browser — no dead screen, no
  desync overlay, no half-started sim (the pending sim is simply
  dropped). Solo passes through the same screen for exactly the
  real local load — no artificial delay. The loading screen
  carries the play page's second waiting-view ad slot (lobby
  sidebar is the first; see `web-preview/ADS.md`); both rotate
  fresh house creatives every 60 s while the player waits,
  pausing on a hidden tab and stopping the moment play begins.
- **Disconnect & rejoin.** A dropped socket pauses the remaining
  side ("Opponent disconnected") with a 90-second grace window.
  Rejoin is by the same link: the remaining peer transfers the
  setup plus the FULL recorded command stream; the rejoiner
  rebuilds the sim from the seed, fast-forwards, and both sides
  rebase to a common frontier turn and resume. Unexecuted bundles
  from the dropped window are discarded on both sides — orders
  issued into them are lost, the honest cost of a drop. After the
  grace window the remaining player may claim victory.
- **Replay.** Every match — online or local — records (seed,
  mode, per-turn command bundles) client-side in one format. The
  end overlay offers a Download replay; the play page's Watch
  replay re-executes it deterministically (play/pause, 0.5×–4×
  speed), rendering fog from the recording seat's point of view.
- **Telemetry.** At match end each client POSTs one small summary
  (region, duration ticks, result, desync count, build version).
  The relay stores ONLY running counters in a dedicated Durable
  Object — matches, ticks, desyncs, per-region / per-result /
  per-build counts. No per-match records; no IP is read; no
  identifier exists. It feeds fidelity and quality work
  (SUSTAINABILITY.md §5), never mechanics changes.
- **The relay deliberately does NOT** run game logic, see game
  state (it forwards opaque bytes), track people (the M9 directory
  counts sockets per room — it knows rooms, never who is in them),
  store anything per-match, or authenticate anyone — the link is
  the capability. Its only extras over the dumb pipe are the probe
  echo, the telemetry counters and the room directory. Relay
  source lives outside this repository while the server-license
  decision is open.
- **Sim/ABI additions (purely additive).** `MatchMode::TwoPlayer`
  (tag 3; the AI step's gate now names the AI modes explicitly so
  the script can never drive a networked seat);
  `Command::SetRallyTc` — the per-Town-Center rally form, the one
  command lockstep application could not express for the second
  seat (`SetRally { building: None }` names the first seat's Town
  Center implicitly); public per-TC/per-player wrappers
  (`train_block_at`, `can_train_villager_at`,
  `research_block_at_tc`, `tc_queue_of`, `tc_progress_bits_of`,
  `tc_rally_of`, `idle_villagers_of`, `idle_military_of`) and the
  matching WASM ABI (`push_train_villager_at`,
  `push_research_tc_at`, `push_set_rally_tc_of`, the `_for`/`_at`
  getters). No existing function changed behavior; the golden hash
  did not move; four new tests in `sim/tests/two_player.rs` pin
  the mode's contract (mirror start; no script over 36,000 ticks;
  per-seat play; bit-identical interleaved streams).

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
