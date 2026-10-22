// Node tests for the waypoint-pennant mapping + shape
// (play/pennants.js). Run: node --test play/pennants.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  routePennants, leaderRoute, pennantsForSelection, pennantShape,
  PENNANT_POLE, PENNANT_W, PENNANT_H,
} from './pennants.js';

test('route -> pennants: flat pairs map in order, with indices', () => {
  const p = routePennants([10, 20, 30, 40, -5, 7]);
  assert.equal(p.length, 3);
  assert.deepEqual(p[0], { x: 10, y: 20, index: 0 });
  assert.deepEqual(p[1], { x: 30, y: 40, index: 1 });
  assert.deepEqual(p[2], { x: -5, y: 7, index: 2 });
});

test('route -> pennants: empty and malformed routes draw nothing', () => {
  assert.deepEqual(routePennants([]), []);
  assert.deepEqual(routePennants(null), []);
  assert.deepEqual(routePennants(undefined), []);
  // A trailing unpaired value is dropped, never a NaN pennant.
  assert.deepEqual(routePennants([3, 4, 9]), [{ x: 3, y: 4, index: 0 }]);
});

test('leader rule: first non-empty route wins, in selection order', () => {
  const a = [0, 0, 4, 0];
  const b = [9, 9];
  assert.deepEqual(leaderRoute([a, b]), a);
  assert.deepEqual(leaderRoute([[], b]), b);
  assert.deepEqual(leaderRoute([[], []]), []);
  assert.deepEqual(leaderRoute([]), []);
  assert.deepEqual(leaderRoute(null), []);
});

test('selection -> pennants: a group order draws one chain, not one per unit', () => {
  // Three selected units carrying the same queued chain (the DE
  // group-order case): the leader's markers only, no stacking.
  const chain = [10, 10, 20, 10, 20, 20];
  const p = pennantsForSelection([chain, chain, chain]);
  assert.equal(p.length, 3);
  assert.deepEqual(p.map((q) => [q.x, q.y]), [[10, 10], [20, 10], [20, 20]]);
  // Nothing selected / no chains: nothing drawn.
  assert.deepEqual(pennantsForSelection([]), []);
  assert.deepEqual(pennantsForSelection([[], []]), []);
});

test('pennant shape: pole rises from the point, pennant flies at its top', () => {
  const s = pennantShape();
  // Pole top is straight above the plant point (screen y down).
  assert.deepEqual(s.poleTop, [0, -PENNANT_POLE]);
  // The triangle: tip out at +x, vertically centred on the
  // pennant band; lower corner back at the pole.
  assert.deepEqual(s.tip, [PENNANT_W, -PENNANT_POLE + PENNANT_H / 2]);
  assert.deepEqual(s.low, [0, -PENNANT_POLE + PENNANT_H]);
  // Proportions: taller than the rally flag's 2.4 wu pole, and
  // the pennant itself is wider than it is tall.
  assert.ok(PENNANT_POLE > 2.4);
  assert.ok(PENNANT_W > PENNANT_H && PENNANT_H > 0);
});
