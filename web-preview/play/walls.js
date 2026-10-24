// Skirmish II — wall line expansion (BLD-13). Pure logic only:
// the cells of a dragged Palisade Wall line, computed as the
// SUPERCOVER of the straight segment between the two anchor
// cells' centers — every cell the segment passes through, walked
// with an exact-integer grid traversal (Amanatides–Woo), the
// IDENTICAL algorithm the sim runs (`wall_line_cells` in
// sim/src/build.rs), so the placement ghost previews exactly the
// segments the sim will validate, in the same order.
//
// Why supercover and not the classic Bresenham this replaced
// (Claudio's live report 2026-10-22: dragged segments "não segue
// a mesma linha" — three disjoint row-runs in his screenshot):
// Bresenham advances one cell per major-axis step, so consecutive
// segments touch only at corners on any slope — a shallow drag
// became separate horizontal runs on different rows, and its
// tie-breaking is direction-dependent (a 13×13 endpoint scan
// found 9,096 of 28,392 drags producing a different set when
// reversed). The supercover walk steps cell to cell across grid
// edges, so consecutive segments are ALWAYS edge-adjacent
// (4-connected): the fence is one continuous line on one row-run
// per slope step, with no gaps and no corner-only joints. Where
// the segment passes exactly through a grid corner, the walk
// fills the corner with the canonically-chosen side cell (the
// candidate with the smaller x — a function of the corner's
// unordered cell pair, so a reversed drag over the same corner
// fills the same cell) before stepping diagonally: the SET is
// identical whichever end the player drags from. A straight
// drag is exactly that row/column; a click (start == end) is a
// single segment; the line always has 1 + |dx| + |dy| cells.
//
// DE's own rasterization is not published (the wiki documents
// 1×1 grid segments placed by dragging, pieces that visually
// join when connected in line, and completed diagonal corners
// blocking passage — see docs/MECHANICS-RULEBOOK.md BLD-13);
// this supercover is the house line that reproduces those
// observable properties.

/**
 * The cells of the wall line from anchor cell (ax, ay) to anchor
 * cell (bx, by), both endpoints included, in drag order.
 * Returns an array of [cx, cy] pairs.
 */
export function wallLineCells(ax, ay, bx, by) {
  const adx = Math.abs(bx - ax);
  const ady = Math.abs(by - ay);
  const sx = bx > ax ? 1 : -1;
  const sy = by > ay ? 1 : -1;
  let x = ax;
  let y = ay;
  let i = 0; // x-steps taken
  let j = 0; // y-steps taken
  const out = [[x, y]];
  while (i < adx || j < ady) {
    if (j >= ady) {
      x += sx;
      i++;
      out.push([x, y]);
      continue;
    }
    if (i >= adx) {
      y += sy;
      j++;
      out.push([x, y]);
      continue;
    }
    // The segment crosses the next vertical grid edge at a
    // parameter proportional to (2i+1)/(2*adx) and the next
    // horizontal edge at (2j+1)/(2*ady); compare exactly in
    // integers: (2i+1)*ady vs (2j+1)*adx.
    const lhs = (2 * i + 1) * ady;
    const rhs = (2 * j + 1) * adx;
    if (lhs < rhs) {
      x += sx;
      i++;
      out.push([x, y]);
    } else if (lhs > rhs) {
      y += sy;
      j++;
      out.push([x, y]);
    } else {
      // Exact corner: the segment passes through the grid
      // corner shared by the current cell, both side cells and
      // the diagonal cell. Fill the canonical side cell (the
      // candidate with the smaller x) so the line stays
      // edge-connected, then take the diagonal cell.
      const fillX = Math.min(x + sx, x);
      const fillY = fillX === x ? y + sy : y;
      out.push([fillX, fillY]);
      x += sx;
      y += sy;
      i++;
      j++;
      out.push([x, y]);
    }
  }
  return out;
}

// ------------------------------------------------------------------
// Piece selection — how a wall segment is DRAWN (client only; the
// sim never sees this). DE picks each segment's art from its
// neighbours and draws it in the tile's isometric frame: a run is a
// band following the run's slope, a corner turns through the tile
// between the two shared edge midpoints with a post at the apex, a
// free end is capped inside the tile, and a diagonal run is a
// staircase of corner pieces whose bands meet at the shared edges
// (catalogue + audit: study dossier DE-WALL-PIECES-ADDENDUM).
// `wallPiece` classifies one segment from its full 8-neighbour
// same-kind mask {e,s,w,n,se,sw,ne,nw} into a baked piece key plus
// an optional horizontal mirror:
//
//   key      the baked art (see gen-wall-pieces.py): 'single' (the
//            family's established panel — DE's "default appearance"
//            for an unjoined piece), 'midx', 'ende', 'endew',
//            'corneres', 'corneren', 'cornerwn', 'tees', 'teen',
//            'cross', 'diagse', 'diagsw', 'diagnw', 'diagns',
//            'diagew'
//   flip     mirror the sprite horizontally. THE ONLY sprite
//            transform that is exact for a projected piece: it is
//            the plan mirror (u,v) -> (v,u), swapping e<->s and
//            w<->n (se and nw are fixed, sw<->ne). A plan rotation
//            or a vertical flip is NOT a sprite transform — height
//            is not symmetric in the projection — so every
//            configuration the mirror cannot reach is baked.
//
// Orthogonal neighbours decide the piece whenever any exists (DE
// joins staircase steps through their shared edges); diagonal
// neighbours speak only for a segment with no orthogonal neighbour
// (two lines touching at a corner, or a lone segment after a
// deletion), where the piece reaches toward the shared corner.
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
