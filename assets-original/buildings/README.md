# Building Art — Base44 Test Set (batch 1)

These building sprites are **Skirmish II's own original art**, generated
with AI image generation on **Claudio Olmedo's Base44 account on
2026-10-18** (app "Skirmish II Art"), from standardized prompts written
for this project. The prompt standard (style block + per-building
subjects) is archived in the transfer repo
`github.com/claudioolmedo/base44-transfer` (`PROMPTS.md`) so the same
prompts can drive any generator, not just Base44.

Clean-room, like everything else in this project: the prompts ask for
original designs inspired by **real medieval architecture** (French
fortresses and timber-frame villages — Pierrefonds, Carcassonne),
never copies of any other game's sprites or art.

## Files

One PNG per building kind, chroma-keyed (flat magenta background in
the raw generations, keyed out in processing), trimmed to content,
RGBA. The copies here are downscaled to max 512 px wide for the web;
full-resolution processed originals live outside the repo in the art
staging area (`~/workspace/skirmish-art/processed/`).

| File | Building |
|---|---|
| `town-center.png` | Town Center |
| `house.png` | House (finished) |
| `house-stage1.png` | House under construction — stage 1: foundation + posts |
| `house-stage2.png` | House under construction — stage 2: stone walls + frame + scaffolding |
| `house-stage3.png` | House under construction — stage 3: walls complete, roof beams, scaffolding |
| `barracks.png` | Barracks |
| `lumber-camp.png` | Lumber Camp |
| `mill.png` | Mill |
| `mining-camp.png` | Mining Camp |
| `archery-range.png` | Archery Range |
| `stable.png` | Stable |
| `blacksmith.png` | Blacksmith |
| `farm.png` | Farm (plot, drawn by the node renderer) |

Status: **test set** — wired into the renderer (M13 + art test) so the
look can be judged in a real match before more batches are generated.
The same image serves both players for now (no per-owner tint yet).

## House construction stages

`house-stage1..3.png` and the finished `house.png` are **one strip
generation**: a single four-frame construction sequence generated on
Claudio's Base44 account on 2026-10-18 (same app, same prompt
standard) and sliced into frames that share one 237×273 canvas with
bottom-aligned ground lines. The finished House art therefore now
comes from the same strip (it replaced the batch-1 `house.png`), so
a House under construction grows stage by stage — foundation +
posts → stone walls + frame + scaffolding → walls complete, roof
beams, scaffolding → finished house — with no design jump at
completion. The renderer picks the frame by build progress
(thirds); if a stage frame fails to load it falls back to
clip-revealing the finished art over the foundation diamond.
