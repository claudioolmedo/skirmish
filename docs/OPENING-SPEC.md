# Skirmish II — Opening Spec: The Zero-Learning Standard Start

*Written 2026-10-16. Companion to [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) (numbers),
[`GAP-ANALYSIS.md`](GAP-ANALYSIS.md) (full parity inventory — items are cited here as
"GAP n.m"), and [`OPENAGE-RESEARCH.md`](OPENAGE-RESEARCH.md) (engine lessons).*

**The acceptance criterion.** the project's rule for the whole project: an
Age of Empires II DE player who opens https://skirmish.online/play
must need **0% learning** — every mechanic their hands already know
has to behave the way their hands expect. Nowhere is that promise
easier to break than in the first minutes of a match, because the
AoE2 opening is a fixed ritual: the first ~15 villagers do roughly
the same things in the same order on every land map, for reasons a
player's muscle memory encodes even when their conscious mind
doesn't. This document specifies that ritual exactly, audits
Skirmish II against it line by line, and sequences the work to close
the gap.

**Scope.** One generic empire (no civilization exceptions — the
transcript's Mayan/Chinese/Hunnic start variants belong to the
empire system, GAP 13.1), standard settings, an Arabia-style land
map, 1 player vs the map (the opponent arrives in M5, GAP 9.1). The
window is **t=0 to the Feudal Age click** — the opening's finish
line.

**Primary source.** The Spirit of the Law video *"Beginner guide to
the first 15 villagers"* (public video transcript), deepened
and cross-checked against public documentation only: the Age of
Empires fandom wiki (Arabia, Wild Boar, Sheep, Deer, Herdable
animal, Hunt, Early Economy, Lumber Camp, Mill, Building comparative
table) and [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md), which had already
verified the economy/combat numbers against the same sources. No
game binary, no copied assets, no copied text.

---

## 0. Conventions

- **Game time** is the in-game clock: 60 ticks = 1 game second, a
  villager trains in 25 game seconds. All times below are game
  time, assuming the Town Center never idles.
- **Villager numbers** (#4, #5, …) count *living* villagers: the
  three starters are #1–#3, the first trained villager is #4.
  "Pop" counts living + queued, as both AoE2 and Skirmish II do.
- **Tiles** are the AoE2 distance unit. Skirmish II's current world is
  200×200 world units on a 50×50 grid of 4-unit cells
  (`sim/src/grid.rs`: `CELL_SIZE`, `GRID_DIM`), with villager speed
  1.0 unit/tick = 60 units/s (`sim/src/steer.rs`: `MAX_SPEED`). No
  tile mapping has been chosen yet; this spec is written in tiles,
  and §3 flags the scale relationship as its own audit line. The
  natural mapping when the start generator lands is 1 tile = 1 cell,
  with map sizes parameterized per GAP 7.2.
- Status legend (as in GAP-ANALYSIS): **HAVE** / **PARTIAL** /
  **MISSING**, each with the file evidence.

---

## 1. The canonical opening timeline (t=0 → Feudal click)

The generic build, per the Spirit of the Law walkthrough. Times are
the no-idle-TC ideal; real games slip a few seconds and recover —
the *order* is the invariant, not the second hand.

| Game time | Pop | Player action | Mechanic it depends on |
|---|---|---|---|
| 0:00 | 3/5 | Queue villager #4 at the Town Center (50 food, paid on enqueue); keep queueing as food allows | TC production queue (15), cost-on-enqueue |
| 0:00 | 3/5 | Villagers #1–#2 place **House A** together; villager #3 places **House B** alone | Build menu; a second villager can join a standing foundation |
| 0:00 | 3/5 | The **Scout Cavalry** rides out to find the sheep | Starting scout (Stand Ground stance default); the starting area is otherwise unseen |
| ~0:05–0:20 | 3/5 | First 4 **sheep** found → they convert on approach, are tasked to wait under the TC, one is killed and eaten | Herdable conversion; sheep move only when tasked; kill one at a time (decay) |
| 0:19 | 3/5 | **House A completes** (2 builders ≈ 19 s) — population room secured before the first deadline | Multi-builder speed formula 3/(n+2) |
| 0:25 | 4/5 | **House B completes** (1 builder, 25 s); villager #4 emerges and is sent to sheep; queue #5 | Housing math: queued villagers stall at the cap, never the reverse |
| 0:25–1:15 | →6/10 | Starters leave the finished houses → sheep; #5 and #6 join them → **6 shepherds**; remaining sheep **shift-queued** in eating order | Shepherd rate 0.330/s; carry 10; queued (shift) orders |
| ~1:40 | 7 | Villager #7 walks to the woodline and builds a **Lumber Camp** (100 wood), then chops | Camp = buildable drop-off building; lumberjack rate 0.388/s |
| ~2:05–2:55 | 8–10 | Villagers #8–#10 join wood → **3–4 lumberjacks** total (4 is the standard) | Drop-off at the camp, not the distant TC |
| ~3:20 | 11 | Villager #11 **lures boar #1**: shoots it, runs to the TC; the boar chases (faster than the villager, hits hard); the 6 shepherds + lurer kill it under the TC → **7 villagers on boar** | Boar aggression/lure/return behavior; villager attack; hunt rate 0.408/s; hunter carry 35; carcass decay 0.4/s |
| ~3:45 | 12 | Villager #12 builds **House #3**, then a **Mill** at the berry patch (100 wood) | Mill = food drop-off, gates Farms, and is Dark Age building #2 for the Feudal requirement |
| ~4:10–5:35 | 13–16 | Villagers #13–#16 → **berries at the Mill** (4–5 foragers) | Forager rate 0.310/s; patch holds 750 food |
| throughout | — | **Force drop-offs** whenever the queue or the 500-food bank would otherwise stall | Task a carrying villager to a drop-off building → it deposits and returns to its previous task |
| boar #1 ≈150 food left | ~15–17 | **Lure boar #2** (a fresh villager, or a rotated full-health hunter) | Second boar; same lure mechanics |
| ~5:35–7:30 | 17–21 | Rebalance: extra hands on food for a fast advance (or wood for a Feudal military plan); TC never idles; deer can be pushed/hunted opportunistically | Deer flee/push behavior; hunt economics |
| before the click | 21–22 | Research **Loom** at the TC (50 gold, 25 s) | TC research; villager +15 HP, +1/+2 armor |
| **~7:30–8:30** | **22–23** | **Click Feudal Age** (500 food, 130 s; requires 2 Dark Age buildings — the Lumber Camp + Mill) | Age advance — the opening's goal state |

Why the shape holds (the one-line economics the transcript
teaches): constant villager production burns 120 food/min; a
shepherd nets a little over 19 food/min after walking, so **six**
shepherds almost — not quite — break even, which is why the boar's
faster hunt rate (0.408 vs 0.330) and the berry patch's 750 free
food exist in the build, and why force drop-off is a pressure valve,
not a habit.

---

## 2. The standard-start spec

### 2.1 Starting position

| Item | Spec |
|---|---|
| Town Center | 1, pre-placed; trains villagers, accepts **all four** resources as drop-off, supports **+5** population; 4×4 tiles |
| Villagers | **3**, standing idle beside the TC. *The player's clicks are the first orders of the match — nothing is pre-tasked.* |
| Scout | **1 Scout Cavalry** beside the TC (see §2.8) |
| Stockpile | **200 food, 200 wood, 100 gold, 200 stone** |
| Population | 3 living of a 5 cap; lobby cap 200 (setting, GAP 8.4/8.8) |
| Camera | Centered on the TC; in the reference the surrounding map starts unseen (§2.3, fog — GAP 7.6/7.9) |

Civilization exceptions to the 3-villager start exist (Mayans +1,
Chinese 6 villagers/0 food, Huns need no houses) — explicitly out
of scope until empires exist (GAP 13.1). The generic start above is
the one to build.

### 2.2 Animals

All animals are Gaia (neutral) entities. None of them are resource
*nodes*: they are units with hit points that become a **carcass
node** when killed, and a carcass loses food to **decay from the
moment of death, including while it is being gathered** — which is
why animals are killed one at a time, under the drop-off point.

**Sheep (herdable)** — GAP 3.7.

| Trait | Spec |
|---|---|
| Count | **8** per player: 4 beside the start, 2 + 2 in two pairs a short distance beyond the TC's sight |
| Food | 100 each (800 total) |
| Capture | Converts to the first player whose unit comes **adjacent** to it; stays that player's while their units are near — if left unguarded, another player's unit passing by takes it (if several players' units are near, the majority holds it) |
| Movement | **Stands still until tasked**; once owned it takes normal move orders at speed **0.7** (slower than a villager's 0.8); LoS ~3 |
| Harvest | Killed (by villagers — a formality, it does not fight), then gathered at the **shepherd rate 0.330/s**, carry 10 |
| Decay | Carcass **0.25 food/s** |

Sheep-handling the spec must support: walk each found sheep to
under the TC *before* killing it; keep the un-killed sheep **alive**
(they do not decay alive); villagers auto-move to the next queued
sheep when the current one is exhausted if the player shift-queued
them (§2.8).

**Boar (aggressive huntable)** — GAP 3.6.

| Trait | Spec |
|---|---|
| Count | **2** per player, in different directions, farther out than the sheep |
| Food | **340** each (note: the fandom Boar article's prose currently reads 300 — the Arabia resource table and `MECHANICS-STUDY §6.3` both say 340; re-pin at implementation) |
| Combat | **75 HP, 7 melee attack** (+3 vs cavalry, +4 vs shock infantry, +8 vs camels), ~2.0 s strike cycle |
| Speed | **1.0 when attacking** (faster than a villager's 0.8), 0.8 roaming |
| Aggression | Passive until attacked. On being attacked it **charges the attacker** and keeps striking them as it catches up. A villager lure = hit it (the official tutorial has the villager shoot **twice** to lock the aggro), then run for the TC |
| De-aggro | If the attacker garrisons, dies, or breaks sight contact — military lurers must hit it **twice** or it gives up after a short distance — the boar **walks back to its post** and can be re-lured |
| Kill math | An un-Loomed villager (25 HP) dies in **4 hits**; with Loom (40 HP, +1 melee armor → 6/hit) in **7 hits**. The lurer must survive to the TC, where **≥4 waiting villagers** plus the lurer kill the boar (75 HP) |
| Harvest | Hunters gather at **0.408/s** with **carry 35** (the one carry exception) |
| Decay | Carcass **0.4 food/s** — eat it promptly, with everyone |

**Deer (docile huntable)** — GAP 3.6.

| Trait | Spec |
|---|---|
| Count | A near herd of **2–4** plus a second herd of **3** farther out |
| Food | **140** each |
| Behavior | Graze in place; **flee a short distance when attacked** (fragile — two villager shots kill); because they flee *away* from an approaching unit, a scout can **push** a herd toward the TC/Mill by walking behind it |
| Harvest | Hunt rate 0.408/s, carry 35; optimal crew 3–4 villagers per carcass |
| Decay | Carcass **0.25 food/s** |

Wolves and other predators exist on Arabia (GAP 7.8) but are not
part of the first-15 ritual; they are noted here only so the animal
system doesn't design them out.

### 2.3 Map features — Arabia-style start (per player)

One TC start on open ground; the near resources in a loose ring;
nothing walled off. Distances are typical generation bands, not
constants — the generator deals a fair variation every match
(GAP 7.1).

| Feature | Count | Placement band (tiles from TC) |
|---|---|---|
| Sheep, near group | 4 | Beside the TC (≈4–8) |
| Sheep, far groups | 2 + 2 | Just beyond TC sight (≈10–16) |
| Berry patch | **6 bushes × 125 = 750 food**, one patch | ≈5–8 |
| Woodline (forest patch) | A real forest — dozens of 100-wood trees | Edge ≈10–15 |
| Straggler trees | A few lone trees (commonly 2–3) | ≈4–8 — deliberately left standing by good players as future farmers' busywork |
| Main gold | **7 tiles × 800** | ≈10–16, front or back |
| Back golds | 2 × 4 tiles | Farther out |
| Main stone | **5 tiles × 350** | ≈10–16 |
| Back stone | 4 tiles | Farther out |
| Boars | 2 (§2.2) | ≈15–30, split directions |
| Deer | 2–4 near + 3 far (§2.2) | ≈12–20 / farther |
| Relics, predators | Standard map dressing | Out of opening scope |

### 2.4 Drop-off buildings

A villager deposits at the **nearest** building that accepts what it
carries; the Town Center accepts everything. The three camps are
also the Feudal-requirement buildings of the opening (Houses and
Farms do **not** count toward the 2-building requirement).

| Building | Cost | Build | Size | Accepts | Also |
|---|---|---|---|---|---|
| Lumber Camp | 100 W | 35 s | 2×2 | Wood | Wood techs (GAP 5.3); Feudal-requirement building |
| Mill | 100 W | 35 s | 2×2 | Food | **Prerequisite for Farms** and Market; farm techs + reseed queue (GAP 3.5); Feudal-requirement building |
| Mining Camp | 100 W | 35 s | 2×2 | Gold, Stone | Mining techs; Feudal-requirement building |

Details the hands expect:

- **Deposit is instant** on reaching the building, and the villager
  then returns to its previous task by itself (the gather loop,
  §2.5's force variant included).
- **Builders drop off on completion:** villagers who finish building
  a camp/Mill deposit *any* resource they are carrying into it the
  moment it completes — the first lumberjack's armful is never
  stranded.
- **Farms need a standing Mill first** (any Mill ever built counts;
  losing it later does not revoke Farms).

### 2.5 Force drop-off

The gesture: select carrying villagers → **right-click the TC (or
any drop-off building that accepts the load)** → each villager
walks there, deposits *immediately* (whatever fraction of its carry
it holds), and **returns to its previous task on its own**. (DE also
binds this to a drop-off hotkey; the click gesture is the
opening-critical path.)

Why it matters, mechanically: shepherds only auto-deposit at a full
carry of 10, so mid-cycle the food the TC needs is sitting in six
pockets of 8–9 each. Force drop-off converts pockets into queue —
at the cost of walking time and of desynchronizing the crew's
deposit rhythm (which is why good players avoid needing it, and why
it must exist anyway).

### 2.6 Loom

| | Spec |
|---|---|
| Researched at | Town Center (Dark Age) |
| Cost / time | **50 gold / 25 s** — it occupies the TC's research slot like a villager occupies its queue |
| Effect | Villagers **+15 HP** (25 → 40), **+1 melee / +2 pierce armor** |
| Opening role | The last TC job before the Feudal click (research it while the 500 food banks); also the safety purchase before a boar lure for less confident micro — un-Loomed lurer dies in 4 boar hits, Loomed in 7 (§2.2) |

### 2.7 The goal: Feudal advance

| | Spec |
|---|---|
| Researched at | Town Center |
| Cost / time | **500 food / 130 s** |
| Requirement | **Any 2 Dark Age buildings** (excluding Houses, Farms, walls, Outposts) — in the canonical opening, the Lumber Camp + the Mill |
| Effect | Age state advances; the Feudal tier unlocks (GAP 5.1). Clicking it on time, fed by the §1 timeline, is the opening's definition of success |

### 2.8 Supporting mechanics the timeline leans on

- **Villager**: 25 HP, 3 melee attack (bow at short range vs
  animals), 0/0 armor, speed 0.8, trains 50 F / 25 s.
- **Scout Cavalry (starting)**: 45 HP, attack 3 (Dark Age), 0/2
  armor, speed **1.2** (Dark Age), LoS 4 (Dark Age). Starts on
  **Stand Ground** stance — it does not chase or auto-attack
  (STUDY §3.6, §9.1). DE additionally offers an auto-scout toggle;
  noted, not required for the opening.
- **Gather rates** (per second, STUDY §6.1 — already the sim's
  targets): wood 0.388, berry 0.310, farm 0.319, **hunt 0.408**,
  **herdable 0.330**, gold 0.379, stone 0.359.
- **Carry**: 10 for everything — **except hunt meat, 35** (STUDY
  §6.2).
- **Construction**: solo build time is the listed time (House 25 s).
  Extra builders help with **diminishing returns**: total time =
  T × 3/(n+2) — 1 builder 25 s, **2 builders ≈ 19 s**, 3 builders
  15 s (GAP 4.18). The 19-second two-builder house is the opening's
  first timing trick, and it only works with this formula.
- **Queued orders**: Shift-click chains orders (shift-clicking sheep
  in eating order is the canonical use). One order slot is not
  enough (GAP 2.11).
- **Housed behavior**: at the population cap, production **queues
  but cannot complete**; it resumes the tick housing frees up
  (GAP 8.4). Queuing itself is never blocked by housing.
- **Rally points**: none set by default — a new villager emerges and
  stands idle beside the TC (GAP 2.12 covers setting them).
- **Idle-villager access**: the `.` key / idle button is how players
  find the villager this timeline just produced (GAP 1.6) — the
  opening is unplayable at speed without it, but it is a client/ABI
  item, not a sim-rule item.

---

## 3. Audit: Skirmish II today vs this spec

Audited 2026-10-16 against `sim/src/*.rs` and `web-preview/app.js`
(after M2.5 + the AoE2-style selection fix + minimap shipped).
**Totals: 37 lines — 10 HAVE · 8 PARTIAL · 19 MISSING.**

### 3.1 What already matches

| # | Spec item | Status | Evidence |
|---|---|---|---|
| A1 | Starting stockpile 200/200/100/200 | **HAVE** | `econ.rs` `Stockpile::starting()`, pinned by its unit test |
| A2 | 3 starting villagers | **HAVE** | `lib.rs` `START_VILLAGER_COUNT = 3`, fixed offsets beside the TC |
| A3 | Villager training: 50 F, 25 s, queue 15, cost on enqueue | **HAVE** | `econ.rs` `VILLAGER_FOOD_COST`, `VILLAGER_TRAIN_TICKS = 1500`, `TRAIN_QUEUE_CAP = 15`; `lib.rs` `issue_train_villager` spends on enqueue |
| A4 | Housed behavior (queue holds, completion stalls at cap, resumes) | **HAVE** | `lib.rs` training gate + `TrainBlock::Housed`; training stalls exactly as §2.8 requires |
| A5 | Population: start 3/5, TC +5, House +5 | **HAVE** | `lib.rs` `population()` / `population_cap()`; `build.rs` `TOWN_CENTER_POP_SUPPORT = 5`, `HOUSE_POP_SUPPORT = 5` |
| A6 | House: 25 W, 25 s solo, +5; second villager can join a standing foundation | **HAVE** | `build.rs` constants; `lib.rs` `BuildVerdict::Resume` assigns a late villager to the same foundation free |
| A7 | Gather → carry 10 → deposit → return loop; tree/berry/gold/stone amounts & rates | **HAVE** | `econ.rs` node table + per-tick rates (verified vs STUDY §6.4: 0.390/0.310/0.380/0.360 vs 0.388/0.310/0.379/0.359 — rounding); `entity.rs` Task cycle |
| A8 | TC accepts all four resources | **HAVE** | `lib.rs` `deposit()` credits whatever `carry_kind` holds |
| A9 | New villager emerges idle beside the TC (no rally set) | **HAVE** | `lib.rs` `spawn_trained_villager()` — spawns with `Task::Idle`; no rally system exists, so the default is the behavior |
| A10 | Camera starts centered on the TC | **HAVE** | `app.js` `resetSim()` → `centerCamera()` on the TC |

### 3.2 What is narrower than the spec

| # | Spec item | Status | Evidence / delta |
|---|---|---|---|
| P1 | Multi-builder speed 3/(n+2) (2 builders ≈ 19 s) | **PARTIAL** | `lib.rs` `construction_tick()`: every `Building`-phase villager adds a full tick of progress — **linear** stacking (T/n): 2 builders finish the house in **12.5 s**, not ≈19 s. The t=0 race is won either way, but every build-order timing a player feels (19 s house, 3-builder timings) is wrong (GAP 4.18) |
| P2 | Deposit at the *nearest* drop-off building | **PARTIAL** | The deposit step works, but the target is hardwired: `lib.rs` `begin_return()` / `deposit()` always use `town_center_pos()`. There is no notion of an alternative drop-off point to be nearest to (§2.4; GAP 3.4) |
| P3 | Berry patch: 6 bushes, 750 food, near the TC | **PARTIAL** | `econ.rs` `NODE_SPECS`: **4** bushes (500 food) at cells (21,24)–(22,25), ~1–2 cells from the TC cell (23,23). Per-bush 125 and the forager rate are right; the patch is two-thirds the size |
| P4 | Main gold: 7 tiles | **PARTIAL** | `NODE_SPECS`: **3** gold tiles at (35,15)–(35,16) — and they sit **behind the arena wall's gap**, so even reaching them teaches a path Arabia never asks for. Per-tile 800 is right |
| P5 | Main stone: 5 tiles | **PARTIAL** | `NODE_SPECS`: **2** stone tiles at (15,40)–(16,40). Per-tile 350 is right |
| P6 | Woodline + straggler trees | **PARTIAL** | 11 trees total (west cluster of 8 at x≈8–10, east copse of 3 at (40,40)) as point nodes. No forest mass, **no straggler trees** near the TC, and the map's segmentation is two compile-time **walls** (`grid.rs` `is_wall_cell`) — Arabia has no walls; its starts are open |
| P7 | Villager speed / travel-time feel (0.8 tiles/s) | **PARTIAL** | `steer.rs` `MAX_SPEED` = 1.0 unit/tick = 60 units/s on a 200-unit world: every opening walk (to the woodline, the boar's chase distance) completes in a fraction of a second. The proportions between train/build times (right) and travel times (≈50× compressed) are off until the tile mapping lands (§0; GAP 7.2) |
| P8 | The match starts live at t=0 | **PARTIAL** | Client shell: `app.js` holds the sim paused behind a start overlay until Play is clicked (`playing = false`). The sim itself exists from load; the reference match simply starts. Shell choice, flagged for the M4 pass |

### 3.3 What is absent

| # | Spec item | Status | Evidence / what parity needs |
|---|---|---|---|
| M1 | Villagers start **idle**; the player's orders are the first orders | **MISSING** (violated) | `app.js` `resetSim()` issues **demo orders**: villagers 1–2 → berries, villager 3 → a tree ("Opening demo orders so the economy starts moving immediately"). An AoE2 player's first three decisions are made *for* them — and made wrong (no house queue first, a lumberjack with no camp) |
| M2 | Starting Scout Cavalry | **MISSING** | `entity.rs` `EntityKind` has only `Villager` and `TownCenter`. No unit exists to find the sheep with (GAP 6.7) |
| M3 | Sheep: existence, conversion, task-to-move, one-at-a-time harvest | **MISSING** | No animal entities or ownership concept anywhere in the sim (GAP 3.7) |
| M4 | Boar: 340 food, 75 HP / 7 attack, aggression–lure–return loop | **MISSING** | As M3; needs the combat substrate first (M8 below) (GAP 3.6) |
| M5 | Deer: 140 food, flee, scout-pushing | **MISSING** | As M3 (GAP 3.6) |
| M6 | Carcass decay (sheep/deer 0.25/s, boar 0.4/s, running during harvest) | **MISSING** | `econ.rs` nodes lose amount only when gathered (`work_tick` Gathering arm); nothing in the sim models time-based loss |
| M7 | Hunt gather rate 0.408/s + hunter carry 35 | **MISSING** | `econ.rs` `CARRY_CAPACITY` is a single constant 10 for all resources; no hunt rate exists |
| M8 | Villager HP/attack; animals can wound and kill | **MISSING** | `entity.rs` `Entity` has no HP/attack/armor fields at all; nothing in the sim can be damaged or die (GAP 6.1, 6.13) |
| M9 | Lumber Camp (100 W / 35 s, wood drop-off, Feudal-requirement) | **MISSING** | `build.rs` `BuildingKind` = { House, Farm } only (GAP 4.11) |
| M10 | Mill (100 W / 35 s, food drop-off, farm prerequisite) | **MISSING** | As M9 (GAP 4.10) |
| M11 | Mining Camp (100 W / 35 s, gold/stone drop-off) | **MISSING** | As M9 (GAP 4.12) |
| M12 | Force drop-off (§2.5) | **MISSING** | No command path exists. The natural gesture actively misleads: `app.js` right-click on the TC becomes a plain move; `lib.rs` `issue_move` clears the task, **keeps the carry, and deposits nothing** — `deposit()` only ever runs from `Task::Returning`, which only a full carry triggers. The villager ends up standing at the TC, food in hand, task forgotten (GAP 2.15) |
| M13 | Builders deposit their carry into a camp they just completed | **MISSING** | `lib.rs` `leave_finished_site()`: farm builders go gather, house builders go idle — no completion deposit exists to generalize (rides with M9–M11) |
| M14 | Farms require a standing Mill | **MISSING** (divergence) | `lib.rs` `validate_build()` checks footprint/cost/reachability only — a Farm is buildable at t=0 with no Mill anywhere. The reference forbids it (§2.4) |
| M15 | Queued (shift) orders — the sheep queue | **MISSING** | `entity.rs`: a single `order: Option<MoveOrder>` slot; every command replaces, never chains (GAP 2.11) |
| M16 | Loom (50 G / 25 s at the TC) | **MISSING** | No research system exists — the TC can only train (GAP 5.2, 5.3) |
| M17 | Feudal advance (500 F / 130 s, 2-building rule) | **MISSING** | No age state exists anywhere in the sim (GAP 5.1). The opening has no finish line |
| M18 | Generated Arabia-style start (seeded, fair, per §2.3) | **MISSING** | Layout is compile-time: `grid.rs` walls + `econ.rs` `NODE_SPECS`; `lib.rs` concedes "The M2 starting layout is fixed … the seed currently feeds only the in-state RNG" (GAP 7.1) |
| M19 | Blind start: map unseen beyond the TC's surroundings | **MISSING** | Everything renders from tick 0; no visibility state exists (GAP 7.6, 7.9). The scout's whole job (§1) presupposes it |

Two structural notes under the table:

- **The sim's fixed arena fights the spec.** `NODE_SPECS` + the two
  walls make a puzzle map, not a start position: gold behind a wall
  gap (P4), a forest split by a wall column, no ring of near
  resources. §2.3 cannot be approximated by editing node coordinates
  alone — it needs the template generator of §4, bucket C.
- **The economy numbers that *are* in are right.** Every rate,
  amount, cost, and time the M2/M2.5 economy implements was verified
  against the public sources (STUDY §6.4) — the gap is *breadth*
  (animals, camps, research, ages), not calibration. The two
  exceptions are P1 (build stacking) and P7 (travel-time scale).

---

## 4. Implementation sequencing

Ordered by **what breaks the zero-learning promise worst if
absent**, grouped by the kind of work each item is. (Milestone
labels follow GAP-ANALYSIS's reconciled map.)

### Step 0 — a deletion, do it today (client only)

- **Remove the demo auto-orders** (M1). `app.js` `resetSim()` stops
  after creating the sim and centering the camera: 3 idle villagers,
  TC selected or not — the player's clicks become the match's first
  orders. No sim change, no test impact beyond the client's own.
  This is the cheapest zero-learning repair in the project.

### Bucket A — lands with M3 combat (animals *are* combat)

Hunting is not an economy feature bolted onto nodes; it is the
combat system exercised against Gaia. In dependency order:

1. **Damage substrate**: HP/attack/armor on entities, death and
   removal (M8; GAP 6.1, 6.13), plus the villager attack order and
   the `Attack` command in the ABI (GAP 2.3).
2. **Animal entities (Gaia class)**: one animal kind with three
   behavior profiles — herdable (stands, converts, is tasked),
   docile huntable (flees), aggressive huntable (aggro/chase/return)
   (M3–M5; GAP 3.6, 3.7, 7.8). Sheep need no combat AI and can land
   first inside this step; the boar needs the whole loop.
3. **Carcasses**: a killed animal becomes a food node *with decay*
   (M6), gathered at the herdable/hunt rates with the hunt carry-35
   exception (M7).
4. **Starting Scout Cavalry** (M2): a unit kind with the §2.8 stats,
   spawned beside the TC, Stand Ground by default (GAP 6.7). It is
   M3's simplest military unit and the opening's second pair of
   hands.

### Bucket B — economy-sim work (no combat dependency; can parallel M3)

5. **Drop-off as a building role**: camps as `BuildingKind`s (M9–
   M11), deposit retargeted to the *nearest accepting building*
   (fixes P2), builders deposit on completion (M13), Farm gated on
   a standing Mill (M14) (GAP 3.4, 3.5, 4.10–4.12).
6. **Force drop-off command** (M12; GAP 2.15): "deposit now, then
   resume" — a `Returning` variant that remembers the task it
   interrupted (the task model already survives deposit for gather
   assignments; force drop extends the same path to a manual order
   and to non-TC targets).
7. **Builder stacking formula** (P1): construction progress per
   tick = builders counted at the 3/(n+2) rate (GAP 4.18). Small,
   surgical, and it repairs the opening's first timing trick.
8. **Queued orders** (M15; GAP 2.11): per-entity order queue in the
   sim + shift handling in the client. The sheep queue is its first
   user.

### Bucket C — map work

9. **Arabia-template start generator** (M18; GAP 7.1): a *fixed
   template* first — §2.3's counts placed in the §2.3 bands relative
   to the TC, rotated/mirrored by the match seed — replacing the
   arena walls + `NODE_SPECS` for the start position. Full random
   map scripts (fairness checks, multiple biomes) stay M7; the
   template is what the opening needs, and it deletes P3–P6's
   deltas in one move.
10. **Blind start / fog of war** (M19; GAP 7.6, 7.9): per-player
    visibility; last in this list only because it is the biggest
    single system here — the opening *wants* it (the scout's job
    exists because the map is dark), and nothing else in the
    opening depends on its internals.

### Goalposts — the opening's finish line

11. **A minimal TC research slot** → **Loom** (M16; GAP 5.2, 5.3),
    then **age state + the Feudal advance** with the 2-building
    rule (M17; GAP 5.1). The full technology tree is M6; the
    opening needs exactly one technology and one advance, and
    shipping those two on the generic research/age scaffolding —
    rather than as one-offs — is the honest way to close the
    opening.

**The order, in one line:** stop auto-playing the opening (0) →
give the player sheep and a scout (A) → give the woodline a camp
and the food a force-drop (B) → deal a real start position (C) →
give the ritual its finish line (goalposts).

---

## 5. Cross-reference index (this spec → GAP-ANALYSIS)

| Spec area | GAP items |
|---|---|
| Scout / exploration start | 6.7, 7.9, 1.8 |
| Sheep (herdables) | 3.7 |
| Boar & deer (hunting) | 3.6 |
| Animals as Gaia entities | 7.8 |
| HP, damage, death (substrate) | 6.1, 6.2, 6.13 |
| Villager attack order | 2.3, 2.2 |
| Drop-off buildings (Mill / Lumber / Mining Camps) | 3.4, 4.10, 4.11, 4.12 |
| Farms & the Mill (reseed, gating) | 3.5 |
| Force drop-off | 2.15 |
| Construction (stacking, join-in-progress) | 4.18 |
| Queued orders (sheep queue) | 2.11 |
| Rally points (absent by default) | 2.12 |
| Loom | 5.3 |
| Research system (substrate) | 5.2 |
| Feudal advance / ages | 5.1 |
| Map generation, sizes, terrain | 7.1, 7.2, 7.3 |
| Fog of war / line of sight | 7.6, 7.7 |
| Idle-villager key (opening ergonomics) | 1.6 |
| Population & housed rules | 8.4 |
| Match settings (pop cap, resources) | 8.8 |

## Sources

Public documentation only, consulted 2026-10-16:

- Spirit of the Law, *Beginner guide to the first 15 villagers*
  (AoE2) — public video transcript: the §1 timeline,
  the 2-builder house race, 6-shepherd math, force drop-off
  doctrine, straggler-tree doctrine, Loom-before-Feudal.
- Age of Empires fandom wiki: Arabia (per-player start resources:
  8 sheep 4+2+2, 2 boars, 2–4 + 3 deer, 6 forage bushes, 7-tile
  main gold, 5-tile main stone) · Wild Boar (75 HP, 7 attack,
  speeds 1.0/0.8, lure strategies, 4-hit/7-hit Loom math) · Sheep
  (conversion by proximity, LoS 3, speed 0.7) · Herdable animal
  (capture by majority presence, decay 0.25/s) · Hunt (decay: deer
  0.25/s, boar 0.4/s, running during harvest) · Deer (140 food,
  flees, two shots, pushable, herds of 2–4) · Early Economy
  (in-game tutorial: one sheep at a time, shift-queued tasks,
  two-shot boar lure) · Lumber Camp / Mill (100 wood, drop-off
  roles, builders deposit on completion, Mill gates Farms, camp +
  Mill as the standard Feudal-requirement pair) · Building
  comparative table (camps 100 W / 35 s / 2×2; TC 4×4; Farm
  requires a Mill).
- [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) (in this docs set): gather rates §6.1, carry
  §6.2, node & animal amounts §6.3, M2 cross-check §6.4, ages §7,
  Loom §8, Scout Cavalry §3.6, stances §9.1, population §9.4.
- Skirmish II source tree (`sim/src/`, `web-preview/app.js`) for every
  status claim in §3.
