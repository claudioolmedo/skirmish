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

1. **Villager** — DONE (v1: idle/walk/gather/carry, LOW tier).
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

## 9. QA checklist (per render)

1. Contact-sheet eyeball: silhouette reads at game zoom; facing is
   unambiguous in all 8 directions; walk cycle has a clear stride.
2. Automated: anchor-margin check on every cell; identical-pose
   frames must be pixel-identical (walk frames 0 and 4).
3. In-game: fallback chain intact (original -> placeholder -> vector),
   `cargo test` still 49/49 (renderer-only change, sim untouched).
