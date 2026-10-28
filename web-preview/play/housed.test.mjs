// HOUSED front regression pins (bot-loop, 2026-10-27).
//
// Claudio's live report of 2026-10-24 ("criar militares nos
// prédios militares não funcionou, mesmo tendo recursos") traced
// to this: with the population at the cap, the client let the
// player ENQUEUE a unit whose training then stalled at 0% behind
// the "Housed" message — resources were charged for a unit that
// could not be produced. His decision of 2026-10-27 (HOUSED
// front): at the cap, training is REFUSED — the train button is
// blocked and the Housed warning shows; nothing is queued and
// nothing is charged. The sim-side gate itself (can_train_at /
// can_train_villager_at on population_of < population_cap_of)
// is pinned in sim/src/housed_tests.rs; these pins keep the
// client half wired:
//
//  - every train button (TC villager, Militia, Spearman, Archer,
//    Skirmisher, Scout) is disabled when its train block is 3
//    (housed), exactly like block 2 (queue full) already was;
//  - the housed wording in every locale says training is
//    REFUSED / no room to train more — never that a queued unit
//    "stalls", which was the misleading half of the old text.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');
const simRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'lib.rs'), 'utf8');

test('the TC train button is blocked while housed', () => {
  assert.ok(
    appJs.includes('disabled: reason === 2 || reason === 3'),
    'Train Villager must be disabled on train block 3 (housed), not only block 2');
});

test('every production-building train button is blocked while housed', () => {
  for (const v of ['blockM', 'blockSp', 'blockA', 'blockSk', 'blockS']) {
    assert.ok(
      appJs.includes(`|| ${v} === 2 || ${v} === 3`),
      `${v}: the train button must be disabled on train block 3 (housed)`);
  }
});

test('the sim refuses the enqueue at the cap on both train paths', () => {
  const gates = simRs.match(/population_of\((?:b\.owner|owner)\) < self\.population_cap_of\((?:b\.owner|owner)\)/g) || [];
  assert.equal(
    gates.length, 2,
    'can_train_at and can_train_villager_at must both gate on population room');
});

test('housed wording says refused, never stalls, in every locale', () => {
  for (const key of ['block.housedLong', 'sel.housedSuffix', 'sel.housedLine']) {
    const hits = i18nJs.match(new RegExp(`'${key.replace('.', '\\.')}':`, 'g')) || [];
    assert.equal(hits.length, 3, `${key} is defined for EN, PT and ES`);
  }
  for (const stale of [
    'training cannot finish until a House stands',
    'training stalls until a House stands',
    'o treinamento não termina até haver uma Casa',
    'o treinamento para até haver uma Casa',
    'el entrenamiento no puede terminar hasta que haya una Casa',
    'el entrenamiento se detiene hasta que haya una Casa',
  ]) {
    assert.ok(!i18nJs.includes(stale), `stale housed wording still shipped: ${stale}`);
    assert.ok(!appJs.includes(stale), `stale housed wording still in app.js: ${stale}`);
  }
});
