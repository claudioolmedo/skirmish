// Skirmish II — wall line expansion (BLD-13). Pure logic only:
// the cells of a dragged Palisade Wall line: the STRAIGHT segment
// between the two anchor cells' centers rasterized as an
// 8-direction line — at each of the n+1 steps of the major axis
// (n = max(|dx|, |dy|)), the cell nearest the ideal point of the
// segment, each coordinate rounded to the nearest integer (halves
// up) by a pure function of the ABSOLUTE ideal position. The
// IDENTICAL algorithm the sim runs (`wall_line_cells` in
// sim/src/build.rs), so the placement ghost previews exactly the
// segments the sim will validate, in the same order — and the
// reversed drag yields the identical cells in exactly reversed
// order.
//
// Why this line (history): the classic Bresenham was replaced on
// 2026-10-22 after Claudio's live report (dragged segments "não
// segue a mesma linha" — disjoint row-runs in his screenshot;
// Bresenham's tie-breaking is also direction-dependent: 9,096 of
// 28,392 drags changed set when reversed). Its replacement, the
// Amanatides–Woo SUPERCOVER (every cell the segment touches,
// 1 + |dx| + |dy| cells, a 4-connected staircase), was itself
// rejected by Claudio live on 2026-10-24: a diagonal drag
// rendered as a zigzag of alternating straight runs — "não existe
// este tipo de muralha; normalmente ele pega uma ponta a outra
// para fazer em uma linha reta; o diagonal como vc sugere nunca
// vi no jogo." In DE a wall drag goes end to end in a straight
// line: this expansion steps diagonally along the line's own run
// (consecutive cells are king-adjacent, Chebyshev step exactly
// 1), so a 45° drag is a pure chain of diagonal neighbours and
// every drag hugs the ideal segment. DE rasterizes a true
// straight line exactly for row/column/45° drags and lays its
// other drags as an L of a diagonal leg + a straight leg at the
// same max(|dx|,|dy|)+1 tile count (official forum, "Get rid of
// L shape when creating walls"); per Claudio's ruling Skirmish
// lays every drag on the ideal line instead — same endpoints,
// same optimal count. The diagonal chain's corner joints are
// sealed for movement by the sim's A* corner rule (see
// docs/MECHANICS-RULEBOOK.md BLD-13 and the correction addendum
// in the study dossier, walls-de/DE-WALL-LINE-CORRECTION.md).

/**
 * The cells of the wall line from anchor cell (ax, ay) to anchor
 * cell (bx, by), both endpoints included, in drag order.
 * Returns an array of [cx, cy] pairs.
 */
export function wallLineCells(ax, ay, bx, by) {
  const dx = bx - ax;
  const dy = by - ay;
  const n = Math.max(Math.abs(dx), Math.abs(dy));
  if (n === 0) return [[ax, ay]];
  // The coordinate of the cell nearest the ideal point at step
  // k along one axis: round(a + d*k/n), halves up, computed
  // exactly as floor((2*(a*n + d*k) + n) / (2*n)) — the same
  // integer formula as the sim (values are tiny; exact in f64).
  const coord = (a, d, k) => Math.floor((2 * (a * n + d * k) + n) / (2 * n));
  const out = [];
  for (let k = 0; k <= n; k++) {
    out.push([coord(ax, dx, k), coord(ay, dy, k)]);
  }
  return out;
}

// ------------------------------------------------------------------
// Piece selection — how a wall segment is DRAWN (client only; the
// sim never sees this). DE picks each segment's art from its
// neighbours and draws it in the tile's isometric frame: a run is a
// band following the run's slope, a corner turns through the tile
// between the two shared edge midpoints with a post at the apex, a
// free end is capped inside the tile, and a diagonal run is a chain
// of pieces whose posts meet AT THE CORNER POINT between
// diagonally-adjacent tiles — the corner-join treatment Claudio
// approved from the pieces proof on 2026-10-24 ("teria que ser
// assim"; catalogue + audit: study dossier DE-WALL-PIECES-ADDENDUM,
// line correction: DE-WALL-LINE-CORRECTION.md). `wallPiece`
// classifies one segment from its full 8-neighbour same-kind mask
// {e,s,w,n,se,sw,ne,nw} into a baked piece key plus an optional
// horizontal mirror:
//
//   key      the baked art (see gen-wall-pieces.py): 'single' (the
//            family's established panel — DE's "default appearance"
//            for an unjoined piece), 'midx', 'ende', 'endew',
//            'corneres', 'corneren', 'cornerwn', 'tees', 'teen',
//            'cross', 'diagse', 'diagsw', 'diagnw', 'diagns',
//            'diagew', and the slope-transition elbows 'elbwse',
//            'elbwne', 'elbenw', 'elbesw'
//   flip     mirror the sprite horizontally. THE ONLY sprite
//            transform that is exact for a projected piece: it is
//            the plan mirror (u,v) -> (v,u), swapping e<->s and
//            w<->n (se and nw are fixed, sw<->ne). A plan rotation
//            or a vertical flip is NOT a sprite transform — height
//            is not symmetric in the projection — so every
//            configuration the mirror cannot reach is baked.
//
// Orthogonal neighbours decide the piece whenever any exists. With
// exactly ONE orthogonal neighbour, a diagonal neighbour lying
// ACROSS the segment from it (its component along the orthogonal
// axis points back through this cell — the straight-line
// expansion's diagonal step continuing the run) selects an ELBOW:
// the straight arm joins the corner arm at the corner post, so a
// slope transition bends once instead of breaking the band. A
// diagonal on the same side as the orthogonal neighbour is not a
// continuation and never selects an elbow. With two or more
// orthogonal neighbours the corner/tee/cross arms already span
// the cell (a diagonal between a corner's two arms is absorbed by
// the corner piece; any other is junction debris). Diagonal
// neighbours speak alone only for a segment with no orthogonal
// neighbour (a pure diagonal run, two lines touching at a corner,
// or a lone segment after a deletion), where the piece reaches
// toward the shared corner.
const ELBOWS = {
  // key: `${ortho}${diag}` -> baked elbow piece (see header). The
  // four baked bases are the w/e variants; the plan mirror covers
  // the n/s variants exactly.
  esw: { key: 'elbesw', flip: false },
  enw: { key: 'elbenw', flip: false },
  wse: { key: 'elbwse', flip: false },
  wne: { key: 'elbwne', flip: false },
  sne: { key: 'elbesw', flip: true },
  snw: { key: 'elbenw', flip: true },
  nse: { key: 'elbwse', flip: true },
  nsw: { key: 'elbwne', flip: true },
};
const ORTHO_VEC = { e: [1, 0], s: [0, 1], w: [-1, 0], n: [0, -1] };
const DIAG_VEC = { se: [1, 1], sw: [-1, 1], ne: [1, -1], nw: [-1, -1] };

export function wallPiece(mask) {
  const has = (d) => !!mask[d];
  const ortho = ['e', 's', 'w', 'n'].filter(has);
  if (ortho.length === 4) return { key: 'cross', flip: false };
  if (ortho.length === 3) {
    // base 'tees' lacks n, base 'teen' lacks s; the mirror turns
    // each into the variant lacking w / e respectively
    if (!has('n')) return { key: 'tees', flip: false };
    if (!has('w')) return { key: 'tees', flip: true };
    if (!has('s')) return { key: 'teen', flip: false };
    return { key: 'teen', flip: true }; // lacks e
  }
  if (ortho.length === 2) {
    if (has('e') && has('w')) return { key: 'midx', flip: false };
    if (has('n') && has('s')) return { key: 'midx', flip: true };
    if (has('e') && has('s')) return { key: 'corneres', flip: false };
    if (has('w') && has('n')) return { key: 'cornerwn', flip: false };
    if (has('e') && has('n')) return { key: 'corneren', flip: false };
    return { key: 'corneren', flip: true }; // {w, s}
  }
  if (ortho.length === 1) {
    // Slope transition first (see header): a diagonal neighbour
    // across the segment from the orthogonal one continues the
    // line — the elbow joins the straight arm to the corner arm.
    // Scan order is fixed (se, sw, ne, nw); at most the two
    // across-side diagonals can qualify, and in a real line only
    // one is ever present.
    const o = ortho[0];
    const ov = ORTHO_VEC[o];
    for (const d of ['se', 'sw', 'ne', 'nw']) {
      if (!has(d)) continue;
      const dv = DIAG_VEC[d];
      const along = ov[0] !== 0 ? dv[0] * ov[0] : dv[1] * ov[1];
      if (along !== -1) continue; // same side: not a continuation
      const elbow = ELBOWS[o + d];
      if (elbow) return { key: elbow.key, flip: elbow.flip };
    }
    // base 'ende' arms toward e (mirror: s); base 'endew' arms
    // toward w (mirror: n)
    if (has('e')) return { key: 'ende', flip: false };
    if (has('s')) return { key: 'ende', flip: true };
    if (has('w')) return { key: 'endew', flip: false };
    return { key: 'endew', flip: true }; // n
  }
  const diag = ['se', 'sw', 'ne', 'nw'].filter(has);
  if (diag.length === 0) return { key: 'single', flip: false };
  if (diag.length === 2 && has('se') && has('nw')) return { key: 'diagns', flip: false };
  if (diag.length === 2 && has('sw') && has('ne')) return { key: 'diagew', flip: false };
  // one diagonal neighbour (or an adjacent pair / triple, which has
  // no through-piece: reach toward the first corner in this order)
  if (has('se')) return { key: 'diagse', flip: false };
  if (has('sw')) return { key: 'diagsw', flip: false };
  if (has('ne')) return { key: 'diagsw', flip: true };
  return { key: 'diagnw', flip: false }; // nw
}
