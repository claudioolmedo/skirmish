// Headless tests for alerts.js — the located attack alert
// (INF-01): which own thing lost HP and where, the minimap
// flash lifetime/blink, and the located-alert rate limit.
// Run: node --test play/alerts.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  ALERT_FLASH_MS, ALERT_BLINK_PERIOD_MS, ALERT_BLINK_ON_MS,
  ALERT_MIN_INTERVAL_MS, firstHpDrop, nearestHpDrop,
  alertFresh, alertFlashOn, alertDue,
} from './alerts.js';

const meta = new Map([
  ['e3', { x: 10, y: 20 }],
  ['e7', { x: -30, y: 5 }],
  ['b1', { x: 40, y: -12 }],
]);

test('firstHpDrop finds the first own loss with a position', () => {
  const prev = new Map([['e3', 25], ['e7', 7], ['b1', 1]]);
  const next = new Map([['e3', 25], ['e7', 5], ['b1', 0.8]]);
  // e7 (iteration order) dropped before b1; e3 unchanged.
  assert.deepEqual(firstHpDrop(prev, next, meta), { key: 'e7', x: -30, y: 5 });
});

test('firstHpDrop: no drop, births and unknown positions are silent', () => {
  const prev = new Map([['e3', 25]]);
  assert.equal(firstHpDrop(prev, new Map([['e3', 25]]), meta), null);
  assert.equal(firstHpDrop(prev, new Map([['e3', 30]]), meta), null); // healed
  // A key absent from prev is a birth, not a drop.
  assert.equal(firstHpDrop(prev, new Map([['e3', 25], ['e9', 1]]), meta), null);
  // A drop with no recorded position cannot be located.
  assert.equal(firstHpDrop(new Map([['zz', 9]]), new Map([['zz', 1]]), meta), null);
  assert.equal(firstHpDrop(null, new Map(), meta), null);
});

test('nearestHpDrop picks the drop closest to the camera center', () => {
  const prev = new Map([['e3', 25], ['e7', 7], ['b1', 1]]);
  const next = new Map([['e3', 20], ['e7', 5], ['b1', 0.5]]);
  const hit = nearestHpDrop(prev, next, meta, 12, 18);
  assert.equal(hit.key, 'e3');
  assert.ok(Math.abs(hit.dist - Math.hypot(2, 2)) < 1e-9);
  assert.equal(nearestHpDrop(prev, prev, meta, 0, 0), null);
});

test('the flash lives ALERT_FLASH_MS and blinks on its cadence', () => {
  const alert = { x: 1, y: 2, at: 1000 };
  assert.equal(alertFresh(alert, 999), false);
  assert.equal(alertFresh(alert, 1000), true);
  assert.equal(alertFresh(alert, 1000 + ALERT_FLASH_MS - 1), true);
  assert.equal(alertFresh(alert, 1000 + ALERT_FLASH_MS), false);
  assert.equal(alertFresh(null, 1000), false);
  // Lit at the start of each period, dark in the tail.
  assert.equal(alertFlashOn(alert, 1000), true);
  assert.equal(alertFlashOn(alert, 1000 + ALERT_BLINK_ON_MS - 1), true);
  assert.equal(alertFlashOn(alert, 1000 + ALERT_BLINK_ON_MS), false);
  assert.equal(alertFlashOn(alert, 1000 + ALERT_BLINK_PERIOD_MS), true);
  assert.equal(alertFlashOn(alert, 1000 + ALERT_FLASH_MS), false);
});

test('located alerts are rate-limited on the under-attack clock', () => {
  const alert = { x: 0, y: 0, at: 5000 };
  assert.equal(alertDue(null, 0), true);
  assert.equal(alertDue(alert, 5000 + ALERT_MIN_INTERVAL_MS - 1), false);
  assert.equal(alertDue(alert, 5000 + ALERT_MIN_INTERVAL_MS), true);
  // The located-alert clock IS the cue's clock (audio.js).
  assert.equal(ALERT_MIN_INTERVAL_MS, 9000);
});
