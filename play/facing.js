// Unit facing (MOV-09) — the client's ONE facing model.
//
// Rulebook MOV-09: "A unit has a facing (16-way); it attacks and
// is rendered in its facing; melee engagement turns the unit
// toward its target." DE renders each unit graphic at 16 angles
// (22.5° sectors — the graphic's declared angle count, per the
// DE modding documentation; SPRITE-PIPELINE §10.5): the unit's
// heading is continuous and the renderer presents the nearest
// of the 16 angles. The classic engine's 8 facings (5 stored +
// 3 mirrored) are the 1999 original, not DE. The sim's formation
// OCTANTS table stays 8 — that is formation slot layout
// geometry, a separate matter from sprite facing.
//
// Facing is presentation only: the WASM ABI exposes no unit
// state, no sim rule consumes facing, and nothing here is sim
// input — the sim and the golden hash never see it. The model is
// DERIVED, per the rulebook's Impl note: from the movement vector
// while a unit moves, from the work target while it works, from
// the shot while it shoots, and turned toward the melee target
// while it fights; an idle unit keeps its last facing (and its
// corpse inherits it). Before this module the derivation lived
// inline in four places in app.js with the melee turn missing;
// now every draw path quantizes through here.
//
// Convention (shared with the sprite pipeline, §10.1): dir 0
// faces +X world, increasing toward +Y, one sector per 22.5°.
// Row 2 (+X+Y) is the down-screen FRONT view, row 10 (−X−Y) the
// up-screen BACK view.

// The 16 facing unit vectors, index = dir. (cos/sin of
// dir × 22.5°.)
export const DIR16_VECTORS = Array.from({ length: 16 }, (_, d) => [
  Math.cos((d * Math.PI) / 8),
  Math.sin((d * Math.PI) / 8),
]);

// The facing for a world vector (dx, dy): the sector whose
// centre the vector's angle rounds to — the renderer's atan2
// quantization, now in one place at DE's 16 sectors. A zero
// vector has no facing: returns −1, and callers keep the unit's
// current facing (turnedDir16 does that for them).
export function dir16FromVector(dx, dy) {
  if (dx === 0 && dy === 0) return -1;
  const d = Math.round(Math.atan2(dy, dx) / (Math.PI / 8));
  return ((d % 16) + 16) % 16;
}

// The facing after turning toward (dx, dy) from where the unit
// stands: the new facing, or `current` when the vector is zero
// (already on top of the target — no turn is meaningful).
export function turnedDir16(current, dx, dy) {
  const d = dir16FromVector(dx, dy);
  return d >= 0 ? d : current;
}

// The side-view flip sign for a facing: +1 when the facing
// points right on screen, −1 left, 0 when it points straight
// up/down screen (rows 2 and 10 — the caller keeps its current
// flip there, as the pose code always has). Screen-x in the
// fixed iso projection follows the sign of (dx − dy) of the
// facing vector — the same projection the pose code derives
// movement flips from.
export function faceForDir16(dir) {
  const v = DIR16_VECTORS[((dir % 16) + 16) % 16];
  const s = v[0] - v[1];
  return s > 0.001 ? 1 : s < -0.001 ? -1 : 0;
}

// The row of a 16-way facing in a sprite sheet with `rows`
// direction rows: the facing mapped onto the stored rows
// (identity at 16; the villager quality tiers store 8/16/32
// rows, so a facing lands on the row the pipeline rendered for
// it — floor(dir × rows / 16), the mapping drawDirUnit has
// always applied: pairs at 8 rows, exact at 16, doubled at 32).
export function sheetRowForDir16(dir, rows) {
  const r = Math.max(1, rows | 0);
  const d = ((dir % 16) + 16) % 16;
  return ((Math.floor((d * r) / 16) % r) + r) % r;
}
