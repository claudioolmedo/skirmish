# Skirmish II — Gap Analysis: Mechanics Parity with Age of Empires II DE

*Written 2026-10-16. Companion to [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) (numbers) and
[`OPENAGE-RESEARCH.md`](OPENAGE-RESEARCH.md) (engine lessons). This document answers one
question: **what is missing for Skirmish II to have the same game
mechanics as Age of Empires II: Definitive Edition?***

Method: every "AoE2 behavior" line comes from public documentation
only (Age of Empires fandom wiki — Hotkey, Unit formation, Battle
Formations, Market, Trade, Coinage, Guilds, Victory, Diplomacy,
Signal Allies, Artificial intelligence, Unit pages; aoe2techtree;
community write-ups) or from our own [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md), which
already verified the combat/economy numbers against those sources.
Every "our status" line was checked against the actual code
(`sim/src/*.rs`, `web-preview/app.js`, `web-preview/play/`) on
2026-10-16, after the M2.5 release and the AoE2-style selection fix
shipped the same day. No game binary, no copied assets, no copied UI.

Status legend: **HAVE** (implemented and working) · **PARTIAL**
(exists but narrower than the reference) · **MISSING** (not present).
Sizes: S ≈ days, M ≈ 1–2 weeks, L ≈ a milestone, XL ≈ several
milestones. Sizes are rough agent-build estimates, not commitments.

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
read M6 / M8 here — the README's M3–M5 were already taken.)

---

## 1. Selection & control

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 1.1 | Single-click select | Click selects exactly one unit | **HAVE** — `app.js` selection Set (shipped 2026-10-16) | — | — | done |
| 1.2 | Drag-box select | Left-drag selects all own units in the box | **HAVE** — screen-space box over unit ground points | — | — | done |
| 1.3 | Shift add/remove | Shift+click toggles a unit; shift+drag adds boxed units | **HAVE** | — | — | done |
| 1.4 | Double-click same type | Selects all units of that kind visible on screen | **HAVE** — kind-agnostic via `entity_kinds()` | — | — | done |
| 1.5 | Control groups | Ctrl+1–0 assigns (20 groups with Alt layer), number selects, double-press/keys center camera, Shift+number adds group, append and remove-from-group commands | **MISSING** — no group storage in client | Client-side group map id→Set, key handling, camera-center on re-press | S | M4 |
| 1.6 | Idle villager key | `.` cycles idle villagers; Shift+`.` all idle villagers; camera follows | **MISSING** — sim doesn't expose "idle" per villager (client can infer: Task::Idle not in ABI) | ABI: per-entity task/activity flag or an `idle_villagers()` query; client key + camera jump | S | M4 |
| 1.7 | Idle military key | `,` cycles idle military; Shift+`,` all idle (again for navy); Alt+`,` on-screen only | **MISSING** — needs military + idle flag | Same ABI as 1.6 + military kinds | S | M4 (after M3) |
| 1.8 | Go-to / cycle building keys | H selects Town Center; Ctrl+letter cycles each building type; Space centers on selection; Backspace revisits recent event locations | **MISSING** — TC is not even selectable today | Building selection (see 11.6) + key map | S | M4 |
| 1.9 | Camera | Edge scroll (option), wheel zoom, minimap click-jump, bookmarks of last events | **PARTIAL** — WASD/arrows + right-drag pan + wheel zoom-to-cursor **HAVE**; no edge scroll, no minimap jump, no event bookmarks | Edge scroll toggle; minimap jump (11.1); last-alert location memory | S | M4 |
| 1.10 | Group movement coherence | A group travels at its slowest member's speed and stays roughly together | **PARTIAL** — each unit paths independently (`steer.rs` separation only); fast units outrun slow ones | Group speed clamp for shared orders, arrival spread slots (ties into formations 2.9) | M | M3/M5 |
| 1.11 | Selection size cap | One selection holds up to 60 units | **PARTIAL** — our Set is unbounded (harmless with 3 villagers) | Cap constant in client selection | S | M4 |

## 2. Commands & unit behavior

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 2.1 | Move | Right-click ground orders movement | **HAVE** — `push_move`, A* + steering | — | — | done |
| 2.2 | Smart context order | Right-click target decides: resource → gather, enemy → attack, shelter → garrison, ship → board | **PARTIAL** — gather/move only (`orderAt`) | Attack (M3), garrison (M6), boarding (M7) context branches | M | M3→M7 |
| 2.3 | Attack (explicit) | Right-click an enemy to attack it; units chase within reason | **MISSING** — no HP, no enemies, no attack command in `wasm.rs` | Combat core (6.1–6.3) | L | M3 |
| 2.4 | Attack-move | Units advance to a point, engaging everything on the way (hotkey R) | **MISSING** | Auto-target acquisition while moving (6.10) | M | M3 |
| 2.5 | Patrol | Units patrol between two points, engaging on sight | **MISSING** | Looping waypoint order | S | M3 |
| 2.6 | Guard / follow | Military follows and protects a friendly unit | **MISSING** | Follow-target order + leash | S | M3 |
| 2.7 | Stop | Halts the current order (hotkey G) | **MISSING** — no stop command; a unit's single order slot can't be cleared by the player | `push_stop` + sim clears task/order | S | M3 |
| 2.8 | Delete unit | Delete key destroys a selected unit/building (Shift+Delete for all selected) | **MISSING** | `push_delete` + death/removal path | S | M3 |
| 2.9 | Stances | Aggressive / Defensive / Stand Ground / No Attack change leash and pursuit (see MECHANICS-STUDY §9) | **MISSING** | Stance field per unit + targeting rules | M | M3 |
| 2.10 | Formations | Line / Box / Staggered / Flank; units slot by class layers (cavalry front … monks/siege center-back); long moves (>30 tiles) collapse into file | **MISSING** — groups arrive as a blob | Formation slot assignment per group order + arrival layout | M | M3 (basic line/box), M5 polish |
| 2.11 | Queued orders (waypoints) | Shift+right-click chains orders; units execute them in sequence; waypoint flags shown | **MISSING** — `Entity.order` is a single slot; commands replace, never queue | Per-entity order queue in sim (data-only commands, hash-covered) + client shift handling + flags | M | M3 |
| 2.12 | Rally / gather points | Every production building + TC sets a rally flag; new units walk there; rally on a resource sends new villagers straight to work | **MISSING** — trained villagers spawn idle at the TC (`lib.rs` train path has no rally) | Rally point per building in sim + set-on-right-click client flow + flag rendering | M | M4 (UI) / M6 (all buildings) |
| 2.13 | Garrison | Units shelter in TC/towers/Castle/transport; heal inside; garrisoned archers add arrows to the building; ungarrison to a point | **MISSING** — no inside/outside state | Garrison capacity per building, shelter task, heal rate, extra-arrow math (STUDY §9.3) | L | M6 |
| 2.14 | Town Bell | One click sends all villagers to nearest shelter; "back to work" returns them to their previous tasks | **MISSING** | Bell command + task snapshot/restore per villager | M | M6 (with 2.13) |
| 2.15 | Force drop-off | Villagers can be told to deposit what they carry immediately (hotkey A) and return | **MISSING** — deposit only happens on a full carry at the node cycle | `push_dropoff` → Returning task from anywhere | S | M3 |
| 2.16 | Repair | Villagers repair damaged buildings, ships, and siege for a resource cost | **MISSING** — nothing can be damaged yet | Damage model (6.1) + repair task + cost drain | M | M6 |
| 2.17 | Attack ground | Siege can bombard a point (hotkey T) | **MISSING** | Area-damage projectiles + point target order | M | M6 (with siege) |

## 3. Economy

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 3.1 | Four resources + stockpile | Food/wood/gold/stone counters; all costs draw from one stockpile per player | **HAVE** — `econ.rs`, HUD bar | — | — | done |
| 3.2 | Gather → carry → deposit | Villager fills carry capacity, walks to nearest drop-off, stockpile increases, returns to the node automatically | **HAVE** — `entity.rs` Task cycle; rates/amounts match the wiki (STUDY §6) | — | — | done |
| 3.3 | Resource nodes | Trees 100 wood, berries 125 food, gold 800, stone 350 | **HAVE** — `NODE_SPECS` | — | — | done |
| 3.4 | Drop-off buildings | Mill (food), Lumber Camp (wood), Mining Camp (gold/stone) are closer drop points and tech sites; TC accepts all | **PARTIAL** — deposit only at the Town Center entity | Camps as buildings with drop-off role (4.x) | M | M6 |
| 3.5 | Farms | Built by villagers (60 wood, 175 food), gathered at farm rate; expire when empty; Mill holds a reseed queue incl. auto-reseed | **PARTIAL** — farm construction + deplete + replant **HAVE** (`build.rs`); no Mill, no reseed queue | Mill building + queue in sim | M | M6 |
| 3.6 | Hunting | Deer/boar give food; hunters carry 35 (exception to the base 10); boar fights back and is lured to the TC | **MISSING** — no animals exist | Animal entities (Gaia), hunt task, boar aggression | M | M7 |
| 3.7 | Herdables | Sheep are captured by proximity, walked home, eaten for 100 food under the TC | **MISSING** | Herdable ownership + follow behavior | M | M7 |
| 3.8 | Fishing | Shore fish for villagers; Fishing Ships harvest shore/deep fish and build Fish Traps (floating farms) | **MISSING** — no water, no ships | Water (7.5) + Dock + fishing ship (6.x navy) | L | M7 |
| 3.9 | Relics | Monks carry relics to a Monastery; each garrisoned relic generates 30 gold/min | **MISSING** | Monk (6.8) + Monastery + relic objects + gold trickle | M | M6 |
| 3.10 | Market exchange | Buy/sell 100-unit lots of food/wood/stone vs gold; one global price per commodity, starts 100/115/130, moves ±2 per trade, floor at rate 20; 30% fee (Guilds → 15%) | **MISSING** — no Market | Market building + global price state in sim + exchange UI | M | M6 |
| 3.11 | Trade units | Trade Carts (Market↔Market) and Trade Cogs (Dock↔Dock) generate gold ∝ distance, forever, incl. with enemy/neutral markets | **MISSING** — needs a second economy to trade with | Trade unit + route logic + another player's buildings (AI or human) | M | M8 (works vs AI from M5 scope if Market lands in M6) |
| 3.12 | Tribute | Send resources to any player; 30% fee, Coinage → 20%, Banking → 0% | **MISSING** | Diplomacy screen + transfer command | S | M8 |

## 4. Buildings

Had today: **Town Center** (trains villagers; +5 pop support; cannot be placed, does not
shoot, no garrison, no age-up research), **House** (25 wood, 25 s, +5 pop), **Farm**
(60 wood, 15 s, 175 food). Everything below is MISSING unless noted.

| # | Building | AoE2 DE role (key facts) | Parity requires | Size | Milestone |
|---|---|---|---|---|---|
| 4.1 | Barracks | Trains the infantry lines; 175 wood, 3×3 | Building + production queue + rally | M | M3 |
| 4.2 | Archery Range | Trains archer lines | Same shell | M | M3/M6 |
| 4.3 | Stable | Trains cavalry lines | Same shell | M | M3/M6 |
| 4.4 | Siege Workshop | Trains siege lines | Same shell | M | M6 |
| 4.5 | Castle | Trains unique units + trebuchets, researches unique techs, shoots, garrisons 20, +20 pop; 650 stone | Building + arrows + garrison | L | M6 |
| 4.6 | Monastery | Trains monks, holds relics, monk techs | Building + relic storage | M | M6 |
| 4.7 | Market | Exchange, tribute, trade carts, Coinage/Banking/Guilds | See 3.10–3.12 | M | M6 |
| 4.8 | Blacksmith | Attack/armor tech lines for all classes | Tech host (5.4) | S | M6 |
| 4.9 | University | Ballistics, Chemistry, Masonry line, Siege Engineers… | Tech host (5.5) | S | M6 |
| 4.10 | Mill | Food drop-off, farm techs, farm reseed queue | See 3.4/3.5 | S | M6 |
| 4.11 | Lumber Camp | Wood drop-off + wood techs | See 3.4 | S | M6 |
| 4.12 | Mining Camp | Gold/stone drop-off + mining techs | See 3.4 | S | M6 |
| 4.13 | Dock | Ships, fishing, trade cogs, dock techs; built on shore | Needs water (7.5) | M | M7 |
| 4.14 | Towers | Watch Tower line shoots, garrisons 5, LoS anchor | Arrows + garrison | M | M6 |
| 4.15 | Walls & gates | Palisade/stone walls block movement; gates open for friends; walls are the map-control tool | Dynamic blocking (our grid already supports a dynamic layer) + gate logic | M | M6 |
| 4.16 | Outpost | Cheap LoS-only tower, no attack | Trivial once towers exist | S | M6 |
| 4.17 | Wonder | Imperial Age victory building (8.2) | Building + victory timer | S | M6 |
| 4.18 | Construction system | Foundations, builders walk to site, HP grows with progress, extra builders help with diminishing returns, buildings can be deleted, unfinished buildings are fragile | **PARTIAL** — placement validation, foundation, progress, multi-builder stacking **HAVE** (`build.rs`); builders add linearly (reference: diminishing), no delete, no HP ramp | M | M6 polish |
| 4.19 | Building combat | Buildings have HP/armor, burn when damaged, TC/towers/Castle shoot (extra arrows per garrisoned archer) | Damage model (6.1) + building weapons | M | M6 |

## 5. Ages & technologies

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 5.1 | Age advancement | Dark → Feudal (500 F, 130 s) → Castle (800 F/200 G, 160 s) → Imperial (1000 F/800 G, 190 s); each needs 2 buildings of the current age; advancing locks the TC and unlocks a tier of everything | **MISSING** — no age concept anywhere | Age state per player in sim + TC research + unlock tables | L | M6 |
| 5.2 | Research system | Buildings research one thing at a time (queue with units in DE), costs + time, cancel refunds; effects rewrite unit stats | **MISSING** — only unit training exists | Generic research queue + stat patch system (data patches, OPENAGE-RESEARCH §2.7) | L | M6 |
| 5.3 | Economy techs | Loom, Wheelbarrow/Hand Cart (TC); Horse Collar line (Mill); Double-Bit Axe line (Lumber Camp); Gold/Stone Mining lines (Mining Camp); Crop Rotation… | **MISSING** | Tech rows on host buildings | M | M6 |
| 5.4 | Blacksmith lines | Melee attack, infantry/cavalry/archer armor, cavalry attack, archer attack — each 3 tiers (STUDY §8) | **MISSING** | Armor-class stat patches | M | M6 |
| 5.5 | University line | Ballistics, Chemistry, Masonry/Architecture, Murder Holes, Siege Engineers, Treadmill Crane… | **MISSING** | Tech rows incl. projectile + building effects | M | M6 |
| 5.6 | Monastery line | Redemption, Atonement, Heresy, Faith, Sanctity, Fervor, Block Printing, Illumination, Theocracy | **MISSING** | Monk system first (6.8) | M | M6 |
| 5.7 | Market line | Coinage, Banking, Guilds, Caravan (trade speed) | **MISSING** | Fee/speed parameters | S | M6 |
| 5.8 | Dock line | Careening line, Shipwright, Gillnets (fish trap capacity) | **MISSING** | Navy first | S | M7 |
| 5.9 | Unique technologies | One Castle-Age + one Imperial-Age unique tech per civilization | **MISSING** | Per-empire tech rows | M | M6+ |
| 5.10 | Tech tree viewer | Full tree UI (F5), per-civ availability | **MISSING** | Data-driven tree render from the same tables the sim uses | M | M6 |

## 6. Military & combat

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 6.1 | Damage model | damage = max(1, elevation × Σ max(0, attackᵢ − armorᵢ)) per armor class; fractional HP, death at HP < 1; hill ×1.25/×0.75 (STUDY §1, §1.3 class table) | **MISSING** — entities have no HP at all | HP + attack/armor class tables (data, parent+deltas) | L | M3 |
| 6.2 | Melee combat | Attack delay (frame) + rate of fire ≈ 2.0 s typical; units face targets; engaged melee may overlap | **MISSING** | Attack state machine + per-unit timing table; separation suppressed while engaged (OPENAGE-RESEARCH §5.1) | M | M3 |
| 6.3 | Ranged combat & projectiles | Visible projectiles with travel time, accuracy %, mishits deal 50%, elevation affects accuracy/damage, Ballistics leads moving targets | **MISSING** | Projectile entities in sim + seeded accuracy rolls (RNG call-order discipline) | M | M3 |
| 6.4 | Auto-targeting | Units acquire closest enemy unit → enemy TC in LoS → other buildings; never farms/walls/herdables (STUDY §9.2) | **MISSING** | Acquisition pass per tick, hash-covered | M | M3 |
| 6.5 | Infantry lines | Militia → Man-at-Arms → Long Swordsman → Two-Handed → Champion; Spearman → Pikeman → Halberdier (+15/+22/+32 vs cavalry…) | **MISSING** | Unit kinds + upgrade techs | M | M3 (militia + spear), M6 (full) |
| 6.6 | Archer lines | Archer → Crossbow → Arbalester; Skirmisher line (anti-archer); Hand Cannoneer; Cavalry Archer line | **MISSING** | Same | M | M3 (archer), M6 (full) |
| 6.7 | Cavalry lines | Scout → Light Cav → Hussar; Knight → Cavalier → Paladin; regional: camels, battle elephants, steppe lancers, eagle warriors (infantry-role) | **MISSING** | Same | M | M3 (scout + knight), M6 (full) |
| 6.8 | Monks | Channel conversion (with resistance/faith meter), healing, carry relics; convertible units change owner | **MISSING** | Conversion timer system + ownership change + heal task | L | M6 |
| 6.9 | Siege | Ram line (anti-building, carries infantry), Mangonel line (area damage, attack ground), Scorpion line (pass-through bolts), Bombard Cannon, packed/unpacked Trebuchet, Petard | **MISSING** | Area damage, building-class bonuses, pack/unpack states | L | M6 |
| 6.10 | Navy | Galley line, Fire line, Demolition line (suicide), Cannon Galleon line, Transport (carries 5→20), Fishing Ship, Trade Cog; ships have own armor classes | **MISSING** — no water | Water terrain + ship movement model (no separation on waves, shoreline rules) | L | M7 |
| 6.11 | Unique units | 1–2 per civilization at the Castle (some at Dock) | **MISSING** — no empire system yet | First empire design (M3) includes its unique | M | M3 (one), ongoing |
| 6.12 | Collision & pushing feel | Reference: units bump → stop → repath; no pushing; melee crowds overlap slightly; formations hold shape under fire | **PARTIAL** — our displacement separation is smoother than the reference (documented divergence, OPENAGE-RESEARCH §4 flag 1) | Hybrid: separation for travel, bump-stop in combat, overlap tolerance while engaged | M | M3/M5 |
| 6.13 | Death & removal | Units die, corpses decay, population frees instantly | **MISSING** — nothing can die | Death path + decay visuals | S | M3 |

## 7. Map, terrain & visibility

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 7.1 | Random maps | Seeded map scripts (Arabia, Arena, Islands…): fair player starts, resources, forests, animals, relics | **MISSING** — one fixed 200×200 layout; obstacles are compile-time constants (`grid.rs` wall + gap); starts hardcoded (`lib.rs`) | Map generator producing grid + nodes + starts from the match seed (deterministic, hash-safe) | L | M7 |
| 7.2 | Map sizes | Tiny (120²) → Ludicrous (480²), population of objects scales | **MISSING** — fixed 200×200 world, 50×50 cells hardcoded in constants | Parameterized world size end-to-end | M | M7 |
| 7.3 | Terrain types | Grass/dirt/sand/snow/ice/water/beach/forest; terrain gates building placement and ship movement | **MISSING** — one flat ground type | Terrain layer in grid + renderer palettes | M | M7 |
| 7.4 | Elevation & cliffs | 7 height levels; cliffs impassable; high ground grants the §6.1 damage bonus | **MISSING** — flat world, no height in sim or render | Height layer in sim (damage hook exists in 6.1 formula) + cliff blocking + iso shading | L | M7 |
| 7.5 | Water & shores | Water tiles for ships only; shores blend; Docks must touch water | **MISSING** | Water layer + passability per domain (land/sea) | M | M7 |
| 7.6 | Fog of war | 3 states per player: unexplored (black), explored (dim, static terrain), visible (live); buildings remembered, units not | **MISSING** — everything always visible | Per-player visibility grid in sim (LoS radius per unit/building, cap 20 tiles) + renderer masking | L | M7 |
| 7.7 | Line of sight | Every unit/building has a LoS stat; Town Watch etc. raise building LoS | **MISSING** | LoS field in stat tables + 7.6 | (incl.) | M7 |
| 7.8 | Animals (Gaia) | Herdables, huntables, predators (wolves attack villagers), straggler trees, decorative fauna | **MISSING** | Gaia entity class + simple animal AI | M | M7 |
| 7.9 | Exploration start | Players start blind except around their TC; a scout unit maps the world | **MISSING** — full map visible from tick 0 | 7.6 + scout in starting set | S | M7 |

## 8. Game structure

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 8.1 | Win / lose | A match ends: conquest (all enemy military buildings/units gone) or the chosen victory condition | **MISSING** — endless sandbox, no defeat check | End-of-match evaluation in sim + end screen | M | M5 (conquest vs AI) |
| 8.2 | Victory conditions | Standard (hold a Wonder or all Relics for a map-size-scaled timer), Conquest (ranked default), Time Limit (highest score), Score target, Last Man Standing | **MISSING** | Timers + score (8.5) + Wonder/relics (4.17/3.9) | M | M8 (Wonder/Relic after M6) |
| 8.3 | Game modes | Random Map, Deathmatch (huge stockpiles), Regicide (protect your King), Empire Wars (Feudal start, pre-built), King of the Hill, Sudden Death (lose TC = lose), Capture the Relic, Defend the Wonder, Wonder Race, Turbo | **MISSING** | Mode = match settings + small rule deltas (King unit, starting state) | M | M8 |
| 8.4 | Population rules | Lobby cap 200 (up to 500); House +5, TC +5, Castle +20; "housed" queues but can't finish | **PARTIAL** — support math + housed stall **HAVE** (`build.rs`, `lib.rs`); no lobby cap setting, no Castle | Cap as match setting | S | M8 (setting), M6 (Castle) |
| 8.5 | Score | Live score = military + economy + technology + society values per player | **MISSING** | Score accumulation in sim, shown in HUD/lobby | S | M8 |
| 8.6 | Diplomacy | Per-player stances Ally/Neutral/Enemy (asymmetric allowed); Neutral: military auto-attacks military only; Locked Teams option; Allied Victory sharing | **MISSING** — single player world | Stance matrix + targeting/diplomacy rules | M | M8 |
| 8.7 | Teams & shared vision | Teammates share LoS after Cartography; team bonuses apply | **MISSING** | Team ids + shared visibility merge | M | M8 |
| 8.8 | Match settings | Map, size, game speed (1.0/1.5/2.0), starting age, starting resources, reveal-map options, population cap | **MISSING** — only `?seed=` exists client-side | Settings struct in match creation (sim already takes seed+settings by design) | M | M8 (lobby), speed S any time |
| 8.9 | Save / load & recorded games | Save mid-match; every match auto-records; replays play back with POV/fog controls | **PARTIAL** — determinism makes replay *possible*: headless replay example exists (`sim/examples/replay.rs`); no user-facing save/record format or viewer | Serialize (seed, settings, command stream) + viewer shell | M | M8 |
| 8.10 | Resign & elimination | Resigning/elimination leaves buildings standing, kills units per rules, frees the slot | **MISSING** | Player lifecycle in sim | S | M8 (basic in M5 vs AI) |

## 9. AI opponents

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 9.1 | Opponent AI | A computer player runs the same economy/army loop: builds orders, ages up, expands, attacks, defends | **MISSING** — the world contains exactly one player's units; no AI of any kind | Scripted opponent issuing ordinary commands into the same command stream (deterministic) | L | M5 |
| 9.2 | Difficulty ladder | Easiest/Easy/Moderate/Hard/Hardest/Extreme change behavior (first attacks, walls/towers, cooperation, wonder building); DE AI takes no resource cheats | **MISSING** | Behavior parameters + build-order variants per level | M | M5 (2 levels), later (full) |
| 9.3 | AI social layer | AI responds to taunts/commands ("attack now", tribute requests), flares are understood, allies coordinate | **MISSING** | Taunt command parser + ally coordination hooks | M | Later |

## 10. Multiplayer, replay & spectate

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 10.1 | Lockstep netcode | Only commands cross the wire, executed at tick N+2 after a ~200 ms communication turn; host speed tuned to slowest machine; per-subsystem checksums detect desyncs (Bettner & Terrano model) | **MISSING** — architecture is ready (commands are data, FIFO at tick start, per-tick FNV-1a hash) but no transport exists | Relay/WebRTC transport + command scheduling + hash comparison | XL | M8 |
| 10.2 | Lobby | Create/join room (in DE: lists + invites; for us: send-a-link), pick empire/color/team, map & settings, ready/start | **MISSING** | Lobby service (in-memory first) + match settings UI | L | M8 |
| 10.3 | Ping & latency UX | Ping shown in lobby and in-game; input delay absorbs variance | **MISSING** | RTT measurement + display | S | M8 |
| 10.4 | Desync handling | Checksum mismatch → match flagged/stopped with diagnostics | **PARTIAL** — per-tick hash exists; nothing compares two machines yet | Hash exchange + mismatch flow | S | M8 |
| 10.5 | Replay viewer | Step/play/pause/speed, jump to events, per-player POV + fog toggles | **MISSING** (see 8.9) | Viewer over recorded command streams | M | M8 |
| 10.6 | Spectating | Join a live match as observer (with delay in ranked) | **MISSING** | Command-stream broadcast to observers | M | M8 |
| 10.7 | In-game chat & taunts | Text chat to all/allies; numbered audio taunts | **MISSING** | Chat channel beside the command stream (never inside the sim hash) | M | M8 |
| 10.8 | Multiplayer pause & restore | Limited pauses per player; dropped matches can be saved/restored | **MISSING** | Pause tokens + snapshot restore | M | M8 |
| 10.9 | Ranked / matchmaking / ELO | Ladder queues, ratings, leaderboards | **MISSING** | Accounts + rating service | XL | Later |

## 11. HUD & UI

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 11.1 | **MINIMAP** ★ | Always-on minimap: terrain colors, player-colored unit/building dots, fog shading, viewport rectangle; left-click jumps the camera, right-click orders movement there; ally flares flash; DE allows resizing/zoom modes | **MISSING** — no minimap anywhere (client renders only the main canvas) | Client minimap canvas fed from sim state (positions/kinds/nodes/terrain) + click/right-click wiring + viewport box; fog shading lands with 7.6 | M | **M4 — top priority** |
| 11.2 | Top resource panel | Icons + counts for 4 resources, population used/cap, current age, idle-villager button with count | **PARTIAL** — one text bar has resources/pop/villagers/queue/HOUSED + tick/hash/seed (`updateHud`); no age, no idle count | Rearranged panel (keep our own layout) + age + idle count (needs 1.6 ABI) | S | M4 |
| 11.3 | Selection panel | Portraits for every selected unit with HP bars, grouped by kind; click focuses, sub-group selection | **PARTIAL** — `#sel-info` text shows count · kind (since the 2026-10-16 fix); no portraits/HP (no HP exists yet) | Panel grid + HP after M3 | M | M4 (HP in M3) |
| 11.4 | Command card | Context button grid for the selection: move/stop/attack/patrol/stances/formations/build/train/research, with tooltips + hotkeys | **PARTIAL** — build menu + Train button exist as separate DOM rows, not a unified card | Unified card driven by selection kind | M | M4 |
| 11.5 | Production queues UI | Per-building queue icons with progress + cancel; DE adds a global queue display | **PARTIAL** — HUD text shows TC queue length only; no cancel, no progress | Queue rendering + cancel command | S | M4 (TC), M6 (all) |
| 11.6 | Building selection & panel | Clicking your building selects it: its queue, techs, rally point, garrison button | **MISSING** — buildings are unselectable; Train is a global button | Building selection state + panel | M | M4 (TC), M6 (rest) |
| 11.7 | Alerts | "You are being attacked", building/research complete, age advanced — sound + text + jump-to-location | **MISSING** | Event feed from sim → HUD + audio (12.3) | M | M4 |
| 11.8 | Order feedback markers | A brief marker where you ordered movement/attack; waypoint flags for queued orders | **MISSING** — orders are invisible after the click | Client-side marker sprites (renderer-only) | S | M4/M5 feel |
| 11.9 | Game clock & speed | F11 toggles time display; game time, not just ticks | **PARTIAL** — tick count shown; no mm:ss clock | Format ticks → game time | S | M4 |
| 11.10 | Diplomacy / objectives / tech-tree screens | F-keys open diplomacy, chat history, tech tree | **MISSING** | Panels (5.10, 8.6) | M | M6/M8 |

## 12. Audio & feedback

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 12.1 | Unit voice acknowledgements | Selection/order barks per unit type ("Yes?", "Ready", attack shouts) | **MISSING** — the client has no audio at all | Original recorded/synth voice set + event hooks (never copied audio) | M | M5 |
| 12.2 | World sounds | Chopping, mining, building, combat hits, deaths, arrows | **MISSING** | Sound engine (WebAudio) + event stream from renderer state | M | M5 |
| 12.3 | Alert & milestone sounds | Attack horn, age-up fanfare, building complete, wonder started (global announcement) | **MISSING** | With 11.7 | S | M5 |
| 12.4 | Music | Civilization theme + rotating ambient soundtrack | **MISSING** | Original compositions only | M | Later |
| 12.5 | UI sounds | Clicks, placement error buzz, chat blip | **MISSING** | Small synth set | S | M5 |

## 13. Empires (civilizations)

| # | Mechanic | AoE2 DE behavior | Our status | Parity requires | Size | Milestone |
|---|---|---|---|---|---|---|
| 13.1 | Empire system | Each civ = bonuses + team bonus + missing techs + 1–2 unique units + 2 unique techs + own Wonder + architecture set | **MISSING** — no empire concept; one generic roster | Empire = data table consumed by sim stat/tech tables | L | M3 (framework + empire #1) |
| 13.2 | Roster size | DE ships 45+ civilizations | **MISSING** — zero (by design: original empires, added one by one; the vision asks for *more* empires than the reference, over time) | Content pipeline (sprites + data per empire) | XL | Ongoing |
| 13.3 | Architecture sets | Building look varies by region set | **MISSING** — single placeholder look | Sprite-pipeline sets per region | L | Ongoing |

---

## Summary counts

| System | HAVE | PARTIAL | MISSING |
|---|---|---|---|
| 1. Selection & control | 4 | 3 | 4 |
| 2. Commands & unit behavior | 1 | 1 | 15 |
| 3. Economy | 3 | 2 | 7 |
| 4. Buildings | 0 (3 buildings had) | 1 | 18 |
| 5. Ages & technologies | 0 | 0 | 10 |
| 6. Military & combat | 0 | 1 | 12 |
| 7. Map, terrain & visibility | 0 | 0 | 9 |
| 8. Game structure | 0 | 2 | 8 |
| 9. AI opponents | 0 | 0 | 3 |
| 10. Multiplayer, replay, spectate | 0 | 1 | 8 |
| 11. HUD & UI | 0 | 5 | 5 |
| 12. Audio & feedback | 0 | 0 | 5 |
| 13. Empires | 0 | 0 | 3 |

The shape is honest: the **economic core loop and the deterministic engine are done**;
almost everything that makes it a *complete game* — combat, ages, map, fog, AI,
multiplayer — is still ahead. That matches the plan: M2.5 was the economy milestone.

## TOP-15 "feel parity" list

The items that most make Skirmish II *feel like the same game*, in order:

1. **Minimap** (11.1) — the single most-missed element; RTS players navigate by it.
2. **Combat core** — HP, damage/armor classes, attack, attack-move, auto-targeting (6.1–6.4, 2.3–2.4). The moment units can fight and die, it becomes the game.
3. **Scripted AI opponent + conquest win/lose** (9.1, 8.1) — turns the sandbox into a match.
4. **Control groups** (1.5) — pure muscle memory for anyone who has played.
5. **Idle villager key + idle indicator** (1.6, 11.2) — the heartbeat of economy play.
6. **Queued orders + rally points** (2.11, 2.12) — orders feel professional instead of one-shot.
7. **Stances + formations** (2.9, 2.10) — army control stops feeling like a blob.
8. **Combat collision feel** — bump-stop in melee, overlap while engaged (6.12).
9. **Ages** — Feudal/Castle/Imperial progression visibly growing the town (5.1).
10. **Fog of war + exploration** (7.6, 7.9) — scouting, tension, surprise.
11. **Random maps** (7.1) — a different Arabia every match; replayability itself.
12. **Selection panel + command card with HP** (11.3, 11.4) — the bottom-of-screen feel.
13. **Garrison + Town Bell** (2.13, 2.14) — defense drama: bell rings, villagers run, arrows fly.
14. **Sound: unit barks + alert horn + completion chimes** (12.1–12.3) — feel is half audio.
15. **Military variety** — knight/archer/siege/monk lines with real counters (6.5–6.9).

*Honorable mention:* Market exchange + trade (3.10–3.11) — the team-game endgame engine;
it ranks just below the 15 only because the roadmap is single-player first.

## Honest scale note

Full DE parity — 45+ civilizations with unique units, techs, wonders and architecture
sets, ~200 technologies, ~60 unit lines, navy, campaigns, a scenario editor — is an **XL
measured in years**, even for a professional studio (the open-source openage engine alone
has been in development for over a decade). It is also *not the goal*: Skirmish II's empires,
art, and content are original by design.

The realistic target — the **"mechanically equivalent core game"** — is:

- the complete generic tree for **1–3 original empires**: 4 ages, ~20 buildings,
  ~15 unit lines including siege and monks, Blacksmith/University/Monastery/Market
  technology subsets, one unique unit + one unique tech per empire;
- land random maps (Arabia-like first), elevation, fog of war, minimap;
- full selection/command model (this document's sections 1–2 complete);
- a scripted AI opponent with 2–3 difficulty levels and conquest + standard victory;
- 2-player lockstep multiplayer with send-a-link lobby, replay viewer, and chat.

That subset contains every *decision* an AoE2 player makes minute-to-minute — boom or
rush, which counter to build, when to age up, when to ring the bell — with a fraction of
the content surface. Everything beyond it (more empires, navy-first maps, campaigns,
ranked) is content growth on a finished mechanical core, which is exactly where an
open, continuously-improved game wants to be.

## Sources

Public documentation only, consulted 2026-10-16: Age of Empires fandom wiki (Hotkey ·
Unit formation · Battle Formations · Market · Trade · Coinage · Guilds · Victory ·
Regicide · Diplomacy · Signal Allies · Artificial intelligence · Unit) · aoe2techtree ·
Bettner & Terrano, GDC 2001 · Pottinger, Coordinated Unit Movement · openage public docs —
plus this project's [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) and [`OPENAGE-RESEARCH.md`](OPENAGE-RESEARCH.md), and the Skirmish II
source tree (`sim/src/`, `web-preview/`) for every status claim.
