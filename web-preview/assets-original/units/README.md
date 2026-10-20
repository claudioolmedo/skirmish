# Unit Animation Art — Generated Action Frames

These unit sprites are **Skirmish II's own original art**, generated
with AI image generation on **2026-10-19** (Muse media pipeline, from
this project's standardized prompts; raws and processing live in
`~/workspace/skirmish-art/` — `raw/unit-anim/` and
`processed/unit-anim/`, sliced/keyed by `process_unit.py`).
Clean-room: original designs after real medieval dress and arms,
never copies of any other game's sprites or art.

Each unit has **one generated strip per action** — the same
character repeated in a row, one pose per frame — sliced into
frames that share one bottom-aligned canvas per unit:

| Unit | Actions (frames) |
|---|---|
| `villager/` | walk ×4, gather (axe) ×3, carry (log bundle) ×2, build (mallet) ×2, idle ×1 |
| `archer/` | walk ×4, shoot (aim → full draw → release) ×3, idle ×1 |
| `militia/` | walk ×4, attack (sword swing) ×3, idle ×1 |
| `maa/` (Man-at-Arms) | walk ×4, attack (sword swing) ×3, idle ×1 |
| `scout/` | gallop ×4, idle ×1 |

**Direction model — read this before extending:** these are
SIDE-VIEW cycles. Every pose faces **left**; the renderer draws a
frame unflipped when the unit moves left on screen and flipped
horizontally when it moves right — the same model as the animal
pose sets. They are **not** 8-direction sheets: a unit walking
straight up or down the screen is shown in profile, exactly like
the animals. Full directional sheets remain sprite-pipeline work.

**Player color:** every human unit's cloth is the one player blue.
The client builds the enemy's red variant at load by palette swap
(`makeRedSheet`, the same swap as the procedural villager sheets),
so only the blue set ships here.

**State → action** is chosen client-side from observable state
only (see `unitArtImage` in `app.js`): position delta for
walk/gallop, the carry heuristic for carry, node proximity for
gather, own-unfinished-building proximity for build, fresh
projectile spawns for the archer's shoot cycle, and enemy adjacency
for melee attack. Any state without art falls back to the previous
drawing (procedural sheets / vector) — the art never blanks a unit.

Chroma-keyed (flat magenta background in the raw generations,
keyed out in processing), RGBA.
