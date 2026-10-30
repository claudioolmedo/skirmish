// Research-button lock regression pins (bot-loop defect,
// 2026-10-29; live formal of session f02a38aa).
//
// Defect: on the Feudal Stable panel, the Castle-Age research
// buttons Light Cavalry and Husbandry did NOT carry the panel's
// disabled flag — they looked enabled, and a click only flashed
// "Requirements not met yet." and queued nothing — while the
// Knight train button beside them was properly disabled with
// its reason. Root cause: every research button's disabled
// expression covered the sim's ResearchBlock 2 (queue full) and
// 3 (already done) but not 4 (Locked: age / prerequisite unmet),
// which is exactly the block the age gate produces. Train
// buttons get their age block from the sim's train block (2),
// so only research buttons showed the gap.
//
// Panel law pinned here: research buttons disable on the HARD
// blocks 2, 3 and 4; the SOFT block 1 (funds) never disables —
// it stays clickable and flashes its reason, exactly like the
// train buttons' no-food block (see the HOUSED comment on the
// Town Center train button). Exception: the two age-advance
// buttons stay active on block 4 by design (their refusal
// explains the building-count requirement; hotkeys smoke
// case 3) — pinned below so nobody "fixes" them by accident.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

// Every research button's disabled expression, by its block source.
// The TC set is Loom (rbTc[0]), Wheelbarrow (rbTc[2]) and Hand Cart
// (rbTc[4]) — the age-advance buttons (rbTc[1] Feudal, rbTc[3]
// Castle) are the deliberate exception pinned separately below.
const TC = [0, 2, 4].map((i) => `rbTc[${i}]`);
const BUILDING = [5, 9, 16, 17].map((t) => `rbB[${t}]`);

test('every TC research button disables on the Locked block (4)', () => {
  for (const src of TC) {
    assert.ok(
      appJs.includes(`${src} === 2 || ${src} === 3 || ${src} === 4`),
      `${src} disables on blocks 2, 3 and 4`,
    );
  }
});

test('the age-advance buttons stay ACTIVE on the Locked block (4)', () => {
  // Deliberate, pre-existing design: the age lock is a building
  // count, the button's title switches to the gate text and its
  // refusal flash explains the requirement (cmd.feudalNeeds /
  // cmd.castleNeeds) — pinned end-to-end by the hotkeys smoke,
  // case (3). Greying these two would break that case.
  assert.ok(
    appJs.includes('disabled: feudalDone || rbTc[1] === 2 || rbTc[1] === 3,'),
    'the Feudal button disables only on blocks 2 and 3',
  );
  assert.ok(
    appJs.includes('disabled: rbTc[3] === 2 || rbTc[3] === 3,'),
    'the Castle button disables only on blocks 2 and 3',
  );
});

test('every building research button disables on the Locked block (4)', () => {
  for (const src of BUILDING) {
    assert.ok(
      appJs.includes(`${src} === 2 || ${src} === 3 || ${src} === 4`),
      `${src} disables on blocks 2, 3 and 4`,
    );
  }
});

test('the shared research-button helpers disable on the Locked block (4)', () => {
  const occurrences = appJs.split('rb === 2 || rb === 3 || rb === 4').length - 1;
  assert.equal(occurrences, 2, 'blacksmith techBtn + economy campBtn both cover block 4');
});

test('no research button disables on the funds block (1) — it flashes on click', () => {
  for (const src of [...TC, ...BUILDING]) {
    assert.ok(!appJs.includes(`${src} === 1`), `${src} never disables on funds`);
  }
  assert.ok(!appJs.includes('rb === 1'), 'helpers never disable on funds');
});
