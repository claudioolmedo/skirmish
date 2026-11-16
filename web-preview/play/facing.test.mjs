// Node tests for the MOV-09 facing model (play/facing.js) plus
// the app.js wiring that makes it THE model: every draw path
// quantizes through it, and melee engagement turns the unit
// toward its target. Run: node --test play/facing.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  DIR8_VECTORS, dir8FromVector, turnedDir8, faceForDir8,
  sheetRowForDir8,
} from './facing.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('the 8 sector centres quantize to their own dir', () => {
  for (let d = 0; d < 8; d++) {
    const a = (d * Math.PI) / 4;
    assert.equal(dir8FromVector(Math.cos(a), Math.sin(a)), d, `dir ${d}`);
    // Scale must not matter — only the angle does.
    assert.equal(dir8FromVector(7 * Math.cos(a), 7 * Math.sin(a)), d);
  }
});

test('axis and diagonal world vectors land on the documented dirs', () => {
  assert.equal(dir8FromVector(1, 0), 0); // +X
  assert.equal(dir8FromVector(1, 1), 1); // +X+Y, down-screen front
  assert.equal(dir8FromVector(0, 1), 2); // +Y
  assert.equal(dir8FromVector(-1, 1), 3);
  assert.equal(dir8FromVector(-1, 0), 4); // −X
  assert.equal(dir8FromVector(-1, -1), 5); // −X−Y, up-screen back
  assert.equal(dir8FromVector(0, -1), 6); // −Y
  assert.equal(dir8FromVector(1, -1), 7);
});

test('sector boundaries sit at the odd 22.5° steps', () => {
  const at = (deg) => dir8FromVector(
    Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180));
  assert.equal(at(22.4), 0);
  assert.equal(at(22.6), 1);
  assert.equal(at(67.4), 1);
  assert.equal(at(67.6), 2);
  assert.equal(at(-22.4), 0);
  assert.equal(at(-22.6), 7);
  assert.equal(at(180), 4);
  assert.equal(at(-180), 4);
});

test('a zero vector has no facing; turnedDir8 keeps the current one', () => {
  assert.equal(dir8FromVector(0, 0), -1);
  assert.equal(turnedDir8(3, 0, 0), 3);
  assert.equal(turnedDir8(3, 0, -5), 6); // straight −Y from anywhere
  assert.equal(turnedDir8(0, -2, 0), 4);
});

test('DIR8_VECTORS are the 8 unit vectors and round-trip', () => {
  assert.equal(DIR8_VECTORS.length, 8);
  for (let d = 0; d < 8; d++) {
    const [dx, dy] = DIR8_VECTORS[d];
    assert.ok(Math.abs(Math.hypot(dx, dy) - 1) < 1e-9, `dir ${d} unit`);
    assert.equal(dir8FromVector(dx, dy), d, `dir ${d} round-trips`);
  }
  assert.deepEqual(DIR8_VECTORS[0], [1, 0]);
  assert.equal(DIR8_VECTORS[4][0], -1);
  assert.ok(Math.abs(DIR8_VECTORS[4][1]) < 1e-9);
  assert.ok(Math.abs(DIR8_VECTORS[2][0]) < 1e-9);
  assert.equal(DIR8_VECTORS[2][1], 1);
});

test('faceForDir8: the side-view flip table for all 8 facings', () => {
  // Screen-x follows sign(dx − dy): right for +X-ish facings,
  // left for −X-ish, straight up/down screen at rows 5 and 1.
  assert.deepEqual(
    [0, 1, 2, 3, 4, 5, 6, 7].map((d) => faceForDir8(d)),
    [1, 0, -1, -1, -1, 0, 1, 1]);
});

test('sheetRowForDir8: identity at 8 rows, scaled at the tiers', () => {
  for (let d = 0; d < 8; d++) {
    assert.equal(sheetRowForDir8(d, 8), d);
    assert.equal(sheetRowForDir8(d, 16), d * 2);
    assert.equal(sheetRowForDir8(d, 32), d * 4);
    assert.equal(sheetRowForDir8(d, 1), 0);
    // The corpse-style back-conversion recovers the facing at
    // every tier: the stored row never loses the 8-way model.
    for (const rows of [8, 16, 32]) {
      const row = sheetRowForDir8(d, rows);
      assert.equal(Math.round((row * 8) / rows), d, `dir ${d} @ ${rows}`);
    }
  }
});

test('app.js wires the facing model (import + shared quantization)', () => {
  assert.match(appJs, /from '\.\/play\/facing\.js'/);
  assert.match(appJs, /dir8FromVector/);
  assert.match(appJs, /sheetRowForDir8\(act\.dir, rows\)/);
  // No private atan2 quantization survives in the pose paths:
  // the old inline forms are gone from app.js.
  assert.ok(!appJs.includes('Math.atan2(dy, dx) / (Math.PI / 4)'));
  assert.ok(!appJs.includes('Math.atan2(dy, dx) / ((Math.PI * 2) / dirs)'));
});

test('app.js: melee engagement turns toward the target (MOV-09)', () => {
  // The nearest-foe scan exists beside foeNearUnit...
  assert.match(appJs, /function foeTargetFor\(i, wx, wy\)/);
  // ...and the turn is applied on the attack paths: the shared
  // unit action picker, the villager sheet path, the wolf and
  // the boar (4 call sites + the definition = 5 mentions).
  const mentions = appJs.split('foeTargetFor(').length - 1;
  assert.ok(mentions >= 5, `foeTargetFor call sites: ${mentions}`);
  assert.match(appJs, /if \(action === 'attack'\) \{\s*\n\s*const tgt = foeTargetFor/);
});
