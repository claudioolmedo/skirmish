// Node tests for the click-marker mapping + animation math
// (play/clickmarker.js). Run: node --test play/clickmarker.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  ClickMarkers, markerForOrder, markerHeads,
  MARKER_MS, MARKER_MS_LOW, MARKER_CAP, MARKER_R0, MARKER_R1,
} from './clickmarker.js';

test('order-type -> marker mapping: ground yes, object never', () => {
  assert.equal(markerForOrder('ground'), 'ground');
  assert.equal(markerForOrder('object'), null);
  assert.equal(markerForOrder('build'), null);
  assert.equal(markerForOrder(''), null);
  assert.equal(markerForOrder(undefined), null);
});

test('object orders spawn nothing; ground orders spawn one marker', () => {
  const m = new ClickMarkers();
  assert.equal(m.add('object', 10, 10, 1000), null);
  assert.equal(m.active(1000).length, 0);
  assert.equal(m.add('ground', 12, 8, 1000), 'ground');
  const act = m.active(1100);
  assert.equal(act.length, 1);
  assert.equal(act[0].x, 12);
  assert.equal(act[0].y, 8);
  assert.ok(act[0].t > 0 && act[0].t < 1);
});

test('markers expire after their life; LOW tier life is shorter but nonzero', () => {
  const m = new ClickMarkers();
  m.add('ground', 0, 0, 0);
  assert.equal(m.active(MARKER_MS - 1).length, 1);
  assert.equal(m.active(MARKER_MS).length, 0);
  m.add('ground', 0, 0, 0);
  assert.equal(m.active(MARKER_MS_LOW - 1, true).length, 1);
  assert.equal(m.active(MARKER_MS_LOW, true).length, 0);
  assert.ok(MARKER_MS_LOW > 0, 'LOW may shorten, never remove');
});

test('concurrent markers are capped, newest kept', () => {
  const m = new ClickMarkers();
  for (let i = 0; i < MARKER_CAP + 6; i++) m.add('ground', i, 0, 1000);
  const act = m.active(1000);
  assert.equal(act.length, MARKER_CAP);
  assert.equal(act[0].x, 6, 'oldest six dropped');
  assert.equal(act[act.length - 1].x, MARKER_CAP + 5);
});

test('heads converge inward and fade only at the end', () => {
  const start = markerHeads(0);
  assert.equal(start.heads.length, 4);
  assert.ok(Math.abs(start.r - MARKER_R0) < 1e-9);
  assert.equal(start.alpha, 1);
  const mid = markerHeads(0.35);
  assert.ok(mid.r < start.r && mid.r > MARKER_R1);
  const met = markerHeads(0.7);
  assert.ok(Math.abs(met.r - MARKER_R1) < 1e-9, 'heads meet at 70%');
  assert.equal(met.alpha, 1);
  const late = markerHeads(0.9);
  assert.ok(late.alpha < 1 && late.alpha > 0, 'fading in the last 30%');
  assert.ok(Math.abs(late.r - MARKER_R1) < 1e-9, 'met heads stay met');
  // Heads sit on the four diagonals, each facing the centre.
  for (const h of start.heads) {
    assert.ok(Math.abs(Math.abs(h.dx) - Math.abs(h.dy)) < 1e-9);
    assert.ok(Math.abs(Math.hypot(h.dx, h.dy) - MARKER_R0) < 1e-9);
  }
});
