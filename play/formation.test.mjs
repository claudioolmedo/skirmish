// MOV-07 client module tests: the formation choice store
// and the wire vocabulary (play/formation.js). Headless —
// run with the other node suites: node --test or the repo's
// test runner over web-preview/play/*.test.mjs.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  FORMATIONS,
  createFormationChoice,
  formationApplies,
  formationIdForTag,
  formationTagForId,
} from './formation.js';

test('the vocabulary is the four documented formations in panel order', () => {
  assert.deepEqual(FORMATIONS.map((f) => f.id), ['line', 'box', 'staggered', 'flank']);
  assert.deepEqual(FORMATIONS.map((f) => f.tag), [0, 1, 2, 3]);
  for (const f of FORMATIONS) {
    assert.equal(formationIdForTag(f.tag), f.id);
    assert.equal(formationTagForId(f.id), f.tag);
  }
  assert.equal(formationIdForTag(4), null);
  assert.equal(formationTagForId('column'), null);
});

test('the choice starts empty and toggles like a setting face', () => {
  const c = createFormationChoice();
  assert.equal(c.current(), null);
  assert.equal(c.tag(), null);
  assert.equal(c.choose('box'), 'box');
  assert.equal(c.current(), 'box');
  assert.equal(c.tag(), 1);
  // A different formation replaces; the same one clears.
  assert.equal(c.choose('flank'), 'flank');
  assert.equal(c.choose('flank'), null);
  assert.equal(c.tag(), null);
  assert.throws(() => c.choose('column'), /unknown formation/);
  c.choose('line');
  c.clear();
  assert.equal(c.current(), null);
});

test('a formation names orders only for groups of 2+', () => {
  const c = createFormationChoice();
  assert.equal(formationApplies(c, 4), false, 'no choice, no formation');
  c.choose('line');
  assert.equal(formationApplies(c, 1), false, 'a lone unit stays a plain move');
  assert.equal(formationApplies(c, 2), true);
  assert.equal(formationApplies(c, 60), true);
});
