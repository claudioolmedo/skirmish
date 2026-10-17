// Headless tests for groups.js (control groups, tap tracker, idle
// cycling). Run: node --test play/groups.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createControlGroups, createTapTracker, nextInCycle, GROUP_COUNT } from './groups.js';

test('assign stores members and replaces on re-assign', () => {
  const g = createControlGroups();
  g.assign(1, [3, 5, 7]);
  assert.deepEqual(g.members(1), [3, 5, 7]);
  g.assign(1, [9]);
  assert.deepEqual(g.members(1), [9], 're-assign must replace, not merge');
  assert.deepEqual(g.members(2), [], 'untouched groups stay empty');
});

test('assign deduplicates and preserves order', () => {
  const g = createControlGroups();
  g.assign(3, [5, 2, 5, 8, 2]);
  assert.deepEqual(g.members(3), [5, 2, 8]);
});

test('addToGroup merges without duplicates', () => {
  const g = createControlGroups();
  g.assign(1, [1, 2]);
  g.addToGroup(1, [2, 3]);
  assert.deepEqual(g.members(1), [1, 2, 3]);
  g.addToGroup(4, [7]);
  assert.deepEqual(g.members(4), [7], 'add to a fresh group creates it');
});

test('recall filters out the dead at recall time', () => {
  const g = createControlGroups();
  g.assign(1, [1, 2, 3, 4]);
  const alive = new Set([2, 4]);
  assert.deepEqual(g.recall(1, (id) => alive.has(id)), [2, 4]);
  // The store itself is untouched — filtering is per-recall.
  assert.deepEqual(g.members(1), [1, 2, 3, 4]);
});

test('clear forgets every group', () => {
  const g = createControlGroups();
  g.assign(1, [1]);
  g.assign(9, [2]);
  g.clear();
  assert.deepEqual(g.members(1), []);
  assert.deepEqual(g.members(9), []);
});

test('group numbers are validated', () => {
  const g = createControlGroups();
  assert.throws(() => g.assign(0, []));
  assert.throws(() => g.assign(GROUP_COUNT + 1, []));
  assert.throws(() => g.members(2.5));
});

test('tap tracker: second rapid tap centers, slow tap selects', () => {
  const t = createTapTracker(350);
  assert.equal(t.tap(1, 1000), 'select');
  assert.equal(t.tap(1, 1200), 'center', 'second tap within the window');
  assert.equal(t.tap(1, 1400), 'center', 'third rapid tap centers again');
  assert.equal(t.tap(1, 5000), 'select', 'a late tap is a fresh select');
  assert.equal(t.tap(2, 5100), 'select', 'a different group never centers');
  t.reset();
  assert.equal(t.tap(2, 5200), 'select', 'reset clears the tap memory');
});

test('nextInCycle walks the list and wraps', () => {
  const ids = [2, 5, 9];
  assert.equal(nextInCycle(ids, -1), 2, 'starts at the head');
  assert.equal(nextInCycle(ids, 2), 5);
  assert.equal(nextInCycle(ids, 5), 9);
  assert.equal(nextInCycle(ids, 9), 2, 'wraps to the head');
  assert.equal(nextInCycle(ids, 4), 5, 'a stale lastId resumes at the next greater id');
  assert.equal(nextInCycle(ids, 100), 2, 'a lastId past the end wraps');
  assert.equal(nextInCycle([], -1), null, 'empty list has no next');
});
