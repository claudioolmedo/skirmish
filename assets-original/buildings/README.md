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
| `palisade-wall.png` | Palisade Wall segment (finished) — procedural, see below |
| `palisade-wall-stage1..3.png` | Palisade Wall construction stages 1–3 |

The palisade set is **not** from the Base44 test set: it is
procedurally drawn by our own script
(`~/workspace/skirmish-art/palisade_gen.py`, PIL — a row of
sharpened oak stakes on two rails, on the canonical contract
canvas, stages at 35/65/90% stake height), in the same palette
as the renderer's vector fallback so the two read as one
object. Panel icon: `play/ui-art/icons/palisade-wall.png`
(128×128, same generator).

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

## Town Center — horseshoe HEIGHT iteration (2026-10-19, iteration 3)

`town-center.png` replaced again (same 512×494 canvas, same draw
contract — drop-in, no renderer change). Claudio's review of the
iteration-2 judge sheet: the villager still read **too big against
the building** — "make it TALLER so the villager sits at the right
height; the problem is the height; taller still fits". The fix is
vertical only: same horseshoe plan, same 4×4 base on the canonical
diamond, the building grows UP the canvas. The blockout was rebuilt
with raised storeys (`tc_blockout4.py`: hall eaves 1.95 z, hip apex
2.95, turret apex 3.15, canopies raised for headroom, and the hall
door drawn as a stone **arch gateway** — the generator paints
furniture-scale doors ~0.3–0.6× a villager no matter the slab, but
an arch is architecture and holds its size). Measured on the final
fitted art against the 73px game-scale villager (winner R, judged
on `tc-lock3-judge.png` with a villager at the door, a villager
under the canopy, a villager entering the open front, and a mounted
scout beside the TC): **door (gateway) 1.29×, canopy clearance
1.39×, hall eaves 1.86×, hall ridge 2.40×, bell turret 3.06×** —
against iteration 2's door 0.31× / ridge 1.93× / turret 2.23×, the
villager now sits at the right height at every feature. The
iteration-2 horseshoe is backed up as
`town-center-horseshoe-v1-backup.png` in the staging area; the
winning prompt and full measurement table are recorded in
`PROMPTS.md` ("TOWN CENTER HORSESHOE — HEIGHT").

## Town Center — ELEVATION iteration (2026-10-19, iteration 4)

`town-center.png` replaced again (same 512×494 canvas, same draw
contract — drop-in, no renderer change). Claudio's verdict on
iteration 3 was "ficou ótimo" (it turned out great) — with one
structural correction: "para manter a ilusão que está caminhando
debaixo, deveria ter outro andar e não ter nada embaixo" (to keep
the illusion that the villager is walking UNDERNEATH, the building
should have another storey and nothing underneath). His reference
photo (a raised-platform building on columns only, study copy in
the staging area): the whole compound stands on COLUMNS ONLY — a
raised floor deck, ground level completely empty — and the
crossing is proven by a figure passing BEHIND the front columns
under the raised floor, occluded by the posts, visible in the
gaps.

The change is structural only — same horseshoe plan, same height
bands: the enclosed hall now starts at FIRST-FLOOR level on a
timber beam + floor-slab band carried by massive posts (four
corners + one center post; no mid posts on the courtyard faces, so
each face is one wide opening); underneath the hall — and across
the whole 4×4 — is open undercroft: posts and bare earth, nothing
enclosed at ground level anywhere. The stone arch moved up with
the floor and is now an upper-level LOGGIA (railed balcony arch,
no door to the ground, no stairs). The side wings are open sheds
on posts (Claudio: "dos lados está certo" — the sides were right),
no walls, no ground-level floors.

Measured on the final fitted art against the 73px game-scale
villager (winner X, judged on `tc-lock4-judge.png` — panel (a) is
the proof composite: a villager under the hall, drawn behind the
front posts, occluded by them and visible in the gap):
**undercroft clearance 1.31×** (inside the 1.25–1.45 band),
**hall ridge 2.98×** and **bell turret 3.27×** — both over their
round-3 bands (2.4–2.7 / 2.8–3.1) on the TALL side, the direction
of every height correction Claudio has given; disclosed here and
in `PROMPTS.md`, where the winning prompt is recorded ("TOWN
CENTER HORSESHOE — ELEVATION"). Rejected siblings: W (same
blockout — the generator crushed its undercroft to ~1.0×, so the
villager's head hits the deck) and Y (undercroft tall but the
turret blew up to 4.03×). Iteration 3's winner R is backed up as
`town-center-horseshoe-v2-backup.png` in the staging area.

## Town Center — EMPTY GROUND + UPPER-FLOOR WALLS iteration (2026-10-19, iteration 5)

`town-center.png` replaced again (same 512×494 canvas, same draw
contract — drop-in, no renderer change). Claudio's refinement of
iteration 4 (22:45): "O primeiro está certo, mas não tem que ter
nada no chão e também paredes no primeiro andar; se vc colocar
aldeão atrás, vai dar a sensação que está atravessando" — the
elevated first floor is right, but (1) the ground plane must be
literally EMPTY apart from the columns, and (2) the enclosed
walls belong on the upper floor. Iteration 4's winner had shipped
with a cart, crates, barrels and hay standing under the sheds,
and its hall face was mostly an open loggia — an open deck, not
a room.

The change (blockout6 extends blockout5; geometry encodes it):
the ground across the whole 4×4 is bare earth + columns ONLY —
no props, no stored goods, no ground deck, no stairs or ladder,
nothing occupying the ground plane; diagonal braces stay HIGH,
tucked directly under the raised floor band. The hall above the
slab is now a CLOSED ROOM VOLUME: solid timber-framed plaster
walls on every face with small arched windows set into the wall
— no loggia, no balcony, no railing; the walls start at the
raised deck and stop at the eaves, and nothing wall-like reaches
the ground. Same horseshoe plan, same massive posts (four
corners + one center post under the hall, exactly two hero posts
per shed), same height structure.

Measured on the final fitted art against the 73px game-scale
villager (winner Z1, judged on `tc-lock5-judge.png` — panel (a)
is the proof composite: a villager under the elevated floor,
drawn behind the front posts, occluded by them and visible in
the gap, standing on completely bare ground; a second villager
stands fully visible in the empty undercroft under the left
shed): **undercroft clearance 1.45×** (top of the 1.25–1.45
band), **hall ridge 2.83×** (over the 2.4–2.7 band on the tall
side, less than iteration 4's shipped 2.98×), **turret 2.78×**
(at the floor of the 2.8–3.1 band). Ground-emptiness verified on
the fitted art: zero non-column elements touch or occupy the
ground plane. Rejected siblings: Z2 (same numbers except turret
3.40× — over the cap), Z4 (ridge 2.40× / turret 2.88× in band
but its undercroft posts measure 1.16× — below the 1.25 floor,
the core rule, rejected like W in round 4), Z3 (the generator
floored both sheds on raised plank platforms — the
whole-compound deck failure of iteration 4's V — rejected on
structure before fitting). The winning prompt is recorded in
`PROMPTS.md` ("TOWN CENTER HORSESHOE — EMPTY GROUND + UPPER
WALLS"). Iteration 4's winner X is backed up as
`town-center-horseshoe-v3-backup.png` in the staging area.

Iteration 6 (2026-10-19, Claudio's verdict on iteration 5:
"Ótimo" — with two literal corrections: "o chão é transparente"
and "o primeiro andar também tem que ter a mesma proporção para
alguém estar nele"): **the ground is now TRANSPARENT** — the
sprite paints no ground whatsoever: inside the 4×4 footprint
every pixel is either structure or alpha (no dirt diamond, no
apron, no painted contact shadow; the posts simply end at the
ground line and the game's terrain renders beneath) — and
**the upper floor is person-scaled**: its wall band (top of the
elevated floor slab to the eaves) must itself measure
≈1.20–1.35× the villager, so a person plausibly stands inside
that floor. Winner BB4: geometry-first blockout
(`tc_blockout8.py` — same horseshoe plan and posts; the
blockout's ground is flat magenta with a guide outline only,
and its wall band is deliberately over-drawn at 1.52 z because
the generator compresses the band to ~0.7–0.85 of the drawn
ratio, the same failure family as the flattened roof pitches),
the iteration-5 restyle prompt plus a TRANSPARENT GROUND
paragraph and TALLEST-ELEMENT wall wording, fitted with a
post-base landmark affine (there is no dirt diamond left to
ground on: the leftmost/rightmost post bases and one hero post
base are the landmarks) and a final ×1.10 vertical proportion
stretch about the equator. Measured on the final fitted art
against the 73px game-scale villager (judged on
`tc-lock6-judge.png`, composited over the game's grass):
**wall band 1.23–1.32×** (90px on the left face, 96px at the
front corner — inside the 1.20–1.35 band; judge panel (c) is
the proof: a villager standing ON the elevated deck beside the
wall, head below the eaves with headroom), **undercroft
≈1.23×** (a hair under the 1.25 floor of its band),
**ridge 3.33×** and **turret 3.70×** — both over their old
bands, disclosed: undercroft + slab + a person-scaled wall +
roof cannot also keep the old roof numbers, and Claudio's
proportion rules win. **Alpha audit** of the fitted sprite
(canonical diamond = 52,441 px): 64.8% transparent, 35.2%
structure (posts, deck, walls, roofs), **ground-fill pixels:
ZERO** — verified by masking the fitted art against the warped
blockout's structure: the generator painted 19 stray px
outside the structure mask and the pipeline removed them; any
painted earth or ground shadow is removed the same way.
Rejected: AA2 (sheds floored on raised plank platforms), BB3
(windows became open arches with a visible interior floor —
the loggia failure — and its turret a brick chimney), BB2
(cloths hanging into the sheds), BB1 (wall band 0.91×,
door-like shutter windows, invented chimney), AA1/AA3/AA4
(wall bands 0.98×/0.86×/0.97× — batch 1, before the over-draw).
Iteration 5's winner Z1 is backed up as
`town-center-horseshoe-v4-backup.png` in the staging area.

## Castle (Castle P3a, 2026-10-29)

| File | Building |
|---|---|
| `castle.png` | Castle (finished) — **original programmatic blockout** drawn in code for this package (flat-shaded stone keep, twin towers, battlements, gate; palette matched to the client's vector fallback), not an AI generation. Placeholder-grade art in the BLD-18 complete-art-only contract: no stage frames; construction clip-reveals the finished art. A generated art pass can replace it file-for-file later. |
