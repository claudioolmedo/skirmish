# Skirmish II — openage & Genie Engine Research

**Date:** 2026-10-16
**Purpose:** Engine-architecture reference for Skirmish II, distilled from the open-source **openage** project (a volunteer clone of the Genie Engine behind Age of Empires I/II) and from public primary sources written by the original engine's developers. This document covers *how the engine works* — architecture, pathfinding, steering, terrain, sprites, fog, networking, data formats. Gameplay numbers (damage, unit stats, gather rates) live in the companion document [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) and are not repeated here.
**Clean-room note:** This study is based exclusively on public documentation: openage's published docs and blog, the original developers' GDC/magazine papers, and community format documentation. **No openage source code was read for implementation, and no Microsoft game files or assets were accessed.** openage is GPL-licensed (see §1.2); everything in this document is concepts, architecture, and published descriptions — the same public-knowledge basis as [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md).

---

## 1. The openage Project

Sources for this section: https://github.com/SFTtech/openage · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/architecture.md · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/game_simulation/README.md · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/converter/workflow.md · https://github.com/SFTtech/nyan

### 1.1 What it is

openage is a volunteer, free-software reimplementation of the **Genie Engine** (AoE1, AoE2 HD, Star Wars: Galactic Battlegrounds), in the family of OpenMW/OpenRA/OpenTTD-style engine clones. It does **not** ship any game data: to play, you must own one of the original games, and openage converts that install's assets into its own formats (see §1.8).

Project status, in the maintainers' own words: *"At the moment, 'gameplay' is basically non-functional."* The team discarded an earlier partially-working simulation and is rebuilding the gamestate for extensibility — currently covering entity basics (idle/move/turn/select) on top of a working converter, renderer, pathfinder, and data layer, with combat, economy, and multiplayer not yet in place. This matters for us as calibration: **Skirmish II (M0–M2 shipped, combat next) is ahead of openage on gameplay**, while openage is far ahead on asset conversion and rendering research — which is exactly the part we can learn from on paper.

### 1.2 License — confirmed

- **openage: GNU GPLv3 *or later*** (README, "License" section, pointing at the repo's `copying.md` and `legal/GPLv3`).
- **nyan** (its data language/libraries): **GNU LGPLv3 or later**.
- Some bundled assets/tooling carry their own licenses, which is why GitHub's license detector reports "Other" for the repo as a whole.

Consequence for Skirmish II: openage code is **copyleft**. Reading its *documentation* and adopting its *ideas* is fine; copying its *code* into our MIT-leaning, original-codebase sim would contaminate our licensing. Our rule, already in force: **concepts yes, code no** (see §3).

### 1.3 Architecture

| Layer | Technology | Notes |
|---|---|---|
| Engine core (`libopenage`) | C++20 | Simulation, renderer, pathfinding, event/time systems |
| Tooling & converter | Python 3 + Cython glue | Asset conversion, in-game console, code generation |
| GUI | Qt6 | Launcher/menus |
| Rendering | OpenGL + GLSL shaders | Terrain renderer, sprite renderer, team-color shader |
| Audio | Opus | Converted game audio is re-encoded to Opus |
| Content data | **nyan** files (plaintext) | All unit/tech/civ definitions (§1.4) |

Subsystem design points that differ sharply from our current design — deliberately, on their part:

- **Event-based simulation, no ticks.** "openage does not have simulation steps or *ticks* since everything in the simulation is event-based and scheduled by time." The sim loop fetches the current simulation time and executes every event queued up to that time. Continuous values (positions, HP) are modeled as **curves** — functions of time that are only evaluated when an event needs them — rather than being integrated step by step.
- **Decoupled threads.** Presenter (renderer/input), simulation, and time management each run in their own loop/thread and communicate through defined interfaces; the renderer and input are optional (headless simulation is possible).
- **Entities.** Everything in the world — units, buildings, trees, projectiles — is a `GameEntity`; terrain is a separate `Terrain` object. Abilities are data-driven components attached to entities via nyan (e.g. `Move` carries `speed`, movement `modes`, and a `path_type`; `Live` carries attributes; `LineOfSight` carries a reveal `range`).
- **Coordinate systems.** World positions use orthonormal **(NE, SE, UP)** axes measured in "terrain units", with origin at the west corner of tile (0,0) and tile centers at (0.5, 0.5). Positions are stored as **fixed-point integers with a 16-bit fractional part** — an independent validation of Skirmish II's I16F16 choice. Elevation ("UP", in terrain elevation units) is displayed scaled by **1/√8** to reproduce AoE2's terrain heights under the 2:1 isometric aspect. The map is grouped into **chunks of 16×16 tiles**. Mouse input is converted back to world coordinates by raycasting, which needs the terrain elevation at the click point ("move here" on a hill).

Sources: https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/architecture.md · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/coordinate-systems.md · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/game_simulation/README.md · https://github.com/SFTtech/openage/blob/HEAD/doc/nyan/api_reference/reference_ability.md

### 1.4 The nyan data format

nyan ("yet another notation") is SFTtech's data-description language, created because no existing format handled Genie's data complexity readably. Its key ideas:

- **Hierarchical objects with inheritance:** a game pack defines API types (`Unit` with `hp`, `animation`…); content derives from them (`OverwatchSoldier(Unit)`), and **changes to a parent propagate to all children**.
- **Patch-based modification:** mods are objects that apply deltas (`hp += 40`, `creates += {OverwatchElite}`) to existing definitions — including mods that patch other mods. Converted game data and community mods use the same mechanism, organized into **modpacks** loaded per game session.
- **Plaintext and diff-friendly**, parsed by a C++ core library (LGPLv3).

This is the modern, moddable evolution of Genie's original `.dat` model (§2.7), and the clearest published blueprint for how to structure data-driven RTS content.

Source: https://github.com/SFTtech/nyan

### 1.5 Pathfinding approach (summary)

Two-stage search: a high-level **A\* over a portal graph** between 16×16-tile sectors selects which sectors a path crosses; then, per visited sector, a **flow field** (integration field → steering vectors) is computed and followed to extract waypoints. The design is explicitly based on Elijah Emerson's *Game AI Pro* chapter "Crowd Pathfinding and Steering Using Flow Field Tiles". Full detail and our takeaways in §2.1.

Source: https://github.com/SFTtech/openage/blob/HEAD/doc/code/pathfinding/README.md

### 1.6 Rendering

OpenGL with separate terrain and sprite renderers. Terrain textures are blended at borders and displaced by per-tile elevation; sprites are drawn from converted texture atlases with a shader pass for **player colors** (team-color regions in the sprite are re-tinted per player — see §2.4). There is no 3D geometry for units: like the original, everything is pre-rendered 2D imagery composited in painter's order, which matches Skirmish II's fixed-angle isometric doctrine.

Sources: https://github.com/SFTtech/openage · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/coordinate-systems.md

### 1.7 Multiplayer / networking model — NOT lockstep

openage's documented goal architecture **rejects the original engine's lockstep model**: "We will have a single authoritative server that is also running the simulation asynchronously. Each client then receives the data visible for it." Networking is a decoupled, optional subsystem that forwards events and simulation parameters; the server filters per client by visibility (which also solves fog-of-war information hiding server-side). As of this writing, multiplayer in openage is a design document, not an implementation — their simulation being event-based and non-deterministic-friendly is precisely what makes the server model viable for them and lockstep unnecessary.

This is a fork in the road where **Skirmish II deliberately takes the other branch** — the original Genie branch (§2.6): fixed-tick deterministic simulation, commands-only networking, per-tick hashes. Our reasons are the same ones Ensemble had (bandwidth independent of unit count, tiny replays, cheat detection by checksum) plus one they didn't have: our sim already *is* deterministic by construction.

Source: https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/architecture.md

### 1.8 Asset conversion — requires the original game (we will NOT use it)

Confirmed: openage ships no assets. Its Python converter runs against **a game install the user provides**, and works in stages:

1. **Detection** — identifies edition/expansions/patch level by expected filenames plus MD5 hashes.
2. **Mounting** — asset folders and container archives (DRS, CAB) are mounted uniformly by media type.
3. **Reader** — each source format gets a parser. The Genie `.dat` is a *typeless* attribute stream, so every attribute's type is hand-declared in a schema and the parser is generated per game version.
4. **Processor** — raw entities are grouped into API concepts (e.g. a *unit upgrade line*: militia → man-at-arms → …) and mapped onto nyan objects; referencing a graphics/sound ID generates an on-demand media export request.
5. **Exporter** — writes plaintext nyan files; media is converted to sane target formats (textures as images, audio as Opus).

**Skirmish II position:** we never run this pipeline and never use its output. Our art is original (SPRITE-PIPELINE: our own 3D models rendered to 2D sheets). The converter is documented here only because its *stage design* (schema-declared data, concept grouping, on-demand media) is a useful reference for our own data tooling.

Source: https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/converter/workflow.md

---

## 2. Engine Knowledge for Our Implementation

### 2.1 Pathfinding: portal A\* + flow fields, and what the original did

**openage's two-stage design** (their pathfinding doc + 2024 devlogs):

- The grid is divided into **sectors** (fixed square size per grid, e.g. 16×16 cells — the same granularity as their terrain chunks).
- Every maximal run of passable cells along a sector edge forms a **portal** to the neighboring sector; portals within a sector are pre-linked if mutually reachable. The result is a small **portal graph**.
- A path request runs **A\* over the portal graph** (cheap: cost scales with portal count, not map size) to get the corridor of sectors.
- For each sector in the corridor (target sector first), an **integration field** is computed: every cell gets the minimum accumulated movement cost to reach the target, integrating outward from the target cell (`own cost + cheapest neighbor's integrated cost` — a Dijkstra flood). Portal cells pass values between sectors so fields join seamlessly.
- The **flow field** then stores, per cell, a steering vector pointing at the neighbor with the lowest integrated cost. A path is extracted by following vectors from the start cell and emitting a **waypoint at every direction change**, so entities travel in straight lines between waypoints.
- **Cost fields:** each cell's movement cost is a byte: 1 (minimum) … 254, with 255 = impassable. The doc notes that for Age of Empires, usually only minimum and impassable are relevant. Costs can change during gameplay (buildings placed, trees felled).
- **Multiple grids:** there is one grid per movement type (land, water, air…), each with its own sector size — a unit's `Move` ability names the grid its path type searches.
- **Line-of-sight shortcut:** cells with a clear line of sight to the target are flagged during integration; flow vectors are unnecessary there (units can steer straight at the target), and LOS blockers (impassable or higher-cost cells) form "vision cone" boundaries in the grid. This doubles as the seed of their visibility work (§2.5).

Why flow fields for an RTS: one field serves **every unit heading to the same target** (group move orders, attack waves, villagers returning to a drop-off), and the same field provides steering for local collision avoidance — at the price of an up-front flood cost per (sector, target), mitigated by the portal corridor and by caching.

**What the original Genie engine did** (for contrast — this is the behavior players remember):

- Per-unit **A\*** on the tile grid. Pottinger has said Ensemble went through **five different implementations of A\*** with several optimizations each, calling pathing the hardest problem on the project — largely because the AI player also pathfinds heavily, on large, dense, *fully dynamic* random maps.
- Units have **no pushing**: in the Definitive Edition (per a 2025 talk by a DE pathfinding developer), "units bump into others, stop and repath around. No pushing allowed," and formations tolerate **some friendly overlap**. The original movement model (§2.2) similarly treats overlap as unavoidable and manages it rather than forbidding it.

**For Skirmish II:** M1 already ships per-unit A\* + steering, matching the original. The openage design is our scaling path *if* group-move cost ever dominates: portal-A\* over 16×16 sectors slots directly onto our chunking, and flow fields are a per-sector cache keyed by target cell. Decision rule: adopt when profiling (not intuition) shows A\* dominating tick time — with hundreds of units on ≤256×256 maps, plain A\* with a good heuristic may never need replacing. Do **not** adopt preemptively in M3.

Sources: https://github.com/SFTtech/openage/blob/HEAD/doc/code/pathfinding/README.md · https://github.com/SFTtech/openage-blog/blob/HEAD/content/blog/devlog_2024_01.md · https://github.com/SFTtech/openage-blog/blob/HEAD/content/blog/devlog_2024_03.md · http://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf · https://aoe.heavengames.com/town/interviews/dave/ · https://meetingcpp.com/mcpp/slides/2025/25+ Years of Pathfinding Problems442903.pdf

### 2.2 Unit steering, collision, and group movement

The primary source is Dave Pottinger (Ensemble, AoE series) in *Game Developer Magazine*: **"Coordinated Unit Movement"** and its follow-up **"Implementing Coordinated Movement"**. The model that shipped in AoE, in concepts:

- **Movement is a managed system, not per-unit physics.** A group move creates coordinated behavior: units keep formation slots (with role-appropriate placement — melee in front, archers behind), and the group has shared state (destination, arrival) instead of N independent agents.
- **Overlap is inevitable and acceptable.** Pottinger's famous line from this work — "no matter how much code you write, your units will always find a way to overlap" (as quoted in later research summaries). The engine therefore *allows* units to overlap somewhat (especially in combat, where forbidding overlap makes units wander into damage) and resolves crowding gradually with give-way/yield behaviors rather than hard collision.
- **Collision handling is behavioral**: units that bump stop, wait or sidestep, and repath if blocked — the DE talk confirms this is still the shipped behavior today (bump → stop → repath; no pushing; formations allow some friendly overlap).

**For Skirmish II:** our M1 steering uses displacement-based separation (units push apart smoothly), which is *not* the reference behavior — see the divergence flag in §4. Neither is "wrong": separation gives smoother crowds; bump-stop-repath gives the authentic melee "shuffle" and makes chokepoints behave like AoE2's. For M3 melee feel specifically, consider a hybrid: keep separation for movement, but let *engaged* melee units tolerate overlap (no separation while attacking) so lines pack and fight instead of shoving — this mirrors the original's "overlap is fine in combat" stance.

Sources: https://www.gamedeveloper.com/programming/coordinated-unit-movement · https://meetingcpp.com/mcpp/slides/2025/25+ Years of Pathfinding Problems442903.pdf

### 2.3 Terrain and height

- **Elevation is per-tile data** (an UP value in terrain elevation units), independent of terrain *type*. Rendering lifts the tile (and everything on it) by elevation × 1/√8 in scene space; gameplay effects (hill bonus) are computed from elevation *differences* — mechanics in [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) §1.4.
- **Terrain and pathfinding are decoupled in openage**: the pathfinder holds its own cost grids; terrain (and gameplay events) merely *influence* cell costs. This separation is worth copying architecturally even in our simpler sim: terrain type/elevation in one table, pathing cost derived from it per movement type.
- **Cliffs** in Genie are a terrain-border concept (an elevation discontinuity), not a height value — they block land movement regardless of the heights involved. Our grid is currently flat (MECHANICS-STUDY §10.3); when elevation lands, store a per-cell height byte as the study suggests and treat cliff borders as cost-255 edges.
- **Coordinate discipline:** openage's insistence on named coordinate systems (world NE/SE vs tile vs pixel vs HUD) exists because isometric picking/rendering bugs breed in unnamed conversions. Our Canvas renderer has fewer systems, but the same rule applies: world↔tile↔screen conversions should be single, named functions (the preview already centralizes these in `app.js`).

Sources: https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/coordinate-systems.md · https://github.com/SFTtech/openage/blob/HEAD/doc/code/pathfinding/README.md

### 2.4 Sprites and directions

How Genie-era sprites work, from community format documentation (SLP is the original sprite container, stored inside DRS archives):

- An SLP holds the frames of one graphic; **each frame carries centering (anchor/hotspot) data** — the pixel offset at which the sprite is pinned to its world position. Anchors are data, not convention: two frames of the same animation can have different anchors.
- **Unit graphics store frames for 5 of the 8 facing directions; the other 3 are produced by horizontal mirroring** at render time. (Documented in modding tutorials: extracted unit BMP sets contain 5 directions, "you have to edit the images and use mirror images.") The `.dat` graphic definition declares how many angles a graphic has and which frames belong to which angle/action.
- **Player color is a palette remap**: sprites are palettized, a reserved index range encodes "player color", and the renderer substitutes the owning player's color ramp (openage does this in a shader). Shadows are separate graphics drawn beneath the unit.
- Animation is frame-sequenced per action (stand, walk, attack, die, decay…), with the attack's damage moment tied to a specific frame — the data-level origin of the "frame delay" mechanic quantified in [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) §2.2.

**For Skirmish II:** our sprite pipeline (original models rendered to sheets at 32/16/8-direction quality tiers) already exceeds the reference's 8 facings — good for smoothness, but the *authenticity-relevant* lessons are the other three: **(1)** per-frame anchors must be exported by our pipeline and honored by the renderer, or feet will slide; **(2)** keep a player-color channel/mask in our sheets so team color is a render-time tint, not baked art — 8 tints of baked art is a content-cost trap; **(3)** the attack frame index is data the sim's attack-delay counter must agree with — define it in the unit table (MECHANICS-STUDY §10.1) alongside frame delay ticks.

Sources: https://forums.civfanatics.com/threads/how-to-extract-units-from-ror-and-aoe.173872/ · https://aoe.heavengames.com/dl-php/showfile.php?fileid=5 · https://aok.heavengames.com/blacksmith/showfile.php?fileid=11002

### 2.5 Fog of war and line of sight

The Genie model (and every RTS of its lineage):

- **Three visibility states per tile, per player:** unexplored (black), explored-but-not-visible (terrain + last-seen buildings, dimmed; units hidden), visible (live). Buildings remain visible in explored areas at their last-seen state; units appear only in currently-visible tiles.
- Visibility is the union of **line-of-sight circles** around the player's units/buildings. In the reference data, a unit's LoS is a stat of its own (MECHANICS-STUDY lists LoS per unit; for ranged units LoS is typically range + 2), **capped at 20 tiles** engine-wide.
- LOS is *radius-based, not raycast-occluded*, in classic AoE2: hills and forests do not block sight (there is no vision-cone occlusion in the shipped game — a deliberate simplicity). openage's devlogs experiment with grid LOS flags and vision cones for their pathfinding integration fields, but that is pathing machinery, not shipped fog occlusion either.
- Note the architectural consequence in openage's design (§1.7): an authoritative server "receives the data visible for it" — fog becomes a **server-side information filter**. In our lockstep model, every client simulates everything, so fog is purely presentational and a determined cheater can reveal the map locally — a known, accepted lockstep trade-off that Bettner & Terrano already documented in 2001 (§2.6).

**For Skirmish II:** fog is a renderer-side per-player tile grid (3 states) fed by sim LoS radii; recompute on unit movement ticks only, and keep it **out of the hashed sim state** (it is derived data — but if it ever feeds gameplay decisions, e.g. AI or auto-targeting, it must be recomputed deterministically inside the sim instead). Targeting must use sim truth, not fog, exactly as the reference does.

Sources: https://ageofempires.fandom.com/wiki/Line_of_Sight · https://github.com/SFTtech/openage-blog/blob/HEAD/content/blog/devlog_2024_03.md · https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/architecture.md

### 2.6 Determinism and lockstep — the original developers' lessons

Primary source: Paul Bettner & Mark Terrano, **"1500 Archers on a 28.8: Network Programming in Age of Empires and Beyond"** (GDC 2001 / Game Developer Magazine) — the Genie multiplayer design, from its authors. Key facts and lessons:

- **Why lockstep:** sending unit state for thousands of objects was impossible on a 28.8k modem (they calculated state sync would cap the game at ~250 moving units). Instead every machine runs the **exact same simulation** and only **player commands** cross the wire — bandwidth becomes independent of unit count.
- **Communication turns:** the network ran on ~**200 ms "communication turns"**, decoupled from render frames. Commands issued during turn *N* were tagged to execute at turn *N+2*, giving a pipeline buffer so the game never stalls waiting for packets.
- **Speed control:** each client reported, in its 2-byte "Turn Done" trailer, the frame rate it could sustain and its longest average ping. The host continuously re-tuned turn length and target frame rate for the *slowest* machine — weighting adjustments to rise fast under lag spikes and settle slowly. **A consistent slower game beat a faster jittery one**; their playtesting found <250 ms command latency unnoticeable, 250–500 ms very playable, >500 ms noticeably degrading — and *variable* latency felt worse than a steady 500 ms.
- **Transport:** UDP with their own ordering, drop detection, and resend ("when in doubt, assume it dropped"); stale-turn messages were discarded.
- **Desync discipline:** they **checksummed everything** — world state, objects, *pathfinding*, *targeting*, and the RNG stream — because out-of-sync was, as predicted to them, "the one stubborn bug that goes all the way to the wire". Subtle divergences multiply: a deer offset at map-gen forages differently, minutes later a villager paths differently, and the visible symptom (different food totals) is far from the cause. Two disciplines fell out of this: (1) simulation code must make the **same number of RNG calls in the same order** on every machine (RNG seeded and synchronized like everything else); (2) simulation code must not depend on any local factor (hardware, settings, spare time).
- **Command metering:** players averaged one command per 1.5–2 s, spiking to 3–4/s in battles; excited repeat-clicking on the same spot caused traffic spikes, fixed by a **filter discarding duplicate same-location commands**. Metering was "king" — and removing it too early let regressions slip in.
- **Hidden benefit:** lockstep is cheat-resistant — any client simulating differently is detected by checksum and the game stops. (Map-reveal cheats remain possible; see §2.5.)
- Engine context from the same paper: Genie is a **2D, single-threaded, tile-based** engine; post-optimization CPU split was roughly 30% rendering / 30% AI & pathing / 30% simulation & maintenance. Pathing+AI is a third of the budget — plan M5 AI expectations accordingly.

Modern corroboration: Glenn Fiedler's "Deterministic Lockstep" and "Floating Point Determinism" (Gaffer on Games) explain why floats are not reliably identical across compilers/architectures — the reason Skirmish II uses fixed point — and GDC 2024's *Realms of Ruin* talk shows a shipped modern lockstep RTS making the same choice (fixed point, per-system RNG streams, state-dump diff tooling). Our per-tick FNV-1a hash is the direct descendant of Bettner & Terrano's checksums; their lesson for us is to hash **subsystems separately** (world / pathing / targeting / RNG state) when debugging, so a desync report names the diverging system instead of just the tick.

Sources: https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires-and-beyond · https://zoo.cs.yale.edu/classes/cs538/readings/papers/terrano_1500arch.pdf · https://gafferongames.com/post/deterministic_lockstep/ · https://gafferongames.com/post/floating_point_determinism/

### 2.7 Data-driven unit definitions

Two generations of the same idea:

1. **Genie `.dat` (1997–):** one binary file holds *all* content — units, techs, civs, graphics references, terrains, sounds — as a typeless attribute stream interpreted by hand-declared schemas (the openage converter still has to declare every attribute's type manually, per game version). Unit definitions are **flat records**: stats, armor/attack *class* tables, graphic IDs per action, terrain restrictions. Techs and civ bonuses are **effect lists** that modify unit records — the data-level basis of the armor-class/bonus system in [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) §1.
2. **nyan (openage):** the same content as plaintext objects with **inheritance** (unit lines as parent→child derivations) and **patches** (techs/bonuses as named delta objects), organized into modpacks (§1.4).

**For Skirmish II:** our unit data already lives in Rust tables; M3 is the moment to formalize them in the nyan *shape* (not the nyan format): base records per unit, line upgrades expressed as deltas from the parent tier, techs/civ bonuses as named patches applied at well-defined points. This keeps balance data reviewable as diffs and makes the M3 data table auditable against [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) §3 line by line. What we should *not* import: the `.dat`'s flatness (IDs pointing at IDs with no type safety) — Rust enums/structs give us the schema the Genie format lacked.

Sources: https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/converter/workflow.md · https://github.com/SFTtech/nyan · https://github.com/SFTtech/openage/blob/HEAD/doc/nyan/api_reference/reference_ability.md

---

## 3. What We Take / What We Never Take

**We take — public knowledge and design lessons:**

- Engine *concepts* documented in public papers and docs: flow fields/portals as a scaling option (§2.1), coordinated-movement group behavior (§2.2), event-vs-tick trade-off knowledge (§1.3, §2.6), nyan's inheritance+patch *shape* for data (§2.7), coordinate-system discipline (§2.3).
- The original developers' published engineering lessons: lockstep turns, speed control, checksum discipline, RNG call-order discipline, command metering (§2.6).
- Sprite *format concepts*: 8 facings via 5 renders + mirroring, per-frame anchors, player-color as a render-time remap, attack-frame indexing (§2.4).
- Published gameplay facts and numbers, already collected in [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md).

**We never take:**

- **openage code.** GPLv3 (engine) / LGPLv3 (nyan) — copying code would copyleft our original codebase and breach the clean-room rule. Concepts and documentation only; our implementation is our own Rust.
- **Converted or original Microsoft assets** — no SLP/DRS graphics, no sounds, no `.dat` data, nothing produced by openage's converter. All Skirmish II art and data are original (public *facts* about the reference game, like "a Knight has 100 HP", are recorded as facts in the mechanics study — facts are not assets).
- **openage's multiplayer architecture** — not for legal reasons but by design: their authoritative-server/event-based model is incompatible with our deterministic lockstep sim (§1.7). We take the *original* Genie network model instead, from its authors' paper.

---

## 4. Cross-Check Against MECHANICS-STUDY.md

Everything in this research was checked against the mechanics study. **No numeric or mechanical conflicts were found** — the studies are consistent on damage, projectiles (fixed trajectory, no homing), gather/economy, and building data, none of which openage's docs contradict (openage has no combat/economy implementation to contradict them with). Four items deserve explicit flags:

| # | Item | Status |
|---|---|---|
| 1 | **Collision feel** — MECHANICS-STUDY §4.2/§10 note Skirmish II's separation steering "matches the original in spirit" | ⚠️ **Behavioral divergence, now documented:** the reference behavior (Pottinger; DE dev talk) is *bump → stop → repath, no pushing, overlap tolerated mainly in combat/formations*. Our smooth separation is a legitimate modernization, but the study's "in spirit" phrasing is generous. §2.2 proposes a hybrid for M3 melee. This is a feel decision for M5 tuning, not an error in either document. |
| 2 | **Simulation model** — MECHANICS-STUDY §10 assumes fixed 60 ticks/s + lockstep | ℹ️ **Deliberate divergence from openage (not from the reference game):** openage uses event-based continuous time and an authoritative server. We follow the *original Genie* model (fixed steps + lockstep). No change needed; flagged so nobody "corrects" our design toward openage's later. |
| 3 | **Line-of-sight cap** — not stated in MECHANICS-STUDY | ➕ **New fact, no conflict:** reference LoS is capped at 20 tiles (§2.5). Add the cap when LoS/fog is implemented. |
| 4 | **Sprite direction count** — SPRITE-PIPELINE tiers assume up to 32 directions | ➕ **Context, no conflict:** the reference uses 8 facings (5 rendered + 3 mirrored). Our higher tiers are a quality upgrade, not a deviation that needs fixing. |

Additionally, two of the mechanics study's open items are *narrowed* by this research: per-unit collision radius (§4.2 of the study) remains undocumented in openage's public material as well — the 0.2-tile default stands as our design choice; and stance leash distances remain undocumented everywhere public.

---

## 5. Most Valuable Findings for M3 (Combat)

1. **Engaged-melee overlap rule (§2.2):** let attacking melee units tolerate overlap (suppress separation while engaged). This is the single cheapest change for authentic AoE2 melee feel, and it prevents separation from shoving units out of attack range mid-fight.
2. **Data shape for combat stats (§2.7):** express unit lines as parent + deltas and bonuses/techs as named patches in the M3 data table. M3 is where the armor-class matrix lands; structuring it as data (not code branches) keeps it auditable against MECHANICS-STUDY §3.
3. **Attack-frame = data (§2.4):** the frame index at which damage lands is part of the unit's graphics data in the reference design. Our sim-side equivalent (attack-delay ticks, MECHANICS-STUDY §10.1) should live in the same table row as the sprite animation definition, so art and sim can't drift apart.
4. **Targeting belongs in the checksum set (§2.6):** Ensemble checksummed *targeting and pathfinding separately* from world state. When M3 adds auto-targeting, extend our hash to cover per-subsystem subtotals (or log them in tests) — the first combat desync will thank us.
5. **RNG discipline gets real in M3 (§2.6):** accuracy rolls are our first combat RNG (MECHANICS-STUDY §10.3). Bettner & Terrano's rule — identical RNG call *count and order* on every machine — is the discipline our fixed iteration order already implements; M3 must not break it (no RNG draws inside rendering, UI, or HashMap iteration).

---

## Source List

All sources are public web pages/documents, consulted 2026-10-16. No game binary, game files, or GPL source code were accessed.

**openage project (documentation only):**
- https://github.com/SFTtech/openage (README — project overview, license, tech stack)
- https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/architecture.md
- https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/game_simulation/README.md
- https://github.com/SFTtech/openage/blob/HEAD/doc/code/pathfinding/README.md
- https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/coordinate-systems.md
- https://raw.githubusercontent.com/SFTtech/openage/master/doc/code/converter/workflow.md
- https://github.com/SFTtech/openage/blob/HEAD/doc/nyan/api_reference/reference_ability.md
- https://github.com/SFTtech/openage-blog/blob/HEAD/content/blog/devlog_2024_01.md (integration/flow fields)
- https://github.com/SFTtech/openage-blog/blob/HEAD/content/blog/devlog_2024_03.md (portals, line-of-sight pass)

**nyan:**
- https://github.com/SFTtech/nyan

**Original developers' publications:**
- Bettner & Terrano, "1500 Archers on a 28.8" (GDC 2001): https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires-and-beyond · PDF: https://zoo.cs.yale.edu/classes/cs538/readings/papers/terrano_1500arch.pdf
- Pottinger, "Coordinated Unit Movement" (Game Developer Magazine, 1999): https://www.gamedeveloper.com/programming/coordinated-unit-movement
- Pottinger interview (AoE Heaven, on A\* iterations and pathing difficulty): https://aoe.heavengames.com/town/interviews/dave/
- "25+ Years of Pathfinding Problems" (meetingcpp 2025, DE pathfinding developer): https://meetingcpp.com/mcpp/slides/2025/25+ Years of Pathfinding Problems442903.pdf

**Technique literature:**
- Emerson, "Crowd Pathfinding and Steering Using Flow Field Tiles" (Game AI Pro): http://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf
- Fiedler, "Deterministic Lockstep": https://gafferongames.com/post/deterministic_lockstep/
- Fiedler, "Floating Point Determinism": https://gafferongames.com/post/floating_point_determinism/

**Community format documentation (sprites/data):**
- https://forums.civfanatics.com/threads/how-to-extract-units-from-ror-and-aoe.173872/ (5 stored directions + mirroring)
- https://aoe.heavengames.com/dl-php/showfile.php?fileid=5 (SLPCNVT — SLP frames + centering data)
- https://aok.heavengames.com/blacksmith/showfile.php?fileid=11002 (Advanced Genie Editor — .dat/graphics/player-color editing)
- https://ageofempires.fandom.com/wiki/Line_of_Sight (LoS cap 20)

*Companion document: [`MECHANICS-STUDY.md`](MECHANICS-STUDY.md) (gameplay numbers and combat math, same date). Together they are the public-knowledge basis for Skirmish II's design.*
