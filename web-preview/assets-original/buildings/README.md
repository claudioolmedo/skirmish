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

## Town Center — lock-plan iteration (2026-10-19)

`town-center.png` was replaced (same 512×494 canvas, same draw
contract — drop-in, no renderer change). The previous art (R8a, a
Base44 v8 generation) read as one closed building mass, which hides
the game's Town Center rule: the TC does NOT block — units pass
through its 4×4 area. The new art follows the DE "lock" plan
(Claudio's study note in `PROMPTS.md`): building mass only at the
PERIMETER — a two-storey hall at the back, gallery wings on the
sides, and a ring of open lean-to canopy roofs on timber posts —
around a clearly EMPTY central courtyard of bare earth, so a unit
crossing the center stays visible. Generated with the media image
pipeline from the winning prompt recorded in `PROMPTS.md`
("TOWN CENTER LOCK PLAN — winning prompt"), judged on the footprint
grid against three sibling candidates (judge sheet in the art
staging area, `tc-lock-judge.png`), chroma-keyed and exact-fit
affine-warped onto the canonical diamond. R8a remains in git
history and as `town-center-r8a-backup.png` in the staging area.

## Town Center — horseshoe iteration (2026-10-19, iteration 2)

`town-center.png` replaced again (same 512×494 canvas, same draw
contract — drop-in, no renderer change). Claudio's screenshot review
of the lock-ring version: it read **out of proportion, with too many
columns, no horseshoe front — and too small**. The corrected plan is
a **horseshoe (U) open at the FRONT**: a tall two-storey hall with a
steep slate hip roof and bell turret fills the back of the diamond,
solid gabled wings run down the two sides, each carrying an open
wooden canopy on exactly **two massive timber posts**, and the bare-
earth courtyard runs unbroken out through the open front — no roof
bar, no wall, nothing crossing the entrance — so the pass-through
reads from the camera and the compound fills its 4×4 diamond with
presence, towering over villagers and a mounted scout.

Process note: open-front wordings repeatedly drifted the generator
into frontal elevations (eight attempts). The winning route seeds
the image pipeline with a **flat-color geometric blockout rendered
in the game's own 2:1 dimetric math** (hall/wings/canopies/posts as
volumes on the diamond) and restyles it — geometry first, detail
second. The blockout script (`tc_blockout2.py`), the winning
candidate and the judge sheet (`tc-lock2-judge.png`: fitted diamond
+ grid, villager in the courtyard, villager entering through the
front gap, scout beside the TC) live in the art staging area; the
corrected winning prompt is recorded in `PROMPTS.md` ("TOWN CENTER
HORSESHOE — winning prompt"). The lock-ring version is backed up as
`town-center-lockring-backup.png` in the staging area.
