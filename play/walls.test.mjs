// Wall line expansion tests (BLD-13): the client's preview
// algorithm must produce exactly the sim's lines — the same
// golden vectors pin `wall_line_cells` in sim/src/build.rs, so
// ghost == committed, cell for cell, by construction (both run
// this one straight-line raster on the same two endpoint cells).
//   node --test walls.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { wallLineCells, wallPiece } from './walls.js';

test('a click is a single segment', () => {
  assert.deepEqual(wallLineCells(5, 5, 5, 5), [[5, 5]]);
});

test('horizontal and vertical drags are exact rows/columns', () => {
  assert.deepEqual(wallLineCells(2, 3, 6, 3),
    [[2, 3], [3, 3], [4, 3], [5, 3], [6, 3]]);
  assert.deepEqual(wallLineCells(4, 1, 4, 4),
    [[4, 1], [4, 2], [4, 3], [4, 4]]);
});

test('a 45-degree drag is a pure diagonal chain', () => {
  // Claudio's 2026-10-24 correction: a diagonal drag is one
  // straight line end to end — diagonally-adjacent segments
  // only, NOT the seven-cell staircase the supercover laid.
  assert.deepEqual(wallLineCells(1, 1, 4, 4),
    [[1, 1], [2, 2], [3, 3], [4, 4]]);
  assert.deepEqual(wallLineCells(0, 0, 6, 6),
    [[0, 0], [1, 1], [2, 2], [3, 3], [4, 4], [5, 5], [6, 6]]);
});

test('a shallow slope hugs the ideal line (diagonal steps spread)', () => {
  assert.deepEqual(wallLineCells(0, 0, 7, 2),
    [[0, 0], [1, 0], [2, 1], [3, 1], [4, 1], [5, 1], [6, 2], [7, 2]]);
});

test('a steep slope is the transposed chain', () => {
  assert.deepEqual(wallLineCells(0, 0, 2, 7),
    [[0, 0], [0, 1], [1, 2], [1, 3], [1, 4], [1, 5], [2, 6], [2, 7]]);
});

test("Claudio's reported drag shape: one line, not three disjoint runs", () => {
  // The old Bresenham laid (0,0)->(18,2) as 19 segments in three
  // horizontal runs on three rows, joined at two corner-only
  // points — the screenshot he sent in 2026-10-22. The straight
  // raster also lays 19 (max(|dx|,|dy|)+1 — the supercover laid
  // 21), spread along the ideal line with diagonal steps at
  // x=4->5 and x=13->14.
  const line = wallLineCells(0, 0, 18, 2);
  assert.equal(line.length, 19);
  assert.ok(line.some(([x, y]) => x === 4 && y === 0));
  assert.ok(line.some(([x, y]) => x === 5 && y === 1));
  assert.ok(line.some(([x, y]) => x === 13 && y === 1));
  assert.ok(line.some(([x, y]) => x === 14 && y === 2));
});

test('line integrity over every drag in a 13x13 field', () => {
  const key = (c) => c[0] + ',' + c[1];
  for (let ax = 0; ax <= 12; ax++) for (let ay = 0; ay <= 12; ay++)
  for (let bx = 0; bx <= 12; bx++) for (let by = 0; by <= 12; by++) {
    const cells = wallLineCells(ax, ay, bx, by);
    // Endpoints included, exact straight-line count.
    assert.deepEqual(cells[0], [ax, ay]);
    assert.deepEqual(cells[cells.length - 1], [bx, by]);
    assert.equal(cells.length,
      1 + Math.max(Math.abs(bx - ax), Math.abs(by - ay)));
    // No gaps, no repeats, no jumps: every consecutive pair is
    // a king move (Chebyshev step exactly 1 — diagonal steps
    // included; the joints they form are sealed by the sim's
    // A* corner rule, pinned in sim/tests/walls.rs).
    for (let k = 1; k < cells.length; k++) {
      const step = Math.max(Math.abs(cells[k][0] - cells[k - 1][0]),
        Math.abs(cells[k][1] - cells[k - 1][1]));
      assert.equal(step, 1, `king step ${ax},${ay} -> ${bx},${by}`);
    }
    // No cell is visited twice.
    assert.equal(new Set(cells.map(key)).size, cells.length);
    // A reversed drag yields the identical cells in exactly
    // reversed order (absolute-position rounding has no bias).
    assert.deepEqual(wallLineCells(bx, by, ax, ay), [...cells].reverse());
  }
});

// ------------------------------------------------------------------
// Piece selection (the angle pass): every neighbour configuration
// maps to the baked piece DE's configuration catalogue calls for,
// with the horizontal mirror (the plan mirror (u,v)->(v,u):
// e<->s, w<->n, se/nw fixed, sw<->ne) as the only transform. These
// cases pin the table in wallPiece; the art files behind the keys
// are pinned by buildingart.test.mjs (every WALL_PIECE_ART suffix
// must exist in BUILDING_ART_FILES with its file on disk).
const M = (dirs) => Object.fromEntries(
  ['e', 's', 'w', 'n', 'se', 'sw', 'ne', 'nw'].map((d) => [d, dirs.includes(d)]));

test('piece: an unjoined segment is the default appearance', () => {
  assert.deepEqual(wallPiece(M([])), { key: 'single', flip: false });
});

test('piece: straight runs are middle sections along their axis', () => {
  assert.deepEqual(wallPiece(M(['e', 'w'])), { key: 'midx', flip: false });
  assert.deepEqual(wallPiece(M(['n', 's'])), { key: 'midx', flip: true });
});

test('piece: free ends cap toward their one neighbour', () => {
  assert.deepEqual(wallPiece(M(['e'])), { key: 'ende', flip: false });
  assert.deepEqual(wallPiece(M(['s'])), { key: 'ende', flip: true });
  assert.deepEqual(wallPiece(M(['w'])), { key: 'endew', flip: false });
  assert.deepEqual(wallPiece(M(['n'])), { key: 'endew', flip: true });
});

test('piece: corners turn with their own pieces', () => {
  assert.deepEqual(wallPiece(M(['e', 's'])), { key: 'corneres', flip: false });
  assert.deepEqual(wallPiece(M(['e', 'n'])), { key: 'corneren', flip: false });
  assert.deepEqual(wallPiece(M(['w', 's'])), { key: 'corneren', flip: true });
  assert.deepEqual(wallPiece(M(['w', 'n'])), { key: 'cornerwn', flip: false });
});

test('piece: T junctions and the crossing', () => {
  assert.deepEqual(wallPiece(M(['e', 'w', 's'])), { key: 'tees', flip: false });
  assert.deepEqual(wallPiece(M(['e', 's', 'n'])), { key: 'tees', flip: true });
  assert.deepEqual(wallPiece(M(['e', 'w', 'n'])), { key: 'teen', flip: false });
  assert.deepEqual(wallPiece(M(['w', 's', 'n'])), { key: 'teen', flip: true });
  assert.deepEqual(wallPiece(M(['e', 'w', 's', 'n'])), { key: 'cross', flip: false });
});

test('piece: diagonal-only neighbours reach toward the shared corner', () => {
  assert.deepEqual(wallPiece(M(['se'])), { key: 'diagse', flip: false });
  assert.deepEqual(wallPiece(M(['sw'])), { key: 'diagsw', flip: false });
  assert.deepEqual(wallPiece(M(['ne'])), { key: 'diagsw', flip: true });
  assert.deepEqual(wallPiece(M(['nw'])), { key: 'diagnw', flip: false });
  assert.deepEqual(wallPiece(M(['se', 'nw'])), { key: 'diagns', flip: false });
  assert.deepEqual(wallPiece(M(['sw', 'ne'])), { key: 'diagew', flip: false });
});

test('piece: a corner absorbs the diagonal between its arms', () => {
  // A junction cell joined on two orthogonal sides: the corner
  // piece bands the turn, and a diagonal neighbour lying between
  // its two arms (or across the junction) changes nothing.
  assert.deepEqual(wallPiece(M(['e', 's', 'se', 'nw'])), { key: 'corneres', flip: false });
  assert.deepEqual(wallPiece(M(['w', 'n', 'se', 'nw'])), { key: 'cornerwn', flip: false });
});

test('piece: a slope transition is an elbow (diagonal across the run)', () => {
  // The transition cells of the straight-line expansion: one
  // orthogonal neighbour plus the diagonal neighbour the line
  // steps to ACROSS the segment — the elbow joins the straight
  // arm to the corner arm so the band bends once, at the corner
  // point, instead of breaking. All eight configurations: the
  // four baked w/e bases and their plan mirrors.
  assert.deepEqual(wallPiece(M(['w', 'se'])), { key: 'elbwse', flip: false });
  assert.deepEqual(wallPiece(M(['w', 'ne'])), { key: 'elbwne', flip: false });
  assert.deepEqual(wallPiece(M(['e', 'nw'])), { key: 'elbenw', flip: false });
  assert.deepEqual(wallPiece(M(['e', 'sw'])), { key: 'elbesw', flip: false });
  assert.deepEqual(wallPiece(M(['n', 'se'])), { key: 'elbwse', flip: true });
  assert.deepEqual(wallPiece(M(['n', 'sw'])), { key: 'elbwne', flip: true });
  assert.deepEqual(wallPiece(M(['s', 'nw'])), { key: 'elbenw', flip: true });
  assert.deepEqual(wallPiece(M(['s', 'ne'])), { key: 'elbesw', flip: true });
  // The canonical cells of a shallow drag (0,0)->(7,2): (2,1)
  // enters from the NW diagonal and leaves E; (1,0) leaves W and
  // steps SE.
  assert.deepEqual(wallPiece(M(['e', 'nw'])), { key: 'elbenw', flip: false });
  assert.deepEqual(wallPiece(M(['w', 'se'])), { key: 'elbwse', flip: false });
});

test('piece: a same-side diagonal is NOT a transition (plain end stands)', () => {
  // A diagonal neighbour on the SAME side as the orthogonal one
  // does not continue the line through this cell — {e, se} is
  // the run's END cell in a shallow drag, and its band is the
  // end piece reaching toward the corner via the neighbour's
  // own elbow/diagonal arm.
  assert.deepEqual(wallPiece(M(['e', 'se'])), { key: 'ende', flip: false });
  assert.deepEqual(wallPiece(M(['w', 'nw'])), { key: 'endew', flip: false });
  assert.deepEqual(wallPiece(M(['n', 'ne'])), { key: 'endew', flip: true });
  assert.deepEqual(wallPiece(M(['s', 'sw'])), { key: 'ende', flip: true });
});

test('piece: every one of the 256 masks resolves to a baked key', () => {
  const baked = new Set(['single', 'midx', 'ende', 'endew', 'corneres',
    'corneren', 'cornerwn', 'tees', 'teen', 'cross', 'diagse', 'diagsw',
    'diagnw', 'diagns', 'diagew',
    'elbwse', 'elbwne', 'elbenw', 'elbesw']);
  const dirs = ['e', 's', 'w', 'n', 'se', 'sw', 'ne', 'nw'];
  for (let bits = 0; bits < 256; bits++) {
    const mask = Object.fromEntries(dirs.map((d, i) => [d, !!(bits & (1 << i))]));
    const p = wallPiece(mask);
    assert.ok(baked.has(p.key), `mask ${bits} -> ${p.key}`);
    assert.equal(typeof p.flip, 'boolean');
  }
});
