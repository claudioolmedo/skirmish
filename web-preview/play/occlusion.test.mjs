// Node tests for the occluded-unit outline decision
// (play/occlusion.js, INF-12). Run: node --test play/occlusion.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  rectsIntersect, intersectionRect, coveredFraction, occlusionFor,
  OCCLUDE_MIN_FRACTION,
} from './occlusion.js';

const R = (x, y, w, h) => ({ x, y, w, h });

test('rectsIntersect / intersectionRect: overlap geometry', () => {
  assert.equal(rectsIntersect(R(0, 0, 10, 10), R(5, 5, 10, 10)), true);
  assert.equal(rectsIntersect(R(0, 0, 10, 10), R(10, 0, 5, 5)), false); // touching edge only
  assert.equal(rectsIntersect(R(0, 0, 10, 10), R(30, 30, 5, 5)), false);
  assert.deepEqual(intersectionRect(R(0, 0, 10, 10), R(5, 5, 10, 10)),
    R(5, 5, 5, 5));
  assert.equal(intersectionRect(R(0, 0, 10, 10), R(20, 0, 5, 5)), null);
  assert.equal(intersectionRect(null, R(0, 0, 1, 1)), null);
});

test('coveredFraction: none, partial, full', () => {
  const u = R(0, 0, 100, 100);
  assert.equal(coveredFraction(u, []), 0);
  assert.equal(coveredFraction(u, [R(200, 200, 10, 10)]), 0);
  assert.equal(coveredFraction(u, [R(0, 0, 100, 100)]), 1);
  assert.equal(coveredFraction(u, [R(0, 0, 50, 100)]), 0.5);
  assert.equal(coveredFraction(u, [R(50, 50, 100, 100)]), 0.25);
  // A cover rect larger than the unit is clipped to the unit.
  assert.equal(coveredFraction(u, [R(-50, -50, 200, 200)]), 1);
});

test('coveredFraction: union never double-counts overlaps', () => {
  const u = R(0, 0, 100, 100);
  // Two halves that overlap each other by 20px in the middle.
  const f = coveredFraction(u, [R(0, 0, 60, 100), R(40, 0, 60, 100)]);
  assert.equal(f, 1);
  // Stacked partial covers in disjoint slabs.
  const g = coveredFraction(u, [R(0, 0, 50, 50), R(50, 50, 50, 50)]);
  assert.equal(g, 0.5);
});

test('coveredFraction: degenerate inputs are safe', () => {
  assert.equal(coveredFraction(null, [R(0, 0, 9, 9)]), 0);
  assert.equal(coveredFraction(R(0, 0, 0, 10), [R(0, 0, 9, 9)]), 0);
  assert.equal(coveredFraction(R(0, 0, 10, 10), null), 0);
  assert.equal(coveredFraction(R(0, 0, 10, 10), [null, R(0, 0, 0, 0)]), 0);
});

test('occlusionFor: the fog gate is absolute', () => {
  const unit = { rect: R(0, 0, 40, 60), depth: 10, fogVisible: false };
  const tc = { rect: R(-20, -40, 120, 120), depth: 18 };
  const r = occlusionFor(unit, [tc]);
  assert.equal(r.occluded, false);
  assert.equal(r.fraction, 0);
  assert.deepEqual(r.cover, []);
  // Explored memory is not visibility: only literal true passes.
  const r2 = occlusionFor({ ...unit, fogVisible: 1 }, [tc]);
  assert.equal(r2.occluded, false);
});

test('occlusionFor: enemy scout fully behind the Town Center (Claudio’s case)', () => {
  // Enemy scout (small rect) standing on the far side of the
  // local TC: the TC's rect swallows it, the TC's painter depth
  // (footprint key = wx + wy + TC_HALF) is strictly greater, and
  // the scout's cell is in current fog visibility (it stands
  // beside OUR TC, inside our sight) -> the outline must show.
  const scout = { rect: R(100, 100, 24, 30), depth: 50, fogVisible: true };
  const tc = { rect: R(60, 40, 110, 100), depth: 58 };
  const r = occlusionFor(scout, [tc]);
  assert.equal(r.occluded, true);
  assert.equal(r.fraction, 1);
  assert.equal(r.cover.length, 1);
  assert.equal(r.cover[0], tc);
});

test('occlusionFor: depth gate — occluders behind or level never count', () => {
  const unit = { rect: R(0, 0, 40, 60), depth: 10, fogVisible: true };
  const behind = { rect: R(0, 0, 40, 60), depth: 9 };
  const level = { rect: R(0, 0, 40, 60), depth: 10 };
  assert.equal(occlusionFor(unit, [behind]).occluded, false);
  assert.equal(occlusionFor(unit, [level]).occluded, false);
  const front = { rect: R(0, 0, 40, 60), depth: 11 };
  assert.equal(occlusionFor(unit, [behind, level, front]).occluded, true);
});

test('occlusionFor: threshold — a graze does not outline, real cover does', () => {
  const unit = { rect: R(0, 0, 100, 100), depth: 1, fogVisible: true };
  // 10% covered: below OCCLUDE_MIN_FRACTION.
  const graze = { rect: R(90, 0, 100, 100), depth: 2 };
  const g = occlusionFor(unit, [graze]);
  assert.equal(g.fraction, 0.1);
  assert.equal(g.occluded, false);
  // Two front occluders whose union crosses the threshold.
  const a = { rect: R(0, 0, 10, 100), depth: 2 };
  const b = { rect: R(90, 0, 10, 100), depth: 3 };
  const both = occlusionFor(unit, [a, b]);
  assert.equal(both.fraction, 0.2);
  assert.equal(both.occluded, true);
  assert.equal(both.cover.length, 2);
  assert.ok(OCCLUDE_MIN_FRACTION > 0 && OCCLUDE_MIN_FRACTION < 0.5);
});

test('occlusionFor: malformed inputs never outline', () => {
  assert.equal(occlusionFor(null, []).occluded, false);
  assert.equal(occlusionFor({ rect: null, depth: 1, fogVisible: true }, []).occluded, false);
  assert.equal(occlusionFor({ rect: R(0, 0, 0, 5), depth: 1, fogVisible: true },
    [{ rect: R(0, 0, 9, 9), depth: 2 }]).occluded, false);
  const u = { rect: R(0, 0, 10, 10), depth: 1, fogVisible: true };
  assert.equal(occlusionFor(u, null).occluded, false);
  assert.equal(occlusionFor(u, [null, { rect: null, depth: 9 }]).occluded, false);
});
