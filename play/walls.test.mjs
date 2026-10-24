// Wall line expansion tests (BLD-13): the client's preview
// algorithm must produce exactly the sim's lines — the same
// golden vectors pin `wall_line_cells` in sim/src/build.rs, so
// ghost == committed, cell for cell, by construction (both run
// this one supercover walk on the same two endpoint cells).
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

test('a 45-degree drag is an edge-connected staircase on the diagonal', () => {
  // Each exact corner is filled on the canonical (smaller-x)
  // side, so no two segments touch only at a corner — the fence
  // is one continuous chain hugging the diagonal.
  assert.deepEqual(wallLineCells(1, 1, 4, 4),
    [[1, 1], [1, 2], [2, 2], [2, 3], [3, 3], [3, 4], [4, 4]]);
});

test('a shallow slope is one chain whose row runs overlap at the steps', () => {
  assert.deepEqual(wallLineCells(0, 0, 7, 2),
    [[0, 0], [1, 0], [2, 0], [2, 1], [3, 1], [4, 1], [5, 1], [5, 2], [6, 2], [7, 2]]);
});

test('a steep slope is the transposed chain', () => {
  assert.deepEqual(wallLineCells(0, 0, 2, 7),
    [[0, 0], [0, 1], [0, 2], [1, 2], [1, 3], [1, 4], [1, 5], [2, 5], [2, 6], [2, 7]]);
});

test("Claudio's reported drag shape: one line, not three disjoint runs", () => {
  // The old Bresenham laid (0,0)->(18,2) as 19 segments in three
  // horizontal runs on three rows, joined at two corner-only
  // points — the screenshot he sent. The supercover lays 21 in
  // one chain; the runs overlap at the corner fills (4,1) and
  // (13,2).
  const line = wallLineCells(0, 0, 18, 2);
  assert.equal(line.length, 21);
  assert.ok(line.some(([x, y]) => x === 4 && y === 1));
  assert.ok(line.some(([x, y]) => x === 13 && y === 2));
});

test('line integrity over every drag in a 13x13 field', () => {
  const key = (c) => c[0] + ',' + c[1];
  for (let ax = 0; ax <= 12; ax++) for (let ay = 0; ay <= 12; ay++)
  for (let bx = 0; bx <= 12; bx++) for (let by = 0; by <= 12; by++) {
    const cells = wallLineCells(ax, ay, bx, by);
    // Endpoints included, exact supercover count.
    assert.deepEqual(cells[0], [ax, ay]);
    assert.deepEqual(cells[cells.length - 1], [bx, by]);
    assert.equal(cells.length, 1 + Math.abs(bx - ax) + Math.abs(by - ay));
    // No gaps: every consecutive pair is edge-adjacent — never
    // a diagonal (corner-only) step.
    for (let k = 1; k < cells.length; k++) {
      const step = Math.abs(cells[k][0] - cells[k - 1][0])
        + Math.abs(cells[k][1] - cells[k - 1][1]);
      assert.equal(step, 1, `edge-adjacent step ${ax},${ay} -> ${bx},${by}`);
    }
    // No cell is visited twice.
    assert.equal(new Set(cells.map(key)).size, cells.length);
    // A reversed drag yields the identical SET (the canonical
    // corner fill makes the line direction-independent).
    const fwd = new Set(cells.map(key));
    const rev = new Set(wallLineCells(bx, by, ax, ay).map(key));
    assert.deepEqual(rev, fwd);
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

test('piece: a staircase step is a corner (orthogonal beats diagonal)', () => {
  // The middle of a 45-degree staircase run: joined E and S (the
  // steps), with diagonal neighbours along the run's line — the
  // corner piece is what bands the run through the tile.
  assert.deepEqual(wallPiece(M(['e', 's', 'se', 'nw'])), { key: 'corneres', flip: false });
  assert.deepEqual(wallPiece(M(['w', 'n', 'se', 'nw'])), { key: 'cornerwn', flip: false });
  // Any orthogonal neighbour at all keeps diagonals silent.
  assert.deepEqual(wallPiece(M(['e', 'se'])), { key: 'ende', flip: false });
});

test('piece: every one of the 256 masks resolves to a baked key', () => {
  const baked = new Set(['single', 'midx', 'ende', 'endew', 'corneres',
    'corneren', 'cornerwn', 'tees', 'teen', 'cross', 'diagse', 'diagsw',
    'diagnw', 'diagns', 'diagew']);
  const dirs = ['e', 's', 'w', 'n', 'se', 'sw', 'ne', 'nw'];
  for (let bits = 0; bits < 256; bits++) {
    const mask = Object.fromEntries(dirs.map((d, i) => [d, !!(bits & (1 << i))]));
    const p = wallPiece(mask);
    assert.ok(baked.has(p.key), `mask ${bits} -> ${p.key}`);
    assert.equal(typeof p.flip, 'boolean');
  }
});
