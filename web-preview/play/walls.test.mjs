// Wall line expansion tests (BLD-13): the client's preview
// algorithm must produce exactly the sim's lines — the same
// golden vectors pin `wall_line_cells` in sim/src/build.rs, so
// ghost == committed, cell for cell, by construction (both run
// this one supercover walk on the same two endpoint cells).
//   node --test walls.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { wallLineCells } from './walls.js';

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
