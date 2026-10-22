// Occluded-unit outlines (INF-12) — when a unit stands behind a
// taller object drawn in front of it (a building, the Town Center,
// a tree), the game shows the unit's silhouette in its owner's
// colour THROUGH the occluder, so a hidden unit is never simply
// gone. Claudio's live-play report (2026-10-22): the enemy scout
// parks behind his Town Center and sits there invisible.
//
// DE evidence (researched 2026-10-22, recorded in rulebook
// INF-12):
// - AoE wiki, "Player" (§ Colors): "If the unit is obscured or
//   behind something, a player-colored outline of the unit will
//   be visible"; animals included — their owner's colour "can
//   still be seen through its outline (if behind a building)".
//   The same page's gallery: "Outlines of units behind a Stone
//   Wall in Age of Empires II: Definitive Edition."
// - The original Age of Kings manual's improved-interface list:
//   "Units behind buildings and trees are visible" — trees are
//   occluders too, not just buildings.
// - Ensemble's AoE2 design document ("Unit Visibility"): the
//   outline shows through the obscuring object when a unit is
//   "partially or completely hidden by a taller object in the
//   foreground", through up to two levels of obscuring objects;
//   "All player controlled units will be visible, including
//   owned units, allied units, and enemy units" — the ENEMY
//   scout behind our TC is exactly the covered case.
// - DE forum report (forums.ageofempires.com, "Unit borders
//   can't be seen behind palisade walls", moved to the DE
//   forum): players expect "a blue silhouette (or whatever
//   colour you are)" behind buildings; the thread lists the
//   buildings where DE shows it (Town Center, House, Mill,
//   Barracks, Stable, Archery Range, ...) and reports the ones
//   where it is missing (Palisade walls/gates, Lumber Camp,
//   Mining Camp, Dock, Trebuchet) AS BUGS — the intended rule
//   is every building occludes with an outline.
// - Setting: AoE2 DE has none for this — it is always on (the
//   adjustable "Obscured Unit Alpha" belongs to AoE III /
//   AoE IV). Ours is likewise always on, at every quality
//   level: it is information, not cosmetics (the quality
//   governor's doctrine — unit visibility is never shed).
//
// This module is the pure decision half: rectangle geometry in
// device pixels (the caller supplies each sprite's alpha-tight
// screen rect + painter depth + fog legitimacy). The canvas
// half (tinting the sprite's own alpha, clipping it to the
// occluders' alpha) lives in app.js. Presentation only: no sim,
// command or hash contact.
//
// THE FAIRNESS GATE is here, not in the caller: a unit whose
// cell is not in the local seat's CURRENT fog visibility
// (fogVisible !== true) can never be outlined, however much it
// overlaps — the outline reveals pixel occlusion only, never
// fog. Explored-memory (fog 1) is not visibility.

// The fraction of the unit's alpha-tight rect that front
// occluders must cover before the outline shows. DE shows the
// outline for a unit "partially or completely hidden" (design
// document); because the painted silhouette is afterwards
// clipped to the occluders' actual alpha, a small covered
// fraction still paints only the truly-hidden sliver — the
// threshold exists to skip negligible grazes (a shoulder behind
// a tree trunk's rect edge), not to gate real hiding.
export const OCCLUDE_MIN_FRACTION = 0.15;

// Rects are {x, y, w, h} (top-left origin, device px).

export function rectsIntersect(a, b) {
  if (!a || !b) return false;
  return a.x < b.x + b.w && b.x < a.x + a.w
    && a.y < b.y + b.h && b.y < a.y + a.h;
}

// The intersection rect, or null when disjoint / degenerate.
export function intersectionRect(a, b) {
  if (!rectsIntersect(a, b)) return null;
  const x = Math.max(a.x, b.x);
  const y = Math.max(a.y, b.y);
  const w = Math.min(a.x + a.w, b.x + b.w) - x;
  const h = Math.min(a.y + a.h, b.y + b.h) - y;
  if (w <= 0 || h <= 0) return null;
  return { x, y, w, h };
}

// The exact fraction of `rect` covered by the UNION of
// `coverRects` (overlaps between cover rects are counted once).
// Slab decomposition: clip every cover rect to `rect`, sweep
// the vertical slabs between consecutive x-edges, and merge
// the y-intervals inside each slab.
export function coveredFraction(rect, coverRects) {
  if (!rect || rect.w <= 0 || rect.h <= 0) return 0;
  const clipped = [];
  for (const c of coverRects || []) {
    const i = intersectionRect(rect, c);
    if (i) clipped.push(i);
  }
  if (!clipped.length) return 0;
  const xs = new Set([rect.x, rect.x + rect.w]);
  for (const c of clipped) {
    xs.add(c.x);
    xs.add(c.x + c.w);
  }
  const edges = [...xs].sort((p, q) => p - q);
  let covered = 0;
  for (let s = 0; s + 1 < edges.length; s++) {
    const x0 = edges[s];
    const x1 = edges[s + 1];
    if (x1 <= x0) continue;
    const spans = [];
    for (const c of clipped) {
      if (c.x <= x0 && c.x + c.w >= x1) spans.push([c.y, c.y + c.h]);
    }
    if (!spans.length) continue;
    spans.sort((p, q) => p[0] - q[0]);
    let merged = 0;
    let cur0 = spans[0][0];
    let cur1 = spans[0][1];
    for (let k = 1; k < spans.length; k++) {
      if (spans[k][0] <= cur1) {
        if (spans[k][1] > cur1) cur1 = spans[k][1];
      } else {
        merged += cur1 - cur0;
        cur0 = spans[k][0];
        cur1 = spans[k][1];
      }
    }
    merged += cur1 - cur0;
    covered += (x1 - x0) * merged;
  }
  const f = covered / (rect.w * rect.h);
  return f > 1 ? 1 : f;
}

// The occlusion decision for one unit.
//   unit:      {rect, depth, fogVisible}
//   occluders: [{rect, depth, ...}] (extra fields pass through
//              into the returned cover list, so the caller can
//              paint from exactly the occluders that counted)
// An occluder counts only when drawn IN FRONT of the unit
// (strictly greater painter depth — the renderer's sort key:
// same key means same ground line, not in front) and its rect
// intersects the unit's. Returns {occluded, fraction, cover}.
export function occlusionFor(unit, occluders) {
  const none = { occluded: false, fraction: 0, cover: [] };
  if (!unit || !unit.rect || unit.rect.w <= 0 || unit.rect.h <= 0) {
    return none;
  }
  // The hard fairness gate: no current fog visibility, no
  // outline — regardless of geometry.
  if (unit.fogVisible !== true) return none;
  const cover = [];
  for (const o of occluders || []) {
    if (!o || !o.rect) continue;
    if (!(o.depth > unit.depth)) continue;
    if (rectsIntersect(unit.rect, o.rect)) cover.push(o);
  }
  if (!cover.length) return none;
  const fraction = coveredFraction(unit.rect, cover.map((o) => o.rect));
  return {
    occluded: fraction >= OCCLUDE_MIN_FRACTION,
    fraction,
    cover,
  };
}
