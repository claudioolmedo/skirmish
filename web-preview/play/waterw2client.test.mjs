// Water W2 CLIENT SLICE wiring pins (ECO-15) — run from
// web-preview/play/:
//   node --test waterw2client.test.mjs
//
// The W2a sim half landed the Fishing Ship, the two fish nodes and
// the Dock's researches (Fishing Lines 45, Gillnets 46) with no
// player-facing surface. This suite pins the client half end to
// end, the way waterw1.test.mjs pinned W1's: the WASM train
// export, both dispatch paths (local + netplay), the Dock panel's
// three buttons, the tech-tree mirror rows, the kind/node render
// tables, the i18n keys in all three locales, and the SVG icons.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { TECH_TREE } from './techtree.js';
import { NODE_COLORS } from './minimap.js';
import { STRINGS } from './i18n.js';
import { PANEL_BUTTONS } from './hotkeys.js';
import { validateCommand } from './netplay.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const netplay = readFileSync(new URL('./netplay.js', import.meta.url), 'utf8');
const wasmRs = readFileSync(new URL('../../sim/src/wasm.rs', import.meta.url), 'utf8');
const indexHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8');

test('the WASM export trains the Fishing Ship at a Dock', () => {
  assert.match(wasmRs, /pub fn push_train_fishing_ship\(&mut self, building: u32\) -> bool \{\s*self\.push_train_at\(building, EntityKind::FishingShip\)/);
});

test('both dispatch paths route Fishing Ship trains to the export', () => {
  // Local orders (app.js applyLocalOrder).
  assert.match(app, /cmd\.k === 23 \? sim\.push_train_fishing_ship\(cmd\.b\)/);
  // Netplay: the train table and the wire validator agree.
  assert.match(netplay, /23: 'push_train_fishing_ship'/);
  assert.equal(validateCommand({ t: 'train_at', b: 3, k: 23 }), true);
  // The Dock's researches validate on the wire (45/46).
  assert.equal(validateCommand({ t: 'research_at', b: 3, tech: 45 }), true);
  assert.equal(validateCommand({ t: 'research_at', b: 3, tech: 46 }), true);
  // The local research dispatch maps the two panel ids.
  assert.match(app, /'research-fishinglines': 45, 'research-gillnets': 46/);
  assert.match(app, /'train-fishing-ship': 23/);
});

test('the tech tree mirrors the Dock researches (tags 45/46)', () => {
  // TECH_TREE.length grew to 48 with Water W3b's Medium
  // Warships (tag 47, pinned in waterw3bclient.test.mjs).
  assert.equal(TECH_TREE.length, 48);
  assert.deepEqual(TECH_TREE[45], {
    tag: 45, host: 23, age: 1, cost: [50, 100, 0, 0], secs: 20, requires: null,
  });
  assert.deepEqual(TECH_TREE[46], {
    tag: 46, host: 23, age: 2, cost: [150, 200, 0, 0], secs: 45, requires: 45,
  });
});

test('the Dock panel exists: context, train block, research rows', () => {
  assert.match(app, /23: 'dock' \};/);
  assert.match(app, /const blockFS = bk && context === 'dock' \? sim\.building_train_block\(bk\.id, 23\) : -1;/);
  assert.match(app, /dock: \[45, 46, 47\],/);
  assert.match(app, /id: 'train-fishing-ship', icon: 'i-fishing-ship', label: 'Fishing Ship',\s*costIcon: 'i-wood', cost: 75,/);
  assert.match(app, /id: 'research-fishinglines', icon: 'i-fishinglines', label: 'Fishing Lines',\s*costIcon: 'i-food', cost: 50, cost2Icon: 'i-wood', cost2: 100,/);
  assert.match(app, /id: 'research-gillnets', icon: 'i-gillnets', label: 'Gillnets',\s*costIcon: 'i-food', cost: 150, cost2Icon: 'i-wood', cost2: 200,/);
  assert.deepEqual(PANEL_BUTTONS.dock,
    ['train-fishing-ship', 'research-fishinglines', 'research-gillnets',
      'train-galley', 'train-fire-galley', 'train-demolition-raft', 'train-hulk',
      'research-mediumwarships', 'train-transport-ship']);
});

test('the Dock is a full production + shelter host client-side', () => {
  // Selection panel production line and queue-name tag.
  assert.match(app, /const PROD_KINDS = \[2, 3, 4, 5, 6, 7, 8, 12, 15, 16, 17, 20, 23, 24, 25, 26, 27, 32\];/);
  assert.match(app, /20: 20, 23: 23 \};/);
  // Shelter: cap 5 on the garrison line, arrows read, and the
  // ungarrison block rides the Dock panel.
  assert.match(app, /23: 5 \}\[bk\.kind\] \?\? 10\)/);
  assert.match(app, /bk\.kind === 23\)\)\s*\? sim\.building_garrison_arrows\(bk\.id\)/);
  // Rally: a right-click sets a Dock rally like the other
  // production buildings.
  assert.match(app, /\[2, 6, 7, 23\]\.includes\(selectedBuilding\.kind\)/);
});

test('rendering tables cover the ship and both fish nodes', () => {
  assert.match(app, /const NODE_MAX = \[100, 125, 800, 350, 175, 340, 100, 200, 250, 715\];/);
  assert.match(app, /drawCarcass, drawCarcass, drawShoreFish, drawDeepFish, drawFishTrap\];/);
  assert.match(app, /function drawShoreFish\(sx, sy, frac\)/);
  assert.match(app, /function drawDeepFish\(sx, sy, frac\)/);
  assert.match(app, /function drawFishingShip\(i, wx, wy\)/);
  assert.match(app, /drawFishingShip, \/\/ 23: Fishing Ship/);
  assert.match(app, /23: 'Fishing Ship',/);
  assert.match(app, /23: 'i-fishing-ship',/);
  assert.equal(NODE_COLORS.length, 10);
});

test('a right-click on a fish spot orders the Fishing Ship to gather, not move', () => {
  // The smart order's gather pairing mirrors the sim's
  // can_gather_pair: villager (0) on any node, Fishing Ship (23)
  // on fish nodes only (7 shore / 8 deep). Before this pin the
  // branch tested kind 0 alone, so a ship's right-click on a
  // fish spot degraded to a Move and the ship never fished
  // (bot-loop live proof, 2026-11-05).
  assert.match(app, /const gathersHere = n && \(st\.kinds\[id\] === 0\s*\|\| \(st\.kinds\[id\] === 23 && \(n\.kind === 7 \|\| n\.kind === 8 \|\| n\.kind === 9\)\)\);/);
  assert.match(app, /if \(gathersHere\) \{\s*if \(issue\(\{ t: 'gather', e: id, n: n\.id, q: queued \|\| undefined \}\)\) any = true;/);
});

test('names and strings exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    for (const key of ['unit.fishingShip', 'tech.fishinglines', 'tech.gillnets',
      'cmd.fishinglinesTitle', 'cmd.gillnetsTitle', 'sel.fishingShipInfo']) {
      assert.ok(STRINGS[loc][key]?.length > 0, `${loc}:${key} present`);
    }
  }
  assert.equal(STRINGS.en['unit.fishingShip'], 'Fishing Ship');
  assert.equal(STRINGS.en['tech.fishinglines'], 'Fishing Lines');
  assert.equal(STRINGS.en['tech.gillnets'], 'Gillnets');
  // PT-BR (Brazilian forms — Claudio's correction of 2026-11-04).
  assert.equal(STRINGS['pt-BR']['unit.fishingShip'], 'Barco de Pesca');
  assert.equal(STRINGS['pt-BR']['tech.gillnets'], 'Redes de Emalhar');
  assert.equal(STRINGS.es['unit.fishingShip'], 'Barco Pesquero');
});

test('the three new icons are defined', () => {
  for (const id of ['i-fishing-ship', 'i-fishinglines', 'i-gillnets']) {
    assert.ok(indexHtml.includes(`<symbol id="${id}"`), `${id} symbol exists`);
  }
});
