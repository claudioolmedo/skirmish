// Click marker (INF-09, forest/click-feedback pass 2026-10-20) —
// the brief order-confirmation animation at the exact point a
// ground order was issued.
//
// DE evidence (Study 1 footage, frame-analysed 2026-10-20;
// rulebook INF-09): a right-click on GROUND plays four red
// arrowheads that start apart at the corners of a small square
// around the clicked point, converge inward until they meet as
// an X, then collapse and vanish — ~0.45 s end to end, red
// (#e33 family). A right-click on an OBJECT (unit, building,
// resource node — e.g. a tree to chop) shows NO ground marker.
// Queued (Shift) waypoints additionally plant persistent flags
// (the manual's "a waypoint marker appears at each point");
// those persistent flags are a separate, still-open half of
// INF-09 — this module is only the transient confirmation.
//
// This module is pure logic + a tiny state list, so the
// order-type -> marker mapping is unit-testable in Node. All
// state is client-side presentation: nothing here touches the
// sim, commands, or determinism.

// Full duration of the converge-and-fade, from the DE frame
// measurement (~13-14 frames at 29.97 fps).
export const MARKER_MS = 450;
// LOW quality tier: shorter, never removed — order feedback is
// information, not decoration.
export const MARKER_MS_LOW = 300;
// Cap concurrent markers: a spam-clicking player gets feedback
// for their latest orders, not an unbounded draw list.
export const MARKER_CAP = 24;

// The order kinds the input layer can report. 'ground' is any
// order whose target is a point on the ground (plain move from
// the main view or the minimap, a queued ground move, a rally
// point set). 'object' is any order whose target is a thing
// (gather node, attack target, repair, drop-off, build
// placement — placement has its own ghost feedback).
export function markerForOrder(kind) {
  return kind === 'ground' ? 'ground' : null;
}

export class ClickMarkers {
  constructor() {
    this.list = []; // {x, y, t0} in world coords + ms clock
  }

  // Add a marker for a resolved order. `kind` goes through
  // markerForOrder, so object orders are a no-op by construction.
  // Returns the marker kind actually spawned (or null).
  add(kind, x, y, now) {
    const m = markerForOrder(kind);
    if (!m) return null;
    this.list.push({ x, y, t0: now });
    if (this.list.length > MARKER_CAP) {
      this.list.splice(0, this.list.length - MARKER_CAP);
    }
    return m;
  }

  // Live markers at `now`, oldest first: {x, y, t} with t in
  // 0..1 over the marker's life. Expired markers are pruned.
  active(now, lowTier = false) {
    const life = lowTier ? MARKER_MS_LOW : MARKER_MS;
    this.list = this.list.filter((m) => now - m.t0 < life);
    return this.list.map((m) => ({
      x: m.x, y: m.y, t: (now - m.t0) / life,
    }));
  }

  clear() {
    this.list.length = 0;
  }
}

// Geometry of one arrowhead at progress t: the four heads sit on
// the diagonals of a square around the point. They start at
// radius R0 (in world units), converge to R1 as t goes 0 -> 0.7
// (ease-out), then hold while the whole marker fades in the last
// 30%. Pure function of t so tests can pin the shape.
export const MARKER_R0 = 1.55; // world units, heads start here
export const MARKER_R1 = 0.34; // world units, heads meet here
export function markerHeads(t) {
  const ct = Math.min(1, t / 0.7);
  const ease = 1 - (1 - ct) * (1 - ct);
  const r = MARKER_R0 + (MARKER_R1 - MARKER_R0) * ease;
  const alpha = t < 0.7 ? 1 : 1 - (t - 0.7) / 0.3;
  const heads = [];
  for (let k = 0; k < 4; k++) {
    // Diagonals in world space (NE, SE, SW, NW of the point);
    // angle points from the head toward the centre.
    const a = Math.PI / 4 + k * (Math.PI / 2);
    heads.push({
      dx: Math.cos(a) * r,
      dy: Math.sin(a) * r,
      angle: a + Math.PI, // facing the centre
    });
  }
  return { heads, alpha, r };
}
