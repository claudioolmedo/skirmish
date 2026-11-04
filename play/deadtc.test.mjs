// Dead-Town-Center panel regression pins (bot-loop, 2026-11-03).
//
// Defect: a Town Center selected when it was razed kept its panel
// forever. Every button (Train Villager, age-ups, Town Bell,
// Ungarrison) rendered greyed with the sim's generic dead-TC
// refusal, "The training queue is full", and the Ungarrison button
// reported "nobody is garrisoned inside" (the death sweep had
// ejected the garrison unharmed, GAR-03). A live session
// (2026-11-03, 14:22 run addendum 2) read that zombie panel as a
// Town Bell soft-lock: the match was not over (living units
// remained, so no defeat screen was owed under VIC-01) and
// nothing ever replaced the dead panel.
//
// Fix: updateSelPanel drops a 'tc' selection whose entity is dead
// (hp < 1) or gone, mirroring the dead-animal bookkeeping directly
// below it. These pins keep that guard in place and ahead of the
// panel reads.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function selPanelHead() {
  const start = appJs.indexOf('function updateSelPanel() {');
  assert.ok(start >= 0, 'updateSelPanel exists in app.js');
  const end = appJs.indexOf('if (selectedAnimal != null) {', start);
  assert.ok(end > start, 'the animal bookkeeping follows the TC guard');
  return appJs.slice(start, end);
}

test('a dead selected Town Center drops the selection', () => {
  const head = selPanelHead();
  assert.ok(
    head.includes("selectedBuilding.type === 'tc'"),
    'the guard is scoped to Town Center selections');
  assert.ok(
    head.includes('stc.hp[tcId * 2] < 1'),
    'the guard reads the TC entity HP from the entity state');
  assert.ok(
    head.includes('selectedBuilding = null;'),
    'a dead TC clears the building selection');
});

test('the dead-TC guard runs before the animal bookkeeping', () => {
  const start = appJs.indexOf('function updateSelPanel() {');
  const guard = appJs.indexOf("selectedBuilding.type === 'tc'", start);
  const animal = appJs.indexOf('if (selectedAnimal != null) {', start);
  assert.ok(guard > start && animal > guard,
    'the TC guard precedes the animal block inside updateSelPanel');
});
