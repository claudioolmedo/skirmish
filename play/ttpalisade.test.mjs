// Palisade Wall missing from the Tech Tree — regression pins
// (bot-loop live sessions, runs #405–#422, most recently #422).
//
// Defect: reading the ENTIRE Tech Tree across all 4 ages on the
// real client found NO Palisade Wall entry anywhere; the only
// wall entry was "Fortified Wall" (the Castle Age / University
// research, TECH_TREE tag 24). In DE the Palisade Wall is a
// Dark Age buildable (villagers build it from the Dark Age;
// BLD-13: 3 wood per segment, 7 s build) and it appears in the
// Tech Tree as a Dark Age building.
//
// Root cause: play/techtree.js's TECH_TREE is a mirror of
// sim/src/tech.rs — TECHNOLOGIES only, pinned one-entry-per-
// TechId by techtree.test.mjs. The Palisade Wall is not a
// technology: it is BuildingKind tag 9, so it could never appear
// in that table, and app.js's buildTechTree() rendered nothing
// but TECH_TREE. No filter hid it — the tree's data model had
// no building entries at all.
//
// Fix (presentation only; no sim, cost or balance change): a
// parallel BUILDING_TREE table in play/techtree.js, in the same
// data shape (age + [food, wood, gold, stone] cost + secs),
// rendered by buildTechTree() as a stand-alone node in its Age
// column (buildings have no host building — villagers build
// them) and painted by refreshTechTree() from BUILD_NAME /
// the same cost line format as the tech nodes. The Palisade
// entry records leadsTo: 24 (Fortified Wall) as data — the
// tree draws no connecting lines, so the entry itself is the
// plain correct Dark Age entry the DE layout calls for.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import * as techtree from './techtree.js';
import { STRINGS } from './i18n.js';

const { TECH_TREE, BUILDING_TREE, buildingState } = techtree;
const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');

test('BUILDING_TREE carries the Palisade Wall as a Dark Age building', () => {
  assert.ok(Array.isArray(BUILDING_TREE), 'BUILDING_TREE is exported from techtree.js');
  const palisades = BUILDING_TREE.filter((e) => e.kind === 9);
  assert.equal(palisades.length, 1, 'exactly one Palisade Wall entry (BuildingKind 9)');
  const p = palisades[0];
  assert.equal(p.age, 0, 'Dark Age (BLD-13: buildable from the Dark Age)');
  assert.deepEqual(p.cost, [0, 3, 0, 0], '3 wood per segment, as the live build panel shows');
  assert.equal(p.secs, 7, '7 s build time (BLD-13, the current DE row)');
});

test('the Palisade entry points at the Fortified Wall research, which stays put', () => {
  const p = BUILDING_TREE.find((e) => e.kind === 9);
  assert.equal(p.leadsTo, 24, 'the wall line leads to Fortified Wall (tag 24)');
  const fw = TECH_TREE.find((e) => e.tag === 24);
  assert.ok(fw, 'Fortified Wall still in TECH_TREE');
  assert.equal(fw.host, 16, 'Fortified Wall is hosted by the University');
  assert.equal(fw.age, 2, 'Fortified Wall is a Castle Age research');
  assert.deepEqual(fw.cost, [200, 100, 0, 0], 'Fortified Wall cost untouched');
});

test('the tech mirror is not polluted: TECH_TREE stays the 58 technologies', () => {
  // techtree.test.mjs pins TECH_TREE one-entry-per-TechId
  // against sim/src/tech.rs; the building entry must live in
  // its own table, never as a fake technology.
  assert.equal(TECH_TREE.length, 58);
  assert.ok(!TECH_TREE.some((e) => e.kind === 9), 'no building entry inside TECH_TREE');
});

test('buildingState: the Palisade Wall is available from the Dark Age', () => {
  assert.equal(typeof buildingState, 'function', 'buildingState helper exported');
  const p = BUILDING_TREE.find((e) => e.kind === 9);
  assert.equal(buildingState(p, 0), 'available', 'buildable in the Dark Age');
  assert.equal(buildingState(p, 3), 'available', 'still buildable in Imperial');
  assert.equal(buildingState({ age: 2 }, 1), 'locked', 'a later-Age building stays locked before its Age');
});

test('app.js renders and refreshes the building entry in the Tech Tree', () => {
  assert.ok(
    app.includes("BUILDING_TREE") && app.includes("from './play/techtree.js'"),
    'app.js imports BUILDING_TREE from play/techtree.js',
  );
  assert.ok(app.includes('tt-bld-${e.kind}'), 'buildTechTree creates a tt-bld-<kind> node');
  assert.ok(app.includes('BUILD_NAME[e.kind]'), 'refreshTechTree names the node from BUILD_NAME (i18n building name)');
  assert.ok(app.includes('buildingState(e, age)'), 'refreshTechTree paints the node state from the player Age');
});

test('the displayed name is the in-game building name in every locale', () => {
  assert.equal(STRINGS.en['building.palisade'], 'Palisade Wall');
  for (const loc of ['pt-BR', 'es']) {
    assert.ok(STRINGS[loc]['building.palisade']?.length > 0, `${loc}: building.palisade present`);
  }
});
