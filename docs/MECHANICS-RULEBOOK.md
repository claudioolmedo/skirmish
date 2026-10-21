# Skirmish II — Mechanics Rulebook

**Date:** 2026-10-16
**Purpose:** The complete behavioral rulebook for Skirmish II. [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md)
holds the *numbers* (unit/building tables, rates); [`GAP-ANALYSIS.md`](GAP-ANALYSIS.md) holds the
*inventory* (what exists vs. what is missing); [`OPENAGE-RESEARCH.md`](OPENAGE-RESEARCH.md) holds the
*engine lessons*. This document holds the **rules** — every behavioral law of
Age of Empires II DE's game mechanics, stated precisely, sourced from public
documentation, each with an implementation note for Skirmish II's architecture
and a status tag grounded in the actual code (`sim/src/*.rs`,
`web-preview/app.js`, `web-preview/play/`) as of 2026-10-16. The start-of-game
rules live in [`OPENING-SPEC.md`](OPENING-SPEC.md); opening-specific rules here cross-reference it.

**Clean-room note:** Compiled exclusively from public documentation — the Age
of Empires II wiki (Fandom), AoK Heaven community research, the original
developers' published papers (Bettner & Terrano; Pottinger), and openage's
public documentation. No game binary, game file, or extracted data was
accessed. Rules are stated in our own words; facts and numbers are recorded
as facts. Values that public sources do not verify are marked **UNVERIFIED**;
they are never guessed.

**Status legend:** **MATCHES** — Skirmish II behaves this way today ·
**DIFFERS** — Skirmish II has the system but behaves differently (the note says
how) · **MISSING** — not present. GAP references (e.g. GAP 6.1) point at
[`GAP-ANALYSIS.md`](GAP-ANALYSIS.md) items.

**Architecture shorthand used in implementation notes:** the sim is pure Rust,
fixed-point I16F16, 60 ticks/s, entities in an id-sorted `Vec` (never a
HashMap), commands as data applied FIFO at tick start, a seeded in-state
PCG32 RNG, and a per-tick FNV-1a hash over a canonical serialization.
"Data table" means a Rust table in the nyan *shape* (base records + named
patches) per OPENAGE-RESEARCH §2.7 — never code branches.

---

## 1. Simulation & time

**SIM-01 — Fixed timestep.** The simulation advances in fixed steps of
1/60 s of game time. Rendering and input are decoupled from the step;
nothing in the game world is integrated with a variable frame delta.
*Source:* Bettner & Terrano (GDC 2001); MECHANICS-STUDY §10.1. · *Impl:*
`TICKS_PER_SECOND = 60` in `lib.rs`; the client runs a fixed-step
accumulator. · *Status:* **MATCHES**.

**SIM-02 — One canonical state.** All gameplay state lives in a single
simulation state. The sim reads no wall clock, no platform entropy, no
local settings; the same (seed, settings, command stream) always produces
the same state on every platform, including WASM.
*Source:* OPENAGE-RESEARCH §2.6. · *Impl:* `Sim::new(seed)` + `tick()` are
pure; floats exist only at the WASM ABI boundary (`wasm.rs`) and are
converted to fixed-point on entry. · *Status:* **MATCHES**.

**SIM-03 — Commands are data, applied at tick boundaries.** Player intent
enters the sim only as command records, queued and applied in FIFO order at
the start of a tick. A later order for the same entity replaces the earlier
one unless the command type defines queuing (SEL-12).
*Source:* Bettner & Terrano; GAP 10.1. · *Impl:* `Command` enum + `commands`
queue drained in `tick()` phase 1. Today: Noop, Move, Gather, TrainVillager,
Build. · *Status:* **MATCHES** (command vocabulary grows per milestone).

**SIM-04 — Seeded RNG inside the state.** All randomness draws from one
seeded generator whose state is part of the game state. Every machine makes
the same number of draws in the same order; no draw ever happens in
rendering, UI, or unordered iteration.
*Source:* OPENAGE-RESEARCH §2.6 (the "RNG call count and order" rule). ·
*Impl:* hand-rolled PCG32 in `rng.rs`, words hashed every tick; gameplay
draws begin with M3 accuracy rolls (PRJ-03). · *Status:* **MATCHES**
(machinery in place; first consumers land in M3).

**SIM-05 — Per-tick state checksum.** After every tick, a checksum over a
canonical serialization of the full state (tick, RNG words, grid, stockpile,
nodes, queues, buildings, every entity field) is produced. Any divergence
anywhere changes the checksum — it is the desync detector and the regression
tripwire.
*Source:* Bettner & Terrano (checksummed "everything"); OPENAGE-RESEARCH
§2.6. · *Impl:* FNV-1a 64 in `hash.rs`, fed by `state_hash()` in a fixed
field order; golden-hash tests pin it. · *Status:* **MATCHES**.

**SIM-06 — Subsystem checksums for forensics.** World state, pathfinding,
targeting, and the RNG stream are checksummed separately in diagnostics, so
a desync report names the diverging subsystem, not just the tick.
*Source:* OPENAGE-RESEARCH §2.6, §5.4. · *Impl:* extend `state_hash()` with
per-subsystem subtotal hashes exposed in tests/debug builds when M3 adds
targeting; the shipped hash stays one value. · *Status:* **MISSING**
(single combined hash only today).

**SIM-07 — Fixed phase order inside a tick.** A tick runs its phases in one
fixed order (today: apply commands → movement steering/integration → economy
(training, work cycles) → construction). The order is part of the rules:
reordering phases changes results and therefore the hash.
*Source:* Skirmish II architecture (ARCHITECTURE.md contract). · *Impl:*
`tick()` in `lib.rs`; new systems append phases in documented positions
(combat resolves after movement, before economy). · *Status:* **MATCHES**.

**SIM-08 — Stable iteration order.** Entities are stored sorted by id and
every system iterates in id order (or another single documented order).
Hash-map iteration order is never observed by sim logic.
*Source:* OPENAGE-RESEARCH §2.6 (determinism disciplines). · *Impl:* `Vec`
sorted by id everywhere; the grid A* uses a total-ordered heap; no HashMap
exists in the sim. · *Status:* **MATCHES**.

**SIM-09 — Fixed-point arithmetic only.** All gameplay math (positions,
HP, damage, resources, rates) is integer/fixed-point (I16F16). Rates that
are not exactly representable are stored as pre-rounded raw-bit constants,
converted once by hand at authoring time, never computed from floats at
runtime.
*Source:* OPENAGE-RESEARCH §1.3 (the reference engine family also uses a
16-bit fraction); Fiedler, "Floating Point Determinism". · *Impl:* the
`fixed` crate; see `econ.rs` header for the rate-constant convention. ·
*Status:* **MATCHES**.

**SIM-10 — World bounds are hard.** Entity positions are clamped to the
world rectangle every tick; nothing can leave the map (see MAP-11).
*Impl:* `clamp_pos` in `lib.rs`, applied on integration and on command
targets. · *Status:* **MATCHES**.

**SIM-11 — Game-speed setting scales time, not rules.** A match speed
setting (lobby speeds 1.0 / 1.5 / 2.0; DE's standard ranked speed is 1.7×)
multiplies how many game-seconds pass per real second. Per-game-second
behavior is identical at every speed.
*Source:* GAP 8.8; DE lobby conventions. · *Impl:* a client/host concern:
run more/fewer ticks per real second (fractional accumulator); the sim
itself never learns the speed. Our preview currently runs exactly 60
ticks per real second and *slows the game* on a slow machine (max-ticks
guard) instead of adapting — acceptable single-player behavior, not the
multiplayer rule (NET-03). · *Status:* **DIFFERS** (no speed setting; no
1.5×/2×).

**SIM-12 — Match identity.** A match is completely identified by (seed,
settings, command stream). This is what makes replays, saves, and lockstep
verification the same mechanism (NET-09, NET-10).
*Source:* Bettner & Terrano; GAP 8.9. · *Impl:* already true by
construction; a headless replay example exists (`sim/examples/replay.rs`).
· *Status:* **MATCHES** (principle); user-facing replay format is NET-09.

---

## 2. Map, terrain & elevation

**MAP-01 — The world is a tile grid; distances are in tiles.** All ranges,
speeds, and line-of-sight values in the data tables are expressed in tiles.
*Source:* MECHANICS-STUDY §4.2. · *Impl:* shipped (M7) — 1 cell = 1 tile
= 4 world units, on a 120×120-tile map (MAP-02); table values are
authored in tiles and converted once in the data tables (unit LoS, for
instance, is stored in world units as tiles × 4 — `units.rs`). ·
*Status:* **MATCHES**.

**MAP-02 — Map sizes.** Square maps from Tiny (120×120 tiles) to Ludicrous
(480×480); object counts and victory timers scale with size (VIC-02).
*Source:* GAP 7.2; Relic wiki (victory duration table by size). · *Impl:*
shipped in part (M7) — the 2-player size, Tiny 120×120 tiles, is the
world (world units [-240, 240), 1 tile = 1 cell = 4 units); the client
reads every dimension from the ABI, so sizes parameterize end-to-end
when more land. · *Status:* **DIFFERS** (Tiny only; the larger sizes
and their object/timer scaling are not implemented).

**MAP-03 — Terrain types gate movement and placement.** Terrains include
grass, dirt, sand, snow, ice, beach, shallows, and deep water. Each terrain
declares which movement domains may enter it and whether buildings may be
placed on it.
*Source:* GAP 7.3, 7.5. · *Impl:* shipped in part (M7) — a terrain
layer in `Grid` beside the blocked layers (land / water); passability
for the one domain that exists (land) is derived: water blocks
movement and placement, land admits both. The compile-time wall layout
is gone. · *Status:* **DIFFERS** (only land and water exist as types —
dirt/sand/snow/ice/beach/shallows are not distinct terrains yet;
shorelines render as sand on the client only).

**MAP-04 — Forests are resource and obstacle at once.** A forest tile holds
one tree node (100 wood). A tree blocks pathing while it stands; when
chopped out, the tile frees immediately (the stump is visual only). Onager-
line attacks can fell trees (CMB-16).
*Source:* MECHANICS-STUDY §6.3; GAP 3.3, 6.9. · *Impl:* shipped (M7) —
node and obstacle are merged: a tree node *is* its cell's block (the
grid's node-block layer, set at generation) until the tick it depletes,
when the cell opens; gather orders path to a standing cell beside the
tree. · *Status:* **MATCHES** (the blocking rule; Onager-line felling
is CMB-16, still missing with the siege line).

**MAP-05 — Elevation is per-tile data.** The map carries a discrete height
level per tile (the reference data model uses 7 levels), independent of
terrain type. Rendering lifts tiles by their height; gameplay reads height
*differences* only.
*Source:* OPENAGE-RESEARCH §2.3; GAP 7.4. · *Impl:* shipped (M7) — a
height byte per cell in `Grid`, hashed per tick, consumed by CMB-01's
elevation factor and the renderer's iso lift. We carry 5 levels
(0–4), not the reference's 7 — the combat rule reads differences only
(MAP-06 is binary), so the count is presentational. · *Status:*
**DIFFERS** (5 levels, not 7).

**MAP-06 — Hill bonus is binary.** Any height advantage — one level is
enough — gives the full damage multiplier (CMB-05). It never scales with
the size of the height difference.
*Source:* MECHANICS-STUDY §1.4, confirmed by AoK Heaven elevation tests. ·
*Impl:* shipped (M7) — sign comparison at damage time inside CMB-01's
formula (`compute_damage_elev`); no float. · *Status:* **MATCHES**.

**MAP-07 — Cliffs are impassable elevation breaks.** A cliff is a border
between tiles where height changes discontinuously; land units can never
cross it, whatever the heights. (Whether the uphill damage penalty applies
across cliffs is AMBIGUOUS in the sources — MECHANICS-STUDY §1.4; the
working default is that the downhill bonus applies, the uphill penalty
does not.)
*Source:* OPENAGE-RESEARCH §2.3; MECHANICS-STUDY §1.4. · *Impl:* not
built (M7) — the generator caps adjacent-cell height deltas at 2 and
flattens starts, so no cliff-like breaks exist to cross; until cliffs
are authored, the damage default from the study applies everywhere
(both the downhill bonus and the uphill penalty, via CMB-01). ·
*Status:* **MISSING**.

**MAP-08 — Water belongs to ships.** Deep water admits only ships;
shallows admit ships (and, in the reference, nothing else — there is no
wading). Land units path only on land. The shoreline is where the domains
meet; Docks are placed straddling it (BLD-14).
*Source:* GAP 7.5. · *Impl:* shipped in part (M7) — water is a terrain
type: land units path only on land (A* and placement both refuse it),
and generated ponds are validated (clear of starts, never severing the
landmass). The water domain itself — ships, shallows, Docks (BLD-14),
fishing (ECO-15) — is not built: Arabia v1 carries water as terrain
only, sparingly (0–2 small ponds, some seeds have none). · *Status:*
**DIFFERS**.

**MAP-09 — Generated maps guarantee fair starts.** A random-map script
places, per player: a Town Center start, a nearby woodline, a berry patch,
main and secondary gold and stone, herdables and huntables in standard
counts/distances, straggler trees, and relics distributed across the map —
roughly symmetric between players. The exact start package is specified in
[`OPENING-SPEC.md`](OPENING-SPEC.md).
*Source:* GAP 7.1; wiki map pages. · *Impl:* shipped (M7) — the Arabia
generator runs *inside* match creation from the match seed
(deterministic sub-seed stream; its output is ordinary state, hashed
like everything else). Start B's package is start A's mirrored through
the map center, so counts and distances are exactly equal. Generation
validates (resources reachable, a land path between the starts, no
start enclosed, shoreline clearance) and regenerates with the next
sub-seed on failure — bounded at 64 attempts, then a fixed fallback
template that validates by construction. Relics are not distributed
yet (they land with §12). · *Status:* **MATCHES** (the start package
and fairness guarantee; relics pending §12).

**MAP-10 — Straggler trees are ordinary trees.** The single trees scattered
near starts follow MAP-04 exactly; their only special property is
positional (players save them for future farmers — a strategy fact, not a
rule).
*Source:* community build-order literature (see OPENING-SPEC). · *Impl:*
nothing special; node kind Tree — and since M7 their placement is the
generator's (MAP-09): 3 stragglers per start in the §2.3 band. ·
*Status:* **MATCHES**.

**MAP-11 — The world edge is a wall.** Out-of-bounds is impassable for
every domain.
*Impl:* `Grid::is_blocked` returns true out of bounds; positions clamp
(SIM-10). · *Status:* **MATCHES**.

**MAP-12 — Gaia owns the wild.** Resource nodes, animals, and relics belong
to player 0 (Gaia) until a rule transfers them (herdable capture AI-06,
relic pickup MNK-09). Gaia never acts except through the animal rules
(§15).
*Source:* wiki (Gaia/Mother Nature); Conversion wiki (Gaia objects). ·
*Impl:* shipped — an `owner` field per entity (and per building):
Gaia owns every node and wild animal until capture/harvest rules
transfer them; the two player seats own the rest (M3/M5 entity model).
· *Status:* **MATCHES**.

---

## 3. Visibility & exploration

**VIS-01 — Three visibility states.** Every tile has, per player, one of
three states: *unexplored* (black), *explored* (terrain shown dimmed,
frozen at last sight), *visible* (live).
*Source:* OPENAGE-RESEARCH §2.5; GAP 7.6. · *Impl:* shipped (M7) — a
per-player byte grid (0 unexplored / 1 explored / 2 visible) for each
player, recomputed in the sim every 15 ticks and at match creation.
**Deviation, recorded deliberately:** the grid lives *inside* `Sim`
and is hash-covered, not client-side as first sketched — VIS-09's own
note sanctions computing visibility deterministically in the sim, and
M7's AI must decide *from* its grid (VIS-09 below), which only replays
bit-identically if the grid is state. The client receives the local
player's grid through the ABI and renders strictly from it. ·
*Status:* **MATCHES**.

**VIS-02 — Visibility is the union of sight circles.** A tile is visible
while it lies within the line-of-sight radius of any of the player's units
or buildings (or a vision-sharing ally's, VIS-08).
*Source:* OPENAGE-RESEARCH §2.5. · *Impl:* shipped (M7) — visibility is
the union of integer sight discs stamped from every living unit and
standing building of the player, rebuilt on a fixed 15-tick interval
(cost: sources × disc area, independent of map area — no per-tick
full-map scans). · *Status:* **MATCHES**.

**VIS-03 — Line of sight is a unit stat.** Every unit and building has a
LoS radius in the data tables (MECHANICS-STUDY §3 lists them per unit; for
ranged units it is typically range + 2). Technologies can raise building
LoS (Town Watch +4, Town Patrol +4).
*Source:* MECHANICS-STUDY §3, §8; Technology wiki. · *Impl:* shipped in
part (M7) — the unit tables carry a `los` column (world units, tiles ×
4) and fog reads the owner's *effective* row, so the Scout's Feudal
LoS rise applies (TEC-06). Building LoS is two **adopted constants —
Town Center 6 tiles, other buildings 4 — flagged UNVERIFIED**: the
project docs do not pin building LoS values. Town Watch / Town Patrol
(+4) are not implemented, and no other tech patches LoS yet. ·
*Status:* **DIFFERS**.

**VIS-04 — LoS is capped at 20 tiles.** No unit or building sees farther
than 20 tiles, whatever its stats or technologies say.
*Source:* wiki, Line of Sight (via OPENAGE-RESEARCH §2.5, §4 item 3). ·
*Impl:* shipped (M7) — clamped at stamp time in the visibility
recompute; a test pins that no visible cell ever sits beyond 20 tiles
of a sight source. · *Status:* **MATCHES**.

**VIS-05 — Sight is not occluded.** Line of sight is a pure radius: hills,
forests, and buildings do not block or shadow vision. There are no vision
cones.
*Source:* OPENAGE-RESEARCH §2.5 (explicit: "radius-based, not
raycast-occluded, in classic AoE2"). · *Impl:* shipped (M7) — integer
disc test only; no raycasting exists anywhere in the visibility path.
· *Status:* **MATCHES**.

**VIS-06 — Explored areas remember buildings, not units.** In explored
(not currently visible) tiles, terrain and buildings are shown at their
last-seen state — including buildings destroyed since, which keep showing
until the tile is seen again. Units are never shown from memory.
*Source:* OPENAGE-RESEARCH §2.5; GAP 7.6. · *Impl:* shipped (M7) — the
client keeps a last-seen snapshot per enemy building, refreshed only
while its ground is visible and dropped when re-seeing finds the
building gone; the main view and minimap both draw from it, dimmed.
The enemy Town Center (an entity in our model) follows the same rule;
units and projectiles render only in currently-visible cells. ·
*Status:* **MATCHES**.

**VIS-07 — The match starts nearly blind.** At tick 0 a player sees only
the surroundings of their starting units and Town Center; everything else
is unexplored. Scouting is the act of converting unexplored → explored.
*Source:* GAP 7.9. · *Impl:* shipped (M7) — the grids are computed at
match creation from the starting set alone (Town Center, villagers,
Scout), so a match opens with only the start's surroundings known;
tests pin that the enemy start reads unexplored for both players. ·
*Status:* **MATCHES**.

**VIS-08 — Allies share vision.** In DE, teammates' line of sight is
shared automatically (the old Cartography technology that once gated this
was removed from DE). Team visibility is the union over the team.
*Source:* Technology wiki (Cartography listed among removed/cut generic
technologies); GAP 8.7 (older gated model, superseded). · *Impl:* merge
per-team on the client; the sim still tracks per-player truth. ·
*Status:* **MISSING** (no teams exist yet — 1v1 and Sandbox only).

**VIS-09 — Fog hides information, not truth.** In the lockstep model every
client simulates the whole world; fog is a presentation filter. Gameplay
systems (auto-targeting, AI) use simulation truth under their own rules —
fog never feeds back into the sim. (The accepted trade-off, documented
since 2001: a tampered client can reveal the map; checksums cannot catch
that.)
*Source:* OPENAGE-RESEARCH §2.5, §2.6. · *Impl:* architectural law,
kept with one recorded refinement (M7): the visibility grids are sim
state (see VIS-01) — fog still never feeds back into *truth*:
movement, combat resolution and the economy read the world itself,
and auto-acquisition's radius is the unit's own LoS, so nothing can
engage what it could not see. What fog gates is *knowledge*: the
client's rendering (via the ABI grid) and the scripted AI's
targeting decisions. No client state ever flows into the sim. ·
*Status:* **MATCHES** as a design rule.

**VIS-10 — The minimap obeys the same fog.** The minimap renders the
player's three visibility states (black / dim / live colors); enemy dots
appear only on visible tiles (INF-04).
*Source:* GAP 11.1. · *Impl:* shipped (M7) — the minimap's ground is a
per-cell terrain base in the player's three fog states (black /
pre-dimmed / live), and its node, building and entity layers are
filtered by the same grid the main view uses. · *Status:*
**MATCHES**.

---

## 4. Selection & orders

**SEL-01 — Click selects exactly one.** A plain click selects the single
own unit (or own building) under the cursor and nothing else. Clicking
empty ground clears the selection.
*Source:* GAP 1.1 (AoE2 DE behavior). · *Impl:* client selection state;
shipped 2026-10-16 (`app.js` selection Set + separate building selection).
· *Status:* **MATCHES**.

**SEL-02 — Drag-box selects own units in the box.** Dragging a rectangle
and releasing selects every own *unit* whose position falls inside it.
Buildings are not box-selected.
*Source:* GAP 1.2. · *Impl:* screen-space box over unit ground points;
shipped. · *Status:* **MATCHES**.

**SEL-03 — Shift modifies selection.** Shift+click toggles one unit in or
out of the selection; shift+drag adds the boxed units to the existing
selection instead of replacing it.
*Source:* GAP 1.3. · *Impl:* shipped with SEL-01/02. · *Status:* **MATCHES**.

**SEL-04 — Double-click selects same type on screen.** Double-clicking a
unit selects all own units of the same kind currently visible on screen.
*Source:* GAP 1.4. · *Impl:* shipped; kind-agnostic over `entity_kinds()`,
so future military kinds inherit it. · *Status:* **MATCHES**.

**SEL-05 — Selection cap is 60.** One selection holds at most 60 units;
box/double-click selection stops at the cap.
*Source:* GAP 1.11. · *Impl:* shipped (M4) — one constant
(`SELECTION_CAP = 60`) enforced in the client's `selectUnits()`, the
single funnel every selection path uses (click, box, shift, double-click,
control-group recall, idle keys), so all of them stop at the cap. ·
*Status:* **MATCHES**.

**SEL-06 — Orders go only to the selection.** A command is issued to the
currently selected units/building and to nothing else. With an empty
selection, order clicks do nothing (the client may flash a hint).
*Source:* the project's zero-learning rule (an AoE2 player's hands); GAP §1–2.
· *Impl:* shipped — the old "no selection = order everyone" behavior was
removed 2026-10-16. · *Status:* **MATCHES**.

**SEL-07 — Control groups.** Ctrl+number assigns the selection to a group
(replacing its contents); pressing the number recalls the group;
double-pressing (or pressing Space with the group selected) centers the
camera on it; Shift+number adds the selection to the group. The Alt layer
provides groups 11–20 (20 groups total).
*Source:* GAP 1.5; wiki, Hotkey. · *Impl:* shipped (M4) for groups 1–9 —
client-only store (`play/groups.js`: assign / recall / double-tap
tracking, unit-tested); groups store *ids*, dead ids filtered at recall
time (the sim remains the truth); double-tap centers on the group's
centroid. Divergences from the reference text: Shift+number adds the
*group* to the current selection (the reference adds the *selection* to
the group); Space-to-center is not bound (double-tap serves); the Alt
layer (11–20) is not implemented. · *Status:* **DIFFERS** (core shipped;
Shift semantics and the Alt layer differ).

**SEL-08 — Idle villager key.** A dedicated key cycles through idle
villagers one by one (selecting each, camera following); its Shift variant
selects all idle villagers at once. A villager is idle when it has no work
task and no move order.
*Source:* GAP 1.6; wiki, Hotkey. · *Impl:* shipped (M4) — the sim exposes
`idle_villagers()` over the ABI (living player villagers with
`Task::Idle`, no move order, no attack target — the definition above);
`.` cycles with select + camera-follow, Shift+`.` selects all. The
top-bar indicator (INF-03) is the same query as a button. · *Status:*
**MATCHES**.

**SEL-09 — Idle military key.** The mirror key cycles idle military units
(Shift: all idle military). Becomes meaningful when military kinds exist.
*Source:* GAP 1.7. · *Impl:* shipped (M4) — `idle_military()` on the same
ABI + `EntityKind::is_military` in the data table (Scout, Militia,
Archer); `,` cycles, Shift+`,` selects all. · *Status:* **MATCHES**.

**SEL-10 — Go-to keys.** H selects and centers the Town Center; Ctrl+letter
cycles buildings of each production type; Space centers on the current
selection; Backspace jumps the camera to the location of the most recent
alert (INF-01).
*Source:* GAP 1.8; wiki, Hotkey. · *Impl:* H shipped (M4) — it selects
and centers the Town Center through the existing building selection.
Still open: Ctrl+letter cycling of production buildings, Space centering
the selection, and the Backspace alert jump (waits on INF-01 alerts). ·
*Status:* **DIFFERS** (H only).

**SEL-11 — Right-click is a smart order.** Right-clicking with a selection
issues the context order implied by the target: resource node → gather;
enemy unit/building → attack; own garrisonable building → garrison; own
building (villager) → build/repair context; ground → move.
*Source:* GAP 2.2. · *Impl:* the client's `orderAt` resolves the target
class, then pushes the matching sim command. Today only gather/move
resolve; attack (M3), garrison and repair (M6) extend the same function. ·
*Status:* **DIFFERS** (gather/move only).

**SEL-12 — Queued orders (waypoints).** Holding Shift while issuing orders
*appends* them to each unit's order queue instead of replacing it; the
unit executes them in sequence. The client draws a flag per queued order.
This applies to moves, gathers, builds, attacks — and to sheep/task
assignments in the opening (see OPENING-SPEC).
*Source:* GAP 2.11; wiki, Early Economy ("queue up multiple tasks … by
holding down the Shift key" — a DE feature). · *Impl:* shipped (M4) —
each entity carries an `order_queue` (Vec of data-only order records) in
the sim, hash-covered; `Command::Queue` appends, any non-queued command
replaces the current order *and* clears the queue; a queue-advance step
at the end of each tick starts the next order whenever an entity is
fully idle (no work task, no move order, no attack target), and an order
that no longer validates at execution (target died, node depleted,
build spot/funds gone) is dropped rather than wedging the chain. Covers
move, gather, attack and build; a queued build's wood is charged at
*execution*. Client: Shift+right-click on the canvas or minimap,
Shift+click in placement mode (placement stays armed). Per-entity cap:
16 queued orders (an implementation bound; no cap is recorded for the
reference game). Not shipped: the per-queued-order flag rendering (the
queue is not exposed to the renderer) and attack-move (an
engage-en-route order — it rides on this queue when it lands). ·
*Status:* **MATCHES** (behavior; the flag rendering is INF-09's).

**SEL-13 — Rally (gather) points.** Every production building and the Town
Center holds a rally point, set by selecting the building and right-
clicking a target. Newly created units walk to it automatically; a rally
on a resource sends new villagers straight to work there; a rally on a
garrisonable building makes them garrison inside on creation (GAR-09).
*Source:* GAP 2.12; wiki, Gather Point. · *Impl:* shipped (M4) — one
rally record per building in the sim (hashed), plus the Town Center's;
set by selecting the building and right-clicking (main view or
minimap). New units walk to the point on spawn; a point within snap
range (4 units) of a live node records that node, and a new *villager*
goes straight to work on it (re-checked live at spawn; if it died, the
villager walks to the point). Unset rally = the unit stays at its spawn
point just outside the building (the reference default). The client
draws the selected building's flag with a dashed line. Garrison-on-
creation stays open with GAR-09 (M6). · *Status:* **MATCHES**.

**SEL-14 — Stop and Delete.** The Stop command halts the selected units'
current order immediately (they become idle; a villager keeps what it
carries). Delete destroys a selected own unit or building deliberately
(clients may gate it behind a confirmation setting).
*Source:* GAP 2.7, 2.8; wiki, Unit (Safe Delete option). · *Impl:*
`Command::Stop` clears task+order; `Command::Delete` needs the death/
removal path (CMB-14). · *Status:* **MISSING** (neither command exists).

**SEL-15 — Building selection is separate from unit selection.** Selecting
a building replaces the unit selection (and vice versa); the command panel
then shows that building's queue, technologies, rally and garrison
controls (INF-06).
*Source:* GAP 11.6. · *Impl:* shipped as client state (`selectedBuilding`
kept apart from the unit Set, TC clickable). Panel content grows with
M6 systems. · *Status:* **MATCHES** (mechanism); content partial.

**SEL-16 — Minimap orders.** On the minimap, left-click moves the camera;
right-click issues the smart order (SEL-11) at the corresponding world
point for the current selection.
*Source:* GAP 11.1. · *Impl:* shipped (M4) — `minimap.js` geometry
converts both directions; left-click/drag moves the camera, right-click
issues the smart order (SEL-11, via the same `smartOrderAt` the main
view uses — including rally-setting with a building selected, and the
Shift-queued form). · *Status:* **MATCHES**.

**SEL-17 — Auto-Explore (Auto Scout).** Any Scout Cavalry can be
toggled into Auto Scout: it then rides the same scouting pattern
the AI uses, exploring on its own, until the player issues it any
manual order (which takes priority and ends the mode) or toggles
the mode off (it stops where it stands). It never initiates
attacks on its own (the Scout's stance is Stand Ground in the Dark
Age, CMB-09). DE added a dedicated panel button on the scout's
command panel and, in current builds, a remappable Autoscout
hotkey (its documented default letter is **UNVERIFIED** from
public sources); Skirmish II binds the toggle to **G** by product
decision, in the units context only, after every existing G
binding (the armed villager eco-page alias and Ctrl+G go-to) has
resolved.
*Source:* Age of Empires official site, Update 35584 spotlight
("Auto Scouting … only available for your starting Scout unit(s),
not for trained Scouts … issuing a manual command will take
priority"); AoE wiki (Scout unit): the feature arrived in update
36202 restricted to the start-spawned scouting unit, and **since
update 111772 every scout unit has the option**; Update 169123
notes ("Normalized the Stop and Autoscout hotkeys behavior on
scouts") confirm a dedicated Autoscout hotkey exists in current
DE. Study nº 2 (video evidence) shows the dedicated panel button
and the scout riding the fog edge alone. · *Impl:* shipped
2026-10-20 — `Command::SetAutoExplore` + `push_auto_explore` ABI.
Eligibility shipped starting-scout-only (derived from spawn
order), matching DE 2020–2023, and was widened the same day on
Claudio's ruling ("tem que ser como no original") to **every
living Scout Cavalry owned by a player** — a kind+owner gate in
`can_auto_explore`, with a per-entity client mirror
(`entity_can_auto_explore`); the old spawn-order accessors
(`is_starting_scout`, `starting_scout_of`, the `starting_scout`
ABI) were removed. Each scout carries its own flag and target
and explores independently. The walker is a pure function of
hashed state: nearest unexplored cell of the owner's fog grid
(squared cell distance, ties to the lowest linear cell index),
existing A*/steering, re-pick on arrival or at the fog cadence
(15 ticks) when the target was explored meanwhile; any
Move/Attack/AttackBuilding/Gather/queued order cancels the mode
in the shared `apply_*` paths. Its state joins the hash only
while the mode is on (the Task::Repair pattern), so untouched
matches hash byte-identically to before. Client: an icon-only
toggle button with the G badge on the Scout Cavalry's command
panel (shown when any selected scout is eligible, pressed state
mirrors the sim flag) + the G key in the units context; the
order flows through the lockstep vocabulary (`auto_explore`).
· *Status:* **MATCHES** (eligibility — all scouts, per current
DE update 111772; frontier exploration; manual-order priority;
toggle-off stop); DE's hotkey default **UNVERIFIED**, G is our
product binding.

---

## 5. Movement, pathfinding & collision

**MOV-01 — Speed is a per-unit stat.** Every unit kind has a base speed in
tiles/sec (MECHANICS-STUDY §4.1: Villager 0.8, Knight line 1.35, Scout line
1.2→1.55, …). Speed modifiers are percentage patches from technologies
(TEC-06), never per-unit code.
*Source:* MECHANICS-STUDY §4.1. · *Impl:* a `speed` column in the unit
table replaces the single global `MAX_SPEED` (1.0 world units/tick) in
`steer.rs`; steering consumes the per-entity value. · *Status:* **DIFFERS**
(one global speed for all entities).

**MOV-02 — Paths are grid A\* with dynamic obstacles.** Movement paths are
computed on the tile grid and recomputed when the destination or the
obstacle set changes (buildings placed/destroyed, trees felled). The
search is fully deterministic: integer costs, fixed neighbor order, total
tie-breaking.
*Source:* OPENAGE-RESEARCH §2.1 (the reference engine: per-unit A*; five
implementations over the project's life). · *Impl:* shipped — `grid.rs`
A* (octile heuristic, no corner cutting, (f, h, cell) heap order) + the
dynamic building layer. Portal/flow-field scaling is a profiling-driven
option, explicitly not adopted preemptively (OPENAGE-RESEARCH §2.1). ·
*Status:* **MATCHES**.

**MOV-03 — No pushing (reference behavior).** Units never displace each
other by force. In the reference engine a unit that bumps into another
stops and repaths around it; crowding is managed behaviorally, not
physically.
*Source:* Pottinger, "Coordinated Unit Movement"; DE pathfinding talk
(2025), both via OPENAGE-RESEARCH §2.2. · *Impl:* **documented divergence**
— Skirmish II uses displacement-based separation (units shove each other
aside smoothly; even idle units are shovable, which prevents head-on
deadlocks found in M2 testing). The M3 hybrid narrows the gap: separation
for travel, suppressed while a melee unit is engaged (MOV-04). The
divergence and its reason live in `steer.rs` comments. · *Status:*
**DIFFERS** (deliberate; see OPENAGE-RESEARCH §4 flag 1).

**MOV-04 — Overlap is tolerated in combat and formations.** Perfect
non-overlap is impossible in a crowded RTS and the reference does not
attempt it: engaged melee units and formation members may overlap somewhat
while fighting or holding shape. In particular, a melee unit that is
attacking does not get pushed out of range by collision resolution.
*Source:* Pottinger via OPENAGE-RESEARCH §2.2; OPENAGE-RESEARCH §5.1 (the
M3 takeaway). · *Impl:* separation term is skipped for entities in the
engaged-attack state (M3 attack state machine); formation slots tolerate
overlap by construction. · *Status:* **MISSING** (no combat state yet;
separation currently applies to everyone always).

**MOV-05 — Collision radius is unit data.** Units occupy a small circle
for steering purposes; the documented typical radius is ≈0.2 tiles
(Karambit Warrior 0.15 is the famous exception). Per-unit numbers are
otherwise UNVERIFIED in public sources — 0.2 is the adopted default.
*Source:* MECHANICS-STUDY §4.2. · *Impl:* `radius` column in the unit
table, replacing the global SEPARATION_RADIUS (2.0 world units); the
separation distance becomes r₁ + r₂. · *Status:* **DIFFERS** (one global
separation radius, no per-unit data).

**MOV-06 — Groups travel at their slowest member's speed.** Units sharing
a move order form a temporary group: the group advances no faster than
its slowest member, so the army arrives together instead of stringing out
by speed.
*Source:* GAP 1.10; Pottinger (groups have shared state: destination,
arrival). · *Impl:* at order issue, stamp the order with a group id and a
speed cap = min over members; steering clamps to the cap. Requires the
order queue (SEL-12) data model. · *Status:* **MISSING** (every unit paths
and travels independently at full speed).

**MOV-07 — Formations assign slots by class layer.** A group ordered in
formation (Line, Box, Staggered, Flank) receives destination slots:
melee units in the front layers, ranged and siege behind, monks/siege at
the center-back of a box. Units path to their own slot and hold the shape
on arrival.
*Source:* GAP 2.10; wiki, Unit formation / Battle Formations. · *Impl:*
slot assignment is a pure function of (group composition in id order,
formation kind, destination, facing) computed at order time — deterministic
and hash-safe since slots are just per-unit targets. · *Status:*
**MISSING**.

**MOV-08 — Long moves collapse into column.** On moves longer than ~30
tiles, a formed group narrows into a file/column for travel and re-forms
near the destination.
*Source:* GAP 2.10. · *Impl:* a slot-layout switch keyed on order distance
in the same assignment function as MOV-07. · *Status:* **MISSING**.

**MOV-09 — Facing is one of 8 directions.** A unit has a facing (8-way);
it attacks and is rendered in its facing; melee engagement turns the unit
toward its target. (Reference sprites store 5 directions and mirror 3 —
a rendering fact that lives in SPRITE-PIPELINE.md, not a sim rule.)
*Source:* OPENAGE-RESEARCH §2.4. · *Impl:* facing is *derived* from
velocity/target each tick for rendering; if a rule ever needs it (none
does today), store it as a hashed byte. · *Status:* **DIFFERS** (facing is
implicit in the renderer's animation state, not modeled).

**MOV-10 — Work and attack trigger on range, not exact arrival.** A unit
begins its task when it comes within the task's range of the target, even
if steering holds it slightly off the exact point (crowding). Movement
orders complete within a small reach tolerance.
*Source:* Skirmish II M2 lesson (distance-triggered transitions prevent
separation deadlocks — `lib.rs` work_tick comments). · *Impl:* shipped
pattern: GATHER_RANGE / DEPOSIT_RANGE / BUILD_RANGE distance checks +
WAYPOINT_REACH tolerance. The reference behaves equivalently (units work
from adjacent positions). · *Status:* **MATCHES**.

**MOV-11 — Movement domains are separate grids.** Land, water (and air,
unused here) path on separate passability grids; a unit only ever paths
inside its own domain. Crossing domains happens only by transport (GAR-10)
or at a Dock.
*Source:* OPENAGE-RESEARCH §2.1 (one grid per movement type); GAP 6.10. ·
*Impl:* `Grid` gains a domain dimension when water lands (M7); the A*
itself is shared code. · *Status:* **MISSING** (single land grid; no
water).

**MOV-12 — Pathing failure means hold position.** If no path exists to an
order's target, the unit stays where it is (its work state retries on later
ticks where the task model defines a retry, e.g. returning to deposit).
A failed path never teleports, never partially applies.
*Source:* Skirmish II M1/M2 behavior, matching the reference's observable
behavior. · *Impl:* shipped — `set_move_order` clears the order on A*
failure; work states re-issue. · *Status:* **MATCHES**.

---

## 6. Economy & drop-off

**ECO-01 — Four resources, one stockpile per player.** Food, wood, gold,
and stone are integer stockpiles. A cost is deducted atomically when the
order is accepted; an unaffordable order is rejected whole — never
partially paid.
*Source:* GAP 3.1. · *Impl:* `stockpile` in `Sim`, hashed; costs checked in
command application and in `validate_build`. Today single-player: the
stockpile becomes per-player with MAP-12 ownership. · *Status:* **MATCHES**.

**ECO-02 — Gather rates are data per source.** Each resource source has a
base gather rate (MECHANICS-STUDY §6.1: wood 0.388, forage 0.310, gold
0.380, stone 0.359, farming 0.316–0.320 — exactly 0.4/s for 7+ farmers on
one farm line, see ECO-14 — shore fish 0.426, hunt 0.408, herdable 0.330,
fishing ship 0.280/0.487 per second). Technology patches scale rates
(TEC-06); nothing about rates lives in code.
*Source:* MECHANICS-STUDY §6.1 (aoe2.net overlay rates, cross-checked). ·
*Impl:* shipped — `econ.rs` stores the rates as pre-rounded fixed-point
per-tick constants; the sim *is* the cross-check (59/59 tests). ·
*Status:* **MATCHES** (land rates; fishing/hunt rates land with their
systems).

**ECO-03 — Carry capacity 10 (hunted meat 35).** A villager carries at
most 10 resources at once before returning to deposit — except food from
hunted animals, of which they carry 35. Carry technologies raise the cap
(Wheelbarrow → 13, Hand Cart → 20) alongside their speed effects.
*Source:* MECHANICS-STUDY §2.4, §6.2 (hunt exception + Wheelbarrow +25% /
Hand Cart +50% capacity figures). · *Impl:* `CARRY_CAPACITY` per task
source + per-player cap patch; the 35 case keys off the node kind (hunt
carcass), not a villager state. · *Status:* **DIFFERS** (the flat 10 and
the hunt 35 both ship — M3; the carry-cap technologies do not exist
yet).

**ECO-04 — The gather loop.** Gather until the carry is full or the node
empties, travel to a drop-off, deposit instantly, return to the node, and
repeat — forever, until re-tasked. If the node empties mid-work, the
villager first hauls what it carries home, then applies ECO-09.
*Source:* GAP 3.2. · *Impl:* shipped — the ToNode → Gathering → Returning
phase machine in `lib.rs`. · *Status:* **MATCHES** (single-node version;
see ECO-09).

**ECO-05 — Deposit is instant on arrival.** Resources are credited the
tick the villager comes within deposit range of a valid drop-off building
(ECO-06). There is no deposit duration and no partial deposit.
*Source:* GAP 3.2. · *Impl:* shipped (`DEPOSIT_RANGE` check in the
Returning phase; since M6 measured to the Town Center's center or to a
camp's footprint edge — a camp's center is unreachable inside its own
solid footprint). · *Status:* **MATCHES**.

**ECO-06 — Drop-off buildings are resource-specific.** The Town Center
accepts all four resources. The Mill accepts food; the Lumber Camp wood;
the Mining Camp gold and stone; the Dock accepts a Fishing Ship's food. A
gatherer returns to the nearest valid drop-off *for the resource it is
carrying* (nearest by travel distance). Building a closer camp is the
core economy optimization of the game.
*Source:* GAP 3.2; MECHANICS-STUDY §5. · *Impl:* shipped (M6) — the
accepts relation is per-kind data on the building table; the Returning
phase re-resolves the nearest valid drop-off for the carried resource
every tick (the Town Center included in the same comparison), so a
camp completing mid-haul reroutes the trip. "Nearest" is straight-line
distance, not path distance — a recorded simplification. The Dock
clause awaits water (MAP-08). · *Status:* **MATCHES**.

**ECO-07 — Force drop-off.** The player can order carrying villagers to
deposit immediately (at their nearest valid drop-off); afterwards they
resume the task they were on.
*Source:* GAP 3.13. · *Impl:* shipped (M6) — the `ForceDeposit`
command flips a carrying worker's phase to Returning; the resume
marker is the kept gather assignment (or construction site), which
the deposit step resumes on its own. A builder who finishes a camp
also deposits its carried load there on the spot when the camp
accepts it (OPENING-SPEC §2.4). · *Status:* **MATCHES**.

**ECO-08 — Depletion frees the world.** A node at zero is removed: trees
stop blocking (MAP-04), berry bushes/mines/farms disappear, carcasses
vanish. A farm building's record retires with its node, freeing the plot.
*Source:* GAP 3.3. · *Impl:* shipped for nodes and farms (`source_node`
cleanup in `lib.rs`); tree un-blocking joins MAP-04. · *Status:*
**MATCHES** (land nodes).

**ECO-09 — Gatherers retarget to a same-type neighbor.** When a
gatherer's node depletes and it carries nothing (or after depositing,
ECO-04), it seeks the nearest node of the *same resource* within a short
search radius instead of going idle. (The exact radius is UNVERIFIED in
public sources; it is a tuned constant, documented in the data table.)
*Source:* GAP 3.2 end; DE Villager behavior. · *Impl:* shipped (M4) — a
bounded radial search over the node set in id order at depletion time
(deterministic; nearest wins, ties to the lowest id). The search is
centered on the *depleted node* and matches on the node's **resource**
(carcasses included — they are Food nodes like any other; the retarget
only ever happens empty-handed or right after a deposit, so the hunt
carry cap can never leak into a half-filled load). Radius:
`econ::RETARGET_RADIUS` = 12 world units (3 cells) — the tuned constant,
UNVERIFIED as above. Discovery points: the gather step noticing the
depletion, the trip home (deposit resolves it — ECO-04's case), and the
walk to a node that died en route. · *Status:* **MATCHES**.

**ECO-10 — Node amounts.** Tree 100, Berry Bush 125, Gold Mine 800, Stone
Mine 350 (MECHANICS-STUDY §6.3, full table there).
*Source:* MECHANICS-STUDY §6.3. · *Impl:* shipped — `econ::node_amount`;
the initial layout uses the same table. · *Status:* **MATCHES**.

**ECO-11 — Hunting turns animals into carcass nodes.** Killing a huntable
animal converts it into a carcass holding its food value (Sheep 100,
Cow 150, Deer 140, Boar 340, and the full animal table in MECHANICS-STUDY
§3.10/§6.3). Villagers gather from the carcass at the hunt rate and carry
35 per trip (ECO-03). Animal combat behavior is AI-07.
*Source:* GAP 3.4; wiki, Hunt / Early Economy. · *Impl:* shipped (M3) —
animals are entities with an HP field; on death a land animal spawns a
carcass node keyed to its position (Sheep 100, Deer 140, Boar 340 from
the study table; the Cow has no instance — no cow exists). · *Status:*
**MATCHES**.

**ECO-12 — Carcasses decay.** Food in an unworked carcass is lost
continuously — approximately 0.25 food per second (e.g. a Sheep worked
slowly loses ≈17 food over one villager's two round trips). This is why
the reference play pattern kills and works one animal at a time.
(Exact rate UNVERIFIED from a primary source; the figure is implied by
published worked examples — treat as a tuned constant pending a better
source.)
*Source:* wiki, Early Economy (decay stated explicitly); StrategyWiki
tactics (worked examples implying ≈0.25/s). · *Impl:* shipped (M3) —
carcasses are the only nodes with a per-tick amount decrease
(fixed-point decrement in the node phase); the rates are pinned per
carcass family by tests: herdable carcasses exactly 0.25/s (273
bits/tick), hunt carcasses 0.40/s (437 bits/tick, MECHANICS-STUDY
§6.3). · *Status:* **MATCHES**.

**ECO-13 — Herdables are captured, not gathered alive.** Sheep and similar
animals become units of the player whose unit comes near them (AI-06);
the owner moves them like units and kills them with villagers to create
carcasses (ECO-11), worked at the shepherd rate 0.330/s.
*Source:* wiki, Sheep / Herdable animal; MECHANICS-STUDY §6.1. · *Impl:*
shipped (M3) — ownership transfers by proximity in the combat phase's
capture step; killing is an ordinary attack order against an own
animal, and the killer auto-harvests the carcass (worked at the
0.330/s shepherd rate for herd carcasses). Extended 2026-10-20 to the
full AI-06 rule (majority conversion for wild sheep, steal of an
unguarded owned sheep, Gaia wander; client owner-colour band and
information selection for animals that are not yours). · *Status:*
**MATCHES**.

**ECO-14 — Farms.** A Farm is a 3×3 building holding 175 food. Only one
villager works a farm; the farm's own inherent work rate (0.4 food/s)
caps how fast it can be harvested no matter who works it. Farm-food
technologies (Horse Collar +75, Heavy Plow +125, Crop Rotation +175)
raise the amount only for farms built — or merely started — after the
research completes; a standing farm never gains retroactively. A depleted
farm's plot is reusable immediately. The Mill keeps a paid reseed queue
and (DE) an auto-reseed toggle.
*Source:* MECHANICS-STUDY §6.4 (amounts/timing pinned); GAP 3.9. · *Impl:*
amount is stamped onto the node at farm creation from the owner's current
tech state (a stored value, not a live lookup — that's what makes
non-retroactivity natural); work-rate cap clamps the gather rate in the
farm case. Today: farm exists with 175 food and the retire-on-deplete
flow; footprint is 2×2 cells (BLD-12 note) and there are no farm techs
yet. · *Status:* **DIFFERS** (core farm loop matches; footprint, cap, and
tech rules pending M6/M7 conventions).

**ECO-15 — Fishing.** Villagers gather shore fish at 0.426/s. Fishing
Ships gather from fish spots (0.280/s shore, 0.487/s deep-sea) and deposit
at a Dock (ECO-06). Fish Traps are floating farm-analogs built by Fishing
Ships on water (food amount UNVERIFIED in the sources consulted).
*Source:* MECHANICS-STUDY §6.1; GAP 3.8. · *Impl:* needs water (MAP-08),
ship units, and the Dock drop-off mask; the gather phase machine is
reused verbatim for ships. · *Status:* **MISSING**.

**ECO-16 — Relics trickle gold.** A relic garrisoned in a Monastery
generates 0.5 gold per second (30/min) for its owner, continuously, until
removed. (At that rate a monk bought for one relic pays back its 100 gold
in 200 s.)
*Source:* wiki, Relic (Age of Empires II). · *Impl:* a per-tick fixed-
point accumulator on the monastery building record (MNK-09). · *Status:*
**MISSING**.

**ECO-17 — Starting stockpile.** A standard match starts at 200 food /
200 wood / 100 gold / 200 stone, with the starting unit package specified
in [`OPENING-SPEC.md`](OPENING-SPEC.md).
*Source:* MECHANICS-STUDY §9.5; GAP 3.1. · *Impl:* shipped constants in
`Sim::new`. · *Status:* **MATCHES**.

---

## 7. Buildings & placement

**BLD-01 — Placement is validated atomically.** A build order is accepted
only if the whole footprint is inside the map, on allowed terrain,
unobstructed (no building overlap, no live node, no static wall), and at
least one adjacent ring cell is reachable by a builder. A rejected order
changes nothing and charges nothing.
*Source:* GAP 4.1; Skirmish II M2.5 spec (the reference validates the same
way observably). · *Impl:* shipped — `validate_build` in `build.rs`,
called from `push_build` (the ABI returns its verdict) and re-checked in
`apply_command`; ordering onto an incomplete same-kind building resumes
it instead of double-charging. Since M6 the same function also
evaluates the availability gates (BLD-15). · *Status:* **MATCHES**.

**BLD-02 — Cost is paid at placement; foundation cancel refunds fully.**
Laying a foundation deducts the full cost immediately. Cancelling a
foundation before completion refunds 100%; a completed building deleted
later refunds nothing (SEL-14).
*Source:* GAP 4.1. · *Impl:* payment shipped; cancellation refunds when
the building-removal path lands with delete (M5/M6). · *Status:*
**DIFFERS** (payment yes; no cancel/delete exists yet).

**BLD-03 — Construction needs a builder present.** A foundation makes
progress only while at least one assigned villager is in work range of
the site. The building is completely non-functional (no production, no
garrison, no support, no LoS, no attack) until progress reaches 100%.
*Source:* GAP 4.18. · *Impl:* shipped — builders in the Building phase
advance `progress_ticks`; completion flips `complete` and applies the
building's live effects (houses raise the cap the tick they finish). ·
*Status:* **MATCHES**.

**BLD-04 — Multiple builders: diminishing returns.** With *n* builders,
total construction time is 3T/(n+2) of the solo time T — i.e. the combined
build rate scales as (n+2)/3, not n. (2 builders: 0.75×T — the 25 s House
takes ≈19 s; 4 builders: 0.5×T.)
*Source:* AoK Heaven, "Benefits of Multiple Builders" (worked derivation
and measurements); consistent with published opening timings. · *Impl:*
the Building phase computes the tick increment from the count of builders
in range as an (n+2)/3-scaled fixed-point rate. **Implemented
2026-10-16 (main `599132f`)**: progress accrues in thirds of a tick —
n builders add exactly (n+2) thirds per tick (`build::build_rate_thirds`),
so 2 builders finish the 25 s House in 1125 work-ticks (18.75 s) and solo
timing is unchanged. · *Status:* **MATCHES**.

**BLD-05 — A building's HP grows with construction.** A building under
construction has HP proportional to its progress; it reaches full HP
exactly at completion. Foundations can be attacked and destroyed (CMB-15)
and can never be converted (MNK-08). (The exact starting fraction is
UNVERIFIED; proportional growth is the documented behavior.)
*Source:* GAP 4.19. · *Impl:* buildings have HP since M5 and can be
attacked and destroyed at any stage — but an under-construction
building carries its *full* HP from placement, not a
progress-proportional value: a recorded v1 simplification.
· *Status:* **DIFFERS**.

**BLD-06 — Footprints are data.** Every building kind has a footprint in
tiles (TC 4×4, House 2×2, Farm 3×3, Tower 1×1, Castle 4×4,
MECHANICS-STUDY §5). The footprint blocks pathing (except farms, BLD-12)
and defines placement validity (BLD-01).
*Source:* MECHANICS-STUDY §5. · *Impl:* footprint columns in the building
table; the full set landed with M6 (2×2 House/Farm/camps, 3×3
Barracks/Range/Stable/Blacksmith). Two divergences remain: the Farm
uses a 2×2-cell footprint where the reference's is 3×3 tiles (ECO-14),
and the TC is an entity rather than a placed building: its 4×4
footprint is placement-reserved but, per BLD-16, passable to units
instead of blocking pathing. · *Status:* **DIFFERS**.

**BLD-07 — Repair.** Villagers repair damaged buildings, ships, and siege.
Restoring a building from 1 HP to full costs 50% of its original build
cost, charged continuously in proportion to HP restored (half the HP =
25% of build cost); if the stockpile runs dry, repairers stop and stand
idle. Repair rate (buildings): the first villager restores 750 HP/min;
each additional villager adds 375 HP/min. Ships and siege repair at 25%
of those rates. Town Center exception: a full TC repair costs 550 wood
and no stone, whatever the formula says. Treadmill Crane does not speed
repair.
*Source:* wiki, Villager (Age of Empires II) — repair section (rates,
cost model, TC exception). · *Impl:* shipped 2026-10-19 for placed
buildings (see BLD-17 for the implementation rule): a villager task
phase (`Task::Repair`) mirroring construction's assignment and
approach; per-tick HP restore at 750 HP/min for the first repairer
plus 375 HP/min per additional one (`REPAIR_HP_PER_MIN_FIRST` /
`REPAIR_HP_PER_MIN_EXTRA` in `build.rs`), with a proportional wood
drip (`REPAIR_COST_FRACTION` = 50% of the build cost over a full
repair; our buildings cost wood only) — both accumulated in
fixed point on the building record so the totals hold exactly;
repairers stop and stand idle when a tick's drip is unaffordable.
Ships and siege do not exist yet; the Town Center is an entity in
our architecture, not a placed building, so TC repair and its
550-wood exception are out of scope. · *Status:* **DIFFERS**
(placed buildings match; ships/siege and the TC exception not
implemented).

**BLD-08 — Population support.** Completed buildings add population room:
House +5, Town Center +5, Castle +20. Losing the building removes the
room; units already alive are never killed by losing support (BLD-10).
*Source:* MECHANICS-STUDY §5, §9.4; GAP 4.17. · *Impl:* shipped —
`population` derived from entities vs. `population_cap` derived from
completed buildings each time it changes. · *Status:* **MATCHES**.

**BLD-09 — Production queues are per building.** Each production building
(including the TC) holds its own queue of units/technologies in one
shared FIFO (TEC-05), capped at 15 entries. Cost is paid on enqueue;
cancelling an entry refunds 100% (TEC-05).
*Source:* GAP 4.10; Skirmish II TC behavior (queue cap 15 shipped). · *Impl:*
shipped — every production building (TC, Barracks, Archery Range,
Stable) and research host (Blacksmith, Barracks) holds the same typed
queue record (`ProdEntry`: unit | technology), cap 15, since M6. The
cancel/refund half lives in TEC-05 and is still open. · *Status:*
**MATCHES**.

**BLD-10 — The housed rule.** Units may be *queued* at the population
cap, but production progress stalls while population ≥ cap and resumes
automatically when room frees (a death, or new support completing). The
stall is not an error state — it is the rule that makes the two-builder
opening house urgent (see OPENING-SPEC).
*Source:* MECHANICS-STUDY §9.4. · *Impl:* shipped —
`train_blocked_housed()` freezes progress without dropping the queue. ·
*Status:* **MATCHES**.

**BLD-11 — Building stats rise with Age automatically.** A building's HP
(and, for later ages, armor) follows its owner's Age column in the data
table with no research and no player action: e.g. House 550 HP (Dark) →
750 (Feudal) → 900 (Castle/Imperial).
*Source:* MECHANICS-STUDY §5 preamble. · *Impl:* ages are per-player
state since M6 and building HP exists since M5 — but the step-up is
not implemented: a building keeps its base row at every age (the
Range/Stable rows are Feudal-keyed only in that the buildings
themselves are Feudal). · *Status:* **DIFFERS**.

**BLD-12 — Farms do not block; other buildings do.** A placed farm never
enters the obstacle grid (its farmer walks and stands on it). Every other
completed *or* in-progress building blocks its footprint from placement
(the foundation reserves the ground) — in our implementation houses
already reserve from placement, which is the reference behavior.
*Source:* GAP 4.1; Skirmish II M2.5. · *Impl:* shipped — houses write the
dynamic grid layer, farms never do. · *Status:* **MATCHES**.

**BLD-13 — Walls and gates.** Wall segments are solid to everyone. A gate
opens automatically for its owner and their allies and never for enemies;
units pass only while it is open. Walls are placed by dragging a line of
segments (a client gesture producing many build orders).
*Source:* GAP 4.12, 4.13. · *Impl:* gate state derives from (owner,
proximity of passable units) at path time — the pathing grid treats gates
as conditionally blocked per querying player's diplomacy (M8 diplomacy
table). · *Status:* **MISSING**.

**BLD-14 — Docks straddle the shoreline.** A Dock's footprint must cover
both land and water tiles. It is the production building for ships and
the drop-off for fishing (ECO-15).
*Source:* GAP 4.14, 6.10. · *Impl:* a placement predicate in the building
table ("shoreline") evaluated by `validate_build` against the terrain
layer (M7). · *Status:* **MISSING**.

**BLD-15 — Buildings are gated by age and prerequisites.** Each building
kind has an available Age and, where applicable, a prerequisite building
(Mill → Market; Barracks → Blacksmith/Stable chain…, full data in
MECHANICS-STUDY §5 notes). The command panel offers only what is
available (INF-06).
*Source:* GAP 4.2–4.9; MECHANICS-STUDY §5. · *Impl:* shipped for the
in-game building set (M6) — availability = (age, built-set) evaluated
in `validate_build`, the same function the client's ghost and build
menu mirror: Farms require a completed Mill; the Archery Range,
Stable and Blacksmith require the Feudal Age; the Range and Stable
also require a completed Barracks. Buildings not yet in the game
(Market, Monastery, …) arrive with their milestones. · *Status:*
**MATCHES**.

**BLD-16 — The Town Center's footprint is passable but reserved.**
The Town Center occupies a 4×4 footprint (Fandom building table:
Town Center size 4×4). Unlike every other building's footprint it
does NOT obstruct movement: units path and walk through it on all
sides, under its canopy roofs — passability is a documented
special property of the Town Center, and the reason its
architecture is a compound around open ground rather than a solid
block. Its ground is nevertheless reserved: no building footprint
may overlap any of a living Town Center's 16 cells, either
player's, so the TC's tiles stay free of construction. Town
Centers require flat terrain (the map generator flattens and dries
both start areas). Other buildings block and reserve normally
(BLD-06, BLD-12).
*Source:* Fandom building table (Town Center 4×4); GAP 4.1. ·
*Impl:* shipped 2026-10-19 — the TC stays an entity that never
enters the nav blocking grid (movement, center-distance deposit
and training spawn unchanged); `validate_build` reserves each
living TC's 4×4 cells (anchor = the TC's cell − 1 on each axis)
against placement, both players'; pinned by
`sim/tests/tc_footprint.rs` (crossing, deposit, rejection,
adjacency). The client's selection/hit-test box matches the 4×4
(TC_HALF = 8 world units). · *Status:* **MATCHES**.

**BLD-17 — Repair as implemented (the BLD-07 mechanics in the
sim).** A villager ordered to repair travels to the building's
ring exactly like a builder (same approach, same in-range test)
and then restores HP at the flat documented rate: 750 HP/min for
the first repairer, +375 HP/min for each additional one —
375·(n + 1) HP/min in total, independent of the building's build
time and max HP (repair is NOT construction run in reverse, and
does not use the multi-builder formula of BLD-04). The cost is
`REPAIR_COST_FRACTION` = 50% of the building's build cost for a
full 0→100% repair, charged to the owner's stockpile as a
continuous drip proportional to the HP restored each tick, for
HP actually restored only (restoration caps exactly at max HP;
completion releases every repairer). Valid targets are only the
repairer's own player's *complete, damaged* placed buildings:
enemy and Gaia buildings, undamaged/full buildings, and
foundations under construction are ignored (a foundation is
resumed via Build); the scripted AI never issues repair. If the
owner cannot cover a tick's drip, that tick restores nothing and
the building's repairers stop and stand idle. A repairer pulled
away (new order, death) or whose building is destroyed leaves
the task cleanly, as builders do; the queued (Shift+click) form
exists and re-validates at execution.
*Source:* BLD-07 (Fandom Villager page — rates, cost model,
stop-on-empty behavior). · *Impl:* shipped 2026-10-19 —
`Command::Repair` + `QueuedOrder::Repair`, the `Task::Repair`
phase carrying its building (hashed like the construction
assignment), `repair_tick` immediately after `construction_tick`,
fixed-point HP and cost accumulators on the building record
(`repair_hp_remainder` / `repair_cost_remainder`, both hashed);
pinned by `sim/tests/repair.rs` (rate, stacking, drip total,
rejections, wood-exhaustion stop, queued form). Scope divergence
from BLD-07: placed buildings only — ships and siege do not
exist yet, and the Town Center is an entity in our architecture
rather than a placed building, so TC repair (and its 550-wood
exception) is out of scope, not implemented. The golden hash is
unchanged: no field is hashed for a state with no repair in
flight and no buildings standing. · *Status:* **DIFFERS**
(placed-building rate, cost and stop behavior match the
documented values; scope is buildings-only, pending
verification of the in-game feel against the reference).

---

## 8. Combat & damage

**CMB-01 — The damage formula.** When an attack lands, damage is computed
per armor class and summed, then multiplied once by the elevation factor,
then clamped to a minimum of 1:

> damage = max(1, k_elev × Σ_classes max(0, attack_class − armor_class))

Only classes the attacker actually has an attack value in participate
(CLS-02, CMB-03). Bonus attacks against classes the target belongs to are
just more terms in the sum.
*Source:* MECHANICS-STUDY §1.1, §1.2 (formula pinned, with worked
examples). · *Impl:* shipped (M3) — `compute_damage` in `combat.rs` over
(class → value) slices from the unit tables, exactly this shape; the
elevation factor landed with M7 (`compute_damage_elev`): k = 1.25 /
0.75 / 1 by the sign of the height difference, applied to the
armor-subtracted sum before the minimum-1 clamp; a projectile carries
its shooter's elevation from release (PRJ-10). · *Status:*
**MATCHES**.

**CMB-02 — HP is fractional; death at HP < 1.** Entities carry fractional
hit points (fixed-point). A unit dies the tick its HP drops below 1 — not
at 0 — and displayed HP is the floored value.
*Source:* MECHANICS-STUDY §1.1. · *Impl:* shipped (M3) — `hp: Fx` on
the entity model; death processed in a single sweep at the end of the
combat phase (ascending id order) so simultaneous kills are
deterministic. · *Status:* **MATCHES**.

**CMB-03 — Non-member classes contribute exactly zero.** If a target does
not belong to an armor class, that class's bonus deals 0 — implemented as
a membership test, never as a sentinel armor value (a sentinel would let
negative-armor cases leak damage).
*Source:* MECHANICS-STUDY §1.2, §10.4 (ambiguity #1, resolved). · *Impl:*
shipped (M3) — class membership is a bitset per entity (CLS-01); the
formula iterates the attacker's attack list and tests membership, so
non-member classes contribute exactly zero by construction.
· *Status:* **MATCHES**.

**CMB-04 — Attack delay (frame delay).** The first attack of an
engagement lands only after the attacker's attack-delay ticks (Archer
≈0.35 s, melee cavalry ≈0.68 s, Militia/Spearman ≈0.50 s, full table in
MECHANICS-STUDY §2.2). If the unit is re-tasked during the delay, the
pending attack is aborted, not stored. Later attacks in the same
engagement are governed by reload alone (CMB-05).
*Source:* MECHANICS-STUDY §2.2. · *Impl:* shipped (M3) — the delay is a
column in the unit table; a re-task clears the combat state, aborting
the pending strike. · *Status:* **MATCHES**.

**CMB-05 — Reload (rate of fire).** After an attack lands, the next
attack is available after the unit's reload time (modal value 2.0 s;
Spearman/Skirmisher 3.0 s; Knight 1.8 s; Ram 5.0 s; Mangonel 6.0 s;
Trebuchet 10.0 s — MECHANICS-STUDY §2.1). Reload starts at the landing
tick.
*Source:* MECHANICS-STUDY §2.1. · *Impl:* shipped (M3) — per-entity
attack clock in the combat state; all times authored in ticks (1.8 s =
108 ticks exactly). · *Status:* **MATCHES**.

**CMB-06 — Melee lands at range at the landing tick.** A melee attack
deals its damage only if the target is still within the attacker's melee
range (0 + unit radii) when the delay completes; otherwise the attack
whiffs and the reload still applies. Melee units therefore chase: the
attack state machine keeps them moving into range between attacks.
*Source:* MECHANICS-STUDY §2.2 (abort semantics); observed reference
behavior. · *Impl:* shipped (M3) — range check at the landing tick in
`combat.rs`; the chase is the attack state machine's move-toward-target
step (an internal order, not a player order, not queued). · *Status:*
**MATCHES**.

**CMB-07 — Kiting asymmetry is emergent, not coded.** Fast-reload,
fast-move ranged units beat slow melee by repositioning between shots
purely from CMB-04/05 + MOV-01. No rule may special-case "kiting".
*Source:* MECHANICS-STUDY §2 (analysis). · *Impl:* nothing is built —
no code path special-cases kiting; ranged/melee outcomes emerge from
CMB-04/05 + MOV-01 and the tables. · *Status:* **MATCHES**.

**CMB-08 — Auto-targeting priority.** A military unit with no explicit
target acquires, in order: (1) the closest attackable enemy *unit*;
(2) if none, an enemy Town Center in line of sight; (3) if none, other
enemy buildings. Rams violate (1): they target buildings only, even when
attacked by units. Trebuchets target buildings and other Trebuchets.
Farms, walls, gates, and herdables are never auto-targeted; killing sheep
is always an explicit order (ECO-13).
*Source:* MECHANICS-STUDY §9.2 (priority chain pinned). · *Impl:* a single
acquisition function in `combat.rs`; ties by squared distance then lowest
entity id — never sqrt, never float. Shipped narrower than the chain
(M3/M5): an idle Militia, Man-at-Arms or Archer with no order acquires
the closest *unit* in line of sight — a boar, or an enemy player's
unit (the enemy Town Center expressly excluded). Buildings and Town
Centers are never auto-acquired; razing is always an explicit order
(or the AI's wave orders, AI-04). · *Status:* **DIFFERS**.

**CMB-09 — Stances govern pursuit, not damage.** Four stances:
*Aggressive* — pursue targets indefinitely. *Defensive* — engage nearby
enemies, chase only within a leash radius of the unit's post, then return
to post and heal nothing. *Stand Ground* — attack only targets already in
range; never move to engage. *No Attack* — never attack (still flees
nothing; just stands). Defaults: military units Aggressive; the starting
Scout is Stand Ground. (Exact leash distances are UNVERIFIED anywhere
public — they are adopted tuning constants, recorded in the data table as
design decisions, not reference facts.)
*Source:* MECHANICS-STUDY §9.1; GAP 2.6. · *Impl:* no stance state
exists yet. Two defaults are observable today: the starting Scout never
auto-acquires (its Stand-Ground default), and other military
auto-acquire only boars (a narrow Aggressive). · *Status:* **DIFFERS**.

**CMB-10 — Attack-move.** Ordered to a point, units move there engaging
every enemy encountered on the way (per stance acquisition, CMB-08),
then hold at the destination.
*Source:* GAP 2.4. Its exact reference semantics are UNVERIFIED in public
documentation (MECHANICS-STUDY §9.1) — Skirmish II adopts the conventional
definition stated here as a design decision. · *Impl:* an order-queue
entry (SEL-12) flagging the move as engage-en-route; combat state handles
the rest. · *Status:* **MISSING**.

**CMB-11 — Patrol.** A patrol order loops the unit between two (or a
chain of) points indefinitely, engaging enemies it encounters like
attack-move, and returns to the patrol path after each engagement.
*Source:* MECHANICS-STUDY §9.1; GAP 2.5. · *Impl:* a cycling waypoint list
in the order record. · *Status:* **MISSING**.

**CMB-12 — Friendly fire only where flagged.** Ordinary attacks never
damage friendly units. Attacks flagged as blast in the data (Mangonel
line, Bombard Cannons, Cannon Galleons, Warwolf Trebuchets) damage
*everyone* in the blast, friend or foe — Scorpions are notably not
flagged.
*Source:* MECHANICS-STUDY §2.3. · *Impl:* shipped for the ordinary half
(M3) — no attack damages a friendly unit, mishits included (PRJ
resolution only ever selects non-friendly bystanders). No blast-flagged
weapon exists yet. · *Status:* **MATCHES** (for the shipped weapon set).

**CMB-13 — Mishits deal half.** A projectile that strikes a unit other
than its intended target deals 50% damage to that unit (certain units are
exempt by data flag — e.g. rams take structured reduced projectile damage
via their armor row instead; see MECHANICS-STUDY §1.3 ram armor).
*Source:* MECHANICS-STUDY §2.3. · *Impl:* shipped (M3) — projectile
impact resolution (PRJ-01) passes a ½ damage multiplier into CMB-01 for
non-intended targets; the exemption is a data flag. · *Status:*
**MATCHES**.

**CMB-14 — Death frees everything.** On death, a unit's population is
freed the same tick, its orders and carried resources vanish with it (a
killed villager's load is lost, not dropped), and any garrison inside it
(transport, ram) is handled by the container rules (GAR-03 analog: ram
passengers eject; transport cargo dies with the ship — GAR-10). The
corpse is a client-side cosmetic with a decay timer, not sim state.
*Source:* MECHANICS-STUDY §9.4 (population timing); GAP 6.1. · *Impl:*
the death sweep (CMB-02) frees population, orders and carry the same
tick — but the entity record stays as a *tombstone* (ids are never
reused; the record is hashed state) instead of being removed from the
Vec: a deliberate deviation from this rule's letter, recorded in
ARCHITECTURE.md. Garrison effects have no instance (no garrison).
· *Status:* **DIFFERS**.

**CMB-15 — Buildings burn, then free their ground.** A destroyed building
is removed, its footprint freed for pathing and placement, its queue
cancelled (no refund — the buildings are gone), its garrison ejected
alive (GAR-03), and its support (BLD-08) withdrawn. Technologies it
already researched persist for the owner.
*Source:* GAP 4.19–4.20; GAR-03. · *Impl:* shipped (M5) — destruction
removes the record, frees the footprint cells, drops the queue without
refund, withdraws support, and leaves the owner's researched
technologies untouched. Garrison ejection has no instance (no
garrison exists). · *Status:* **MATCHES**.

**CMB-16 — Attack ground.** Certain siege units can be ordered to attack
a *point*: projectiles land there with their blast (PRJ-07) whether or
not a unit stands there. This is also how Onager-line units fell trees
(MAP-04).
*Source:* GAP 2.3; wiki, Attack (Mangonel-line behavior). · *Impl:* an
attack order whose target is a position rather than an entity id, in the
same order record (tagged union). · *Status:* **MISSING**.

---

## 9. Projectiles & ballistics

**PRJ-01 — Projectiles are simulated entities with fixed trajectories.**
An arrow/bolt/stone is spawned at release with a fixed target point and a
precomputed flight time; it travels ballistically and resolves on arrival.
There is no homing and no mid-flight retargeting.
*Source:* MECHANICS-STUDY §2.3, §10.3 (flight time precomputed in ticks at
spawn). · *Impl:* shipped (M3) — a projectile list in sim state (hashed),
each record (spawn tick, impact tick, origin, target point, weapon row,
intended target id); iteration in spawn order after entity-id order
(MECHANICS-STUDY §10.3). · *Status:* **MATCHES**.

**PRJ-02 — Flight time comes from distance ÷ projectile speed at spawn.**
Each weapon has a projectile-speed stat (documented for individuals only:
Mangonel stone 3.5 tiles/s, Janissary bullet 7.5 — the generic arrow speed
is UNVERIFIED; it is adopted as a per-weapon data constant). Elevation
difference does not change flight time in the documented model.
*Source:* MECHANICS-STUDY §10.4 (#12). · *Impl:* shipped (M3) — ceiling
division at spawn; the value lives in the weapon row. The generic arrow
speed is an adopted constant (UNVERIFIED, as this rule records).
· *Status:* **DIFFERS** (mechanism shipped; the arrow-speed constant is
adopted, not documented).

**PRJ-03 — Accuracy is one roll per shot, at release.** A ranged attack
rolls once against the attacker's accuracy (Archer 80/85/90% by tier,
Skirmisher 90%, Cavalry Archer 50%, Hand Cannoneer 75%, Thumb Ring →
100% for archers — MECHANICS-STUDY §2.3) to decide whether the projectile
flies true to its aim point or scatters (PRJ-04). The roll is drawn from
the sim RNG in fixed iteration order (ascending attacker id, then
projectile spawn order) — this is the first gameplay RNG consumer and the
reason SIM-04 exists.
*Source:* MECHANICS-STUDY §2.3, §10.3. · *Impl:* shipped (M3) — the draw
happens in the combat phase inside `tick()`, never at command time
(commands may arrive in any real-time order across clients; the draw
must not). · *Status:* **MATCHES**.

**PRJ-04 — Misses scatter around the aim point.** A failed accuracy roll
(or a target that moved away, PRJ-06) puts the impact at a scattered
point within the weapon's dispersion radius of the aim point. Dispersion
radii are per-weapon data and are UNVERIFIED in public sources (a
prototype program exists to measure the reference — MECHANICS-STUDY
§10.4 #11); adopt tuned constants and record them as design values.
*Source:* MECHANICS-STUDY §2.3, §10.4. · *Impl:* shipped (M3) — scatter
direction and distance from two further RNG draws in the same fixed
order (PRJ-03); fixed-point integer vector math — never float sin/cos
in the sim. The dispersion radius is an adopted constant (UNVERIFIED,
as this rule records). · *Status:* **DIFFERS**.

**PRJ-05 — Ballistics leads the target; without it, aim is at the
release-time position.** Aiming is decided once, at release: with the
Ballistics technology, the aim point is the target's position led by its
current velocity over the flight time; without it, the aim point is where
the target stood at release. There is no other target leading.
*Source:* MECHANICS-STUDY §2.3. · *Impl:* the no-Ballistics half ships
(M3) — aim is fixed at the target's release-time position, always. The
Ballistics technology does not exist yet. · *Status:* **DIFFERS**.

**PRJ-06 — Moving targets dodge geometrically.** A hit on a moving target
is never re-rolled: if the target has left the impact point (true or
scattered) when the projectile arrives, the projectile hits ground (or a
bystander, CMB-13) instead. This is what makes staggered formations and
zig-zag micro work, and it applies even to 100%-accuracy Thumb Ring
archers.
*Source:* MECHANICS-STUDY §2.3 (explicit: accuracy % is measured against a
stationary target). · *Impl:* shipped (M3) — impact resolution tests
"intended target within its radius of the impact point" — nothing else.
· *Status:* **MATCHES**.

**PRJ-07 — Blast damage falls off and rolls per unit.** Weapons with a
blast radius damage every unit in the radius at impact, at the weapon's
blast fraction of full damage (Mangonel-line: distance-tapered from 100%
at center; War Elephant 50%; Armored Elephant 33%; Battle Elephant 25% —
MECHANICS-STUDY §2.3). For blast weapons, accuracy is rolled per unit in
the blast rather than per shot; the primary target takes the full
computed damage.
*Source:* MECHANICS-STUDY §2.3 (radii: Mangonel 1.0, Onager 1.25, Siege
Onager 1.5 tiles; fractions as above). · *Impl:* blast resolution
iterates entities in id order within the radius — the RNG draw order for
per-unit rolls is therefore id order, fixed by construction. ·
*Status:* **MISSING**.

**PRJ-08 — Minimum range is a dead zone.** A ranged unit or building
cannot fire at targets closer than its minimum range (Mangonel 3,
Skirmisher 1, Scorpion 2, towers/Castle 1, archers 0 — MECHANICS-STUDY
§2.3). The Murder Holes technology removes building minimum range.
Targets inside the dead zone must be engaged by moving away (ranged
units do so automatically when able) or not at all.
*Source:* MECHANICS-STUDY §2.3. · *Impl:* the range band is data
(`range_min` in the unit row) and every shipped ranged row has minimum
0 — the Archer's 0 included — so the dead zone is observable only as
its absence. No minimum-range weapon (and no Murder Holes) exists yet.
· *Status:* **DIFFERS**.

**PRJ-09 — Some projectiles pass through.** Scorpion bolts damage every
unit along their path (friend and foe alike take pass-through damage,
but this is a path effect, not blast — CMB-12's flag list does not
include Scorpions for *blast* friendly fire; their bolts do hit friends
in the line). Ordinary arrows stop at the first unit at the impact point.
*Source:* MECHANICS-STUDY §2.3 (scorpion pass-through; friendly-fire
flag list). · *Impl:* a `pass_through` flag in the weapon row; resolution
walks the segment in id order. · *Status:* **MISSING**.

**PRJ-10 — Projectiles ignore terrain height for targeting.** Height
enters projectile combat only through the damage multiplier (CMB-01)
comparing attacker and target elevation. There is no line-of-sight ray
blocking by hills (VIS-05) and no plunging-fire bonus.
*Source:* MECHANICS-STUDY §1.4; OPENAGE-RESEARCH §2.3. · *Impl:* nothing
is built — the impact test (PRJ-06) is purely positional, and no
elevation exists to compare. · *Status:* **MATCHES** (recorded to
prevent an occlusion implementation).

---

## 10. Unit classes & counters

**CLS-01 — The class taxonomy is data.** Every unit and building belongs
to a set of armor/attack classes (base classes: Melee, Pierce, All
Buildings, Standard Buildings, …; unit classes: Infantry, Archery, Cavalry,
Siege, Ships, Monks, Villagers, …; building classes: Towers, Castles,
Walls & Gates, …; Gaia classes: Huntable and Predator animals — the full
ID table is MECHANICS-STUDY §1.3). Membership drives the damage formula
(CMB-01/03), acquisition rules (CMB-08), and tech patching (TEC-06).
*Source:* MECHANICS-STUDY §1.3. · *Impl:* shipped (M3) — membership as a
bitset on the unit table row in `classes.rs` (13 classes); buildings
carry their classes in the building table. · *Status:* **MATCHES**.

**CLS-02 — Counters are emergent from bonus rows.** There is no
"Pikeman beats Knight" code anywhere: a counter exists exactly where a
unit's attack table carries a bonus against a class the victim belongs
to (Spearman-line +22 vs Cavalry being the canonical case). Balance work
edits tables; the engine never branches on unit *identity* for damage.
*Source:* MECHANICS-STUDY §1.2, §3 (line tables carry the bonuses). ·
*Impl:* shipped (M3) — CMB-01's shape enforces it: attack data is
(class, value) pairs (the Archer's anti-spearman row, the Boar's
anti-cavalry rows), so there is nothing else to branch on. · *Status:*
**MATCHES**.

**CLS-03 — Line upgrades transform existing units in place.** When a unit-
line upgrade research completes (Militia → Man-at-Arms, Archer →
Crossbowman, …), every owned unit of the old tier instantly becomes the
new tier, keeping its position, HP *fraction* behavior per the new max
(the standard: current HP is kept, max rises — units do not heal from
upgrading), orders, and stance. New production uses the new tier.
*Source:* GAP 5.2 (unit upgrade lines); standard reference behavior. ·
*Impl:* shipped for the first line (M6, Man-at-Arms at the Barracks) —
on completion the sim sweeps the owner's Militia in id order and swaps
their kind; kind is hashed state, the sweep order is fixed, current HP
is kept while the max rises, and new production spawns the new tier.
· *Status:* **MATCHES**.

**CLS-04 — Some bonuses arrive free with the Age.** A few units change
stats automatically when their owner advances an Age, with no research:
Scout Cavalry gains attack, speed (1.2 → 1.55), and LoS on reaching
Feudal. These are age-keyed patches in the unit's data row, applied by
the same mechanism as BLD-11.
*Source:* MECHANICS-STUDY §3.6, §4.1 (Scout line note). · *Impl:*
shipped (M6) — the Scout's Feudal layer (attack 3→5, speed 1.20→1.55
tiles/s, LoS 4→6 tiles) is applied in `Sim::stats_for`, the same lookup
the tech patches use; no special code. · *Status:* **MATCHES**.

**CLS-05 — Villagers are civilians with a weak attack.** Villagers carry
a small melee attack (3, plus bonuses vs a few classes — MECHANICS-STUDY
§3.1) and use it when ordered to attack or set on an animal; they are
never treated as military by stance defaults (CMB-09) or the idle-
military key (SEL-09). Their attack upgrades come from their own data
row and Loom (villager HP/armor/speed tech), not from the infantry lines.
*Source:* MECHANICS-STUDY §3.1; GAP 2.15. · *Impl:* shipped — the
villager row carries its 3 melee attack and fights as an ordinary melee
attacker when ordered; it is excluded from the idle-military key and
from military acquisition; Loom patches its row through TEC-06. (The
study's minor villager bonus rows are not in the table.) · *Status:*
**MATCHES**.

**CLS-06 — Kings and heroes are special entities.** A King is a unit
whose death eliminates its owner in Regicide (VIC-06); heroes (scenario
units) regenerate HP and are conversion-immune (MNK-08). Both are data
flags, not kinds with special code paths beyond those two rules.
*Source:* GAP 5.4; wiki, Conversion (hero immunity). · *Impl:* flags in
the unit row: `regicide_unit`, `immune_conversion`, `regenerates`
(rate column). · *Status:* **MISSING**.

**CLS-07 — Ships are a separate combat ecology.** Ship classes (Ships,
Fire Ship, Turtle Ship, …) interact through their own bonus rows; ship
armor values and the Fire Ship's short-range spray follow the same
CMB-01 math on water (MOV-11). Demolition Ships are blast units whose
attack is their own destruction.
*Source:* MECHANICS-STUDY §1.3 (ship classes), §3 (naval lines via GAP
6.10). · *Impl:* nothing structural — classes + domain data. · *Status:*
**MISSING** (no water, no ships).

**CLS-08 — The Monk class has no attack.** Monks belong to their own
class (which other monks' Atonement and various bonuses key off); their
offense is conversion and their support is healing (§12). A monk never
appears in CMB-08 acquisition as an *attacker*; as a *target* it is an
ordinary unit (and can be killed normally — conversion immunity is about
being converted, MNK-08, not about being attacked).
*Source:* MECHANICS-STUDY §3.8 (monk row: no attack fields); wiki, Monk.
· *Impl:* monk rows carry no attack table; the combat system skips
attack-less kinds in the attacker role automatically. · *Status:*
**MISSING**.

---

## 11. Technologies & age advancement

**TEC-01 — Ages advance in a fixed ladder with set costs.** Dark →
Feudal: 500 food, 130 s. Feudal → Castle: 800 food + 200 gold, 160 s.
Castle → Imperial: 1000 food + 800 gold, 190 s. Age research happens at
the Town Center.
*Source:* MECHANICS-STUDY §7. · *Impl:* the age ladder is a tiny data
table keyed by (from-age); the TC queue holds an "age research" entry
like any other (TEC-03). · *Status:* **DIFFERS** (the Dark → Feudal
rung ships with M6 at exactly these numbers — 500 food, 130 s; the
Castle and Imperial rungs wait for their content).

**TEC-02 — Age-up requires two qualifying buildings.** Advancing requires
owning any two buildings of the *current* age's tier, with exclusions:
for Feudal, Houses, Farms, walls, and Outposts don't count; for Castle,
walls and towers don't count; for Imperial, a Castle alone suffices, or
any two Castle-Age buildings.
*Source:* MECHANICS-STUDY §7 (requirements pinned, incl. exclusions). ·
*Impl:* shipped for the Feudal rung (M6) — a predicate over the owner's
completed-building set (`BuildingKind::counts_toward_feudal`: the
Barracks and the three camps count; Houses and Farms do not),
evaluated at enqueue time; the client disables the button from the
same rule. Later rungs arrive with their ages. · *Status:*
**MATCHES**.

**TEC-03 — Age research occupies the Town Center.** While an Age is
researching, the TC's queue is busy: villager production queued behind it
waits, and nothing else can be inserted ahead. On completion, the owner's
age flips for the whole empire at once (TEC-04).
*Source:* GAP 5.1; standard reference behavior. · *Impl:* shipped (M6) —
the train queue is a queue of typed entries (unit | technology) per
building, one record type, so stalls (BLD-10) behave identically for
units and research (a technology head is never population-stalled).
· *Status:* **MATCHES**.

**TEC-04 — Age-up applies empire-wide at the completion tick.** At the
tick research completes: building HP/armor columns step up (BLD-11),
automatic unit bonuses apply (CLS-04), and the new age's buildings,
units, and techs unlock (BLD-15) — all as lookups against the owner's new
age, so there is no empire-wide state sweep to get wrong.
*Source:* MECHANICS-STUDY §5 preamble, §7. · *Impl:* shipped in part
(M6) — age is one field per player; the automatic unit bonuses
(CLS-04) and the unlocks (BLD-15) apply as lookups against the new
age at the completion tick, with no empire-wide sweep to get wrong.
The building HP/armor step-up (BLD-11) does not happen. · *Status:*
**DIFFERS**.

**TEC-05 — Research is queued, paid up front, refunded on cancel.** A
technology is researched by one building, one item at a time in its
queue (shared with unit production). Cost is deducted at enqueue;
cancelling any queued item — unit or tech — refunds 100%. Research in
progress when its building is destroyed is lost with the building
(cancelled without refund only in the destruction case — the queue dies
with the building, CMB-15).
*Source:* GAP 5.1; standard reference behavior (queue/cancel model). ·
*Impl:* one queue implementation reused by all production buildings —
units and technologies, Town Center included — since M6; costs flow
through the same atomic stockpile API as everything else (ECO-01).
· *Status:* **DIFFERS** (queued + paid at enqueue shipped for units
and techs; cancelling — and therefore refunds — does not exist yet).

**TEC-06 — Tech effects are data patches applied at completion.** A
technology's effect is a named patch: flat additions to attack/armor
values, percentage scalars on rates/speeds/capacities, new ability flags
(Ballistics PRJ-05, Murder Holes PRJ-08), or stat additions (Loom:
villager +15 HP, +1/+2 armor, +0.1 speed). Patches apply to all owned
*and future* entities of the affected rows, and are computed by layering
(base row + line-upgrade row + patch objects) at lookup time.
*Source:* MECHANICS-STUDY §8 (the generic tech list with costs/times/
effects); OPENAGE-RESEARCH §2.7 (patch-object data shape). · *Impl:* the
owner's researched-tech set is hashed per-player state; `Sim::stats_for`
folds the owner's age layer and researched patches into the base row at
lookup time, so no tech is a code branch in the consumers. Shipped
patches (M6) are all flat: Loom (exactly the example above, +0.1 speed
included), Fletching, Scale Mail Armor, Padded Archer Armor, and the
Man-at-Arms line upgrade (CLS-03). Percentage patches have no instance
yet. · *Status:* **MATCHES**.

**TEC-07 — Techs chain by prerequisites.** Technologies require an age, a
building, and often a prior tech (Wheelbarrow → Hand Cart; Devotion →
Faith; Double-Bit Axe → Bow Saw → Two-Man Saw). The tech tree UI shows
the whole graph with met/unmet prerequisites (INF-06).
*Source:* MECHANICS-STUDY §8; Technology wiki (full generic list). ·
*Impl:* prerequisite fields in the tech table, evaluated by the same
availability function as BLD-15. Shipped prerequisites are (age,
host building) pairs — the first two links of every chain; no shipped
technology yet requires a prior technology, and the tree UI (INF-06)
does not exist. · *Status:* **DIFFERS**.

**TEC-08 — Percent effects apply before armor subtraction; flat effects
are part of the stat.** Where a tech grants "attack +X%", it scales the
attack value before CMB-01's per-class subtraction (e.g. Siege Engineers
+20% vs buildings). Flat blacksmith bonuses (+1 attack etc.) simply add
to the attack/armor stat — both are just patch arithmetic (TEC-06), but
the *order* matters and is fixed here: percentages act on the pre-armor
attack value, per class.
*Source:* MECHANICS-STUDY §1.1 (post/pre-armor clarifications). · *Impl:*
the layered lookup applies flat adds into the stat row before CMB-01
ever sees it — true for every shipped technology (M6). No percentage
technology exists yet to exercise the ordering's other half.
· *Status:* **DIFFERS**.

**TEC-09 — Production-speed techs scale work rates.** Technologies like
Conscription (+33% work rate at military production buildings, Siege
Workshop excluded) multiply the *progress rate* of queues in the affected
buildings — units and techs alike in that building. Treadmill Crane
(+20% construction speed) likewise scales BLD-04's rate — and explicitly
not repair (BLD-07).
*Source:* Technology wiki (Conscription, Treadmill Crane entries);
MECHANICS-STUDY §8. · *Impl:* a per-building progress-rate scalar from the
owner's patch set, multiplied into queue progress and build progress. ·
*Status:* **MISSING**.

**TEC-10 — Monastery line (rule-relevant subset).** The Monk technologies
modify §12's rules directly: Redemption (475G, 50 s) unlocks siege/
building conversion; Atonement (325G, 40 s) monk-vs-monk conversion;
Heresy (1000G, 60 s) converted units die (MNK-11); Sanctity (+15 monk
HP); Fervor (monks +15% speed); Block Printing (+3 conversion range);
Theocracy (group conversion rest rule, MNK-06); Devotion/Faith
(resistance-time modifiers, MNK-05); Illumination (faith recharge 62 s →
33 s, MNK-02); Herbal Medicine (garrison healing ×6, GAR-04).
*Source:* Technology wiki (Monastery table). · *Impl:* all are ordinary
TEC-06 patches/flags — this rule exists so §12's implementation notes can
reference one list. · *Status:* **MISSING**.

**TEC-11 — Unique technologies are an extension point.** Each empire has
one Castle-Age and one Imperial-Age unique technology, researched at the
Castle. Mechanically they are ordinary techs (TEC-05/06/07); their
*content* is empire design and out of scope for this rulebook (§C).
*Source:* GAP 5.5. · *Impl:* two reserved per-empire rows in the tech
table with the standard prerequisite shape (Castle, age). · *Status:*
**MISSING**.

---

## 12. Monks, conversion & relics

**MNK-01 — The monk's three actions.** A monk can *convert* (range 9),
*heal* (range 4), or *carry a relic*. Monk base numbers: 30 HP, speed 0.7,
LoS 11, cost 100 gold, train time 51 s (MECHANICS-STUDY §3.8).
*Source:* MECHANICS-STUDY §3.8. · *Impl:* conversion and healing are task
states in the monk's entity record (target id + progress counters), ticked
in a dedicated phase after combat. · *Status:* **MISSING** (GAP 6.8).

**MNK-02 — Faith gates conversion.** A monk must be at 100% faith to
begin converting. A successful conversion drains faith to 0; faith then
regenerates, reaching full after 62 s (Illumination: 33 s). A monk with
partial faith cannot start a conversion, but healing (MNK-10) is always
available.
*Source:* wiki, Conversion / Faith / Illumination (current DE model). ·
*Impl:* faith is a fixed-point 0..100 value per monk, regenerated in the
monk phase; the conversion state refuses to start below full. ·
*Status:* **MISSING**.

**MNK-03 — Conversion works in intervals (units).** While a target stays
in range, the converting monk advances one conversion interval (CI) of
≈1.25 s each. The first 4 CIs are warm-up with no conversion chance. From
the 5th CI on, each completed CI rolls a 38% chance to convert; if all
rolls fail, conversion is guaranteed at the end of the 9th CI. A unit
therefore converts somewhere in the 5th–9th CI (≈6.25–11.25 s) or not at
all (target escaped/died). *(This is the current DE model after the
update-99311 overhaul; the pre-overhaul model — a flat per-second chance
between fixed min/max times — is superseded.)*
*Source:* wiki, Conversion (interval model, chances, guarantee points). ·
*Impl:* the monk's conversion record holds (target id, CI count); each CI
boundary draws one RNG roll in monk-phase id order (SIM-04 discipline);
the guarantee is a counter check, not a roll. · *Status:* **MISSING**.

**MNK-04 — Buildings convert slower and at contact range.** Converting a
building requires Redemption and a monk standing *adjacent* to it (not at
range 9). Building conversion starts its first attempt at the 15th CI,
rolls 13% per CI, and is guaranteed at the 25th CI. Rams, Armored
Elephants, and Trebuchets likewise require an adjacent monk even for unit
conversion attempts.
*Source:* wiki, Conversion (building timings, contact-conversion units).
· *Impl:* the conversion record distinguishes (building | contact-unit |
ranged-unit) timing rows in the monk data table; adjacency uses the same
range check as melee (CMB-06). · *Status:* **MISSING**.

**MNK-05 — Resistance divides the chance, shifts the clock.** Each target
type has a resistance *level* n: the per-CI chance is 38/max(1, n) %.
Scouts are level 2 (→19%/CI) and start attempts +3 CIs later / guarantee
+1 CI later; buildings are level 3; ships start +2 / guarantee +2.
Technologies and bonuses shift the same counters: Devotion +1/+1, Faith
+4/+4 (all own units), Teutons team bonus +3/+1. (The wiki displays these
as min/max *time* modifiers; the counters here are the same facts in CI
units, 1 CI ≈ 1.25 s.)
*Source:* wiki, Conversion (resistance levels and modifier list). ·
*Impl:* resistance rows keyed by target class in the monk table; shifts
are integer CI adds applied when a conversion starts. Empire-specific
rows (Teutons) are extension content (§C). · *Status:* **MISSING**.

**MNK-06 — Group conversion: independent rolls, shared rest.** Several
monks converting one target do not add their chances — each monk runs its
own CI counter and rolls independently; the first success converts the
target, and then *every* monk that participated loses its faith (MNK-02)…
unless the owner has Theocracy, in which case only one monk rests.
*Source:* wiki, Conversion ("chance is not added up"; Theocracy rest
rule). · *Impl:* on a conversion success, sweep the participant set
(monks whose conversion record names this target) in id order: drain all,
or the lowest-id one under Theocracy. · *Status:* **MISSING**.

**MNK-07 — CI progress is sticky across targets.** Switching a monk to a
new target does *not* reset its CI progress — with one exception: after
attempting a *building*, switching to a unit resets the counter. The
player manually ordering the monk to move resets progress; the monk's own
automatic chase (target walks out of range) does not, and stored progress
resumes when the monk re-engages.
*Source:* wiki, Conversion (persistence rules, incl. the update-95810
building→unit reset). · *Impl:* the CI counter lives on the *monk*, not
the pairing; only (manual-move command | building→unit switch) zeroes it.
· *Status:* **MISSING**.

**MNK-08 — Conversion immunity list.** Never convertible: heroes, Kings.
Convertible only with Redemption: siege units and buildings — and even
then never: Town Centers, Castles, Kreposts, Monasteries, Fortified
Churches, walls, gates, Wonders, Farms, Fish Traps, and any foundation.
Monks are convertible only if the owner has Atonement. Villagers and
ordinary military are always convertible.
*Source:* wiki, Conversion (eligibility rules). · *Impl:* an eligibility
function over (target class flags × owner's tech set) in the monk table —
flags per CLS-01, gates per TEC-10. · *Status:* **MISSING**.

**MNK-09 — Relics: carried by monks, garrisoned in Monasteries.** A monk
ordered onto a relic picks it up (carrying one reduces his LoS by 2 and
he cannot convert while carrying — he delivers it instead). Depositing it
in a Monastery starts its gold trickle (ECO-16) and counts it toward
relic victory (VIC-03). A carrying monk who dies drops the relic where he
fell, for anyone's monk to take. *(Monastery relic capacity: UNVERIFIED —
no cap is documented in the sources consulted; treat as uncapped until a
source says otherwise.)*
*Source:* wiki, Relic; MECHANICS-STUDY §3.8 (LoS −2 while carrying). ·
*Impl:* relic is an entity with states (ground | carried-by | garrisoned-
in); the trickle is an accumulator on the monastery record. · *Status:*
**MISSING** (GAP 6.8).

**MNK-10 — Healing.** A monk heals one allied unit at a time within
range 4 at 150 HP/min (2.5 HP/s), automatically choosing nearby wounded
allies when idle. Healing never heals buildings, siege weapons, or ships
(villagers repair those — BLD-07), never drains faith, and never
overheals past max HP.
*Source:* MECHANICS-STUDY §3.8; wiki, Monk (healing behavior). · *Impl:*
the monk phase's auto-task: nearest wounded healable ally in range —
same tie-break discipline as CMB-08 (squared distance, then id). ·
*Status:* **MISSING**.

**MNK-11 — Heresy turns conversion into death.** If a player with Heresy
would lose a unit to conversion, the unit dies instead of changing sides.
Buildings are exempt (they convert normally).
*Source:* wiki, Conversion (Heresy note); Technology wiki. · *Impl:* at
the conversion-success moment, check the *victim owner's* tech set: if
Heresy and target is a unit → route to the death sweep (CMB-14) instead
of the ownership transfer. · *Status:* **MISSING**.

**MNK-12 — Converted units keep their attributes.** A converted unit
keeps exactly the stats and upgrades it had at the moment of conversion:
the new owner's later technologies do not affect it, and the old owner's
did until conversion. A converted villager thereafter builds using the
*new* owner's available building set (per standard reference behavior).
*Source:* wiki, Conversion (attribute locking). · *Impl:* the layered
stat lookup (TEC-06) normally reads the *owner's* patch set — conversion
breaks that: on transfer, stamp the unit with a frozen patch snapshot
(the attack/armor/stat-relevant patch ids it carried). This is the one
place per-unit patch state exists; document it in the entity record. ·
*Status:* **MISSING**.

**MNK-13 — The monk is fragile by design.** Monks have 0/0 armor and
30 HP (45 with Sanctity) and no attack: any military unit that reaches a
monk kills it quickly, which is the intended counter to conversion —
conversions are strong because monks are easy to kill during the
6–11 s window (MNK-03). No separate rule is needed; this records that
the timing window *is* the balance mechanism, so MNK-03's numbers must
not be "rounded for feel".
*Source:* MECHANICS-STUDY §3.8; wiki, Conversion. · *Impl:* acceptance
test for §12: a lone monk vs. one Knight reproduces the documented
outcome distribution from the CI model. · *Status:* **MISSING**.

---

## 13. Trade, market & tribute

**TRD-01 — The Market exchanges in lots of 100.** Buying converts gold
into 100 units of a commodity; selling converts 100 units into gold.
Prices come from the shared exchange rate (TRD-02), not from a per-player
book.
*Source:* wiki, Market (Age of Empires II); GAP 3.11. · *Impl:* a Market
command carrying (commodity, buy|sell); stockpile and rate updates are
atomic in the economy phase. · *Status:* **MISSING**.

**TRD-02 — One global price per commodity, with a fee spread.** Each
commodity has a single exchange rate in gold per 100 units, shared by
all players: starting rates are wood 100, food 115, stone 130. The sell
price is rate × (1 − fee) and the buy price is rate × (1 + fee); the
default fee is 30% (Guilds reduces it to 15% for that player — the
*rate* is still shared, the *spread* is personal).
*Source:* wiki, Market (starting prices, fee structure, worked numbers:
wood starts 70 to sell / 130 to buy). · *Impl:* rates are per-match
global state (hashed); the fee is a per-player patch (TEC-06). ·
*Status:* **MISSING** (GAP 3.11 had the components but not the
shared-rate model).

**TRD-03 — Every transaction moves the world price.** Each buy raises
that commodity's rate by 2; each sell lowers it by 2 — for everyone,
immediately. Rates are clamped to [20, 9999]. The floor (sell 14 / buy 26
at default fee) is the intended "doomsday economy" backstop.
*Source:* wiki, Market (rate movement and clamps). · *Impl:* integer rate
updates in the same atomic step as TRD-01; nothing else in the game reads
or writes rates. · *Status:* **MISSING**.

**TRD-04 — Tribute costs the sender a fee.** A player may tribute any
amount of any resource to any player (allies, and in DE even enemies).
The sender pays the amount plus the tribute fee: 30% by default, 20%
with Coinage, 0% with Banking; the recipient receives the full amount.
*Source:* wiki, Tribute; Technology wiki (Coinage/Banking entries);
GAP 3.12. · *Impl:* a diplomacy-screen command validated against the
sender's stockpile for amount × (1 + fee) — integer math, fee rounded
per the market convention (the exact rounding is UNVERIFIED; adopt
"round the fee up to whole resources", the conservative reading). ·
*Status:* **MISSING**.

**TRD-05 — Trade units run a perpetual two-leg route.** A Trade Cart
(Market↔Market) or Trade Cog (Dock↔Dock) tasked to a foreign depot
travels there empty, then returns to its home depot loaded; the gold is
credited on arrival home, and the cycle repeats forever until re-tasked
or killed. A trade unit generates nothing one-way.
*Source:* wiki, Market / Gold; GAP 3.10. · *Impl:* the route is a task
state machine like the gather loop (ECO-04) with two waypoints and a
gold-credit event at the home-deposit moment; gold per trip computed by
TRD-06 at credit time from the depot positions. · *Status:* **MISSING**.

**TRD-06 — Trade gold scales ~quadratically with distance.** Gold per
trip = 0.46 × d × (d / map_size + 0.3), where map_size is the map's edge
in tiles and d is the depot-to-depot distance measured center-to-center
with a 5-tile inset subtracted per axis (and floored at 0.1). The formula
embeds cart speed and a Spanish-ally factor in its constant; the best
corner-to-corner routes yield ≈0.58 gold/s averaged over the round trip.
*Source:* wiki, Gold (formula and constants pinned). · *Impl:* computed
in fixed-point at credit time from the two depot anchor positions; since
depots are static, the per-route value can be cached in the task record
and recomputed only on re-tasking. · *Status:* **MISSING**.

**TRD-07 — Anyone's depot is a valid trade target.** Trade works with
any player's Market/Dock regardless of diplomacy state — ally, neutral,
or enemy — and even with a resigned player's depots. The depot's owner
gains and loses nothing; their units may still kill your carts per
stance rules.
*Source:* wiki, Gold (trade target rules). · *Impl:* trade tasking never
consults the diplomacy table for validity, only for the combat exposure
that follows from it. · *Status:* **MISSING**.

**TRD-08 — Caravan speeds trade units.** The Caravan technology (Feudal,
Market) raises Trade Cart and Trade Cog speed by 20% (an ordinary MOV-01
percentage patch — which, via the round-trip time, also raises gold/s:
TRD-06's constant absorbs the *base* speed only).
*Source:* Technology wiki (Caravan). · *Impl:* TEC-06 patch on the two
trade kinds. · *Status:* **MISSING**.

---

## 14. Garrison, transport & repair

**GAR-01 — Garrison capacities and eligibility.** Town Center 15, towers
5, Castle 20, other production buildings 10. Only foot units may garrison
in buildings: villagers, infantry, archers, monks. Cavalry and siege
never garrison in buildings (their containers are GAR-10/11).
*Source:* MECHANICS-STUDY §9.3; GAP 4.16. · *Impl:* capacity + eligible-
class columns in the building table; garrisoned units are a list on the
building record (hashed), removed from the field and from pathing. ·
*Status:* **MISSING**.

**GAR-02 — Garrisoned units are untouchable.** While inside, a unit
cannot be damaged, converted, or healed by monks (it heals by GAR-04
instead), and does not count as idle for SEL-08/09.
*Source:* MECHANICS-STUDY §9.3. · *Impl:* garrisoned entities are skipped
by every field system (targeting, blast, conversion) by construction —
they are not in the spatial iteration at all. · *Status:* **MISSING**.

**GAR-03 — Ejection keeps the garrison alive.** If a building is
destroyed, converted, or drops to ≤20% HP, its garrison is ejected
unharmed beside it. Voluntary ungarrisoning places units at the
building's ungarrison point, idle.
*Source:* MECHANICS-STUDY §9.3. · *Impl:* ejection runs in the building-
removal path (CMB-15) and the HP-threshold check in the combat phase;
spawn positions use the building's ring cells (the same helper as trained-
unit spawn). · *Status:* **MISSING**.

**GAR-04 — Shelter heals.** Units garrisoned in a Town Center or tower
heal at 0.1 HP/s; in a Castle, 0.2 HP/s. Herbal Medicine multiplies
garrison healing ×6. Healing inside other buildings/docks: none.
*Source:* MECHANICS-STUDY §9.3; Technology wiki (Herbal Medicine: "6×
faster" garrison healing). · *Impl:* a per-building heal-rate column
folded into the garrison record tick. · *Status:* **MISSING**.

**GAR-05 — Garrison adds arrows by DPS, with caps.** Only villagers and
archer-type units add firepower to a defensive building. Each contributes
its ranged DPS; the building gains floor(total contributed DPS ÷ the
building's own DPS) extra arrows per volley. A villager counts as 2.5
DPS. Caps: towers at most +4 arrows, a Town Center at most 10 arrows in
total, a Castle at most +16 extra arrows.
*Source:* MECHANICS-STUDY §9.3 (formula and caps pinned). · *Impl:*
recomputed when the garrison changes (event, not per tick): store the
arrow count on the building record; each arrow is an ordinary projectile
(PRJ-01) from the building's weapon row. · *Status:* **MISSING**.

**GAR-06 — The Town Center fires only when garrisoned.** Towers and
Castles shoot on their own (their base volley, GAR-05 adds to it). A Town
Center has a weapon (5 pierce, range 6, RoF 2.0) but fires *only* while it
holds a garrison, with the arrow count from GAR-05 (minimum one arrow
while garrisoned).
*Source:* MECHANICS-STUDY §5 (TC row note), §9.3. · *Impl:* the building
attack state machine checks (has-weapon × garrison-present) for the TC
kind; towers/castles check has-weapon alone. · *Status:* **MISSING**.

**GAR-07 — Town Bell.** Ringing a Town Center's bell sends every villager
within 23 tiles to garrison in the nearest shelter; their current tasks
and positions are snapshotted. Ringing again ("back to work") ungarrisons
them and restores each villager to its snapshot task and location.
*Source:* MECHANICS-STUDY §9.3; wiki, Town Bell; GAP 2.13. · *Impl:* the
bell is a TC command that sweeps own villagers in radius (id order) into
a bell-snapshot record per villager; the second press replays it. The
snapshot record is exactly what makes this implementable in our task
model. · *Status:* **MISSING**.

**GAR-08 — Rally-garrison on creation.** If a production building's rally
point is set on a garrisonable building (SEL-13), newly trained units
walk there and garrison inside automatically on arrival.
*Source:* wiki, Gather Point (rally behavior); GAP 4.16. · *Impl:* the
spawn logic (SEL-13) issues a garrison task instead of a move task when
the rally target is a garrisonable building. · *Status:* **MISSING**.

**GAR-09 — Transports carry anyone, and cargo dies with the ship.** A
Transport Ship loads land units up to its capacity (5) at a shoreline and
unloads at another; while loaded, units are immune and inert (not
healed), and if the ship sinks, everything aboard dies with it. (The
"5 → 20" capacity progression in GAP-ANALYSIS 6.10 is **not verified**
for AoE2 DE — the transport ship's documented capacity is 5; treat the 20
figure as an error until a source supports it.)
*Source:* GAP 6.10 (existence); capacity corrected against the DE unit
data as consulted for MECHANICS-STUDY — flagged here rather than silently
changed. · *Impl:* transport contents are a list on the ship record like
a building garrison (GAR-01), with the death routing inverted (CMB-14).
· *Status:* **MISSING**.

**GAR-10 — Rams are armored containers.** A battering ram carries up to
6 foot units inside. Each passenger adds +0.05 tiles/s speed (to a 0.9
maximum) and +10 attack against buildings. Passengers are protected like
any garrison (GAR-02) and eject alive if the ram is destroyed.
*Source:* MECHANICS-STUDY §3.9 (garrison effects pinned). · *Impl:* the
ram's effective speed/anti-building attack are derived from its passenger
count at lookup time (like GAR-05) — no per-passenger state beyond the
list. · *Status:* **MISSING**.

*(Repair is a building-system rule: see BLD-07.)*

---

## 15. AI behavior & difficulty

**AI-01 — The AI is a player, not a cheat layer.** The computer opponent
plays by issuing ordinary commands through the same command path as a
human. The simulation neither knows nor cares which seat is human.
*Source:* GAP §9 (skirmish AI structure); Bettner & Terrano (command
model). · *Impl:* the AI is a separate crate/process producing `Command`
lists per tick; the sim has no AI hooks. This also makes AI-vs-AI test
games and replay verification free. · *Status:* **DIFFERS** (M5,
2026-10-16: the opponent ships as a deterministic module inside the sim
crate (`sim/src/ai.rs`), not the separate crate/process the sketch
above envisions — but the discipline the rule protects holds exactly:
it issues only ordinary Commands through the same queue, on a fixed
decision cadence, derived from sim state alone, so AI matches replay
bit-identically and the sim treats both seats alike.)

**AI-02 — DE-class AI does not cheat resources.** On every difficulty,
the AI gathers, pays, and trains under the same costs and rules as a
human (the historical exception is the legacy HD-era AI on its hardest
setting, which received hidden resource grants — a model DE's AI dropped
and Skirmish II does not adopt).
*Source:* wiki, Artificial intelligence (DE AI behavior). · *Impl:*
verification rule: AI command streams are replayable headless
(SIM-12) — any resource anomaly would surface as an impossible command
stream. · *Status:* **MATCHES** (M5, 2026-10-16: the scripted AI gathers,
pays full costs and trains under the human's rules at both
difficulties; no resource grants exist anywhere in the sim. Verified
by replay: an AI match's hash stream reproduces bit-for-bit.)

**AI-03 — Difficulty changes behavior, not physics.** The difficulty
ladder (Easiest → Extreme in DE) varies: time of first attack, army
sizes and composition quality, defensive building use (walls, towers),
ally cooperation, and whether the AI attempts a Wonder in standard
games. Unit stats, costs, and the damage formula are identical at every
difficulty.
*Source:* wiki, Artificial intelligence (difficulty table). · *Impl:*
AI parameters live in a per-difficulty profile table consumed by the AI
crate only; the sim crate contains no difficulty concept. · *Status:*
**MATCHES** (M5, 2026-10-16: two rungs of the ladder ship — Easy and
Normal — as pure behavior parameters in the AI's own config (decision
cadence, economy and army targets, wave size and cooldown). Unit
stats, costs and the damage formula are identical at both; the sim's
only difficulty-shaped state is the match mode tag that selects the
profile.)

**AI-04 — The AI follows an economy script.** AI economy behavior mirrors
a build order: continuous villager production, age-up timings, expansion
to new resource camps, and army production from a composition plan —
parameterized by AI-03's profile.
*Source:* wiki, Artificial intelligence; GAP 9.1–9.3. · *Impl:* Skirmish II's
M5 AI is a scripted build-order player (GAP roadmap), issuing commands on
a tick schedule with state checks; open-ended strategic AI is future
work, not a rule change. · *Status:* **MATCHES** (M5, 2026-10-16, at v1
depth: continuous villager production to a profile target,
deficit-balanced resource assignment with food first, houses built
ahead of the population cap, a Barracks, alternating Militia/Archer
production, and a rally staged toward the enemy. M6, 2026-10-16: the
script also raises a Mill by its berries and a Lumber Camp by its
woodline — its Feudal pair — researches Loom once the economy rolls,
advances to the Feudal Age when the food allows, and fields its
Archers from an Archery Range; it does not use Farms, the Mining
Camp, the Stable, the Blacksmith or Man-at-Arms yet.)

**AI-05 — The AI resigns when hopeless.** An AI with no Town Center, no
villagers, and no path to rebuild resigns rather than playing to the
last unit; human players resign manually (VIC-11).
*Source:* wiki, Artificial intelligence (resignation behavior). · *Impl:*
an AI-side predicate evaluated on a slow cadence (every ~30 s of game
time); resignation is a command like any other. · *Status:* **MATCHES**
(M5, 2026-10-16: the predicate is evaluated on the AI's decision
cadence — no Town Center and no villagers left means resigned, and the
match checker reads it as the opponent's win even if a stray building
or soldier remains. Manual human resignation (VIC-11) is not shipped.)

**AI-06 — Herdable capture by proximity.** An unowned herdable animal
becomes the property of the player whose unit comes within capture
proximity of it. It stays owned while any friendly unit remains nearby;
if it is left without friendly units nearby and another player's unit
passes, ownership transfers to them (sheep can be stolen this way).
*Source:* wiki, Sheep ("Nearing a neutral Sheep will cause it to become
aligned to that player's faction… the Sheep will stay under the control
of the player that first discovers it, as long as friendly units are in
close proximity… if no friendly units are nearby and another team's
unit happens to pass by, the Sheep will fall under the control of that
player"; the article's Celts exception does not arise — Skirmish II has
no civilizations) and Herdable animal ("Capturing an animal is
accomplished simply by moving any of their units next to it. If
multiple civilizations have units near an animal, the animal will be
owned by the player who has the most units nearby"; "Cows, Sheep, and
Turkeys have 2 Line of Sight, while the rest have 3"); Gaia ("all
predator, huntable, and herdable animals can wander in a specific
area"). · *Impl:* a per-tick presence sweep in the combat phase over
herdable entities: only living mobile non-herdable units of a player
count (buildings and other herdables never hold a herd). A wild sheep
converts to the player with the most units in range (an exact tie
holds — the sources name no tie-break, so holding is the adopted
deterministic choice); an owned sheep transfers only when its owner
has no unit in range and an enemy does. Conversion clears the sheep's
orders — it stops and waits for its new owner. The capture radius is
UNVERIFIED as a number in the sources (the rule is stated as proximity
/ "next to it") — adopt **1 tile** as the tuned constant. A sheep's
Line of Sight is 2 tiles: the Herdable animal per-animal breakdown is
adopted over the Sheep article's prose value of 3, and the conflict is
recorded here. Wild sheep also wander: the animal AI gives Gaia sheep
short seeded-RNG steps (≤1 tile) around their spawn post (leash
~2 tiles) on a lazy 12–30 s decision cadence that mostly grazes
(three rounds in four) — reference sheep shuffle a little, they do
not march; owned sheep never wander. Client
side, an owned sheep wears its owner's colour as a band on the animal
and rings in that colour when selected; a wild sheep (and the enemy's)
can be clicked for an information selection, portrait and hit points
only. · *Status:* **MATCHES** (reworked 2026-10-20 after a live
play-test found the gaps: M3 shipped the first half only — first
in-range unit won, ownership was permanent, and Gaia sheep had no
wander behavior at all, so they stood at spawn and herds could not be
stolen. The full rule above now ships, with sim tests for wander
determinism, the unguarded steal, owned-sheep stillness, and the
2-tile sheep sight.)

**AI-07 — Huntable behavior: deer flee, boars fight.** Deer and similar
prey graze idly and flee from an attacker that wounds or approaches them.
A boar is passive until damaged; once attacked it charges its attacker
and keeps fighting it (which is exactly what makes the two-shot lure to
the Town Center work: one villager shoots twice, runs, and the boar
follows into the TC's garrison fire). If the attacker garrisons or dies,
the boar disengages. (Whether a disengaged boar returns to its exact
start point is UNVERIFIED; wandering near the disengage point is the
adopted default.)
*Source:* wiki, Early Economy (boar lure described step by step); Hunt
(animal behavior classes). · *Impl:* animals get a minimal state machine
in the combat phase (idle / flee / chase-attacker) driven by the same
damage events as CMB-01; no separate animal engine. · *Status:*
**DIFFERS** (M3, 2026-10-16: deer flee and boars charge their attacker,
disengaging when it dies or escapes. One deliberate divergence from the
adopted default above: a disengaged boar returns to its original post
and heals there, instead of wandering near the disengage point.)

**AI-08 — Predators hunt villagers.** Wolves and similar predators are
Gaia combat units: idle until a unit (typically a villager) enters their
aggression radius, then they attack it, without chasing across the map.
They are never auto-targeted by passing military under CMB-08 until they
engage (they are Gaia, not enemies, until combat starts — and military
defends itself when attacked).
*Source:* wiki, Hunt (predator class); MECHANICS-STUDY §1.3 (Predator
Animals class). · *Impl:* same animal state machine as AI-07 with a
"attack intruder" branch; the aggression radius is a data constant
(UNVERIFIED numerically). · *Status:* **MISSING**.

**AI-09 — Gaia structures capture by presence (King of the Hill).** The
KotH Monument is a Gaia building that belongs to the team with the most
units close to it — ownership flips automatically by presence, with no
attack or conversion involved (VIC-08).
*Source:* wiki, Conversion (Gaia structures auto-conversion note). ·
*Impl:* a periodic majority-count sweep like AI-06, with its own radius
constant. · *Status:* **MISSING**.

---

## 16. Victory, scoring & game modes

**VIC-01 — Conquest elimination.** In Conquest, a player is eliminated
when they have no units left and no building capable of producing units
(in practice: all units dead and all production buildings destroyed or
converted). The last surviving player — or team, under allied victory
(VIC-12) — wins.
*Source:* GAP 8.1; wiki, Victory. · *Impl:* an elimination predicate
evaluated on the slow cadence after death sweeps; eliminated players'
state follows VIC-11. · *Status:* **DIFFERS** (M5, 2026-10-16: Conquest
ships with a stricter predicate than the rule's letter — a player is
eliminated only when they have no buildings of any kind left
(under-construction counts as a building) AND no population-counting
units; the rule as written would also eliminate a player whose
production buildings stand but who fields no units, where the shipped
game keeps them alive on the building alone. Two match-ending details:
if both players fall on the same tick the human loses (defeat is
checked first), and AI resignation (AI-05) ends the match as a win.
The match state is hash-covered and exposed as ongoing / victory /
defeat; the sim keeps ticking harmlessly after the decision.)

**VIC-02 — Wonder victory (standard games).** In Standard victory mode,
holding a completed Wonder starts a countdown; if the Wonder still stands
when it expires, its owner wins. The countdown length scales with map
size: 100 years (Tiny) to 500 years (Huge/Ludicrous), one game year =
5 s — i.e. 8:20 to 41:40 of game time (the full table is in the Relic
wiki article and shared with VIC-03). Destroying the Wonder cancels the
countdown.
*Source:* wiki, Relic (duration table by map size); GAP 8.2. · *Impl:*
the countdown is a per-player tick counter in match state (hashed),
started/stopped by Wonder existence events. · *Status:* **MISSING**.

**VIC-03 — Relic victory (standard games).** Holding *all* of the map's
relics garrisoned in Monasteries starts the same map-size countdown
(VIC-02's table); losing any relic from a monastery cancels it. Under
allied victory, a team's relics pool together.
*Source:* wiki, Relic (victory rule + pooled team relics). · *Impl:*
relic state lives on relic entities (MNK-09); the win checker counts
garrisoned relics per player/team each cadence tick. · *Status:*
**MISSING**.

**VIC-04 — The score formula.** Score is shown live and decides score
victories (VIC-05). Total = Military + Economy + Technology + Society,
where: **Military** = 20% of the resource cost of all enemy units and
buildings the player destroyed or converted. **Economy** = 10% of
(resources currently held + total resources paid in tribute) + 20% of the
resource cost of surviving units and standard buildings (Castles and
Wonders excluded — they count in Society). **Technology** = 20% of the
resource cost of every researched technology (free/auto-granted techs
don't count) + 10 points per 1% of the map explored. **Society** = 20%
of the cost of Castles and Wonders constructed. Team score = the
arithmetic mean of members' scores.
*Source:* wiki, Score (all four components pinned). · *Impl:* run
counters (destroyed/converted value, tribute paid, researched-tech cost)
are per-player hashed state incremented at event time — the formula must
never be recomputed by scanning history. Map-explored % comes from
VIS-01's per-player grid (a count maintained incrementally). · *Status:*
**MISSING** (GAP 8.3 had "score exists" as a line; the formula itself was
not in GAP-ANALYSIS).

**VIC-05 — Time-limit and score-target victories.** If a match has a
time limit, the highest score at expiry wins. In score-target mode, the
first player to reach the target score wins immediately.
*Source:* wiki, Score (the two modes where score is decisive); GAP 8.6.
· *Impl:* the win checker reads VIC-04's live total on the same cadence.
· *Status:* **MISSING**.

**VIC-06 — Regicide.** Each player starts with a King unit (CLS-06). A
player whose King dies is eliminated immediately, whatever else they
own.
*Source:* GAP 8.4. · *Impl:* the death sweep checks the `regicide_unit`
flag and routes to elimination. · *Status:* **MISSING**.

**VIC-07 — Sudden Death.** Losing your (last) Town Center eliminates
you. No rebuild window.
*Source:* GAP 8.5. · *Impl:* elimination predicate variant evaluated on
TC destruction events. · *Status:* **MISSING**.

**VIC-08 — King of the Hill.** Hold the central Monument (AI-09) for
the map-size countdown to win; losing majority presence at the Monument
loses the hold and the countdown.
*Source:* GAP 8.6; wiki, Conversion (Monument capture note). · *Impl:*
VIC-02's countdown machinery keyed to AI-09's ownership state. ·
*Status:* **MISSING**.

**VIC-09 — Death Match is a settings delta.** Same rules as Conquest/
Standard, but players start with very large stockpiles and the economy
begins effectively post-Dark-Age. No rule in this book changes.
*Source:* GAP 8.6. · *Impl:* match settings override ECO-17's stockpile.
· *Status:* **MISSING** (settings framework pending M8).

**VIC-10 — Empire Wars is a start delta.** Players start in the Feudal
Age with a pre-built working economy (villagers already distributed on
resources, basic buildings placed) instead of the standard opening.
*Source:* GAP 8.6. · *Impl:* an alternative start-package generator
beside OPENING-SPEC's standard one. · *Status:* **MISSING**.

**VIC-11 — Resignation leaves ruins.** When a player resigns (or is
eliminated), their units are removed, but their buildings remain
standing on the map (inert, owner = resigned player). Their Markets and
Docks remain valid trade targets (TRD-07).
*Source:* GAP 8.10; wiki, Gold (trade with resigned players). · *Impl:*
resignation is a command that sweeps own units to the death path
(without score credit to anyone) and flags the player inactive. ·
*Status:* **MISSING**.

**VIC-12 — Allied Victory pools team outcomes.** With Allied Victory
enabled, a team wins or loses together: conquest requires eliminating
every opposing player, relic/Wonder countdowns read team state (VIC-03),
and team score is the mean (VIC-04).
*Source:* GAP 8.7; wiki, Relic. · *Impl:* win predicates take a
(team | player) scope from match settings. · *Status:* **MISSING**.

---

## 17. Multiplayer determinism rules

**NET-01 — Lockstep: only commands cross the wire.** In multiplayer, no
client sends state. Every client receives every player's commands and
simulates the identical game locally (SIM-02/12). Bandwidth is therefore
trivial (≈1 command per player per 1.5–2 s in typical play); latency is
absorbed by scheduling, not by prediction.
*Source:* Bettner & Terrano (whole paper); OPENAGE-RESEARCH §2.6; GAP
10.1. · *Impl:* the M8 net layer (play/netplay.js) transports only the
semantic command vocabulary, applied on both clients in slot order;
state never crosses the wire. · *Status:* **MATCHES** (shipped M8,
2026-10-18).

**NET-02 — Commands execute two turns ahead.** Commands are stamped for
execution at tick N+2 communication turns (≈200 ms turns in the reference
design), giving every packet a full turn to arrive. The delay is fixed
per match settings, never adaptive mid-match.
*Source:* Bettner & Terrano (the N+2 rule). · *Impl:* the transport
stamps each bundle for an execution TURN `delay` turns ahead (turns of
3 ticks / ≈29.4 ms wall at 1.7× game speed, not the reference's ≈200 ms turns) and holds it until
that turn; the delay starts at 4 turns (≈118 ms) and adapts UP when the
link lags, never fixed per match settings; local single-player input
does not take the bundle path (it applies immediately, delay 0).
· *Status:* **DIFFERS** (scheduling-ahead matches; turn size,
adaptivity, and the unshared local path differ).

**NET-03 — The slowest machine sets the pace.** If any client cannot keep
up, the whole match slows (never drops sim time): the design deliberately
prefers a steady slower game to a jittery one. A client ahead simply
waits at the turn boundary.
*Source:* Bettner & Terrano ("slower-but-steady beats jitter"). ·
*Impl:* turn gating in the M8 session: a turn executes only when
both bundles have arrived; a client ahead waits at the turn boundary
and sim time is never dropped — a slow peer slows the match's wall
clock, not its logic. · *Status:* **MATCHES** (shipped M8).

**NET-04 — Per-tick hash comparison catches desyncs.** Clients exchange
the per-tick checksum (SIM-05); a mismatch means the match has diverged
and stops with diagnostics (tick, and per SIM-06 the subsystem) rather
than playing on in two different worlds.
*Source:* Bettner & Terrano; OPENAGE-RESEARCH §2.6. · *Impl:* the state
hash is exchanged and compared after every TURN (every 3 ticks); a
mismatch stops both clients with the turn number and the recorded
replay as diagnostics. · *Status:* **DIFFERS** (comparison and stop
match; the cadence is per-turn, not per-tick, and SIM-06's
per-subsystem diagnostics are not yet produced).

**NET-05 — No local factor may influence the sim.** Graphics settings,
adaptive quality tier, hardware speed, frame rate, spare CPU — none may
alter any sim input, iteration order, or RNG draw. Visual fidelity scales
freely; the simulation is identical for a phone and a desktop.
*Source:* Bettner & Terrano ("simulate identically regardless of spare
time"); OPENAGE-RESEARCH §2.6. · *Impl:* enforced structurally — the sim
crate has no quality/settings parameters; the adaptive-quality system
lives entirely in the renderer. · *Status:* **MATCHES** (as an
architecture rule — the renderer already scales independently, and
the M8 session's wall clock paces execution without ever entering
the sim).

**NET-06 — RNG discipline is a networking rule.** Because all machines
must draw identically (SIM-04), any system that consumes RNG must do so
inside the sim's fixed iteration, in an order defined by entity ids —
never from rendering, never from unordered containers, never from
real-time events. (First application: PRJ-03.)
*Source:* OPENAGE-RESEARCH §2.6. · *Impl:* code-review rule for the sim
crate; the RNG is unreachable from `wasm.rs` exports. · *Status:*
**MATCHES** (discipline in place; the first consumer, PRJ-03,
landed with M3, and the M8 transport consumes no RNG either).

**NET-07 — Spam commands are filtered.** Issuing the identical command
to the identical location repeatedly (a common accidental double-issue)
is deduplicated by the command layer; command volume per player is
bounded by the transport.
*Source:* Bettner & Terrano (duplicate-command filtering; players average
an order every 1.5–2 s). · *Impl:* shipped in M8's bundle assembly:
exact duplicates within one bundle collapse, bundle size is capped
(256), and every inbound command is structurally validated before it
can reach the sim — the sim still executes what it is given.
· *Status:* **DIFFERS** (the accidental double-issue case and the
volume bound match; there is no cross-turn dedup or per-player rate
limit beyond the turn structure itself).

**NET-08 — A replay is the match.** (Seed, settings, command stream) is
a complete recording (SIM-12); playing it back re-simulates the match,
with observer controls (free camera, per-player fog choice — VIS states
are derived, never stored) layered on top.
*Source:* GAP 8.9, 10.5. · *Impl:* M8 shipped the file format and the
viewer: every match (online or local) records (seed, mode, command
stream) client-side; the play page downloads it and re-executes it
under Watch replay with play/pause and speed controls and a free
camera. · *Status:* **DIFFERS** (the recording and re-simulation
match; playback fog is the recording seat's — there is no per-player
fog choice in the viewer yet).

**NET-09 — Save/restore snapshots the sim.** A saved game is a full
serialization of sim state; its hash (SIM-05) verifies the snapshot, and
restoring re-enters lockstep from that tick as if the match had never
paused.
*Source:* GAP 10.6. · *Impl:* the canonical serialization behind
`state_hash()` is the save format's core (plus a version header); no
renderer state is saved. · *Status:* **MISSING** (hash serialization
exists; no save/load API — M8's rejoin restores matches by
re-simulation from the recorded stream, a different mechanism).

**NET-10 — Chat and taunts never enter hashed state.** In-game chat,
taunts, and flares-as-messages travel beside the command stream; they
must not consume RNG, alter iteration, or be required to reproduce a
match (a replay need not contain them, though it may record them for
display).
*Source:* Bettner & Terrano (command channel discipline); GAP 10.2. ·
*Impl:* the transport's
non-command messages (presence, handshake, telemetry) travel beside
the bundle stream and never enter hashed state; the sim never sees
them. In-game *effects* of chat-adjacent features (flare position
INF-04) are client-side rendering. · *Status:* **DIFFERS** (the
channel discipline is in place and load-bearing since M8; chat and
taunts themselves are not implemented yet).

**NET-11 — Diplomacy is match state, not sim physics.** Player stances
(ally/neutral/enemy) live in a diplomacy table consulted by targeting
(CMB-08), gates (BLD-13), and trade (TRD-07 — mostly by ignoring it).
Changing stance is a command; the table is hashed state. In locked-team
matches it cannot change.
*Source:* GAP 10.3. · *Impl:* a small per-match table in `Sim`;
M8 shipped 1v1 only, where "everyone else is an enemy" remains the
implicit row — the table lands with team games. · *Status:*
**MISSING**.

---

## 18. Information rules — what the player sees

**INF-01 — Attack alerts are throttled and located.** When your units or
buildings come under attack, you get an audible warning and a flashing
marker at the location (minimap + Backspace jumps there, SEL-10).
Alerts are rate-limited so a raid cannot spam the channel into useless-
ness.
*Source:* GAP 11.2; wiki, Mini map (signal conventions). · *Impl:*
client-side: damage events must therefore be *observable* — the ABI
gains an event/diff feed at M3 (or the client diffs HP arrays; the
events feed is cleaner and is what audio needs too, INF-10). ·
*Status:* **MISSING** (no combat, no alerts, no audio).

**INF-02 — Milestones are announced.** Your own completions (research
finished, building finished, wonder progress) and global events (any
player advances an Age, a Wonder is completed anywhere, a relic/wonder
countdown starts — VIC-02/03) appear as messages/sounds. Countdowns are
visible to all players while active.
*Source:* GAP 11.2; standard reference UI behavior. · *Impl:* the same
event feed as INF-01, with per-event audience flags resolved client-
side. · *Status:* **MISSING**.

**INF-03 — Idle and housed indicators are always visible.** The UI shows
a live idle-villager indicator (with the SEL-08 key behind it), an idle-
military indicator, and a housed warning when production is stalled by
BLD-10.
*Source:* GAP 11.3, 11.9. · *Impl:* shipped (M4) — the top bar carries
both idle indicators (icon + live count, lit while anyone stands free),
each a button running the SEL-08/09 cycle; the housed warning ships as
before (the top bar tints population at cap; the Train button explains
itself). · *Status:* **MATCHES**.

**INF-04 — Minimap signals.** The minimap shows terrain, resources,
player-colored units/buildings through fog states (VIS-10), flashes at
attack alerts (INF-01), and displays ally flares: a flare is a visible
X marker + sound placed for allies only (Signal Allies).
*Source:* wiki, Mini map / Signal Allies; GAP 11.1. · *Impl:* minimap
rendering shipped (terrain, nodes, buildings, entities, viewport rect);
fog shading shipped with M7 (VIS-10); flares are a client command + net
message (NET-10). · *Status:* **DIFFERS** (map and fog yes;
flares/alert flashes no).

**INF-05 — Selection panel shows identity and health.** A single selected
unit/building shows its portrait, HP bar, attack/armor stats, and (for
your production buildings) queue and rally (INF-06). A group selection
shows a grid of portraits with per-unit HP tinting. Enemy entities show
HP when visible; their orders, queues, and cargo never.
*Source:* GAP 11.5, 11.6. · *Impl:* the selection panel ships with kind
name + HP (M3's HP model + ABI) and, for production buildings, the
queue contents — units and research alike (M6); attack/armor stats
are not shown, and rally points have no visual marker yet.
· *Status:* **DIFFERS**.

**INF-06 — The command panel is the tech tree's front door.** A selected
unit/building's panel shows exactly its available actions (build menu,
train buttons with cost tooltips, research buttons, stance buttons,
special commands like Town Bell / Set Rally), greyed with reasons when
unavailable (prerequisites TEC-07, housed BLD-10, unaffordable ECO-01).
A full-screen tech-tree view shows the whole graph.
*Source:* GAP 11.4. · *Impl:* the command panel ships in basic form
(villager → House/Farm with placement flow; TC → Train with the
queue count + progress in the selection info line, no badge on the
button face (2026-10-19), and housed explanations) — buttons are data-driven from the same
availability functions the sim validates with (BLD-15), so panel and sim
can never disagree. · *Status:* **DIFFERS**.

**INF-07 — Public vs. private information.** Always public in-match:
every player's score (VIC-04), current Age, and population. Always
private: stockpiles, exact army composition (beyond what scouting sees),
research in progress, and queue contents. The diplomacy screen shows
stances and hosts tribute (TRD-04).
*Source:* GAP 11.7; wiki, Score (live visibility). · *Impl:* the ABI
exposes per-player public fields only; even in single-player the client
should read through that lens so multiplayer needs no information rework
(VIS-09's discipline applied to stats). · *Status:* **MISSING** (one
player; nothing to hide yet).

**INF-08 — The game clock counts game time.** The displayed clock is
elapsed *game* time (ticks ÷ 60), formatted mm:ss (h:mm:ss past an hour),
so speed settings (SIM-11) stretch it exactly as they stretch the game.
*Source:* GAP 11.9. · *Impl:* shipped — the top bar derives mm:ss from
`tick_count()`. · *Status:* **MATCHES**.

**INF-09 — Order feedback is drawn, not simulated.** Move markers,
attack pings, queued-order flags (SEL-12), and the placement ghost are
pure client renderings of commands the player just gave; they appear in
no hash and no replay data beyond the commands themselves.
*Source:* GAP 11.8; NET-08/10 (what replays contain). · *Impl:* the
placement ghost ships (invalid = red); the order queue shipped with M4
(SEL-12), but move/attack markers and queued-order flags are still not
rendered — the sim's queue is not exposed to the renderer. · *Status:*
**DIFFERS**.

**INF-09a — The click marker (transient order confirmation).** A
right-click whose order resolves to a GROUND target (plain move from
the main view or the minimap, a queued ground move, a rally-point set)
plays a brief marker at the exact clicked point: four red arrowheads
start apart on the diagonals, converge until they meet, then fade —
~0.45 s total. A right-click on an OBJECT (resource node, unit,
building — gather / attack / repair / drop-off) shows NO ground
marker, and no target pulse is drawn either: none is visible in the
evidence. Build placement shows no marker (its ghost is the feedback).
*Evidence:* Study 1 DE footage (`~/workspace/skirmish-study/video1/`,
Spanish beginner tutorial), frame-analysed 2026-10-20 — the marker is
visible mid-animation in `frames/f_011.jpg` (red X in the fog at the
scout's destination) and its full life was measured from the raw
video at 29.97 fps: heads start separated, converge to an X over
~0.3 s, collapse and vanish by ~0.45 s; red throughout (sampled
pixels in the #d2342c–#e23b2e family). The same marker fires on
repeated ground clicks throughout the video (e.g. the scout's
re-tasking clicks at ~80.7–83 s), including clicks into unexplored
fog — it is the player's own order echo, not world information. The
original Age of Kings manual separately documents the QUEUED form:
"A waypoint marker appears at each point" (Shift + right-click) —
in current DE those queued markers are persistent flags (Study 2,
flag grounded in `frames/f_006.jpg`); the persistent-flag half
remains open below. The official support article "Waypoint Markers"
adds that, for queued group orders, only the group leader's markers
display. Attack-move does not exist as an order in Skirmish II's
current scope, so no red-variant is invented for it. *Impl
(2026-10-20):* `play/clickmarker.js` — pure mapping (resolved order
kind -> marker) + animation math, unit-tested; `app.js` spawns the
marker from `smartOrderAt`/`orderAt` (rally sets and pure ground
moves only; any object resolution returns earlier and spawns
nothing) and draws it after the fog dim, like DE. LOW tier shortens
it to 300 ms, never removes it. Client-only: no sim, command or
hash impact. · *Status:* **MATCHES** for the transient marker;
queued-order persistent flags remain **DIFFERS** under INF-09.

**INF-10 — Sound is an information channel.** Unit acknowledgements,
work sounds, combat noise scaled by distance to the camera, alert horns
(INF-01), and milestone stingers carry game state to the player's ears.
All audio is original (clean-room), generated/synthesized in-house; no
reference audio asset may be used.
*Source:* GAP §12 (audio system); clean-room doctrine (see the clean-room section of the project README). ·
*Impl:* renderer-side only, driven by the event feed (INF-01); zero
audio exists in the client today. · *Status:* **MISSING**.

**INF-11 — Scores and statistics close the loop.** During the match, a
scoreboard shows live scores/ages/populations (INF-07). After the match,
the statistics screen presents the full timeline: scores over time,
military/economy breakdowns (VIC-04), units killed/lost, resources
gathered, technologies, and the map revealed.
*Source:* GAP 11.10. · *Impl:* post-game stats are an aggregation over
the VIC-04 run counters + a score history sampled per minute (client-
side sampling of public data). · *Status:* **MISSING**.

---

## A. Rule counts

216 rules total. Counts verified mechanically against the status tags in
this document (2026-10-18, M8 audit; re-verified for M9 the same day —
the open-games lobby changed no status, see the M9 note in section B;
updated 2026-10-19 for BLD-16, the Town Center footprint rule —
§7 gains one rule, one MATCHES; updated 2026-10-19 for BLD-17,
repair — §7 gains one rule, one DIFFERS, and BLD-07 moves
MISSING → DIFFERS; updated 2026-10-20 for INF-09a, the click
marker — §18 gains one rule, one MATCHES; SEL-17's status was
clarified the same day — its eligibility widened to all scouts,
so the entry is now a full MATCHES against current DE, with no
change to the totals: the entry's lead tag was already MATCHES).

| § | System | Rules | MATCHES | DIFFERS | MISSING |
|---|--------|------:|--------:|--------:|--------:|
| 1 | Simulation & time (SIM) | 12 | 10 | 1 | 1 |
| 2 | Map, terrain & elevation (MAP) | 12 | 7 | 4 | 1 |
| 3 | Visibility & exploration (VIS) | 10 | 8 | 1 | 1 |
| 4 | Selection & orders (SEL) | 16 | 12 | 3 | 1 |
| 5 | Movement, pathfinding & collision (MOV) | 12 | 3 | 4 | 5 |
| 6 | Economy & drop-off (ECO) | 17 | 13 | 2 | 2 |
| 7 | Buildings & placement (BLD) | 17 | 9 | 6 | 2 |
| 8 | Combat & damage (CMB) | 16 | 10 | 3 | 3 |
| 9 | Projectiles & ballistics (PRJ) | 10 | 4 | 4 | 2 |
| 10 | Unit classes & counters (CLS) | 8 | 5 | 0 | 3 |
| 11 | Technologies & age advancement (TEC) | 11 | 3 | 5 | 3 |
| 12 | Monks, conversion & relics (MNK) | 13 | 0 | 0 | 13 |
| 13 | Trade, market & tribute (TRD) | 8 | 0 | 0 | 8 |
| 14 | Garrison, transport & repair (GAR) | 10 | 0 | 0 | 10 |
| 15 | AI behavior & difficulty (AI) | 9 | 4 | 3 | 2 |
| 16 | Victory, scoring & game modes (VIC) | 12 | 0 | 1 | 11 |
| 17 | Multiplayer determinism (NET) | 11 | 4 | 5 | 2 |
| 18 | Information rules (INF) | 12 | 3 | 4 | 5 |
| | **Total** | **216** | **95** | **46** | **75** |

Reading: the foundation systems (§1, §4–§7) are largely in place; the
combat stack (§8–§10) landed with M3, the age/technology layer
(§11) with M6, and the world itself (§2/§3: generated maps, terrain,
elevation, fog) with M7, so the remaining greenfield is concentrated
in the systems whose content has not been built yet — monks (§12),
trade (§13), garrison (§14), the water domain (ships, docks,
fishing), and cliffs. Multiplayer's transport (§17) landed with M8;
its remaining gaps are snapshots (NET-09) and diplomacy (NET-11).

---

## B. Implementation order — the dependency chain

Rules unlock in dependency order; each milestone below closes the rules
listed (the roadmap in GAP-ANALYSIS §"Recommended sequencing" owns the
milestones — this section maps rules onto them).

**M3 — Combat core + first empire.** The big one: CMB-01…16 (the damage
formula, HP model, attack cycle, acquisition, stances), PRJ-01…10
(projectiles, RNG-accuracy discipline per SIM-04/NET-06), CLS-01…05, 08
(class tables), MOV-01 (per-unit speeds), MOV-04 (separation suppression
while engaged), MOV-05 (collision radius data), SIM-06 (subsystem
hashes). Prerequisite refactor: HP/owner fields on entities (MAP-12's
owner lands here), an event feed for INF-01/02/10. Acceptance: CMB-07's
duel test; golden-hash determinism tests for the first RNG consumer
(PRJ-03).

**M4 — Input & HUD completion.** SEL-05, SEL-07, SEL-08, SEL-09, SEL-10,
SEL-12 (order queues — sim change), SEL-13 (rally points), SEL-16;
INF-03, INF-05, INF-06 completed forms; MOV-06, MOV-07, MOV-08
(groups/formations ride on the order queue). Also the known
*behavioral divergences* worth fixing while touching economy code:
ECO-09 (gatherer retargeting). (BLD-04, the multi-builder formula, was
fixed ahead of schedule on 2026-10-16 — see §7.) *(Shipped
2026-10-16: SEL-05/08/09/12/13/16, ECO-09 and INF-03 complete;
SEL-07 core (groups 1–9; the Alt layer and the reference Shift
semantics stay open); SEL-10 partial — H only; MOV-06/07/08
formation groundwork landed as formation-keeping group moves in
the client, the rules themselves stay open; INF-05/INF-06
completed forms still open.)*

**M5 — Feel, AI, animals, win/lose.** AI-01…05 (scripted AI), AI-06…08
(herdables/huntables/predators), ECO-11, ECO-12, ECO-13 (animals complete
the economy), VIC-01 (conquest win/lose — the first victory rule; the
rest of §16 waits for its systems), SEL-14 (stop/delete), BLD-02
(cancel/refund). *(Shipped 2026-10-16: AI-01…05 — a deterministic
scripted opponent (Easy/Normal profiles) inside the sim, playing only
through the Command queue against per-player state (stockpiles,
population, building ownership); AI-06/AI-07 confirmed in their M3
forms and exercised by the AI; VIC-01 Conquest with the stricter
all-buildings-and-units predicate (see §16). ECO-11…13, SEL-14, BLD-02
and AI-08 stay open.)*

**M6 — Ages, techs, full building set.** TEC-01…11 (age ladder, patch
engine), BLD-06 (real footprints), BLD-07 (repair), BLD-11, BLD-15,
ECO-06 (Mill/Lumber/Mining camps — zero-learning-critical), ECO-07
(force drop-off), ECO-14 completed (farm techs, reseed), GAR-01…08
(garrison, arrows, Town Bell), MNK-01…13 (monks, conversion, relics),
TRD-01…08 (market, trade), VIC-02…05 (standard victory, score), INF-07,
INF-11. *(Shipped 2026-10-16, scoped to the first age rung: the
Dark → Feudal ladder with TEC-02/03 and the shared TC queue; Loom and
the first Blacksmith line (Fletching, Scale Mail, Padded Archer
Armor) plus the Man-at-Arms line upgrade through the TEC-06 patch
engine; ECO-06/07 complete (nearest-valid drop-off routing, force
drop-off, the Farm-needs-Mill gate); the Feudal buildings (Archery
Range — Archers move there from the Barracks — Stable, Blacksmith)
with BLD-15 gating; CLS-03/04. Carried to later milestones: the
Castle/Imperial rungs, BLD-07 repair, BLD-11's automatic building
step-up, ECO-14's farm techs/reseed, GAR-01…08 garrison + Town Bell,
MNK-01…13 monks, TRD-01…08 market/trade, VIC-02…05, and queue
cancel/refund (TEC-05). §8–§10 were also re-audited against the
shipped M3 code in this pass — their MISSING tags were stale.)*

**M7 — World: maps, terrain, fog, water.** *(Shipped 2026-10-17:
the seeded Arabia generator (MAP-09) at Tiny size (MAP-02 in part)
with mirrored fair starts; terrain + water layers (MAP-03/08 in
part — water as terrain only); a 5-level height field (MAP-05) and
the binary hill bonus through CMB-01 (MAP-06); MAP-04 completed
(trees as blocking nodes); fog of war end-to-end (VIS-01…07,
VIS-10, INF-04's fog half) with the AI's targeting bound by its own
grid. Trimmed, deliberately: MAP-07 cliffs, the water domain —
BLD-14 docks, ECO-15 fishing, ships, MOV-11's water domain — and
the larger map sizes; VIS-08 waits on teams (M8).)*

**M8 — Multiplayer & match framework.** NET-01…11 (lockstep transport,
N+2 scheduling, hash comparison, replay/save formats, diplomacy),
VIC-06…10, VIC-12 (mode deltas), SIM-11 (speed settings), AI-09 +
VIC-08 (KotH), SEL-15/INF-06 completed forms for team play.
*Status: shipped 2026-10-18 for the lockstep core (NET-01…08 and
NET-10 re-audited above); the VIC/SIM-11/AI-09 items and NET-09/11
remain future work.*

**M9 — Open-games lobby.** A public directory of waiting games on
the relay (rooms report their own socket counts; `GET /lobbies`
lists them) and the Lobby Browser the play page opens on.
*Status: shipped 2026-10-18. Re-audit: no NET or INF status
changes — no rule in §17 or §18 describes a lobby browser, and the
directory is pre-match transport metadata (room id, region, socket
count) that never enters hashed state, so NET-01's letter is
untouched and INF-07's in-match information split is unaffected.
The counts in section A are unchanged.*

**TC footprint rule (2026-10-19).** BLD-16 shipped: the Town
Center's 4×4 is passable to units (it never enters the nav
blocking grid) and reserved against building placement, both
players'. No other rule's status changed; section A's counts are
updated for the one new rule (214 total, 94 MATCHES). The golden
hash is unchanged — the reservation runs only inside build
validation.

**Repair (2026-10-19).** BLD-17 shipped, and BLD-07 re-audited
from MISSING to DIFFERS: villagers repair their own complete,
damaged placed buildings at the documented flat rate (750 HP/min
first repairer, +375 HP/min each additional) for a drip totaling
50% of the build cost over a full repair, stopping idle when the
owner cannot pay; the queued form and the lockstep wire case
landed with it, and the scripted AI never repairs. Section A's
counts are updated for the one new rule and the one status move
(215 total, 94 MATCHES, 46 DIFFERS, 75 MISSING). The golden hash
is unchanged — repair state is hashed only when a repair is in
flight or a building stands. Town Center repair stays out of
scope (the TC is an entity, not a placed building).

Cross-cutting at every milestone: NET-05/06 discipline (no local factor
in the sim; RNG only in fixed iteration) and VIS-09 (fog hides
information, never truth — it gates knowledge consumers only) are
invariants, not milestones.

---

## C. Out of scope for this rulebook

The following hook into the rules above but are **empire/content design**,
not mechanics law; they will be specified per-empire when Skirmish II's
original empires are designed:

- **Per-civilization bonuses** (e.g. cheaper buildings, faster-working
  villagers, free technologies on age-up) and **team bonuses** (e.g. the
  Teutons' conversion-resistance bonus that appears in MNK-05 as a table
  row). They are TEC-06 patches / MNK-05 rows — the hooks are documented;
  their values are content.
- **Unique units** (one–two per empire, Castle/Dock-trained) and
  **unique technologies** (TEC-11 reserves their slots; their effects
  are content).
- **Civilization availability deltas** (which generic techs/units an
  empire lacks) — data-table presence flags, not rules.
- **Regional/architecture sets and unit skins** — presentation (see
  SPRITE-PIPELINE.md).
- **Campaigns, scenarios, and the scenario editor** — GAP-ANALYSIS
  excludes them from the target as well; the rules here describe
  skirmish/multiplayer matches. (Scenario-only mechanics noted where a
  source forced the issue: hero regeneration CLS-06, Gaia object
  behavior AI-06 note.)
- **Cheat codes** — inputs outside the command discipline (NET-01);
  if ever added, they must enter as commands like everything else.
- **Exact per-unit/per-building stat values** — MECHANICS-STUDY.md owns
  the tables; this book owns behavior.
- **The starting package** (units, scouting, first-minutes tasks) —
  OPENING-SPEC.md owns it; ECO-17/MAP-09/VIS-07 reference it.
