# Skirmish II — Gap Analysis: Mechanics Parity with Age of Empires II DE

*Written 2026-10-16. Companion to [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) (numbers) and
[`OPENAGE-RESEARCH.md`](OPENAGE-RESEARCH.md) (engine lessons). This document answers one
question: **what is missing for Skirmish II to have the same game
mechanics as Age of Empires II: Definitive Edition?***

***Re-audited 2026-10-22** (Codex review follow-up, Priority 1): every
status below was re-checked against the shipped code (`sim/src/*.rs`,
`sim/tests/*.rs`, `web-preview/app.js`, `web-preview/play/`) at
`origin/main` revision `0f303ca`; the follow-up branch was rebased onto
`474084e` (2026-10-23), which landed the animation pass and the Farm
footprint change, and the affected rows were re-checked there. The
2026-10-16 audit predated M3–M14,
so most of its MISSING rows described systems that have since shipped;
each corrected status names its implementation and its tests. The
per-rule record lives in [`MECHANICS-RULEBOOK.md`](MECHANICS-RULEBOOK.md);
the milestone narrative lives in [`ROADMAP.md`](ROADMAP.md), which is
canonical for milestone numbering.*

Method: every "AoE2 behavior" line comes from public documentation
only (Age of Empires fandom wiki — Hotkey, Unit formation, Battle
Formations, Market, Trade, Coinage, Guilds, Victory, Diplomacy,
Signal Allies, Artificial intelligence, Unit pages; aoe2techtree;
community write-ups) or from our own [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md), which
already verified the combat/economy numbers against those sources.
No game binary, no copied assets, no copied UI.

Status legend: **HAVE** (implemented and working) · **PARTIAL**
(exists but narrower than the reference) · **MISSING** (not present).
Sizes: S ≈ days, M ≈ 1–2 weeks, L ≈ a milestone, XL ≈ several
milestones. Sizes are rough agent-build estimates, not commitments,
and are kept only on rows that are still open.

## Milestone map (reconciled)

The repo README fixes the near roadmap: **M3** combat + first
original empire, **M4** renderer + input shell + adaptive quality,
**M5** feel tuning + scripted AI opponent. The web preview already
shipped parts of "M4" early (Canvas renderer, input shell, HUD), so
this document uses:

| Label | Meaning |
|---|---|
| **M3** | Combat + first original empire (README) |
| **M4** | Input/HUD completion: minimap, control groups, idle keys, selection panel, alerts (rest of README's M4 shell) + adaptive-quality governor |
| **M5** | Feel tuning + scripted AI opponent + win/lose (README) |
| **M6** | Ages & technologies + full building set (incl. Market, Monastery, Castle) |
| **M7** | Map generation, terrain/elevation, fog of war, water & navy, animals |
| **M8** | Multiplayer: lockstep netcode, lobby, replay viewer, spectate, diplomacy, trade |
| **Later** | More empires, campaigns/scenarios, editor, ranked |

(Where the original task brief said "M4 ages&techs / M5 multiplayer",
read M6 / M8 here — the README's M3–M5 were already taken. Milestones
as actually shipped run M0–M14 plus dated fidelity batches; see the
ROADMAP's "What exists today" table.)

---

## 1. Selection & control

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 1.1 | Single-click select | Click selects exactly one unit | **HAVE** — `app.js` selection Set (shipped 2026-10-16) | — | — | done |
| 1.2 | Drag-box select | Left-drag selects all own units in the box | **HAVE** — screen-space box over unit ground points | — | — | done |
| 1.3 | Shift add/remove | Shift+click toggles a unit; shift+drag adds boxed units | **HAVE** | — | — | done |
| 1.4 | Double-click same type | Selects all units of that kind visible on screen | **HAVE** — kind-agnostic via `entity_kinds()` | — | — | done |
| 1.5 | Control groups | Ctrl+1–0 assigns (20 groups with Alt layer), number selects, double-press/keys center camera, Shift+number adds group, append and remove-from-group commands | **HAVE** — client group map in `web-preview/play/groups.js` (SEL-07); tests: `groups.test.mjs`, hotkeys smoke | — | — | done (M4; extended to 20 groups 2026-10-21) |
| 1.6 | Idle villager key | `.` cycles idle villagers; Shift+`.` all idle villagers; camera follows | **HAVE** — sim exposes idle lists (`idle_villagers_of`, SEL-08) and the top bar carries the idle pill (INF-03); client key + camera jump in `app.js`; tests: hotkeys smoke, `feed.test.mjs` | — | — | done (M4) |
| 1.7 | Idle military key | `,` cycles idle military; Shift+`,` all idle (again for navy); Alt+`,` on-screen only | **HAVE** — SEL-09, mirror of 1.6 in `app.js` (navy variants moot: no navy) | — | — | done (M4) |
| 1.8 | Go-to / cycle building keys | H selects Town Center; Ctrl+letter cycles each building type; Space centers on selection; Backspace revisits recent event locations | **PARTIAL** — H (SEL-10), Ctrl+letter building cycles (M13, `play/hotkeys.js`) and the Backspace/Home alert jump (INF-01) **HAVE**; Space-center is not bound | Space binding in the client key map | S | M4 remainder |
| 1.9 | Camera | Edge scroll (option), wheel zoom, minimap click-jump, bookmarks of last events | **HAVE** — edge scroll (M13), wheel zoom-to-cursor, minimap click-jump (SEL-16, `play/minimap.js`), alert-location jumps (INF-01) | — | — | done |
| 1.10 | Group movement coherence | A group travels at its slowest member's speed and stays roughly together | **PARTIAL** — formation-keeping group moves **HAVE** (M4: shared orders keep relative offsets); the sim-side slowest-member clamp (MOV-06) is still MISSING — each unit paths at its own speed | Sim clamp for shared orders (MOV-06) | M | M3/M5 remainder |
| 1.11 | Selection size cap | One selection holds up to 60 units | **HAVE** — cap enforced in the one selection funnel (SEL-05) | — | — | done (M4) |

## 2. Commands & unit behavior

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 2.1 | Move | Right-click ground orders movement | **HAVE** — `push_move`, A* + steering | — | — | done |
| 2.2 | Smart context order | Right-click target decides: resource → gather, enemy → attack, shelter → garrison, ship → board | **PARTIAL** — gather and attack contexts **HAVE** (`orderAt`, M3); garrison and boarding contexts missing with their systems (2.13, §7 water) | Garrison context (M6 content), boarding (water) | M | M3 done; rest with 2.13/§7 |
| 2.3 | Attack (explicit) | Right-click an enemy to attack it; units chase within reason | **HAVE** — combat core in `sim/src/combat.rs` (M3); tests: `sim/tests/combat.rs` | — | — | done (M3) |
| 2.4 | Attack-move | Units advance to a point, engaging everything on the way (hotkey R) | **HAVE** — standing order in `combat.rs` (CMB-10); tests: `sim/tests/orders_stances.rs`, orders smoke | — | — | done (2026-10-22) |
| 2.5 | Patrol | Units patrol between two points, engaging on sight | **HAVE** — looping waypoint order (CMB-11); tests: `sim/tests/orders_stances.rs` | — | — | done (2026-10-22) |
| 2.6 | Guard / follow | Military follows and protects a friendly unit | **MISSING** — no guard/follow order in the command vocabulary | Follow-target order + leash | S | M3 remainder |
| 2.7 | Stop | Halts the current order (hotkey G) | **HAVE** — `push_stop` + sim clears task/order (SEL-14); tests: `sim/tests/cancel_delete.rs` | — | — | done (2026-10-21) |
| 2.8 | Delete unit | Delete key destroys a selected unit/building (Shift+Delete for all selected) | **HAVE** — `push_delete` + death/removal path, foundation refunds (SEL-14); tests: `sim/tests/cancel_delete.rs` | — | — | done (2026-10-21) |
| 2.9 | Stances | Aggressive / Defensive / Stand Ground / No Attack change leash and pursuit (see MECHANICS-STUDY §9) | **HAVE** — stance field per unit + stance-gated acquisition/chase (CMB-09); tests: `sim/tests/orders_stances.rs` | — | — | done (2026-10-22) |
| 2.10 | Formations | Line / Box / Staggered / Flank; units slot by class layers (cavalry front … monks/siege center-back); long moves (>30 tiles) collapse into file | **PARTIAL** — formation-keeping group moves **HAVE** (M4); selectable formation types and class-layer slotting (MOV-07) and the long-move column (MOV-08) are MISSING | Formation slot assignment per group order + arrival layout | M | M3 remainder (basic), M5 polish |
| 2.11 | Queued orders (waypoints) | Shift+right-click chains orders; units execute them in sequence; waypoint flags shown | **HAVE** — per-entity order queue in sim (hash-covered, SEL-12) + client shift handling + waypoint pennants (INF-09) | — | — | done (M4; pennants 2026-10-21) |
| 2.12 | Rally / gather points | Every production building + TC sets a rally flag; new units walk there; rally on a resource sends new villagers straight to work | **HAVE** — rally point per building in sim + set-on-right-click client flow + flag rendering (SEL-13) | — | — | done (M4) |
| 2.13 | Garrison | Units shelter in TC/towers/Castle/transport; heal inside; garrisoned archers add arrows to the building; ungarrison to a point | **MISSING** — no inside/outside state (rulebook §14 GAR: all MISSING; design follow-up: occupancy first, see IMPROVEMENT-SUGGESTIONS Priority 6) | Garrison capacity per building, shelter task, heal rate, extra-arrow math (STUDY §9.3) | L | M6 content |
| 2.14 | Town Bell | One click sends all villagers to nearest shelter; "back to work" returns them to their previous tasks | **MISSING** — depends on 2.13 | Bell command + task snapshot/restore per villager | M | M6 content (with 2.13) |
| 2.15 | Force drop-off | Villagers can be told to deposit what they carry immediately (hotkey A) and return | **HAVE** — `push_dropoff` → Returning task from anywhere (M6); tests: `sim/tests/economy.rs` | — | — | done (M6) |
| 2.16 | Repair | Villagers repair damaged buildings, ships, and siege for a resource cost | **PARTIAL** — building repair **HAVE**: `Command::Repair`, 750 HP/min first villager +375 each additional, 50%-of-cost drain (BLD-17, M14), and the scripted AI repairs too (2026-10-22); ships/siege do not exist yet and DE's Town-Center repair exception is unimplemented (BLD-17 DIFFERS); tests: `sim/tests/repair.rs`, `sim/tests/ai_repair.rs` | Ship/siege repair with their systems; TC exception ruling | M | M6 remainder |
| 2.17 | Attack ground | Siege can bombard a point (hotkey T) | **MISSING** — no siege, no area-damage projectiles | Area-damage projectiles + point target order | M | M6 content (with siege) |

## 3. Economy

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 3.1 | Four resources + stockpile | Food/wood/gold/stone counters; all costs draw from one stockpile per player | **HAVE** — `econ.rs`, HUD bar | — | — | done |
| 3.2 | Gather → carry → deposit | Villager fills carry capacity, walks to nearest drop-off, stockpile increases, returns to the node automatically | **HAVE** — `entity.rs` Task cycle; rates/amounts match the wiki (STUDY §6) | — | — | done |
| 3.3 | Resource nodes | Trees 100 wood, berries 125 food, gold 800, stone 350 | **HAVE** — `NODE_SPECS` | — | — | done |
| 3.4 | Drop-off buildings | Mill (food), Lumber Camp (wood), Mining Camp (gold/stone) are closer drop points and tech sites; TC accepts all | **HAVE** — camps are buildings with the drop-off role and nearest-valid-deposit routing (M6, `build.rs`/`econ.rs`); tests: `sim/tests/economy.rs`, `sim/tests/tech.rs` | — | — | done (M6) |
| 3.5 | Farms | Built by villagers (60 wood, 175 food), gathered at farm rate; expire when empty; Mill holds a reseed queue incl. auto-reseed | **PARTIAL** — farm construction + deplete + replant and the Mill **HAVE** (`build.rs`, M6; farms gated on a Mill); the Mill reseed queue / auto-reseed is still missing (ECO-14) | Reseed queue in sim tied to the Mill | M | M6 remainder |
| 3.6 | Hunting | Deer/boar give food; hunters carry 35 (exception to the base 10); boar fights back and is lured to the TC | **HAVE** — Gaia deer/boar with carcasses, decay and the hunt carry of 35 (M3); deer also flee approaching units (AI-07, 2026-10-22); tests: `sim/tests/sheep_boar.rs`, `sim/tests/combat.rs`, `sim/tests/deer_push.rs` | — | — | done (M3) |
| 3.7 | Herdables | Sheep are captured by proximity, walked home, eaten for 100 food under the TC | **HAVE** — conversion by presence (strictly-most-units within 1 tile; ties hold; unguarded sheep are stolen), owner-colour band (AI-06); tests: `sim/tests/sheep_boar.rs` | — | — | done (M3; reworked 2026-10-20) |
| 3.8 | Fishing | Shore fish for villagers; Fishing Ships harvest shore/deep fish and build Fish Traps (floating farms) | **MISSING** — no water domain, no ships | Water domain (7.5) + Dock + fishing ship | L | M7 remainder |
| 3.9 | Relics | Monks carry relics to a Monastery; each garrisoned relic generates 30 gold/min | **MISSING** — no monks, no Monastery (rulebook §12 MNK: all MISSING) | Monk (6.8) + Monastery + relic objects + gold trickle | M | M6 content |
| 3.10 | Market exchange | Buy/sell 100-unit lots of food/wood/stone vs gold; one global price per commodity, starts 100/115/130, moves ±2 per trade, floor at rate 20; 30% fee (Guilds → 15%) | **MISSING** — no Market building on main (rulebook §13 TRD: all MISSING) | Market building + global price state in sim + exchange UI | M | M6 content |
| 3.11 | Trade units | Trade Carts (Market↔Market) and Trade Cogs (Dock↔Dock) generate gold ∝ distance, forever, incl. with enemy/neutral markets | **MISSING** — needs a second economy to trade with | Trade unit + route logic + another player's buildings (AI or human) | M | M8 remainder (works vs AI once the Market lands) |
| 3.12 | Tribute | Send resources to any player; 30% fee, Coinage → 20%, Banking → 0% | **MISSING** — no diplomacy screen, no transfer command | Diplomacy screen + transfer command | S | M8 remainder |

## 4. Buildings

Had today (re-audited 2026-10-22): **Town Center** (trains villagers,
researches Loom and the Feudal Age advance in the shared queue; +5 pop
support; an entity rather than a placed building — BLD-16; does not
shoot, no garrison), **House** (25 wood, 25 s, +5 pop), **Farm**
(60 wood, 15 s, 175 food, gated on a Mill; 3×3 cells since the
2026-10-23 landing adopted the reference footprint — ECO-14's
footprint clause is closed; it stays DIFFERS on the work-rate cap
and the reseed queue), **Barracks**,
**Archery Range**, **Stable**, **Blacksmith**, **Lumber Camp**,
**Mill**, **Mining Camp**, **Palisade Wall**. Everything below is
MISSING unless noted.

| # | Building | AoE2 DE role (key facts) | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 4.1 | Barracks | Trains the infantry lines; 175 wood, 3×3 | **HAVE** — building + production queue + rally (M3); trains Militia and Spearman, hosts the Man-at-Arms upgrade; tests: `sim/tests/combat.rs`, `sim/tests/feudal_techs.rs` | — | — | done (M3) |
| 4.2 | Archery Range | Trains archer lines | **HAVE** — same shell (M6); trains Archer and Skirmisher; tests: `sim/tests/combat.rs` | — | — | done (M6) |
| 4.3 | Stable | Trains cavalry lines | **HAVE** — same shell (M6); trains Scout Cavalry, hosts Bloodlines; tests: `sim/tests/feudal_techs.rs` | — | — | done (M6) |
| 4.4 | Siege Workshop | Trains siege lines | **MISSING** | Building + production queue + rally | M | M6 content |
| 4.5 | Castle | Trains unique units + trebuchets, researches unique techs, shoots, garrisons 20, +20 pop; 650 stone | **MISSING** | Building + arrows + garrison | L | M6 content |
| 4.6 | Monastery | Trains monks, holds relics, monk techs | **MISSING** | Building + relic storage | M | M6 content |
| 4.7 | Market | Exchange, tribute, trade carts, Coinage/Banking/Guilds | **MISSING** — see 3.10–3.12 | See 3.10–3.12 | M | M6 content |
| 4.8 | Blacksmith | Attack/armor tech lines for all classes | **HAVE** — tech host (M6); the full Feudal tier shipped 2026-10-22 (Fletching, Scale Mail, Padded Archer Armor, Forging, Scale Barding Armor); tests: `sim/tests/tech.rs`, `sim/tests/feudal_techs.rs` | — | — | done (M6; Feudal tier 2026-10-22) |
| 4.9 | University | Ballistics, Chemistry, Masonry line, Siege Engineers… | **MISSING** | Tech host (5.5) | S | M6 content |
| 4.10 | Mill | Food drop-off, farm techs, farm reseed queue | **PARTIAL** — building + drop-off + farm techs (Horse Collar) **HAVE** (M6); reseed queue missing (see 3.5) | See 3.4/3.5 | S | M6 remainder |
| 4.11 | Lumber Camp | Wood drop-off + wood techs | **HAVE** — drop-off + Double-Bit Axe (M6; tech completed 2026-10-22); tests: `sim/tests/feudal_techs.rs` | — | — | done (M6) |
| 4.12 | Mining Camp | Gold/stone drop-off + mining techs | **HAVE** — drop-off + Gold Mining / Stone Mining (M6; techs completed 2026-10-22); tests: `sim/tests/feudal_techs.rs` | — | — | done (M6) |
| 4.13 | Dock | Ships, fishing, trade cogs, dock techs; built on shore | **MISSING** — needs the water domain (BLD-14) | Water domain (7.5) | M | M7 remainder |
| 4.14 | Towers | Watch Tower line shoots, garrisons 5, LoS anchor | **MISSING** — no tower building on main (BLD-18 follow-on) | Arrows + garrison | M | M6 content |
| 4.15 | Walls & gates | Palisade/stone walls block movement; gates open for friends; walls are the map-control tool | **PARTIAL** — Palisade Wall **HAVE**: 1×1 blocking segments, drag-line placement on one supercover line (BLD-13; line fix 2026-10-22), tests: `sim/tests/walls.rs`; stone walls and gates are missing (BLD-13 DIFFERS) | Stone wall tier + gate logic | M | M6 remainder |
| 4.16 | Outpost | Cheap LoS-only tower, no attack | **MISSING** — lands with towers (4.14) | Trivial once towers exist | S | M6 content |
| 4.17 | Wonder | Imperial Age victory building (8.2) | **MISSING** | Building + victory timer | S | M6 content |
| 4.18 | Construction system | Foundations, builders walk to site, HP grows with progress, extra builders help with diminishing returns, buildings can be deleted, unfinished buildings are fragile | **PARTIAL** — placement validation, foundation, progress, multi-builder stacking, delete with foundation refunds (SEL-14) and proportional construction HP (BLD-05) **HAVE** (`build.rs`; tests: `sim/tests/construction.rs`, `sim/tests/cancel_delete.rs`); the multi-builder rate curve is ours, not yet reconciled with the reference's diminishing curve | Rate-curve reconciliation against public evidence | M | M6 polish |
| 4.19 | Building combat | Buildings have HP/armor, burn when damaged, TC/towers/Castle shoot (extra arrows per garrisoned archer) | **PARTIAL** — building HP/armor and the damage path **HAVE** (`damage_building` in `combat.rs`), with the fire/rubble lifecycle on the client (M14); no building shoots yet (TC never shoots here; towers/Castle unbuilt) and garrison arrows wait on 2.13 | Building weapons with towers/Castle | M | M6 content |

## 5. Ages & technologies

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 5.1 | Age advancement | Dark → Feudal (500 F, 130 s) → Castle (800 F/200 G, 160 s) → Imperial (1000 F/800 G, 190 s); each needs 2 buildings of the current age; advancing locks the TC and unlocks a tier of everything | **PARTIAL** — the Dark → Feudal rung **HAVE**: per-player age state in `sim/src/tech.rs`, researched at the TC, two-qualifying-buildings rule (M6); tests: `sim/tests/tech.rs`. Castle/Imperial rungs are missing (Castle Age package in design: 800 F/200 G, 160 s) | Castle + Imperial rungs with their content | L | M6 remainder |
| 5.2 | Research system | Buildings research one thing at a time (queue with units in DE), costs + time, cancel refunds; effects rewrite unit stats | **HAVE** — shared production/research queues, cancel refunds (TEC-05), per-player stat patches folded into every stat lookup (TEC-06, `tech.rs`); tests: `sim/tests/tech.rs`, `sim/tests/feudal_techs.rs`, `sim/tests/cancel_delete.rs` | — | — | done (M6) |
| 5.3 | Economy techs | Loom, Wheelbarrow/Hand Cart (TC); Horse Collar line (Mill); Double-Bit Axe line (Lumber Camp); Gold/Stone Mining lines (Mining Camp); Crop Rotation… | **PARTIAL** — Loom, Wheelbarrow, Horse Collar, Double-Bit Axe, Gold Mining, Stone Mining **HAVE** (M6 + 2026-10-22 completion); Hand Cart and the Castle-age tiers wait on the Castle rung (ECO-03) | Castle-rung tech rows | M | M6 remainder |
| 5.4 | Blacksmith lines | Melee attack, infantry/cavalry/archer armor, cavalry attack, archer attack — each 3 tiers (STUDY §8) | **PARTIAL** — the Feudal tier is complete (Fletching, Scale Mail, Padded Archer Armor, Forging, Scale Barding Armor; Bloodlines at the Stable); Castle/Imperial tiers missing with their ages | Later tiers with the Castle/Imperial rungs | M | M6 remainder |
| 5.5 | University line | Ballistics, Chemistry, Masonry/Architecture, Murder Holes, Siege Engineers, Treadmill Crane… | **MISSING** — no University | Tech rows incl. projectile + building effects | M | M6 content |
| 5.6 | Monastery line | Redemption, Atonement, Heresy, Faith, Sanctity, Fervor, Block Printing, Illumination, Theocracy | **MISSING** — no monks | Monk system first (6.8) | M | M6 content |
| 5.7 | Market line | Coinage, Banking, Guilds, Caravan (trade speed) | **MISSING** — no Market | Fee/speed parameters | S | M6 content |
| 5.8 | Dock line | Careening line, Shipwright, Gillnets (fish trap capacity) | **MISSING** — no navy | Navy first | S | M7 remainder |
| 5.9 | Unique technologies | One Castle-Age + one Imperial-Age unique tech per civilization | **MISSING** — no empire system (see §13) | Per-empire tech rows | M | M6+ |
| 5.10 | Tech tree viewer | Full tree UI (F5), per-civ availability | **MISSING** — verified absent from the client (2026-10-22) | Data-driven tree render from the same tables the sim uses | M | M6 content |

## 6. Military & combat

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 6.1 | Damage model | damage = max(1, elevation × Σ max(0, attackᵢ − armorᵢ)) per armor class; fractional HP, death at HP < 1; hill ×1.25/×0.75 (STUDY §1, §1.3 class table) | **HAVE** — HP + attack/armor class tables in `sim/src/combat.rs` (CMB-01/02/03), fractional fixed-point HP, elevation factor since M7; the mishit factor composes inside the same clamp (CMB-13, verified 2026-10-22); tests: combat unit tests in `combat.rs`, `sim/tests/combat.rs` | — | — | done (M3; elevation M7) |
| 6.2 | Melee combat | Attack delay (frame) + rate of fire ≈ 2.0 s typical; units face targets; engaged melee may overlap | **HAVE** — attack state machine + per-unit timing table in `combat.rs`; separation suppressed while engaged; tests: `sim/tests/combat.rs`, `sim/tests/melee_building.rs` | — | — | done (M3) |
| 6.3 | Ranged combat & projectiles | Visible projectiles with travel time, accuracy %, mishits deal 50%, elevation affects accuracy/damage, Ballistics leads moving targets | **HAVE** — projectile entities in sim + seeded accuracy rolls (PRJ-01/03), mishit 50% (CMB-13); Ballistics itself is a University tech (5.5, missing with it); tests: `sim/tests/combat.rs` | — (Ballistics tracked at 5.5) | — | done (M3) |
| 6.4 | Auto-targeting | Units acquire closest enemy unit → enemy TC in LoS → other buildings; never farms/walls/herdables (STUDY §9.2) | **HAVE** — the full acquisition chain (CMB-08), stance-gated since 2026-10-22; tests: `sim/tests/combat.rs` (cmb08_*), `sim/tests/orders_stances.rs` | — | — | done (M3; chain completed 2026-10-22) |
| 6.5 | Infantry lines | Militia → Man-at-Arms → Long Swordsman → Two-Handed → Champion; Spearman → Pikeman → Halberdier (+15/+22/+32 vs cavalry…) | **PARTIAL** — Militia → Man-at-Arms and Spearman **HAVE** (M3; Spearman 2026-10-22); the Castle-age steps (Long Swordsman, Pikeman) and Imperial steps are missing | Remaining line steps with the Castle/Imperial rungs | M | M3 done (first steps); rest M6 content |
| 6.6 | Archer lines | Archer → Crossbow → Arbalester; Skirmisher line (anti-archer); Hand Cannoneer; Cavalry Archer line | **PARTIAL** — Archer and Skirmisher **HAVE** (M3; Skirmisher 2026-10-22, with its minimum range PRJ-08); Crossbow line, Hand Cannoneer and Cavalry Archer are missing | Remaining lines with the Castle rung | M | M3 done (first steps); rest M6 content |
| 6.7 | Cavalry lines | Scout → Light Cav → Hussar; Knight → Cavalier → Paladin; regional: camels, battle elephants, steppe lancers, eagle warriors (infantry-role) | **PARTIAL** — Scout Cavalry **HAVE** (M3, the starting scout); Light Cavalry, the Knight line and all regional lines are missing (Castle content) | Cavalry lines with the Castle rung | M | M3 done (scout); rest M6 content |
| 6.8 | Monks | Channel conversion (with resistance/faith meter), healing, carry relics; convertible units change owner | **MISSING** (rulebook §12 MNK: all MISSING) | Conversion timer system + ownership change + heal task | L | M6 content |
| 6.9 | Siege | Ram line (anti-building, carries infantry), Mangonel line (area damage, attack ground), Scorpion line (pass-through bolts), Bombard Cannon, packed/unpacked Trebuchet, Petard | **MISSING** — no siege kinds; the Scorpion's pass-through rule is already pinned in the rulebook (PRJ-09, corrected 2026-10-22: bolts never deal friendly damage) | Area damage, building-class bonuses, pack/unpack states | L | M6 content |
| 6.10 | Navy | Galley line, Fire line, Demolition line (suicide), Cannon Galleon line, Transport (carries 5→20), Fishing Ship, Trade Cog; ships have own armor classes | **MISSING** — no water domain | Water terrain + ship movement model (no separation on waves, shoreline rules) | L | M7 remainder |
| 6.11 | Unique units | 1–2 per civilization at the Castle (some at Dock) | **MISSING** — no empire system yet (see §13) | First empire design includes its unique | M | M3 remainder (one), ongoing |
| 6.12 | Collision & pushing feel | Reference: units bump → stop → repath; no pushing; melee crowds overlap slightly; formations hold shape under fire | **PARTIAL** — our displacement separation is smoother than the reference (documented divergence, OPENAGE-RESEARCH §4 flag 1; MOV-04) | Hybrid: separation for travel, bump-stop in combat, overlap tolerance while engaged | M | M3/M5 remainder |
| 6.13 | Death & removal | Units die, corpses decay, population frees instantly | **HAVE** — the death sweep (CMB-02/CMB-14) frees population the tick HP falls below 1; animals leave carcasses (3.6); client corpse decals decay (M14); tests: `sim/tests/combat.rs` | — | — | done (M3) |

## 7. Map, terrain & visibility

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 7.1 | Random maps | Seeded map scripts (Arabia, Arena, Islands…): fair player starts, resources, forests, animals, relics | **HAVE** — a deterministic seeded Arabia generator in `sim/src/mapgen.rs` (M7): mirrored per-start packages, validated connectivity with a fixed fallback; tests: `sim/tests/world.rs`, mapgen unit tests. Other named maps (Arena, Islands…) are still missing | More map scripts on the same generator | L | M7 (Arabia done); more maps later |
| 7.2 | Map sizes | Tiny (120²) → Ludicrous (480²), population of objects scales | **PARTIAL** — Tiny 120×120 only; the larger sizes are not parameterized yet | Parameterized world size end-to-end | M | M7 remainder |
| 7.3 | Terrain types | Grass/dirt/sand/snow/ice/water/beach/forest; terrain gates building placement and ship movement | **PARTIAL** — land/water layers + forest in the grid and renderer (M7); sand/snow/ice/beach variants are missing | Full terrain palette + placement gates | M | M7 remainder |
| 7.4 | Elevation & cliffs | 7 height levels; cliffs impassable; high ground grants the §6.1 damage bonus | **PARTIAL** — a 5-level height field feeding the binary hill bonus **HAVE** (MAP-06, CMB-01; tests: combat elevation unit tests, `sim/tests/world.rs`); cliffs are missing | Cliff layer + blocking + iso shading | L | M7 remainder |
| 7.5 | Water & shores | Water tiles for ships only; shores blend; Docks must touch water | **PARTIAL** — the water terrain layer exists in mapgen/grid (M7) and gates land placement; the ship domain (movement, Docks — BLD-14) is missing | Water layer + passability per domain (land/sea) | M | M7 remainder |
| 7.6 | Fog of war | 3 states per player: unexplored (black), explored (dim, static terrain), visible (live); buildings remembered, units not | **HAVE** — per-player visibility grids in sim (hash-covered, recomputed on a bounded interval), renderer masking (M7; rulebook §3 VIS); tests: `sim/tests/world.rs`, `sim/tests/two_player.rs` | — | — | done (M7) |
| 7.7 | Line of sight | Every unit/building has a LoS stat; Town Watch etc. raise building LoS | **HAVE** — LoS field in the stat tables feeding §7.6 (VIS-03; the Town Center's sight is the documented 8 tiles); tests: `sim/tests/world.rs` | — | — | done (M7) |
| 7.8 | Animals (Gaia) | Herdables, huntables, predators (wolves attack villagers), straggler trees, decorative fauna | **PARTIAL** — herdables + huntables **HAVE** (3.6/3.7) incl. deer pushing (AI-07); predators are missing on main (AI-08 MISSING) and decorative fauna with them | Gaia predator kind + simple animal AI | M | M7 remainder |
| 7.9 | Exploration start | Players start blind except around their TC; a scout unit maps the world | **HAVE** — fog from tick 0 + the starting Scout Cavalry (M3/M7) | — | — | done (M7) |

## 8. Game structure

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 8.1 | Win / lose (single player) | Conquest: destroy all enemies' units + production; also resigning | **HAVE** — elimination check + win/lose overlay (M5; VIC-01; tests: `sim/tests/match.rs`); sandbox mode never ends | — | — | done (M5) |
| 8.2 | Victory conditions | Standard (Conquest + relic hold 200 yrs + Wonder hold 200 yrs), Regicide, King of the Hill, Time Limit, Score target, Last Man Standing, Sudden Death | **PARTIAL** — Conquest only; the other condition types are missing (the score formula itself is implemented — 8.5, VIC-04) | Victory evaluator parameterized per condition | M | M5 (Conquest done); M6 rest |
| 8.3 | Game modes | Random Map, Empire Wars, Death Match, King of the Hill, Regicide, Capture the Relic, Defend the Wonder, Battle Royale, co-op campaigns, scenarios, campaigns | **PARTIAL** — Random Map vs the scripted AI, online 1v1 and a sandbox mode **HAVE**; every other mode is missing | Mode presets on top of 8.2 + map scripts | M | Later |
| 8.4 | Population model | Soft cap from houses (+5), TC (+5), Castle (+20); hard cap set at lobby (25–500, default 200); housed = production stalls with warning | **PARTIAL** — support math, housed stall + warning **HAVE** (M3 economy); Castle support (4.5) and a lobby population-cap setting are missing | Castle (4.5) + lobby setting | S | M3 done (math); rest M6/M8 |
| 8.5 | Score | Military + economy + technology + society tallies, live total (STUDY §10) | **PARTIAL** — the DE formula is implemented and shown on the end-of-match screen (VIC-04, 2026-10-22); tests: `sim/tests/score.rs`. A live in-match score readout is missing | Live score surface in the HUD or player screen | S | M5 remainder |
| 8.6 | Diplomacy | Per-player stance (ally/neutral/enemy) set in the Diplomacy screen; shared vision with allies (F7 signal/flare) | **MISSING** — 1v1 enemy-only today (NET-11) | Diplomacy state + screen | M | M8 remainder |
| 8.7 | Teams & shared vision | 1–8 players in teams; allies share vision; team bonuses and team positions | **MISSING** | Lobby teams + vision union | M | M8 remainder |
| 8.8 | Match settings | Map, size, game speed (1.0/1.5/2.0), starting age/resources, reveal map modes, allow cheats, lock teams/speed, AI difficulty per slot | **PARTIAL** — seed/map (Arabia), AI slot + difficulty and an online region setting **HAVE** (lobby); speed is fixed at DE Normal 1.7× by design, starting age/resources are fixed, reveal-map modes are missing | Wider settings surface in the lobby | M | M8 remainder |
| 8.9 | Save & record | Single-player saves; every match auto-records; replays scrub/play/pause with per-player PoV | **PARTIAL** — recordings: every match is a re-downloadable full command stream with a scrubbing replay viewer (M8; tests: `sim/tests/net_session.rs`, replay smokes); rejoin after a drop rebuilds the match from the same stream (2026-10-22). Standalone save files for a live match are missing | Save-file format + load | M | M8 remainder |
| 8.10 | Resigning & elimination | Resign leaves buildings standing; elimination removes the player from play; score finalizes | **PARTIAL** — resign (button + overlay) and Conquest elimination **HAVE** (VIC-01; `sim/tests/two_player.rs`); the fine print is 1v1-shaped today | — | — | done for 1v1 (M5/M12); generalize with 8.7 |

## 9. Artificial intelligence

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 9.1 | Scripted AI opponent | Builds a base, trains an army, attacks, difficulty-graded (STUDY §11 researched the population/economy curves + taunts) | **HAVE** — a full economy/build/train/wave script driving the same `Command` vocabulary as a player (`sim/src/ai.rs`, M5); tests: `sim/tests/ai_repair.rs`, `sim/tests/match.rs`, `sim/tests/two_player.rs` | — | — | done (M5) |
| 9.2 | Difficulty levels | Easiest → Extreme (5 tiers) differing in economy caps, aggression, added in DE: no resource cheating — behavior only | **PARTIAL** — Easy/Normal only; behavior-only per the DE rule (AI-03); the harder tiers (tighter build orders, army control) are missing | More difficulty profiles | M | M5 remainder |
| 9.3 | AI social layer | Taunts ("30", "11"…), AI resigns when beaten, AI chat flares | **MISSING** | Taunt map + resign condition | S | M5 remainder |

## 10. Multiplayer (online play is now a **core requirement**)

The original scope said "own-account systems" only. That scope
decision was **reversed by the user on 2026-10-16 (see OPENAGE-RESEARCH
§8)**: online multiplayer is part of the core product, not an extra.

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 10.1 | Architecture | Server-relayed lockstep; commands only; server stores nothing but routing | **HAVE** — lockstep over the Cloudflare relay (M8): 1 turn = 3 ticks, input delay 4 turns (adaptive to 10), per-turn hash compare; dropped players rejoin within a 90 s grace (rebuilt from the command stream; fix verified live 2026-10-22); tests: `sim/tests/net_session.rs`, `play/netplay.integration.mjs` | — | — | done (M8) |
| 10.2 | Lobby | Browse/create games, ready-up, map/slot/difficulty settings, ping per player | **HAVE** — the Lobby Browser opens the site; create/join flows, slots, ready-up and region choice (M9–M12); tests: `play/lobby*.test.mjs`, lobby smokes | — | — | done (M9–M12) |
| 10.3 | Matchmaking UX | Region choice; ping shown as colored squares | **HAVE** — live relay ping per region before the match, connection-quality squares in the lobby | — | — | done (M9) |
| 10.4 | Desync handling | Out-of-sync detected, game stops safely, recording preserved | **HAVE** — per-turn hash compare stops play at the first divergent turn; the desync dialog offers the replay (`btn-desync-*`); subsystem checksums localize faults (SIM-06). Two rulebook gaps remain open in §17 (NET-09 snapshot restore, NET-11 diplomacy) | — | — | done (M8) for detection |
| 10.5 | Replay viewer | Scrub forward/back, speed control, per-player view | **HAVE** — viewer with scrub + speed + fog toggle on the recorded command stream (M8); tests: replay smokes | — | — | done (M8) |
| 10.6 | Spectating | Observers watch live (DE: server-side spectator slots / "watch") | **MISSING** | Observer slot consumes the stream without input rights | M | M8 remainder |
| 10.7 | Chat & taunts | In-game chat, numbered taunts with audio, ally flares | **MISSING** — verified absent from the client (2026-10-22) | Chat channel over the relay + taunt table | M | M8 remainder |
| 10.8 | Pause | Each player limited pause tokens; resume countdown | **PARTIAL** — local pause **HAVE**; online restore exists via rejoin/resync (10.1); per-player pause tokens are missing | Pause-token protocol | S | M8 remainder |
| 10.9 | Ranked & profiles | Ladder, ELO, player stats server-side | **MISSING** — an out-of-account product decision, not code | Account system + server ratings | XL | Later |

## 11. HUD & UI

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 11.1 | Minimap | Terrain + units + buildings colored by owner; TC/age icons; flare signals; click to jump camera | **HAVE** — canvas minimap (`play/minimap.js`, M4) with owner colours and camera click-jump; tests: `play/minimap.test.mjs` | — | — | done (M4) |
| 11.2 | Top panel | Resource icons + counts, population current/cap, current age, idle villager button with count | **HAVE** — the top bar carries all four incl. the age chip and the idle pill (INF-03); tests: `play/feed.test.mjs`, HUD smokes | — | — | done (M4/M6) |
| 11.3 | Selection panel | Portraits of up to 60 selected with HP bars; clicking a portrait selects just that unit | **HAVE** — selection panel with kind portraits, live HP bars and per-kind double-click (M4) | — | — | done (M4) |
| 11.4 | Command card | 3×5 context grid (up to 3 pages) of orders/buildings/techs with hotkey letters and tooltips | **HAVE** — one unified grid panel, DE layout + pure positional hotkeys (M13, `cmd-grid`; name/cost only in the hover tooltip); tests: `play/orders.smoke.mjs`, panel smokes | — | — | done (M13) |
| 11.5 | Production queues (UI) | Queue icons per building with cancel; global queue display optional in DE via mods | **PARTIAL** — per-building queue icons with progress % and cancel **HAVE** (middle selection panel; sim cancel + refunds, TEC-05); tests: `sim/tests/cancel_delete.rs`. DE-style global queue readout is missing | Global queue surface | S | M6 remainder |
| 11.6 | Building selection | Production buildings show their queue, techs and rally flag | **HAVE** — building selection drives the command card (queue, techs, rally) (M4/M6) | — | — | done (M6) |
| 11.7 | Alerts & minimap pings | Text + voice alert on attacks, no-housing, tech done, age reached; colored ping on the minimap at the event site | **HAVE** — event feed + alert system (`play/feed.js`, `play/alerts.js`; INF-01/02) with localized attack alerts and location jumps; tests: `play/feed.test.mjs`, `play/alerts.test.mjs` | — | — | done (M4/M14) |
| 11.8 | Order markers | Movement/attack flags flash at the destination; waypoint pennants on queued orders | **HAVE** — the DE click marker on ground clicks (INF-09a) and waypoint pennants on chained orders (INF-09), both shipped 2026-10-21 | — | — | done (2026-10-21) |
| 11.9 | Game clock & speed UI | F11 clock shows game time; pause + speed controls (-/+) change 0.5×–3× in single player | **PARTIAL** — the match runs at DE Normal (1.7×) and pause exists (10.8); a game-time clock display and speed controls are missing | Clock readout + speed control | S | M5 remainder |
| 11.10 | Diplomacy & objectives screens | Diplomacy (stances), Objectives, Tech Tree (F5) screens | **MISSING** — none of the three screens exists (5.10, 8.6) | Screens (depend on 5.10/8.6) | M | M8 remainder |

## 12. Audio

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 12.1 | Voice barks | Every unit acknowledges orders & reports in civilization language; taunts voiced | **MISSING** — all audio is synthesized cues; no voice layer exists | Sample set per unit (original recordings), non-critical | M | M5 remainder |
| 12.2 | World sounds | Chopping, mining, building, combat hits — positional/volume-scaled | **HAVE** — fully synthesized work + combat cues, distance-scaled (`web-preview/audio.js`; INF-10); tests: `play/audio.test.mjs` | — | — | done (M14) |
| 12.3 | Alert sounds | Horn on attack, dings on completion, pop-cap warning | **HAVE** — synthesized alert horn + completion/cap cues (`audio.js`, INF-10); tests: `play/audio.test.mjs` | — | — | done (M14) |
| 12.4 | Music | Dynamic ambient score | **MISSING** | Original ambient loop (generate/compose) | M | Later |
| 12.5 | UI sounds | Clicks, placement thunks, error buzz | **PARTIAL** — synthesized event cues cover completions and alerts; a full UI click/error set is missing | Small cue set | S | M5 remainder |

## 13. Empires (civilizations)

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 13.1 | Civilization system | 45 civs, each: bonuses, team bonus, 1–2 unique units, 2 unique techs, architecture/language set | **MISSING** (by design — the first original empire lands with the M3 line below) | Civ definition format in data; bonuses as stat patches (5.2) | L | M3 remainder (first), ongoing |
| 13.2 | Architecture sets | Distinct building looks per culture group | **MISSING** | Palette/skins per set (art pipeline) | L | M3 remainder |
| 13.3 | Balance pass | Civs are balanced against each other | **MISSING** | Tuning harness (auto-matches + win stats) | M | Later |

---

## Summary (recomputed at the 2026-10-22 re-audit)

| Section | HAVE | PARTIAL | MISSING |
|---|---|---|---|
| 1. Selection & control | 9 | 2 | 0 |
| 2. Commands & behavior | 10 | 3 | 4 |
| 3. Economy | 6 | 1 | 5 |
| 4. Buildings | 6 | 4 | 9 |
| 5. Ages & technologies | 1 | 3 | 6 |
| 6. Military & combat | 5 | 4 | 4 |
| 7. Map/terrain/visibility | 4 | 5 | 0 |
| 8. Game structure | 1 | 7 | 2 |
| 9. AI opponent | 1 | 1 | 1 |
| 10. Multiplayer | 5 | 1 | 3 |
| 11. HUD & UI | 7 | 2 | 1 |
| 12. Audio | 2 | 1 | 2 |
| 13. Empires | 0 | 0 | 3 |
| **Total (131 rows)** | **57** | **34** | **40** |

Totals recount: HAVE 9+10+6+6+1+5+4+1+1+5+7+2+0 = 57; PARTIAL
2+3+1+4+3+4+5+7+1+1+2+1+0 = 34; MISSING 0+4+5+9+6+4+0+2+1+3+1+2+3 =
40. 57 + 34 + 40 = 131 ✓.

The minimum to be "mechanically equivalent" for a focused player:
sections 1, 2 (except garrison/town bell), 3 (except
fishing/market/trade), 4 (the Feudal set + construction), 5 (first two
age rungs), 6.1–6.7, 7.1–7.7, 8.1/8.4/8.5 and 11 — the core WAS the
game at the 2026-10-16 audit, and the re-audit confirms that core is
now **shipped and live**. What remains open is the Castle-age content
bank (Castle, Siege Workshop, Monastery, Market, University, towers,
stone walls/gates, monks, siege, garrison), the water domain, the
multiplayer tail (chat, spectate, diplomacy/teams), empires, and the
score/clock UI surfaces.

## TOP 15 things a player would notice as "not Age-of-Empires-like"

1. ~~**No garrisoning** — you cannot hide villagers in the TC during a
   raid~~ — **still open** (2.13). The single most missed verb; it also
   removes TC arrows and the Town Bell. → §14 GAR / Priority 6.
2. ~~**No attack-move**~~ — **shipped 2026-10-22** (2.4, CMB-10).
3. **No Market** — no buy/sell safety valve, no trade, no tribute
   (3.10–3.12). → §13 TRD.
4. **No monks** — no conversions, no healing, no relics (6.8, 3.9).
   → §12 MNK.
5. **No siege** — nothing breaks walls or massed units; no rams
   (6.9). → Castle package P4.
6. ~~**No control groups / idle buttons**~~ — **shipped** (1.5–1.7).
7. ~~**No stances**~~ — **shipped 2026-10-22** (2.9, CMB-09); the
   related formations (2.10) remain open.
8. **No water** — no fishing, no navy (3.8, 6.10). → §7 remainder.
9. **No fog of war** — ~~the map is fully visible; no scouting
   game~~ — **shipped M7** (7.6–7.9).
10. ~~**Only one map, flat**~~ — **largely shipped**: seeded Arabia with
    a 5-level height field (7.1, 7.4); more map scripts remain open.
11. ~~**No formations**~~ — **partial**: formation-keeping group moves
    shipped (M4); selectable Line/Box/Staggered/Flank slots remain
    open (2.10).
12. **Waves of identical units** — one militia texture for everyone;
    no civilization identity (13.1).
13. **No walls that matter** — ~~stone walls~~: palisades shipped
    (4.15); stone walls + gates remain open.
14. ~~**Silence**~~ — **largely shipped**: synthesized work/combat/alert
    sounds (12.2, 12.3); voice barks and music remain open (12.1, 12.4).
15. **No knights** — ~~sheep, spearmen and swordsmen only~~: Spearman
    and Skirmisher shipped 2026-10-22; the Knight line, siege and
    monks wait on the Castle Age package.
