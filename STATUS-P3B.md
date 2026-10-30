# Castle P3b — GAR core — STATUS (2026-10-29, bot loop)

Branch `botloop-1822-p3b` from origin/main `fd194a2`. Build only —
not pushed, not published. Parent runs the full gate.

## What shipped, per sub-slice

1. **Garrison state (GAR-01/02)** — `sim/src/garrison.rs` (new):
   host abstraction `GarrisonRef::{Building, TownCenter}` (the TC
   is an entity in this architecture, so its garrison/weapon/bell
   state lives on `TcState`; placed-building garrisons on
   `Building`). Eligibility per the RESOLVED scope column: TC 15
   foot-only; Watch Tower/Outpost 5 foot-only; Castle 20
   foot+mounted; production buildings 10, production-garrison ONLY
   via rally/newly-trained (eject-only, no re-entry); siege never;
   Petard not garrisonable. GAR-02 by construction + guards:
   garrisoned entities are excluded from the movement snapshot,
   grid-integrity displacement, work tick, visibility stamps, idle
   lists, queue advance, `orderable()`, sheep-capture counts,
   acquisition (attacker and target), `attack_step`, `apply_hit`,
   projectile intended/mishit resolution, Petard blast, and
   building/TC weapon targeting.
2. **Garrison order (SEL-11)** — commands `GarrisonBuilding` /
   `GarrisonTc`; units walk to the host's approach point and enter
   when within 3.0 world units of the footprint edge; capacity
   enforced at order time and at entry. Client: right-click on an
   own TC/tower/Castle issues the order from `orderAt` (a loaded
   villager clicking the TC still drops off first — ECO-07 keeps
   priority).
3. **Ejection (GAR-03)** — host destroyed (hooked in
   `destroy_building` and the TC death path) or ≤20% HP (checked
   in the garrison tick) ejects the garrison unharmed at ring exit
   points; re-garrison blocked while ≤20% HP (`hp*5 <= max`);
   voluntary Ungarrison button ejects everyone, idle.
4. **Shelter healing (GAR-04 narrowed, NOT flipped)** — base rates
   shipped: TC/towers 6 HP/min (0.1 HP/s), Castle 12 HP/min
   (0.2 HP/s), production 0, via a per-entity remainder
   accumulator. Herbal Medicine ×6 remains for P5.
5. **Arrows + TC weapon (GAR-05/06)** — total volley stored on the
   host record, recomputed on garrison change only: tower
   1 + min(extra, 4); Castle 5 + min(extra, 16); TC
   min(max(extra, 1), 10) while garrisoned, 0 when empty. extra =
   floor(contributed DPS ÷ building own DPS); villager = 2.5 DPS;
   foot archers contribute their effective pierce DPS via
   `stats_for`; mounted contribute 0. TC weapon
   (`TOWN_CENTER_WEAPON`: 5 pierce, range 6 tiles, RoF 2.0 s,
   adopted 21-tick delay, accuracy 100, no minimum range) fires
   ONLY while garrisoned, through the CMB-08 chain; towers/Castle
   release their stored volley through the existing BLD-18
   machinery.
6. **Town Bell (GAR-07)** — `TownBell` command + TC panel button:
   ring sends own villagers within 23 tiles (92 wu) of the TC, in
   entity-id order, to the nearest shelter with room (own TCs in
   tc-state order, then defensive buildings in id order),
   snapshotting position/task/node/building per villager; ring
   again ejects the bell villagers individually and restores the
   gather/build/repair task when its target still stands, else
   walks the villager back to the snapshot position.
7. **Rally-garrison (GAR-08)** — `apply_spawn_rally` resolves a
   rally point inside a garrisonable host's footprint to a
   garrison order on the SEL-13 spawn path; production hosts
   accept entry only on this path.
8. **Client** — wasm exports (`push_garrison_building/tc`,
   `push_ungarrison_building/tc`, `push_town_bell`,
   `building/tc_garrison_count`, `building/tc_garrison_arrows`,
   `tc_bell_active`, `entity_garrisoned`); local + netplay command
   dispatch and validation for the five new commands; Ungarrison
   button on TC/tower/Outpost/Castle/Barracks/Range/Stable panels
   and Town Bell button on the TC panel — icon-only, UNBADGED (the
   pinned DE panel letters leave those cells to their own
   occupants; the buttons answer to the mouse, like building
   Delete); garrison count + arrows-per-volley on the selection
   info line; garrisoned units hidden by the renderer and dropped
   from selection; i18n EN/PT/ES for all new strings; `i-bell` /
   `i-ungarrison` glyphs.

## Tests (exact counts)

- `cargo test -p skirmish-sim --lib`: **116 passed, 0 failed**
  (105 pre-existing + 11 new in `sim/src/garrison_tests.rs`:
  eligibility/capacities at data and command level, tower
  capacity, untouchable + not-idle, ≤20% ejection + re-entry
  block at the exact threshold, destruction + voluntary
  ungarrison, healing rates, arrow formula + caps, TC silent
  ungarrisoned then firing garrisoned, Town Bell shelter +
  restore, rally-garrison into tower and production building +
  no re-entry).
- `cargo test -p skirmish-sim --test determinism --test towers`:
  **7 + 2 passed, 0 failed**.
- `node --test web-preview/play/*.test.mjs`: **435 passed,
  0 failed** (new: netplay vocabulary + applyCommand mapping for
  the five commands; a panels source pin for the garrison
  buttons).
- WASM pkg rebuilt via `web-preview/build.sh` (pkg is gitignored;
  rebuilt so the parent's gate/serve uses the final sim).
- NOT run (per task): the full 28-suite cargo gate — the parent
  runs it.

## Golden hash attribution

`607d975bdd71e8eb` → **`77bebfe5bb56356b`** (seed 42, 360 ticks).
Probe: with ONLY the two unconditional garrison hash blocks
(building + Town Center garrison lists/arrow counts in
`hash_state`) disabled, the golden returns to exactly
`607d975bdd71e8eb`. All other P3b state (entity garrison flags,
bell snapshots, TC weapon state) is hashed conditionally behind
markers (0xB9/0xBE/0xB8) and never enters the no-command scenario.
Pin updated in `sim/tests/determinism.rs` with the attribution
note. NOTE for the parent: during this run a probe-restore copy
silently failed and was caught and repaired before commit — the
committed tree was re-verified end-to-end after the repair (lib
116, determinism 7, towers 2, node 435, all green).

## Rulebook flips (same commit, docs/MECHANICS-RULEBOOK.md)

- GAR-01, GAR-02, GAR-03, GAR-05, GAR-06, GAR-07, GAR-08 →
  **MATCHES**; GAR-01's text amended to the resolved per-building
  eligibility wording.
- SEL-11 → **MATCHES** (garrison context was its only remainder).
- GAR-04 **narrowed only** (base healing shipped; Herbal Medicine
  ×6 remains for P5) — status stays MISSING.
- BLD-18 **narrowed** (GAR-05 arrows remainder closed) — status
  stays DIFFERS.
- Counts recounted mechanically over the Status tags: §14 =
  7/0/3, §4 = 16/1/0, book = 222 rules, **142 / 26 / 54**.

## Incomplete / notes

- Nothing in scope is incomplete. Out-of-scope remainders stand
  as recorded in the rulebook: GAR-09 (transports), GAR-10 (rams),
  GAR-04's Herbal Medicine ×6 (P5).
- The Outpost is treated as a tower shelter (capacity 5 foot,
  heal 6/min, no weapon — it stores 0 arrows and never fires).
