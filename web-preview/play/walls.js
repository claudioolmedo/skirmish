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
