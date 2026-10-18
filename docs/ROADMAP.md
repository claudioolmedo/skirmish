# Skirmish II — Roadmap: What's Missing (and how you can build it)

**This document is the contributor's map of what is missing.** Skirmish II's
design target is precise: a player who knows Age of Empires II should need
**0% learning** to play — every mechanic behaves the way their hands already
expect. We measure ourselves against a complete, public rulebook of the
reference game's mechanics, rule by rule. Whatever the rulebook marks
DIFFERS or MISSING is work waiting for a builder. **That means you.**

If you build one of the missing rules, you have moved the game. Pick
something below, check its rule ID in the
[Mechanics Rulebook](MECHANICS-RULEBOOK.md), and see
[CONTRIBUTING.md](../CONTRIBUTING.md) for how to build, test, and submit.
Why the project is built to outlive its makers — and to fund itself doing
it: [The Self-Sustaining System](SUSTAINABILITY.md).

---

## What exists today

| Milestone | What shipped |
|-----------|--------------|
| **M0** ✅ | Deterministic simulation scaffolding: pure-Rust headless sim, fixed-point math, seeded RNG inside the state, per-tick state hash, headless replay tests |
| **M1** ✅ | Movement: A* pathfinding on a grid + steering (separation, arrival), real Move commands |
| **M2** ✅ | Economy: 4 resources, nodes (tree / berry / gold / stone), villagers that gather → carry → deposit at reference rates, Town Center trains villagers |
| **M2.5** ✅ | Construction: House (+5 population) and Farm built by villagers with in-sim placement validation, population cap, start/pause menus, fullscreen play page at skirmish.online/play |
| **M3** ✅ | Combat core: HP and damage with armor classes, attack command + auto-acquisition, projectiles with seeded-RNG accuracy, Gaia animals (sheep, boar, deer) with the hunt economy (carcasses, decay, hunt carry 35), starting Scout Cavalry, Barracks training Militia and Archer |
| **M4** ✅ | Command completeness: queued (shift) orders, rally points, control groups, idle villager/military keys, selection cap 60, minimap orders, formation-keeping group moves, gatherer retargeting |
| **M5** ✅ | Scripted AI opponent + Conquest: per-player stockpiles/population/buildings, mirrored enemy start, an economy-and-army script (Easy/Normal) playing only through the command queue, victory/defeat |
| **M6** ✅ | Ages & technologies (first rung): Dark → Feudal research and Loom in the Town Center's shared queue, per-player tech patches applied at stat lookup, the drop-off camps (Lumber Camp / Mill / Mining Camp) with nearest-valid-deposit routing and force drop-off, Farm gated on a Mill, Feudal buildings (Archery Range — Archers move there — Stable, Blacksmith with Fletching / Scale Mail / Padded Archer Armor), the Man-at-Arms line upgrade, and the AI taught the same layer |

The interface on the play page already follows the classic layout — top
resource bar, bottom-left command panel, bottom-center selection panel,
bottom-right minimap — with 100% original art.

## What's missing — the rulebook scoreboard

The [Mechanics Rulebook](MECHANICS-RULEBOOK.md) states **213 behavioral
rules** across 18 systems (audit dated 2026-10-16; statuses are re-checked
as milestones land):

| § | System | Rules | Match | Differ | Missing |
|---|--------|------:|------:|-------:|--------:|
| 1 | Simulation & time (SIM) | 12 | 10 | 1 | 1 |
| 2 | Map, terrain & elevation (MAP) | 12 | 2 | 2 | 8 |
| 3 | Visibility & exploration (VIS) | 10 | 1 | 1 | 8 |
| 4 | Selection & orders (SEL) | 16 | 12 | 3 | 1 |
| 5 | Movement, pathfinding & collision (MOV) | 12 | 3 | 4 | 5 |
| 6 | Economy & drop-off (ECO) | 17 | 13 | 2 | 2 |
| 7 | Buildings & placement (BLD) | 15 | 8 | 4 | 3 |
| 8 | Combat & damage (CMB) | 16 | 10 | 3 | 3 |
| 9 | Projectiles & ballistics (PRJ) | 10 | 4 | 4 | 2 |
| 10 | Unit classes & counters (CLS) | 8 | 5 | 0 | 3 |
| 11 | Technologies & age advancement (TEC) | 11 | 3 | 5 | 3 |
| 12 | Monks, conversion & relics (MNK) | 13 | 0 | 0 | 13 |
| 13 | Trade, market & tribute (TRD) | 8 | 0 | 0 | 8 |
| 14 | Garrison, transport & repair (GAR) | 10 | 0 | 0 | 10 |
| 15 | AI behavior & difficulty (AI) | 9 | 4 | 3 | 2 |
| 16 | Victory, scoring & game modes (VIC) | 12 | 0 | 1 | 11 |
| 17 | Multiplayer determinism (NET) | 11 | 2 | 1 | 8 |
| 18 | Information rules (INF) | 11 | 2 | 4 | 5 |
| | **Total** | **213** | **79** | **38** | **96** |

Companion documents: the [Gap Analysis](GAP-ANALYSIS.md) inventories every
missing feature against the code; [Opening Spec](OPENING-SPEC.md) specifies
the standard start minute-by-minute; [Mechanics Study](MECHANICS-STUDY.md)
holds the numbers (stats, rates, costs); [openage Research](OPENAGE-RESEARCH.md)
holds the engine lessons; the [Sprite Pipeline](SPRITE-PIPELINE.md) documents
how original art is produced.

Full DE parity (45+ civilizations, campaigns, editor) is years of work and
is **not** the target. The target is the **mechanically equivalent core
game**: the complete generic tree for 1–3 original empires, land random
maps with fog and minimap, the full selection/command model, a scripted AI
opponent, conquest + standard victory, and 2-player lockstep multiplayer.

## Most wanted — the top-15 "feel parity" list

From the [Gap Analysis](GAP-ANALYSIS.md): the items that most make Skirmish II
*feel like the same game*, in order. Status notes are current.

1. **Minimap** — ✅ shipped on the play page, orders via minimap included (SEL-16, M4)
2. **Combat core** — ✅ shipped in M3 (queued orders ✅ M4; attack-move itself still open)
3. **Scripted AI opponent + conquest win/lose** — ✅ shipped in M5; the sandbox is now a match
4. **Control groups** — ✅ shipped in M4 (SEL-07, groups 1–9); pure muscle memory for anyone who has played
5. **Idle villager key + idle indicator** — ✅ shipped in M4 (SEL-08, INF-03); the heartbeat of economy play
6. **Queued orders + rally points** — ✅ shipped in M4 (SEL-12, SEL-13); orders feel professional instead of one-shot
7. **Stances + formations** — army control stops feeling like a blob
8. **Combat collision feel** — bump-stop in melee, overlap while engaged (partly in via MOV-04)
9. **Ages** — ✅ shipped in M6 (the Dark → Feudal rung, with Loom and the first technology lines; Castle/Imperial rungs come with their content)
10. **Fog of war + exploration** — M7; scouting, tension, surprise
11. **Random maps** — M7; a different Arabia every match; replayability itself
12. **Selection panel + command card with HP** — ✅ shipped on the play page
13. **Garrison + Town Bell** — defense drama: bell rings, villagers run, arrows fly (planned for M6; carried to a later milestone)
14. **Sound: unit barks + alert horn + completion chimes** — feel is half audio
15. **Military variety** — knight/archer/siege/monk lines with real counters (archer line started in M3)

## Milestone order

- **M4 — Input & HUD completion.** ✅ Shipped 2026-10-16. Control groups,
  idle keys + indicators, go-to TC (H), queued (shift) orders, rally
  points, minimap orders, selection cap 60; formation-keeping group
  moves; the ECO-09 retargeting fix. (Rulebook §B: SEL, INF, MOV-06…08 —
  stop/delete moved to M5 with SEL-14.)
- **M5 — Scripted AI opponent + win/lose.** ✅ Shipped 2026-10-16. A
  deterministic scripted opponent inside the sim (`sim/src/ai.rs`)
  playing only through the Command queue: mirrored enemy start,
  per-player stockpiles/population/buildings, an economy script
  (villagers, houses, Barracks, Militia/Archers) with Easy/Normal
  profiles, attack waves, home defense, and Conquest victory/defeat
  (hash-covered match state). Sandbox remains as a mode. (Rulebook §B
  and §15/§16 carry the per-rule record; stop/delete (SEL-14) remains
  open. The animal-economy tags ECO-11…13 were confirmed MATCHES in
  the M6 re-audit.)
- **M6 — Ages & technologies.** ✅ Shipped 2026-10-16, scoped to the
  first rung. The Dark → Feudal ladder (500 food, 130 s, two qualifying
  buildings) and Loom, researched in queues shared with unit production;
  a per-player technology substrate (age layers + researched patches
  folded into every stat lookup, hash-covered); the economy buildings —
  Lumber Camp, Mill, Mining Camp — with nearest-valid-deposit routing,
  force drop-off, and Farms gated on a Mill; the Feudal buildings
  (Archery Range, Stable, Blacksmith) and the Man-at-Arms upgrade; the
  scripted AI taught the same layer. Carried to later milestones: the
  Castle/Imperial rungs, Market/Monastery/University, garrison + Town
  Bell, monks and relics, market and trade, repair, and queue cancel.
- **M7 — Terrain, fog & random maps.** ✅ Shipped 2026-10-17. A
  deterministic seeded Arabia generator in the sim replaces the fixed
  arena: Tiny 120×120 tiles, per-start resource packages (OPENING-SPEC
  §2.3) mirrored for exact fairness, validated connectivity (bounded
  regeneration, fixed fallback — never a hang). Terrain layers (land /
  water), a 5-level height field feeding the binary hill bonus
  (CMB-01), trees as blocking nodes, and per-player fog of war —
  hash-covered, recomputed on a bounded interval — with the scripted
  AI's targeting bound by its own fog and its economy assigned by
  Town-Center proximity. Trimmed, deliberately: cliffs, the water
  domain (ships, docks, fishing), and the larger map sizes — the
  generator's vocabulary and validation already cover water as
  terrain; the rest lands with the naval content.
- **M8 — Lockstep multiplayer.** Commands-only networking on the
  deterministic sim (per-tick hash is the desync detector), send-a-link
  lobby, replay viewer. **Multi-region from day one**: one relay per
  match, placed near its players on edge hosting — ping is the
  product. The region list is fixed and the same for everyone, and
  while the player base is small the PLAYER picks the server:
  the lobby shows the live ping for every region (nearest one
  pre-highlighted), so friends choose the same location and
  everyone knows their ping before they play. Capacity grows by
  adding regions, not bigger machines. Anonymous match
  telemetry and replay submission land with it, feeding the
  fidelity-and-quality loop (SUSTAINABILITY.md §5).

## Good first contributions

Small, self-contained, and each one closes a named rule. The rulebook entry
for each ID gives the exact behavior and an implementation note.

*The first batch below all landed in M4 (2026-10-16) — kept as a record of
what a good first contribution looks like here. New candidates are gathered
from the rulebook's DIFFERS/MISSING entries as issues are opened.*

| Rule | What was built | Scope note |
|------|---------------|------------|
| **ECO-09** ✅ | Gatherers retarget to a same-resource neighbor node when theirs depletes, instead of going idle | Sim-only; a bounded deterministic search at depletion time + tests |
| **SEL-05** ✅ | Any single selection is capped at 60 units | Client-only; enforced in the one selection funnel |
| **SEL-16** ✅ | Right-click on the minimap issues the smart order at that world point | Client-only; minimap geometry conversion existed both ways |
| **SEL-07** ✅ | Control groups: Ctrl+number assigns, number recalls, double-press centers | Client-only; groups store unit ids, the sim stays the truth |
| **SEL-10** ✅ (H) | Go-to keys: H selects and centers the Town Center | Client-only keymap; the TC was already selectable |
| **SEL-08** (+ **INF-03**) ✅ | Idle villager key that cycles idle villagers, plus the always-visible idle indicator in the top bar | One small ABI addition (idle id lists) + client key/indicator wiring |
| **SEL-09** ✅ | Idle military key, the mirror of SEL-08 | Rode on SEL-08's ABI; the military flag per unit kind in the data table |

Want something bigger? Anything marked MISSING in the rulebook is fair
game — open an issue or a draft PR describing the rule IDs you intend to
implement, and build it the way [CONTRIBUTING.md](../CONTRIBUTING.md)
describes.
