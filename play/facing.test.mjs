// Node tests for the MOV-09 facing model (play/facing.js) plus
// the app.js wiring that makes it THE model: every draw path
// quantizes through it, and melee engagement turns the unit
// toward its target. The model is DE's 16 directions (22.5°
// sectors — sk-research sk-06 adjudication, 2026-11-16).
// Run: node --test play/facing.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  DIR16_VECTORS, dir16FromVector, turnedDir16, faceForDir16,
  sheetRowForDir16,
} from './facing.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const facingJs = readFileSync(join(here, 'facing.js'), 'utf8');

test('the 16 sector centres quantize to their own dir', () => {
  for (let d = 0; d < 16; d++) {
    const a = (d * Math.PI) / 8;
    assert.equal(dir16FromVector(Math.cos(a), Math.sin(a)), d, `dir ${d}`);
    // Scale must not matter — only the angle does.
    assert.equal(dir16FromVector(7 * Math.cos(a), 7 * Math.sin(a)), d);
  }
});

test('axis and diagonal world vectors land on the documented dirs', () => {
  assert.equal(dir16FromVector(1, 0), 0); // +X
  assert.equal(dir16FromVector(1, 1), 2); // +X+Y, down-screen front
  assert.equal(dir16FromVector(0, 1), 4); // +Y
  assert.equal(dir16FromVector(-1, 1), 6);
  assert.equal(dir16FromVector(-1, 0), 8); // −X
  assert.equal(dir16FromVector(-1, -1), 10); // −X−Y, up-screen back
  assert.equal(dir16FromVector(0, -1), 12); // −Y
  assert.equal(dir16FromVector(1, -1), 14);
});

test('sector boundaries sit at the odd 11.25° steps', () => {
  const at = (deg) => dir16FromVector(
    Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180));
  assert.equal(at(11.24), 0);
  assert.equal(at(11.26), 1);
  assert.equal(at(33.74), 1);
  assert.equal(at(33.76), 2);
  assert.equal(at(-11.24), 0);
  assert.equal(at(-11.26), 15);
  assert.equal(at(180), 8);
  assert.equal(at(-180), 8);
});

test('a zero vector has no facing; turnedDir16 keeps the current one', () => {
  assert.equal(dir16FromVector(0, 0), -1);
  assert.equal(turnedDir16(6, 0, 0), 6);
  assert.equal(turnedDir16(6, 0, -5), 12); // straight −Y from anywhere
  assert.equal(turnedDir16(0, -2, 0), 8);
});

test('DIR16_VECTORS are the 16 unit vectors and round-trip', () => {
  assert.equal(DIR16_VECTORS.length, 16);
  for (let d = 0; d < 16; d++) {
    const [dx, dy] = DIR16_VECTORS[d];
    assert.ok(Math.abs(Math.hypot(dx, dy) - 1) < 1e-9, `dir ${d} unit`);
    assert.equal(dir16FromVector(dx, dy), d, `dir ${d} round-trips`);
  }
  assert.deepEqual(DIR16_VECTORS[0], [1, 0]);
  assert.equal(DIR16_VECTORS[8][0], -1);
  assert.ok(Math.abs(DIR16_VECTORS[8][1]) < 1e-9);
  assert.ok(Math.abs(DIR16_VECTORS[4][0]) < 1e-9);
  assert.equal(DIR16_VECTORS[4][1], 1);
});

test('faceForDir16: the side-view flip table for all 16 facings', () => {
  // Screen-x follows sign(dx − dy): right for +X-ish facings,
  // left for −X-ish, straight up/down screen at rows 2 and 10.
  assert.deepEqual(
    Array.from({ length: 16 }, (_, d) => faceForDir16(d)),
    [1, 1, 0, -1, -1, -1, -1, -1, -1, -1, 0, 1, 1, 1, 1, 1]);
  // The classic 8 facings (the even dirs) keep the old table.
  assert.deepEqual(
    [0, 2, 4, 6, 8, 10, 12, 14].map((d) => faceForDir16(d)),
    [1, 0, -1, -1, -1, 0, 1, 1]);
});

test('sheetRowForDir16: identity at 16 rows, pairs at 8, doubled at 32', () => {
  for (let d = 0; d < 16; d++) {
    assert.equal(sheetRowForDir16(d, 16), d);
    assert.equal(sheetRowForDir16(d, 8), Math.floor(d / 2));
    assert.equal(sheetRowForDir16(d, 32), d * 2);
    assert.equal(sheetRowForDir16(d, 1), 0);
    // The corpse-style back-conversion recovers the facing
    // exactly at the tiers that store it (16 and 32 rows); at
    // 8 rows it recovers the pair's even member — the stored
    // row never pretends to hold more than it does.
    for (const rows of [16, 32]) {
      const row = sheetRowForDir16(d, rows);
      assert.equal(Math.round((row * 16) / rows), d, `dir ${d} @ ${rows}`);
    }
    const row8 = sheetRowForDir16(d, 8);
    assert.equal(Math.round((row8 * 16) / 8), d - (d % 2), `dir ${d} @ 8`);
  }
});

test('app.js wires the facing model (import + shared quantization)', () => {
  assert.match(appJs, /from '\.\/play\/facing\.js'/);
  assert.match(appJs, /dir16FromVector/);
  assert.match(appJs, /sheetRowForDir16\(act\.dir, rows\)/);
  // No dir8 remnant survives anywhere in the client model or
  // its wiring, and no private atan2 quantization survives in
  // the pose paths: the old inline forms are gone from app.js.
  assert.ok(!appJs.includes('Dir8'));
  assert.ok(!facingJs.includes('Dir8'));
  assert.ok(!appJs.includes('Math.atan2(dy, dx) / (Math.PI / 4)'));
  assert.ok(!appJs.includes('Math.atan2(dy, dx) / ((Math.PI * 2) / dirs)'));
  // The dangling SPRITE-PIPELINE §12 citation (there is no §12)
  // is gone from both files; §10.5 is the real reference.
  assert.ok(!appJs.includes('SPRITE-PIPELINE §12'));
  assert.ok(!facingJs.includes('SPRITE-PIPELINE §12'));
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
