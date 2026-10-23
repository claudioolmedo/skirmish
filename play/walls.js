// Skirmish II — wall line expansion (BLD-13). Pure logic only:
// the cells of a dragged Palisade Wall line, computed with the
// classic integer Bresenham line between the two anchor cells —
// the IDENTICAL algorithm the sim runs (`wall_line_cells` in
// sim/src/build.rs), so the placement ghost previews exactly the
// segments the sim will validate, in the same order. Consecutive
// cells are 8-connected: a diagonal drag lays corner-touching
// segments, a straight drag is exactly that row/column, and a
// click (start == end) is a single segment.

/**
 * The cells of the wall line from anchor cell (ax, ay) to anchor
 * cell (bx, by), both endpoints included, in drag order.
 * Returns an array of [cx, cy] pairs.
 */
export function wallLineCells(ax, ay, bx, by) {
  let x = ax;
  let y = ay;
  const dx = Math.abs(bx - x);
  const sx = x < bx ? 1 : -1;
  const dy = -Math.abs(by - y);
  const sy = y < by ? 1 : -1;
  let err = dx + dy;
  const out = [];
  for (;;) {
    out.push([x, y]);
    if (x === bx && y === by) break;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y += sy;
    }
  }
  return out;
}
