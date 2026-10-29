# Castle P3a — STATUS (bot-loop front, run 2026-10-29 13:22 PT)

Branch `botloop-1322-p3a` in worktree `~/workspace/skirmish-botloop-1322-p3a`,
based on `origin/main` 21938db. **Nothing pushed, nothing published.**
The package itself was committed by the parent run as `2398680`
("Castle P3a: Castle building + weapon volley + Petard + client
wiring (bot-loop front 9, 2026-10-29)"); this STATUS file rides in a
follow-up commit by the front worker with the final test tally.

## What landed (complete P3a scope)

- **Sim — Castle building** (`sim/src/build.rs`): `BuildingKind::Castle`
  (tag 15, append-only): 650 stone / 0 wood, 200 s build (12000 ticks),
  4×4, HP 4800, armor 8 melee / 11 pierce, LoS 11, +20 pop support,
  Castle Age, no building prerequisite, production building
  (`is_production`/`produces_units`), trains Petard only.
- **Sim — Castle weapon** (BLD-18 second user): `BuildingWeapon` gains a
  `volley` field (Watch Tower 1 — unchanged fire; Castle 5).
  `CASTLE_WEAPON`: 11 pierce +2 vs Spearmen, range 8 (min 1), RoF 2.0 s,
  accuracy 100%. `combat::building_weapon_release` looses `volley`
  projectiles per release (same aim/flight/snapshot; no extra RNG at
  accuracy 100).
- **Sim — Petard** (CMB-17): `EntityKind::Petard` (tag 15): 65 F / 20 G,
  25 s train at the Castle only, HP 50, speed 0.8, LoS 4, armor 0/2,
  attack 25 melee +900 vs Walls&Gates +500 vs Standard Buildings
  +100 vs Castles +60 vs Siege (inert — new `classes::SIEGE` has no
  member kind until P4; `classes::CASTLE` is the Castle building's
  membership in `building_spec`). Detonation: `Sim::petard_detonate`
  called from both melee strike resolutions — primary takes the strike,
  everyone else (friend or foe, CMB-12) within 0.5 tiles of the
  detonation point takes the attack table, Petard is consumed (death
  sweep removes it the same tick). A whiffed strike does not detonate.
- **WASM**: `push_build_castle` (+`_queued`), `push_train_petard`.
- **Client**: kind tables appended (BUILD_COST 0 / BUILD_COST_STONE 650 /
  BUILD_SIZE 4 / BUILD_MAXHP 4800 / names / icons / minimap color);
  villager build menu gains the Castle (Castle-Age gate in the panel and
  in the placement mirror); Castle panel context with icon-only Train
  Petard button (cost in the title text, per panel law); netplay
  vocabulary (build k≤15, train_at k=15, TRAIN_AT_FNS/BUILD_FNS);
  hotkeys (Castle panel Q; build page — see compromise below);
  i18n EN/PT/ES (Castle/Castelo/Castillo, Petard/Petardo/Petardo);
  original art: programmatic blockout `assets-original/buildings/castle.png`
  + `play/ui-art/icons/castle.png` + `i-castle`/`i-petard` SVG symbols +
  vector fallback style/feature in `drawNewBuilding` + `drawPetard`.
- **Rulebook**: new **CMB-17 (MATCHES)**; BLD-18 narrowing note (Castle
  weapon + volley; GAR-05 arrows remain the P3c remainder); summary
  table recounted — §8: 17 rules 16/0/1; book: **222 rules,
  133 MATCHES / 28 DIFFERS / 61 MISSING** (delta verified mechanically
  against HEAD: exactly +1 MATCHES entry in §8, no flips).

## Tests (this front, shared CARGO_TARGET_DIR)

- `cargo test -p skirmish-sim --lib`: **105 passed, 0 failed** (incl. the
  8 new `castle_p3_tests`: table rows, placement gates + 650 stone charge
  + pop support, Castle-only Petard training with exact charges, bonus
  table via the damage formula, 5-projectile volley + kill, wall
  detonation, blast friendly fire).
- `cargo test -p skirmish-sim --test determinism`: **7 passed** —
  **golden hash UNCHANGED** (see below).
- `cargo test -p skirmish-sim --test towers`: **2 passed**.
- `cargo test -p skirmish-sim --test castle_age`: **3 passed**
  (1358 s wall under CPU contention — full age-progression games).
- Also green: `--test combat` **58**, `--test construction` **15**,
  `--test melee_building` **7** — 197 sim tests total across the suites
  run here; the parent gate runs the full 28-suite set.
- Play smokes (pkg built): airoom, auto-explore, carryprops,
  fullscreen, hotkeys (after pinning the eco page's new 8th cell),
  lobby, orders, prematch, reload **OK**; i18n all cases passed;
  feedback Act A **passed (exit 0)** — the feedback smoke plays two
  full AI matches per act in real time (~19 min/act on this 2-core
  box) and exceeds ad-hoc 180–300 s timeouts; Act B was left running
  with a 2400 s budget (log `/tmp/fb_b.log`). The feedback widget is
  untouched by this package.
- `node --test web-preview/play/*.test.mjs`: **425 passed, 0 failed**
  (3 tests updated for the 16th building: page partition, eco-page
  layout, netplay vocabulary).
- `cargo check --target wasm32-unknown-unknown`: OK. The pkg was then
  built with `web-preview/build.sh` (gitignored output) and the play
  smoke scripts run against it — see the smoke line in the final
  report (first attempt without a pkg fails every wasm-loading smoke
  with ENOENT on `pkg/skirmish_sim_bg.wasm`, as expected pre-build).

## Golden hash

**Did not move.** Probe attribution: the golden scenario (seed 42, no
commands) contains no Castle, no Petard and no armed-building fire, and
no existing state byte changed — new tags are append-only (building 15,
entity 15, classes 19/20), the volley field is not hashed state (it is a
static profile constant; the tower's value is 1 = old behavior), and the
detonation path runs only for the Petard kind. `determinism` suite green
with the pre-P3a expected values untouched.

## Compromises / notes for the parent

- **Build-page placement**: the villager Military page is full
  (8 kinds + Back in the 3×3 grid), so the Castle takes the Economic
  page's last free cell (slot 7, badge X), beside the Town Center.
  DE puts the Castle on the military page; moving it there needs a
  page redesign — recorded here rather than improvised.
- **Petard attack delay / reload** adopt the foot-melee design constants
  (30 ticks / 2.0 s), recorded in `units.rs` — the dossier does not pin
  them and the reload never governs (single strike by construction).
- **Blast falloff**: CMB-17 ships the single documented 0.5-tile radius
  at full table damage; DE's falloff tiers are PRJ-07's siege package.
- The scripted AI does not build Castles or train Petards (same recorded
  gap as BLD-18's towers).
- `web-preview/pkg/` is gitignored build output; it was built locally
  only to run the smokes and is not part of the commit.
