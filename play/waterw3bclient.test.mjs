// Water W3b CLIENT SLICE wiring pins (CLS-07 → MATCHES,
// Castle Age ships) — run from web-preview/play/:
//   node --test waterw3bclient.test.mjs
//
// The W3b sim half landed Medium Warships (TechId 47) and the
// four Castle tiers (War Galley 28, Fire Ship 29, Demolition
// Ship 30, War Hulk 31) as upgrade-only kinds on the CLS-03
// pattern: no new train buttons and no new WASM train exports
// (the Dock's four Feudal buttons keep queuing the base kinds;
// the sim converts on completion and at spawn, exactly like
// Militia → Man-at-Arms). This suite pins what the client
// does gain: the Medium Warships research button at the Dock
// (Castle gate, research-block law, dispatch map, RB_TAGS),
// the hotkey row's X cell, the render tables for the four
// tiers, the i18n names/tech in all three locales, and the
// SVG icons.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';
import { PANEL_BUTTONS, PANEL_KEYS } from './hotkeys.js';
import { validateCommand } from './netplay.js';
import { TECH_TREE } from './techtree.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const netplay = readFileSync(new URL('./netplay.js', import.meta.url), 'utf8');
const wasmRs = readFileSync(new URL('../../sim/src/wasm.rs', import.meta.url), 'utf8');
const indexHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8');

const SHIPS = [
  [28, 'warGalley', 'War Galley', 'i-war-galley', 'drawWarGalley'],
  [29, 'fireShip', 'Fire Ship', 'i-fire-ship', 'drawFireShip'],
  [30, 'demolitionShip', 'Demolition Ship', 'i-demolition-ship', 'drawDemolitionShip'],
  [31, 'warHulk', 'War Hulk', 'i-war-hulk', 'drawWarHulk'],
];

test('the Castle tiers are upgrade-only in the client too', () => {
  // No WASM train exports and no netplay train rows for tags
  // 28–31: a human can never queue a War Galley directly, so
  // neither dispatch path may offer one (the validator keeps
  // rejecting them).
  for (const [tag] of SHIPS) {
    assert.equal(validateCommand({ t: 'train_at', b: 3, k: tag }), false,
      `train_at k=${tag} must stay invalid`);
  }
  assert.ok(!wasmRs.includes('push_train_war_galley'), 'no war-galley train export');
  assert.ok(!wasmRs.includes('push_train_fire_ship'), 'no fire-ship train export');
  assert.ok(!netplay.includes('28:'), 'no netplay train row for 28');
  // The production set is unchanged: the tiers are not
  // production kinds anywhere in the panel tables.
  assert.match(app, /const PROD_KINDS = \[2, 3, 4, 5, 6, 7, 8, 12, 15, 16, 17, 20, 23, 24, 25, 26, 27, 32\];/);
});

test('Medium Warships is wired as Dock research, tag 47', () => {
  // TECH_NAME (default + localized refresh) carries the tag.
  assert.match(app, /45: 'Fishing Lines', 46: 'Gillnets', 47: 'Medium Warships',/);
  assert.match(app, /47: t\('tech\.mediumwarships'\),/);
  // The panel signature reads its research block.
  assert.match(app, /dock: \[45, 46, 47\],/);
  // The click dispatch maps the button to the tag.
  assert.match(app, /'research-mediumwarships': 47,/);
  // The button itself: Castle gate in the title until Castle,
  // 150 wood + 100 gold, 50 s, research-block law.
  assert.match(app, /const mwDone = techMaskHas\(techMask, 47\);/);
  assert.match(app, /id: 'research-mediumwarships', icon: 'i-mediumwarships', label: 'Medium Warships',\s*costIcon: 'i-wood', cost: 150, cost2Icon: 'i-gold', cost2: 100,/);
  assert.match(app, /disabled: underConstruction \|\| mwDone \|\| rbB\[47\] === 2 \|\| rbB\[47\] === 3 \|\| rbB\[47\] === 4,/);
  assert.match(app, /t\('cmd\.buildGate', \{ name: TECH_NAME\[47\], gate: t\('gate\.castle'\) \}\)/);
  assert.match(app, /t\('cmd\.mediumwarshipsTitle', \{ name: TECH_NAME\[47\], cost: costText\(\[\[150, 'wood'\], \[100, 'gold'\]\]\), secs: 50 \}\)/);
  // The research itself issues through the generic path.
  assert.equal(validateCommand({ t: 'research_at', b: 3, tech: 47 }), true);
  // The tech-tree mirror row (INF-06): Dock host, Castle Age,
  // no prerequisite (the upgrade keys off the age, not off
  // Gillnets).
  assert.equal(TECH_TREE.length, 53); // 48 at W3b; +Careening/Clinker (W5); +Dry Dock/Carvel Hull/Shipwright (W6)
  assert.deepEqual(TECH_TREE[47], {
    tag: 47, host: 23, age: 2, cost: [0, 150, 100, 0], secs: 50, requires: null,
  });
});

test('panel order + letters: the research button takes the X cell', () => {
  assert.deepEqual(PANEL_BUTTONS.dock,
    ['train-fishing-ship', 'research-fishinglines', 'research-gillnets',
      'train-galley', 'train-fire-galley', 'train-demolition-raft', 'train-hulk',
      'research-mediumwarships', 'train-transport-ship', 'train-trade-cog']); // + Transport (W4), + the Cog (W5)
  assert.deepEqual(PANEL_KEYS.dock['research-mediumwarships'], ['KeyX', 'X']);
  // The W2/W3a letters are untouched.
  assert.deepEqual(PANEL_KEYS.dock['train-galley'], ['KeyA', 'A']);
  assert.deepEqual(PANEL_KEYS.dock['train-hulk'], ['KeyZ', 'Z']);
});

test('rendering tables cover the four tiers', () => {
  for (const [tag, , name, icon, draw] of SHIPS) {
    assert.ok(app.includes(`${tag}: '${name}',`), `KIND_NAME ${tag}`);
    assert.ok(app.includes(`${tag}: '${icon}',`), `KIND_ICON ${tag}`);
    assert.match(app, new RegExp(`function ${draw}\\(i, wx, wy\\)`));
  }
  assert.match(app, /'fireGalley', 'demolitionRaft', 'hulk', 'warGalley', 'fireShip', 'demolitionShip', 'warHulk',/);
  assert.match(app, /drawWarGalley, \/\/ 28: War Galley/);
  assert.match(app, /drawFireShip, \/\/ 29: Fire Ship/);
  assert.match(app, /drawDemolitionShip, \/\/ 30: Demolition Ship/);
  assert.match(app, /drawWarHulk, \/\/ 31: War Hulk/);
  assert.match(app, /drawTransportShip, \/\/ 32: Transport Ship/);
  assert.match(app, /drawTradeCog\]; \/\/ 33: Trade Cog/); // the table ends at the Cog since the W5 merge
});

test('names and the tech exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    for (const [, id] of SHIPS) {
      assert.ok(STRINGS[loc][`unit.${id}`]?.length > 0, `${loc}:unit.${id} present`);
    }
    assert.ok(STRINGS[loc]['tech.mediumwarships']?.length > 0, `${loc}:tech.mediumwarships present`);
    assert.ok(STRINGS[loc]['cmd.mediumwarshipsTitle']?.includes('X'), `${loc}:cmd.mediumwarshipsTitle names the X hotkey`);
  }
  assert.equal(STRINGS.en['unit.warGalley'], 'War Galley');
  assert.equal(STRINGS.en['unit.fireShip'], 'Fire Ship');
  assert.equal(STRINGS.en['unit.demolitionShip'], 'Demolition Ship');
  assert.equal(STRINGS.en['unit.warHulk'], 'War Hulk');
  assert.equal(STRINGS.en['tech.mediumwarships'], 'Medium Warships');
  // PT-BR (Brazilian forms — Claudio's correction of 2026-11-04).
  assert.equal(STRINGS['pt-BR']['unit.warGalley'], 'Galé de Guerra');
  assert.equal(STRINGS['pt-BR']['unit.fireShip'], 'Navio Incendiário');
  assert.equal(STRINGS['pt-BR']['unit.demolitionShip'], 'Navio de Demolição');
  assert.equal(STRINGS['pt-BR']['unit.warHulk'], 'Hulk de Guerra');
  assert.equal(STRINGS.es['unit.warGalley'], 'Galera de Guerra');
  assert.equal(STRINGS.es['unit.fireShip'], 'Barco de Fuego');
  assert.equal(STRINGS.es['unit.demolitionShip'], 'Barco de Demolición');
  assert.equal(STRINGS.es['unit.warHulk'], 'Hulk de Guerra');
});

test('the five new icons are defined', () => {
  for (const [, , , icon] of SHIPS) {
    assert.ok(indexHtml.includes(`<symbol id="${icon}"`), `${icon} symbol exists`);
  }
  assert.ok(indexHtml.includes('<symbol id="i-mediumwarships"'), 'i-mediumwarships symbol exists');
});
