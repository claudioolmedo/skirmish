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

For proposed follow-up work, see [Suggestions for improving Skirmish II](IMPROVEMENT-SUGGESTIONS.md): review suggestions with priorities and acceptance criteria, not implemented features.

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
  **Fix (2026-10-18) — pace correction.** Unit speeds had kept the
  M1/M2 prototype scale (ratios only), which on M7's real tile
  scale made travel ≈12× faster than the reference; all speeds are
  now the absolute study values at tiles/s × 4 ÷ 60, with
  `sim/tests/pace.rs` as the regression pin, plus the steering
  separation cap the slower scale required. See ARCHITECTURE.md,
  "Pace correction".
- **M8 — Lockstep multiplayer (shipped 2026-10-18).** Commands-only
  networking on the deterministic sim — the per-turn state hash is
  the desync detector — with a send-a-link lobby and a replay
  viewer. **Multi-region from day one**: one relay room per match,
  its Durable Object placed in the region the players picked —
  ping is the product. The region list is fixed and the same for
  everyone, and while the player base is small the PLAYER picks
  the server: the lobby shows the live ping for every region
  (measured as an echo round trip to an object placed in that
  region; the nearest one is pre-highlighted), so friends choose
  the same location and everyone knows their ping before they
  play. Capacity grows by adding regions, not bigger machines.
  Lockstep shape: 3-tick turns, commands stamped 4 turns (≈118 ms wall at 1.7×)
  ahead, adapting up when the link lags; a dropped player gets a
  90-second rejoin window (the match is rebuilt from its recorded
  command stream). Anonymous match telemetry (counters only) and
  downloadable replays landed with it, feeding the
  fidelity-and-quality loop (SUSTAINABILITY.md §5). The sim itself
  gained only additive pieces — a two-player mode, one per-seat
  command form, per-seat ABI wrappers; no behavior changed. See
  ARCHITECTURE.md, "M8 — lockstep multiplayer".
- **M9 — Open-games lobby (shipped 2026-10-18).** Online is the
  front door: the play page opens on a Lobby Browser in the
  traditional shape — a compact table of open games (auto-generated
  title, server/region, players as 1/2, your measured ping to that
  region), sortable by location, refreshed automatically every
  ~5 s with a manual Refresh beside it. Double-click a row (or
  select it and press Join Lobby) to join; Create Lobby hosts with
  the M8 region pick and link; solo play is one quiet tap under
  the table. The one server addition is a directory: each room
  reports its own socket count to a singleton Lobby object on the
  relay (`GET /lobbies`); it stores only the room's public id, its
  region and the count, pruned after 120 s without an update — no
  players, no state, no identifiers. The lobby carries the play
  page's single ad slot in a right sidebar (house promo today),
  visible only while waiting — never over the game. Zero sim
  changes. See ARCHITECTURE.md, "M8 — lockstep multiplayer"
  (directory bullet).
- **M10 — Room model refinement (shipped 2026-10-18).** Rooms
  work the way the reference game's lobbies do. Create **Public**
  (listed, anyone can join) or **Private** — listed WITH a lock
  (🔒), like DE's passworded games, never hidden; joining one, by
  list or link, runs a password check against the host's client
  over the room pipe (a courtesy lock: the password lives only
  in the host's browser memory, the relay stores nothing but the
  locked flag). The room screen is calm: the two seats with
  presence and ready states, one Start button per player — and
  the match begins by itself when BOTH players are ready (the
  big link field is demoted to one quiet "Invite link — copy"
  line). Starting passes through a **loading screen** where the
  heavy load actually happens — the lobby itself is light now:
  the .wasm and the artwork load only once a match begins (both
  players ready, or Play solo) — with each player's loaded state
  shown; the host sends 'go' only when both are loaded, and a
  player who leaves or fails to load (90 s) cancels the match
  for everyone, back to the Lobby Browser. One live-found bug
  fixed on the way: a waiting host's room no longer ages out of
  the directory (rooms heartbeat to it every ~45 s). The lobby
  and loading waiting slots rotate fresh house creatives every
  60 s while players wait. Zero sim changes. See
  ARCHITECTURE.md, "M8 — lockstep multiplayer" (room model,
  loading phase, directory bullets).
- **M11 — Lobby Browser fidelity pass (shipped 2026-10-18).**
  The browser is reshaped on the reference game's Lobby Browser
  (the Steam "Beginner's Guide to Multiplayer" screens Claudio
  pointed at — structure mirrored, all art and text our own): a
  named "Lobby Browser" screen whose table reads **Game Name ·
  Players · Map · Location · Server** — the Server cell is your
  measured ping to that region with a colored quality square
  (green under 120 ms, yellow under 220 ms, red above) — over
  one controls row: a live **Search** box, **Refresh**, a
  **Password** checkbox that shows or hides 🔒 rooms, and a
  **Server** filter (Any + the seven regions), all working on
  the real directory data. The sidebar keeps the waiting ad
  slot and gains a **Connection Quality** panel beneath it:
  every region with your live ping and its quality square,
  refreshed by the same probe cycle. One bottom action bar —
  Play solo (quiet), **Join Lobby** (enabled only when a
  visible row is selected; double-click still joins at once),
  **Create Lobby** (opens the M10 create panel) — replaces the
  per-row Join buttons and sortable headers, and an auto-
  refresh preserves the selection while its game still exists.
  And the pre-match state is absolute (Claudio, live phone
  test): until a match actually begins there is no game on
  screen at all — the canvas and every HUD panel stay hidden
  behind a single body-class toggle, and the lobby sits on the
  plain page background. Deliberately NOT built, because
  nothing backs them yet —
  they arrive with their features, not as dead UI: the Ranked /
  Spectate Games / Clans / Leaderboard tabs, the Game Mode /
  Map Style / Game Speed / Treaty / Victory filters, and a
  Restore button. Zero sim changes; relay untouched. See
  ARCHITECTURE.md, "M8 — lockstep multiplayer" (Lobby Browser
  shape bullet).
- **M12 — Configurable rooms: AI slot + match settings (shipped
  2026-10-18).** The host configures the room the way the
  reference game's lobby does. The guest seat gets a selector —
  in the create panel and again on the room screen — **Open**
  (default; a human may join) / **AI · Easy** / **AI · Normal** /
  **Closed**; it locks while a human guest is present, so a seat
  can never be reconfigured out from under a person. With an AI
  chosen, the ready gate becomes the host's alone (the AI seat
  always counts as ready, and as loaded the moment the host's
  own build verifies on the loading screen, which shows both
  seats as usual) — the full Lobby Browser → room → ready →
  loading → match flow runs solo, for testing, and the match
  itself is a LOCAL match against the in-sim scripted AI at the
  chosen difficulty: there is no second client and no lockstep
  session, and the docs say so plainly. Its pause menu and end
  overlay behave like a local match's (Restart replays the same
  configuration; Back to menu returns to the Lobby Browser), and
  it records a replay like a local match. The create panel shows
  the match settings as the reference's create lobby does —
  **Map: Arabia · Victory: Conquest · Players: 2** — as stated
  facts, not fake dropdowns, plus the guest-seat choice; the
  room screen carries one configuration summary line ("1v1 ·
  Arabia · Conquest · Guest: AI · Normal"). Directory rule: a
  seat filled by an AI or closed is not an open game — the room
  leaves the public list while sealed and returns when reopened
  (the host's client declares it on its create connection, and
  steers it later with one control frame the room object honors
  only from the host's connection and never forwards; the relay
  gained no new classes, /health version 4). Both-ready for
  human guests, the password flow, the loading/cancel rules,
  the reload consume-once and the pre-match invisibility are
  untouched. Deliberately NOT built, because nothing backs them
  yet — they arrive with their features, not as dead UI: other
  maps, team sizes beyond 1v1, other victory conditions, game
  speed and population settings. Zero sim changes. See
  ARCHITECTURE.md, "M8 — lockstep multiplayer" (guest slot
  bullet).
- **M12.1 — Mobile pass: the pre-match screens on phones
  (shipped 2026-10-18).** The lobby experience was shaped on
  the reference game's wide browser; Claudio tests on his
  iPhone ("Para o celular"). One ≤700 px media query in
  play.css (the breakpoint the HUD already uses) reflows the
  four pre-match screens — Lobby Browser, Create panel, Room
  screen, Loading screen — and nothing else: the games table
  condenses to two lines per row (title + players, then
  map · region · ping with its quality square) by rearranging
  the same rendered spans with grid areas, so selection and
  double-tap join are untouched; Search takes a full-width
  line with Refresh / Password / Server beneath it; every
  control is a ≥40 px touch target (44 px for region rows,
  slot selectors, password fields and primary buttons); the
  action bar sticks to the bottom of the scrolling lobby
  card; room seats and the guest-slot selector go full-width
  with a full-row Start; the 300×250 ad keeps its desktop
  contract (ADS.md) and scales down inside its slot at the
  same aspect on narrower phones instead of overflowing.
  Desktop rendering is byte-identical CSS above the query.
  Zero sim changes. Verified by construction — this
  environment has no layout engine, so the lobby smoke
  asserts the shipped phone rules exist and the desktop
  rules survive; Claudio's phone is the visual check, as
  with every UI pass.
- **M13 — The classic grid hotkeys + edge scrolling (shipped
  2026-10-18).** The keyboard an AoE2 DE player already
  knows, which is the zero-learning rule applied to input.
  With villager(s) selected, **Q** opens the economic build
  page and **W** the military one — the command panel switches
  to that page's buildings, each badged with its letter, and
  the letter places the building exactly like clicking it.
  Every letter is the button's place in the physical keyboard
  grid (panels are three wide: rows **Q W E / A S D / Z X C**)
  — **Q, Q raises a House**, the Farm holds E, the Lumber
  Camp A, the Mining Camp S, and each page's Back sits on C
  (pure-grid pass, 2026-10-19, superseding the brief Q, E =
  House exception). With a building selected, the panel's
  buttons take their slot letters (Blacksmith: Fletching
  **Q**, Scale Mail **W**, Padded Archer Armor **A**) — the
  Town Center keeping DE's own letters (**Q** Villager, **A**
  Loom, **W** Feudal Age; 2026-10-19 study fix). **A** drops off a villager's
  load; **Ctrl+Z / I / G / S / B / A** cycle to a Lumber
  Camp, Mill, Mining Camp, Blacksmith, Barracks or Archery
  Range (Ctrl+Shift selects all of the kind; Ctrl+L for the
  Stable is the browser's and stays unbound — both
  divergences are recorded in ARCHITECTURE.md, "M13"). A
  hotkey is a click by another name: same handlers, same
  disabled-button gating, same `issue()` choke point online;
  nothing fires while typing, paused, or under an overlay.
  And **edge scrolling**: rest the mouse at the game area's
  edge and the camera pans — corners diagonally — at the pan
  keys' speed, over the canvas only, never the UI bars,
  paused with the game like the other pans. The mapping
  tables and resolution are a pure module
  (`web-preview/play/hotkeys.js`, unit-tested); a stub-DOM
  smoke drives a real sandbox match by synthetic keys and
  hand-pumped frames (Q,Q → House placement, TC+Q → queued
  villager, a real Lumber Camp built and jumped to with
  Ctrl+Z, the pan read off the minimap's viewport rect).
  Zero sim changes.

- **M14 — Directional sprites, player color, building lifecycle
  (shipped 2026-10-19).** Claudio's three art rules, landed
  together. (1) **Direction-true sprites**: every unit and animal
  is now rendered from the procedural models in
  `tools/sprite-pipeline/models/` for the fixed camera AND the
  facing direction — 8 directions × action cycles (villager adds
  Build; the scout gallops; animals graze), the sheets leading
  every draw chain with the generated side-view sets kept as the
  fallback beneath (spec: docs/SPRITE-PIPELINE.md §10; the
  reference's classic set is 8 facings, DE ships 16 — our MEDIUM
  tier count, a re-render away). (2) **Player color by mask**:
  designated areas (tunics, the archer's shirt, the MAA's tabard,
  shield faces, the scout's saddle blanket) ship a per-sheet
  shade mask and the enemy's red is a true palette remap at draw
  — the reference engine's mechanism (a stored shade offset
  remapped per player), at posterized fidelity. (3) **The
  building lifecycle**: smoke at HP ≤ 75%, flames at ≤ 50%,
  rubble decals over the freed footprint, and **Repair** (BLD-17)
  — a villager restores an own damaged building at the
  documented 750 HP/min (+375 per extra repairer) for 50% of its
  build cost, via right-click, the villager root panel's third
  button, or hotkey **E**. The AI does not repair yet (gap).

## Good first contributions

Small, self-contained, and each one closes a named rule. The rulebook entry
for each ID gives the exact behavior and an implementation note.

*The first batch below all landed in M4 (2026-10-16) — kept as a record of
what a good first contribution looks like here. New candidates are gathered
from the rulebook's DIFFERS/MISSING entries as issues are opened — the live,
claimable list is the
[`good first issue`](https://github.com/claudioolmedo/skirmish/labels/good%20first%20issue)
label on GitHub. Comment on an issue to claim it (first come, first
served); the flow is described in
[docs/CONTRIBUTORS-SYSTEM.md](CONTRIBUTORS-SYSTEM.md).*

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
