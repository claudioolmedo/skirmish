# Sprite pipeline — tools

Generates Skirmish II's original sprite sheets from procedural low-poly
models. The full method spec lives in `SPRITE-PIPELINE.md` in the goal
files (camera, tiers, animation states, anchor and player-color
conventions).

## Usage

```bash
python3 tools/sprite-pipeline/render.py                 # default: player blue, into web-preview/assets-original/
python3 tools/sprite-pipeline/render.py --tiers         # + MEDIUM/HIGH tiers (tier16/, tier32/)
python3 tools/sprite-pipeline/render.py --only villager # villager sheets only
python3 tools/sprite-pipeline/render.py --only house    # house stages only
python3 tools/sprite-pipeline/render.py --player-color c0392b
python3 tools/sprite-pipeline/render.py --out /tmp/sprites
```

Requires Python 3 with numpy and Pillow. One run regenerates every
sheet (villager: idle / walk / gather / carry, LOW tier = 8 directions;
house: stage0 / stage1 / stage2 / complete) in a few seconds.

`--tiers` additionally renders the villager sheets at 16 and 32
directions into `assets-original/tier16/` and `tier32/`, saving those
sheets palette-optimized (256-color libimagequant — the posterized art
makes it visually lossless at ~half the bytes), and copies the house
stages into both folders (the house has one fixed orientation, so its
art is identical at every tier). The client's adaptive-quality governor
loads exactly one tier — see `web-preview/play/quality.js` and
ARCHITECTURE.md ("Adaptive quality governor").

## Files

- `render.py` — model definitions (villager v1: tapered-box humanoid,
  joint-grouped; house v1: timber-frame cottage with per-stage
  construction variants), pose functions per animation state, and the
  software renderer (orthographic 2:1 dimetric projection, z-buffered
  flat shading, 2x supersampling).

Sheet constants (villager cell 128x128, feet anchor at (64, 110);
house canvas 896x704, footprint-center anchor at (448, 448); both at
46 px per world unit) are mirrored in `web-preview/app.js`
(`VIL_ORIGINAL`, `HOUSE_ORIGINAL`) — change them in both places
together.
