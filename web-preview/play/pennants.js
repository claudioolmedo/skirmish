// Waypoint pennants (INF-09, study-2 pass 2026-10-21) — the
// persistent flags planted at the points of a QUEUED order chain
// (Shift + right-click), the visibility half of study 2's gap 1:
// a DE player programs the scout's route and verifies it by the
// flags before looking away; ours executed the chain blind.
//
// DE evidence (Study 2, `~/workspace/skirmish-study/video2/`): a
// persistent pennant is planted at every queued waypoint and
// stays until the order is reached — grounded in frames/f_006.jpg
// (blue pennant at normalized (206, 631)), where the flag sits IN
// THE BLACK FOG at a point the scout has not reached: the flags
// are the player's own order echo, not world information, so they
// render after the fog dim, like the click marker (INF-09a). The
// official "Waypoint Markers" support article adds that, for
// queued group orders, only the GROUP LEADER's markers display
// (recorded under INF-09a) — so with several units selected,
// only the first selected unit's chain is drawn.
//
// The data is exact, not tracked: the sim exposes each entity's
// order route (`entity_order_route` — the current order's target
// while it belongs to a chain, then every queued order's target,
// object orders resolved to their target's position). This
// module is the pure mapping from those flat point lists to the
// pennants to draw + the pennant's shape, so the queue -> flags
// mapping is unit-testable in Node. A pennant exists exactly
// while its order is in the route the sim reports this frame —
// completion, replacement and death remove it by construction,
// nothing is cached here. Presentation only: no sim, command or
// hash impact.

// Pennant shape in world units (the renderer scales by camS,
// like the rally flag's proportions — a little taller, so a
// queued waypoint never reads as a rally point): a thin pole
// rising from the plant point, and a triangular pennant flying
// from its top, pointing +x in screen space.
export const PENNANT_POLE = 2.9; // plant point -> pole top
export const PENNANT_W = 1.35; // pole -> pennant tip
export const PENNANT_H = 0.9; // pennant height at the pole

// The pennant's outline relative to its plant point, in
// screen-oriented offsets (x right, y DOWN, world units):
// the pole segment plus the pennant triangle's three corners
// (top of pole, tip, lower corner). Pure function of the
// constants so tests can pin the shape.
export function pennantShape() {
  return {
    poleTop: [0, -PENNANT_POLE],
    tip: [PENNANT_W, -PENNANT_POLE + PENNANT_H / 2],
    low: [0, -PENNANT_POLE + PENNANT_H],
  };
}

// One flat ABI route ([x0, y0, x1, y1, ...]) -> the pennants to
// draw, in route order: [{x, y, index}]. A trailing unpaired
// value is dropped (defensive — the ABI always emits pairs).
export function routePennants(flat) {
  const out = [];
  if (!flat) return out;
  for (let i = 0; i + 1 < flat.length; i += 2) {
    out.push({ x: flat[i], y: flat[i + 1], index: out.length });
  }
  return out;
}

// The leader rule: of the selected units' routes (in selection
// order), the drawn one is the FIRST NON-EMPTY route — for a
// group order every member carries the same chain and the
// primary selected unit leads it; a selection whose earlier
// units hold no chain falls to the first unit that does. An
// all-empty selection draws nothing.
export function leaderRoute(routes) {
  if (!routes) return [];
  for (const r of routes) {
    if (r && r.length >= 2) return r;
  }
  return [];
}

// Selection (routes in selection order) -> pennants to draw.
export function pennantsForSelection(routes) {
  return routePennants(leaderRoute(routes));
}
