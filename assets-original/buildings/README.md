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
| `barracks.png` | Barracks (finished) |
| `barracks-stage1..3.png` | Barracks construction stages 1–3 |
| `lumber-camp.png` | Lumber Camp (finished) |
| `lumber-camp-stage1..3.png` | Lumber Camp construction stages 1–3 |
| `mill.png` | Mill (finished) |
| `mill-stage1..3.png` | Mill construction stages 1–3 |
| `mining-camp.png` | Mining Camp (finished) |
| `mining-camp-stage1..3.png` | Mining Camp construction stages 1–3 |
| `archery-range.png` | Archery Range (finished) |
| `archery-range-stage1..3.png` | Archery Range construction stages 1–3 |
| `stable.png` | Stable (finished) |
| `stable-stage1..3.png` | Stable construction stages 1–3 |
| `blacksmith.png` | Blacksmith (finished) |
| `blacksmith-stage1..3.png` | Blacksmith construction stages 1–3 |
| `farm.png` | Farm (plot, drawn by the node renderer) — the ripe state |
| `farm-state1..4.png` | Farm growth states: 1 planted, 2 shoots, 3 green, 4 ripe |

Status: **test set** — wired into the renderer (M13 + art test) so the
look can be judged in a real match before more batches are generated.
The same image serves both players for now (no per-owner tint yet).

## Construction stages (every building)

For every building kind, `<kind>-stage1..3.png` and the finished
`<kind>.png` are **one strip generation**: a single four-frame
construction sequence generated on Claudio's Base44 account on
2026-10-18 (same app, same prompt standard) and sliced into frames
that share one canvas per set with bottom-aligned ground lines. The
finished art of each kind therefore comes from its own strip (it
replaced the batch-1 single images), so a building under
construction grows stage by stage — foundation + posts → walls +
scaffolding → structure complete, roof beams → finished building —
with no design jump at completion. The renderer picks the frame by
build progress (thirds), drawn on the finished art's draw contract;
if a stage frame fails to load it falls back to clip-revealing the
finished art over the foundation diamond. (The House set was the
first, generated as a 237×273-canvas strip; the other kinds followed
the same pattern.)

## Farm growth states

`farm-state1..4.png` are one four-frame strip of the same field at
four growth states (planted → shoots → green → ripe), sliced the
same way. `farm.png` is the ripe frame. While a completed farm is
being gathered, the renderer picks the state by the food fraction
remaining (ripe → green → shoots → planted as it depletes), on the
same fitted-to-the-diamond draw contract the single farm image used.
