// Wall line expansion tests (BLD-13): the client's preview
// algorithm must produce exactly the sim's lines — the same
// cases pin `wall_line_cells` in sim/src/build.rs.
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

test('a 45-degree drag is a pure diagonal staircase', () => {
  assert.deepEqual(wallLineCells(1, 1, 4, 4),
    [[1, 1], [2, 2], [3, 3], [4, 4]]);
});

test('a shallow slope hugs the ideal line, one cell per column', () => {
  const line = wallLineCells(0, 0, 7, 2);
  assert.deepEqual(line[0], [0, 0]);
  assert.deepEqual(line[line.length - 1], [7, 2]);
  assert.equal(line.length, 8);
  for (let i = 1; i < line.length; i++) {
    const dx = Math.abs(line[i][0] - line[i - 1][0]);
    const dy = Math.abs(line[i][1] - line[i - 1][1]);
    assert.ok(dx <= 1 && dy <= 1 && dx + dy >= 1, '8-connected step');
  }
});

test('a steep slope has one cell per row', () => {
  const line = wallLineCells(0, 0, 2, 7);
  assert.equal(line.length, 8);
  assert.deepEqual(line[line.length - 1], [2, 7]);
});

test('reversed drags expand from the new start', () => {
  // The drag direction is part of the gesture: the line is
  // expanded from wherever the player pressed, and both the
  // sim and the preview agree because they run the same code
  // path on the same endpoints.
  assert.deepEqual(wallLineCells(6, 3, 2, 3),
    [[6, 3], [5, 3], [4, 3], [3, 3], [2, 3]]);
  assert.deepEqual(wallLineCells(4, 4, 1, 1),
    [[4, 4], [3, 3], [2, 2], [1, 1]]);
});
