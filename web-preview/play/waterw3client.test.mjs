// Water W3a CLIENT SLICE wiring pins (CLS-07, Feudal combat
// ships) — run from web-preview/play/:
//   node --test waterw3client.test.mjs
//
// The W3a sim half landed the four Feudal combat ships (Galley
// 24, Fire Galley 25, Demolition Raft 26, Hulk 27) with their
// blast/spray profiles and Blacksmith/Ballistics patch rows.
// This suite pins the client half end to end, the way
// waterw2client.test.mjs pinned W2's: the WASM train exports,
// both dispatch paths (local + netplay), the Dock panel's four
// new train buttons and their train blocks, the kind render
// tables, the i18n names in all three locales, and the SVG
// icons.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';
import { PANEL_BUTTONS, PANEL_KEYS } from './hotkeys.js';
import { validateCommand } from './netplay.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const netplay = readFileSync(new URL('./netplay.js', import.meta.url), 'utf8');
const wasmRs = readFileSync(new URL('../../sim/src/wasm.rs', import.meta.url), 'utf8');
const indexHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8');

const SHIPS = [
  [24, 'galley', 'Galley', 'push_train_galley', 'train-galley', 'i-galley', 'blockGa', 90, 30, 36],
  [25, 'fireGalley', 'Fire Galley', 'push_train_fire_galley', 'train-fire-galley', 'i-fire-galley', 'blockFG', 75, 45, 49],
  [26, 'demolitionRaft', 'Demolition Raft', 'push_train_demolition_raft', 'train-demolition-raft', 'i-demolition-raft', 'blockDR', 70, 80, 45],
  [27, 'hulk', 'Hulk', 'push_train_hulk', 'train-hulk', 'i-hulk', 'blockHu', 75, 35, 42],
];

test('the WASM exports train the four combat ships at a Dock', () => {
  for (const [tag, , , fn, , , , , ,] of SHIPS) {
    const kind = { 24: 'Galley', 25: 'FireGalley', 26: 'DemolitionRaft', 27: 'Hulk' }[tag];
    assert.match(wasmRs, new RegExp(`pub fn ${fn}\\(&mut self, building: u32\\) -> bool \\{\\s*self\\.push_train_at\\(building, EntityKind::${kind}\\)`));
  }
});

test('both dispatch paths route combat-ship trains to the exports', () => {
  for (const [tag, , , fn] of SHIPS) {
    assert.match(app, new RegExp(`cmd\\.k === ${tag} \\? sim\\.${fn}\\(cmd\\.b\\)`));
    assert.match(netplay, new RegExp(`${tag}: '${fn}'`));
    assert.equal(validateCommand({ t: 'train_at', b: 3, k: tag }), true);
  }
  // The attemptTrain / refusal maps and the runCommandButton
  // train list name all four buttons.
  for (const [, , , , btn] of SHIPS) {
    assert.ok(app.includes(`'${btn}':`), `${btn} in the kindTag maps`);
    assert.ok(app.includes(`id === '${btn}'`), `${btn} in runCommandButton`);
  }
  assert.match(app, /'train-galley': 24, 'train-fire-galley': 25,/);
  assert.match(app, /'train-demolition-raft': 26, 'train-hulk': 27,/);
});

test('the Dock panel gains the four train buttons behind the Feudal gate', () => {
  for (const [tag, , name, , btn, icon, block, wood, gold, secs] of SHIPS) {
    assert.match(app, new RegExp(`const ${block} = bk && context === 'dock' \\? sim\\.building_train_block\\(bk\\.id, ${tag}\\) : -1;`));
    assert.match(app, new RegExp(`id: '${btn}', icon: '${icon}', label: '${name}',\\s*costIcon: 'i-wood', cost: ${wood}, cost2Icon: 'i-gold', cost2: ${gold},`));
    assert.match(app, new RegExp(`t\\('cmd\\.buildGate', \\{ name: KIND_NAME\\[${tag}\\], gate: t\\('gate\\.feudal'\\) \\}\\)`));
    assert.match(app, new RegExp(`costText\\(\\[\\[${wood}, 'wood'\\], \\[${gold}, 'gold'\\]\\]\\), secs: ${secs} \\}\\)`));
  }
  // Panel order + letters: the W2 three keep Q/W/E; the ships
  // take the positional grid's next cells A/S/D/Z.
  assert.deepEqual(PANEL_BUTTONS.dock,
    ['train-fishing-ship', 'research-fishinglines', 'research-gillnets',
      'train-galley', 'train-fire-galley', 'train-demolition-raft', 'train-hulk']);
  assert.deepEqual(PANEL_KEYS.dock['train-galley'], ['KeyA', 'A']);
  assert.deepEqual(PANEL_KEYS.dock['train-fire-galley'], ['KeyS', 'S']);
  assert.deepEqual(PANEL_KEYS.dock['train-demolition-raft'], ['KeyD', 'D']);
  assert.deepEqual(PANEL_KEYS.dock['train-hulk'], ['KeyZ', 'Z']);
});

test('rendering tables cover the four ships', () => {
  for (const [tag, , name, , , icon] of SHIPS) {
    assert.ok(app.includes(`${tag}: '${name}',`), `KIND_NAME ${tag}`);
    assert.ok(app.includes(`${tag}: '${icon}',`), `KIND_ICON ${tag}`);
  }
  assert.match(app, /'fishingShip', 'galley',\s*'fireGalley', 'demolitionRaft', 'hulk'\];/);
  assert.match(app, /function drawGalley\(i, wx, wy\)/);
  assert.match(app, /function drawFireGalley\(i, wx, wy\)/);
  assert.match(app, /function drawDemolitionRaft\(i, wx, wy\)/);
  assert.match(app, /function drawHulk\(i, wx, wy\)/);
  assert.match(app, /drawGalley, \/\/ 24: Galley/);
  assert.match(app, /drawFireGalley, \/\/ 25: Fire Galley/);
  assert.match(app, /drawDemolitionRaft, \/\/ 26: Demolition Raft/);
  assert.match(app, /drawHulk\]; \/\/ 27: Hulk/);
  // The production-building set names the Dock's roster kinds.
  assert.match(app, /const PROD_KINDS = \[2, 3, 4, 5, 6, 7, 8, 12, 15, 16, 17, 20, 23, 24, 25, 26, 27\];/);
});

test('names exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    for (const [, id] of SHIPS) {
      assert.ok(STRINGS[loc][`unit.${id}`]?.length > 0, `${loc}:unit.${id} present`);
    }
  }
  assert.equal(STRINGS.en['unit.galley'], 'Galley');
  assert.equal(STRINGS.en['unit.fireGalley'], 'Fire Galley');
  assert.equal(STRINGS.en['unit.demolitionRaft'], 'Demolition Raft');
  assert.equal(STRINGS.en['unit.hulk'], 'Hulk');
  // PT-BR (Brazilian forms — Claudio's correction of 2026-11-04).
  assert.equal(STRINGS['pt-BR']['unit.galley'], 'Galé');
  assert.equal(STRINGS['pt-BR']['unit.fireGalley'], 'Galé Incendiária');
  assert.equal(STRINGS['pt-BR']['unit.demolitionRaft'], 'Jangada de Demolição');
  assert.equal(STRINGS.es['unit.galley'], 'Galera');
  assert.equal(STRINGS.es['unit.demolitionRaft'], 'Balsa de Demolición');
});

test('the four new icons are defined', () => {
  for (const [, , , , , icon] of SHIPS) {
    assert.ok(indexHtml.includes(`<symbol id="${icon}"`), `${icon} symbol exists`);
  }
});
