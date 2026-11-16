// Unit facing (MOV-09) — the client's ONE facing model.
//
// Rulebook MOV-09: "A unit has a facing (8-way); it attacks and
// is rendered in its facing; melee engagement turns the unit
// toward its target." Source: OPENAGE-RESEARCH §2.4 (the classic
// engine's units carry one of 8 facings; the 5-stored + 3-mirrored
// sprite split is a storage fact — SPRITE-PIPELINE §12 — not the
// facing model, and neither are our 16/32-row quality tiers).
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
// faces +X world, increasing toward +Y, one sector per 45°.
// Row 1 (+X+Y) is the down-screen FRONT view, row 5 (−X−Y) the
// up-screen BACK view.

// The 8 facing unit vectors, index = dir. (cos/sin of dir × 45°;
// components are 0, ±1 or ±√½.)
export const DIR8_VECTORS = Array.from({ length: 8 }, (_, d) => [
  Math.cos((d * Math.PI) / 4),
  Math.sin((d * Math.PI) / 4),
]);

// The facing for a world vector (dx, dy): the sector whose
// centre the vector's angle rounds to — exactly the renderer's
// long-standing atan2 quantization, now in one place. A zero
// vector has no facing: returns −1, and callers keep the unit's
// current facing (turnedDir8 does that for them).
export function dir8FromVector(dx, dy) {
  if (dx === 0 && dy === 0) return -1;
  const d = Math.round(Math.atan2(dy, dx) / (Math.PI / 4));
  return ((d % 8) + 8) % 8;
}

// The facing after turning toward (dx, dy) from where the unit
// stands: the new facing, or `current` when the vector is zero
// (already on top of the target — no turn is meaningful).
export function turnedDir8(current, dx, dy) {
  const d = dir8FromVector(dx, dy);
  return d >= 0 ? d : current;
}

// The side-view flip sign for a facing: +1 when the facing
// points right on screen, −1 left, 0 when it points straight
// up/down screen (rows 1 and 5 — the caller keeps its current
// flip there, as the pose code always has). Screen-x in the
// fixed iso projection follows the sign of (dx − dy) of the
// facing vector — the same projection the pose code derives
// movement flips from.
export function faceForDir8(dir) {
  const v = DIR8_VECTORS[((dir % 8) + 8) % 8];
  const s = v[0] - v[1];
  return s > 0.001 ? 1 : s < -0.001 ? -1 : 0;
}

// The row of an 8-way facing in a sprite sheet with `rows`
// direction rows: the facing mapped onto the stored rows
// (identity at the standard 8; the villager quality tiers'
// 16/32 rows are storage resolution, so a facing lands on the
// row the pipeline rendered for it — floor(dir × rows / 8),
// the mapping drawDirUnit has always applied).
export function sheetRowForDir8(dir, rows) {
  const r = Math.max(1, rows | 0);
  const d = ((dir % 8) + 8) % 8;
  return ((Math.floor((d * r) / 8) % r) + r) % r;
}
