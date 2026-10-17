# Sprite pipeline — tools

Generates Skirmish II's original sprite sheets from procedural low-poly
models. The full method spec lives in `SPRITE-PIPELINE.md` in the goal
files (camera, tiers, animation states, anchor and player-color
conventions).

## Usage

```bash
python3 tools/sprite-pipeline/render.py                 # default: player blue, into web-preview/assets-original/
python3 tools/sprite-pipeline/render.py --only villager # villager sheets only
python3 tools/sprite-pipeline/render.py --only house    # house stages only
python3 tools/sprite-pipeline/render.py --player-color c0392b
python3 tools/sprite-pipeline/render.py --out /tmp/sprites
```

Requires Python 3 with numpy and Pillow. One run regenerates every
sheet (villager: idle / walk / gather / carry, LOW tier = 8 directions;
house: stage0 / stage1 / stage2 / complete) in a few seconds.

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
