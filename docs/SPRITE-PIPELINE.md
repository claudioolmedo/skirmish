# Skirmish II — Sprite Pipeline

How Skirmish II's original art is produced: **original low-poly 3D models,
built procedurally in code, rendered from one fixed isometric camera
into 2D sprite sheets** — the same production method as the classic
engine we are inspired by, with 100% our own art. No external models,
no reference art, no ripped assets (clean-room rule).

Pipeline code: `tools/sprite-pipeline/` in the repo.
One command regenerates everything:

```bash
python3 tools/sprite-pipeline/render.py [--player-color RRGGBB] [--out DIR]
```

Current implementation: a Python software renderer (numpy + Pillow) —
orthographic projection, z-buffered flat-shaded triangles, 2x
supersampled. (Blender headless was evaluated as an alternative host
for the same model scripts; the software renderer won for v1 because
it is deterministic, dependency-light, and the flat-shaded low-poly
aesthetic is exactly its native output.) Requires only Python 3 +
numpy + Pillow.

---

## 1. Camera spec (fixed, never changes)

- **Projection:** orthographic 2:1 dimetric, identical to the game
  renderer (`web-preview/app.js`, function `w2s`):
  - `screen_x = (X - Y) * P`
  - `screen_y = ((X + Y) / 2 - Z) * P`
  where `P` = pixels per world unit. No rotation, no perspective,
  ever. The camera in the game only pans and scale-zooms these sprites.
- **Render resolution:** `P = 46` px per world unit at 1x (supersampled
  2x internally, downscaled with premultiplied-alpha averaging so edges
  have no dark halo).
- **Lighting:** one fixed world-space sun, direction
  `normalize(-0.52, 0.28, 0.81)` (from the surface toward the light) —
  screen upper-left, above. Fixed in *world* space, so a unit's shading
  rotates with its facing like a real sun. Flat shading, posterized to
  4 levels (0.47 / 0.64 / 0.81 / 1.0 over an 0.47 ambient floor): the
  baked two-tone look, no gradients, no textures.
- **Depth:** per-pixel z-buffer on `X + Y` (larger = nearer the
  viewer); exact for this projection.

## 2. Model space and proportions

- Axes: `+X` right, `+Y` forward (the way the unit faces), `+Z` up.
  Origin at the ground point between the feet.
- Models are assembled from boxes and tapered boxes (frustums) in
  code, grouped by joints (legs about hip pivots, arms about shoulder
  pivots, upper body about a waist pivot). Poses are joint-angle
  dictionaries per frame — animation is data, not keyframe files.
- Villager v1 proportions (world units, total height ~1.66): boots
  0.00–0.11, legs to 0.82, tunic skirt 0.72–0.99, belt 0.93–1.02,
  torso 0.98–1.34, head 1.36–1.65, hair cap to ~1.69. Chunky,
  readable-at-game-zoom proportions are deliberate.

## 3. Directions and quality tiers

- **Direction index:** row `d` (0–7+) = facing angle `d * (360 / tier)`
  in world XY, measured from `+X` toward `+Y`. Direction 0 faces `+X`
  (screen down-right); 2 faces `+Y` (screen down-left); 4 faces `-X`;
  6 faces `-Y`. The renderer quantizes a unit's velocity angle to the
  nearest direction of the loaded tier.
- **Tiers** (adaptive quality: the renderer picks one tier at load by
  device capability; the sim is identical for everyone):
  - **LOW: 8 directions** (45° apart) — ships first, for every unit.
  - **MEDIUM: 16 directions** (22.5°) — same pipeline, one flag.
  - **HIGH: 32 directions** (11.25°) — same pipeline, one flag.
  Generating a higher tier is a re-render, never new art: the 3D model
  is the single source of truth.
  Status (2026-10-16): all three tiers are generated and shipped —
  `render.py --tiers` emits `assets-original/tier16/` and `tier32/`
  (palette-optimized PNGs; the four villager sheets measure ≈191 KB
  at MEDIUM and ≈389 KB at HIGH, vs ≈205 KB for the LOW truecolor
  set), and the client's adaptive-quality governor
  (`web-preview/play/quality.js`) loads exactly one tier per device,
  chosen by probe + live FPS — see ARCHITECTURE.md.

## 4. Animation states and frame counts

Per unit, one PNG per state: **rows = directions, columns = frames**,
all cells the same size, no per-frame trimming (anchor stability beats
file size).

| State  | Frames (LOW) | Playback | Notes |
|--------|--------------|----------|-------|
| idle   | 4 | ~2.5 fps loop | breathing bob, tiny arm sway |
| walk   | 8 | stride-locked: phase advances with distance travelled (~1.5 world units per cycle) | legs ±30°, arms counter-swing, body bob |
| gather | 4 | ~7 fps loop | chop cycle: raise, strike, impact, recover; legs in a work stance |
| carry  | 8 | stride-locked, same phase as walk | walk pose + arms raised holding a bundle overhead |
| build  | 4 | ~7 fps loop | planned with the construction system: hammer pose, shares the gather rig |
| attack / death | TBD | — | arrive with M3 combat design |

## 5. Canvas and anchor conventions

- **Cell: 128 x 128 px**, square, identical for all unit sprites.
- **Anchor: feet-center at (64, 110)** inside every cell — the model's
  ground origin maps exactly there in every frame and direction, so
  animation never drifts or jitters. Sprites are drawn by placing the
  anchor on the unit's ground point.
- **Margins:** every frame must keep ≥3 px of transparency on all
  sides (verified automatically after render; the swinging boot corner
  and the carry bundle are the binding constraints).
- **In-game scale:** the renderer draws the cell scaled by
  `k = camS * 1.35 / 46`, i.e. sprites render at 1.35x true world
  scale (classic RTS readability cheat); a villager stands ~2.27
  screen-units tall. These constants are mirrored in `app.js`
  (`VIL_ORIGINAL`) and must be changed in both places together.
- Ground shadows are NOT baked in; the game renderer draws the
  elliptical shadow (so it stays correct at every zoom).
- **Building canvas (house, the first building):** buildings have a
  single fixed orientation, so a building is one image per state —
  no direction rows. The canvas is sized to the footprint: the
  house uses **896×704 px** with the ground anchor (center of the
  2×2-cell footprint) at **(448, 448)**, at the same 46 px/unit, and
  is drawn at **true scale (1.0)** — the 1.35 readability factor
  above is a unit-only cheat. Constants mirrored in `app.js`
  (`HOUSE_ORIGINAL`). Construction stages are separate images the
  client selects by build progress (see §8 item 2).

## 6. Player color

- Each player owns a base color. Clothing designated as player color
  (the villager's tunic + sleeves) is rendered from a palette ramp
  derived from that single base color by the pipeline's quantized
  shading — no other material uses those hues.
- Sheets are generated per player color with
  `--player-color RRGGBB` (regeneration takes seconds). The default
  sheet shipped in `web-preview/assets-original/` uses player blue
  `#2e6fd8`. A future runtime palette-swap can reuse the same ramp
  definition; team-color regions are never anti-aliased against other
  colors except at silhouette edges.

## 7. File naming and layout

- `web-preview/assets-original/<unit>-<state>.png`
  (e.g. `villager-walk.png`), plus a `README.md` in that folder with
  provenance (our pipeline, script-generated, model source in
  `tools/sprite-pipeline/`).
- Placeholder CC0 art lives in `web-preview/assets-placeholder/`; as
  each original lands, the placeholder's README marks it replaced.
- Pipeline output is committed; anyone can regenerate byte-similar
  sheets from the model source.

## 8. Replacement order

1. **Villager** — DONE (v1: idle/walk/gather/carry; all tiers —
   LOW 8 / MEDIUM 16 / HIGH 32 directions — generated 2026-10-16).
2. **House** (incl. construction stages) — DONE (v1, 2026-10-16):
   timber-frame cottage, four stage images (foundation → frame +
   scaffold → walls + roof rising → complete) on an 896×704 canvas
   (footprint-anchored, §5); the client picks the stage by build
   progress instead of clip-revealing the placeholder. Kenney house
   retained as load-failure fallback.
3. **Trees and resource nodes** (tree, berry bush, gold, stone).
4. **Town Center and other buildings.**
5. Further units (M3 combat units) are born original — no placeholder
   stage needed once the pipeline exists.

## 10. Directional milestone (2026-10-19) — units, animals, masks, lifecycle

Claudio's ruling: side-view cycles + horizontal flip (the generated
UNIT_ART / ANIMAL_ART sets) are INTERIM. The standard is the classic
one — art rendered for the fixed camera **and** the facing direction
(back view going up-screen, front coming down, 3/4 views on the
diagonals), designated areas swapping color per player, and buildings
living the full damage lifecycle (fire, rubble, repair). This section
is the spec for that milestone; §1–§8 and §11 remain in force.

### 10.1 Sheet layout (all units and animals)

- One PNG per (unit, state): **rows = 8 directions, columns =
  frames**, cell 128×128, anchor feet-center (64, 110), 46 px/unit,
  drawn at the unit scale factor 1.35 (§5) — the villager v1
  contract, generalized. Scout Cavalry is the one exception: cell
  160×160, anchor (80, 138) (horse + rider exceeds the 128 cell).
- Direction row `d` = facing angle `d × 45°` in world XY from +X
  toward +Y (§3). Consequences the QA eyeball must confirm: row 1
  (facing +X+Y, down-screen) is the FRONT view; row 5 (−X−Y,
  up-screen) is the BACK view; rows 0/2 are front 3/4, rows 4/6
  back 3/4, rows 3/7 profiles. All 8 rows are rendered from the 3D
  model — no runtime mirroring (the model is the source of truth;
  mirroring is how the reference engine economized on *stored* art,
  not a property we need to reproduce).
- States and frame counts (LOW tier): idle 4 · walk 8 · gallop 8
  (scout) · gather 4 · carry 8 · build 4 (villager) · attack 4
  (militia, man-at-arms) · shoot 4 (archer) · graze 4 (animals).
  Facing comes from the unit's movement/order vector quantized to
  8; an idle unit keeps its last facing.
- Models live in `tools/sprite-pipeline/models/<unit>.py`
  (articulated low-poly part groups + pose functions, the villager
  pattern); `tools/sprite-pipeline/units.py` renders every sheet +
  mask and can emit contact sheets for QA. The side-view generated
  sets remain in the client's fallback chain beneath these sheets.

### 10.2 Player color — the mask mechanism

The reference engine stores player-color pixels in a reserved
palette range and remaps them to the player's ramp at draw time.
Our PNG equivalent, per sheet:

- Alongside `<unit>-<state>.png`, the pipeline renders
  `<unit>-<state>-pcmask.png`: same layout, same rasterization, but
  player-color-material pixels are grayscale at their posterized
  shade level (0.47 / 0.64 / 0.81 / 1.0 → gray 120 / 163 / 207 /
  255) and all other pixels are black; alpha is identical to the
  color pass, so the mask aligns pixel-for-pixel.
- Model parts are flagged as player color in the model source
  (`PC_MATS`): villager tunic + sleeves; archer's cloth shirt;
  militia tunic + shield cloth; man-at-arms tabard + shield cloth;
  scout rider's tunic + saddle blanket. Animals carry none.
- The client builds the red player's variant once per sheet at
  load: for each pixel with mask gray > 0, output = red base
  (#d2342c) × (gray / 255); all other pixels copy the color sheet.
  Blue is the native render — no variant needed. This replaces the
  channel-swap approximation (`makeRedSheet`) wherever a mask
  exists; the swap remains the fallback for mask-less sheets.

### 10.3 Building lifecycle visuals

- **Fire** (client-only overlay): the two documented stages —
  **smoke puffs at HP ≤ 75%**, **flame clusters at HP ≤ 50%**
  (cluster size scales with damage inside the flame stage, a
  presentation detail) — anchored on the building's roof/body.
  Smoke and flames are original procedural canvas art (animated,
  flickering), drawn in the building's own painter pass so depth
  stays correct; shown for any building the local player can
  currently see. Farms and under-construction buildings do not burn.
- **Rubble** (client-only decal): when a building leaves the
  building list (destroyed), a rubble decal — one procedural heap
  image per footprint class (2×2, 3×3, 4×4) — is drawn at the site,
  fading out over the rubble duration (§10.5). The sim frees the
  footprint at destruction (verified: destroyed buildings leave
  `buildings()` and their cells unblock the same tick), so rubble
  is pure decoration over free ground; the Town Center (4×4)
  leaves rubble the same way.

### 10.4 Repair (sim rule)

See the rulebook (BLD section) for the normative text. Summary: a
villager ordered onto its own complete, damaged building walks to
it (ring cell, build range — the construction approach) and
restores HP at the documented flat rate — **750 HP/min for the
first repairer, +375 HP/min per additional repairer** — while the
owner pays a continuous wood drip proportional to HP restored,
totalling 50% of the building's build cost for a full 0→100%
repair (`REPAIR_COST_FRACTION`); if the owner cannot pay, the
repairer stops and idles. Undamaged, unfinished, enemy and Gaia
buildings reject the order silently. The AI does not repair in
this milestone (recorded gap).

### 10.5 Reference values (Phase 0 study, 2026-10-19)

From public documentation only (openage SLP spec, AoE2 Fandom,
AoEZone mechanics tests, the Age of Kings manual, the Ensemble
design document):

- **Directions.** Classic engine: 8 facings per unit = 5 stored
  (S, SW, W, NW, N) + 3 mirrored at runtime (SE, E, NE), with a
  separate graphic per action — DOCUMENTED (openage). AoE2 DE
  itself uses **16 facings** (COMMUNITY-CONSENSUS; the stored/
  mirrored split for DE's SMX is UNVERIFIED). Our LOW tier (8)
  matches the classic standard; 16 is our MEDIUM tier count and
  remains a re-render away for every model in §10.1.
- **Player color.** Classic: player-color pixels are stored as a
  **shade offset 0–15** inside the player's 16-color palette block
  (real index = offset + player × 16), remapped at draw —
  DOCUMENTED (openage SLP commands 0x06/0x0A; outlines 0x4E/0x5E).
  DE stores separate data/damage graphics in SMX instead
  (COMMUNITY-CONSENSUS). Our mask (§10.2) is the same mechanism at
  posterized fidelity: the mask gray IS the shade offset, ours has
  4 levels instead of 16. Which body areas carry the color is an
  art decision per asset (UNVERIFIED as a rule) — ours are listed
  in §10.2.
- **Repair.** First villager **750 HP/min**, each additional
  **+375 HP/min** (total 375·(n+1) HP/min) — DOCUMENTED (Fandom +
  AoEZone tests). Cost: **50% of build cost, proportional to HP
  restored**, deducted as the repair progresses; if the owner runs
  out, the villagers stop and idle — DOCUMENTED. Allied repair is
  allowed in DE with the owner paying (manual) — out of our scope
  (no alliances in 1v1). The Town Center is special-cased in DE
  (full repair 550 wood, 0 stone); our TC is an entity, not a
  placed building, and TC repair is out of this milestone's scope.
  Repairing unfinished buildings is not a DE concept — foundations
  are *constructed* (cost prepaid) — matching our rule that Repair
  rejects under-construction targets.
- **Fire.** Two documented stages only: **smoke at HP ≤ 75%**,
  **flames at HP ≤ 50%** (Age of Kings manual + Ensemble design
  doc — DOCUMENTED). A third flame phase exists in the reference
  art but its threshold is UNVERIFIED; our client scales flame
  cluster size continuously with damage inside the ≤50% stage as a
  presentation detail, not a claimed threshold. (DE's ≤20%
  garrison-eject has no counterpart — we have no garrison.)
- **Rubble.** Destroyed buildings are "reduced to rubble and
  eventually disappear" (manual — DOCUMENTED); the duration is
  UNVERIFIED in public sources. DE blocks building (but not
  movement) on the footprint during its collapse animation
  (COMMUNITY-CONSENSUS). Our sim has no collapse state — the
  footprint frees at destruction — so the client decal (§10.3) is
  pure presentation over free ground, with an **adopted fade of
  ~20 s (UNVERIFIED value, presentation only)**.

### 10.6 As built (2026-10-19)

Shipped in this milestone:

- **Models** (`tools/sprite-pipeline/models/`, rendered by
  `units.py`): villager (idle/walk/gather/carry/build), archer
  (idle/walk/shoot), militia + man-at-arms (idle/walk/attack),
  scout cavalry (idle/gallop, 160px cells), sheep/boar/deer
  (idle/walk/graze) — every sheet 8 directions × the §10.1 frame
  counts, contact sheets verified per unit (dir 1 = front,
  dir 5 = back). Player-color masks ship for every humanoid +
  scout state (villager/archer shirts and tunics `cloth`, militia
  tunic + shield face, MAA tabard + shield face, scout tunic +
  saddle blanket); animals carry no player color.
- **Client**: the directional sheets lead every unit/animal draw
  chain (facing from the movement vector quantized to the 8 rows,
  idle keeps the last facing); the mask-remapped red variant is
  built once per sheet at load and replaces the channel swap
  wherever a mask exists (villager included, at the LOW tier;
  MEDIUM/HIGH villager tiers still use the swap — their folders
  carry no masks yet). The side-view generated sets remain the
  fallback beneath. Buildings: smoke ≤75% HP, flames ≤50% with
  damage-scaled clusters, rubble decals ~20 s over the freed
  footprint, all fog-gated — BLD-18 / BLD-19 in the rulebook.
- **Repair**: sim command + client paths (right-click, root-panel
  button on E, hotkey E) per §10.4 — BLD-17. The AI does not
  repair (recorded gap).
- **Known limits**: the militia's wide armed stance touches the
  cell edges on two walk frames (nothing clipped; the villager's
  3px-margin standard is met by every other sheet); the sheep's
  wool body is a single ring of boxes by design (head proud of
  the ring, ears + muzzle read at game scale); the deer's slim
  antler boxes read as a crown in pure side views; the scout's
  neck root can show daylight in two side views (cosmetic).

## 11. QA checklist (per render)

1. Contact-sheet eyeball: silhouette reads at game zoom; facing is
   unambiguous in all 8 directions; walk cycle has a clear stride.
2. Automated: anchor-margin check on every cell; identical-pose
   frames must be pixel-identical (walk frames 0 and 4).
3. In-game: fallback chain intact (original -> placeholder -> vector),
   `cargo test` still 49/49 (renderer-only change, sim untouched).
