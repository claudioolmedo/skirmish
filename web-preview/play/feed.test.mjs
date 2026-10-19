// Headless tests for feed.js (the event notification feed's
// diff -> message mapping). Run: node --test play/feed.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { diffFeedEvents } from './feed.js';

const NAMES = {
  buildingName: (k) => ['House', 'Farm', 'Barracks', 'Lumber Camp', 'Mill',
    'Mining Camp', 'Archery Range', 'Stable', 'Blacksmith'][k],
  unitName: (k) => ({ 0: 'Villager', 2: 'Scout Cavalry', 3: 'Militia', 4: 'Archer', 8: 'Man-at-Arms' })[k],
};
const snap = (over = {}) => ({
  built: {}, units: {}, techs: {}, age: 0, sheep: 0, ...over,
});

test('match start (null prev) is not news', () => {
  assert.deepEqual(diffFeedEvents(null, snap({ built: { 1: 0 }, units: { 0: 3 } }), NAMES), []);
  assert.deepEqual(diffFeedEvents(snap(), null, NAMES), []);
});

test('a building completing reports "<Building> Built" once', () => {
  const prev = snap({ built: { 10: 0 } });
  const next = snap({ built: { 10: 0, 11: 3, 12: 2 } });
  assert.deepEqual(diffFeedEvents(prev, next, NAMES), ['Lumber Camp Built', 'Barracks Built']);
  // No repeat while it simply stands there.
  assert.deepEqual(diffFeedEvents(next, next, NAMES), []);
});

test('trained units report "<Unit> Created" per completion', () => {
  const prev = snap({ units: { 0: 3, 3: 2 } });
  const next = snap({ units: { 0: 4, 3: 2, 4: 1 } });
  assert.deepEqual(diffFeedEvents(prev, next, NAMES), ['Villager Created', 'Archer Created']);
  // Two finishing between snapshots: two lines.
  assert.deepEqual(
    diffFeedEvents(snap({ units: { 3: 0 } }), snap({ units: { 3: 2 } }), NAMES),
    ['Militia Created', 'Militia Created']);
  // Deaths are not events.
  assert.deepEqual(diffFeedEvents(next, prev, NAMES), []);
});

test('research completions report "<Tech> Research Complete"', () => {
  const prev = snap({ techs: { loom: false, fletching: false } });
  const next = snap({ techs: { loom: true, fletching: true } });
  assert.deepEqual(diffFeedEvents(prev, next, NAMES),
    ['Loom Research Complete', 'Fletching Research Complete']);
  assert.deepEqual(diffFeedEvents(next, next, NAMES), [], 'no repeat');
});

test('the Feudal advancement reports as the study observed it', () => {
  assert.deepEqual(diffFeedEvents(snap({ age: 0 }), snap({ age: 1 }), NAMES),
    ['Feudal Age Research Complete']);
});

test('sheep becoming ours reports "Sheep Found"; losing them does not', () => {
  assert.deepEqual(diffFeedEvents(snap({ sheep: 0 }), snap({ sheep: 1 }), NAMES), ['Sheep Found']);
  assert.deepEqual(diffFeedEvents(snap({ sheep: 4 }), snap({ sheep: 3 }), NAMES), []);
});

test('a combined batch keeps a sane order: built, created, age, tech, sheep', () => {
  const prev = snap();
  const next = snap({
    built: { 9: 0 }, units: { 0: 1 }, age: 1,
    techs: { loom: true }, sheep: 2,
  });
  assert.deepEqual(diffFeedEvents(prev, next, NAMES), [
    'House Built', 'Villager Created', 'Feudal Age Research Complete',
    'Loom Research Complete', 'Sheep Found', 'Sheep Found',
  ]);
});
