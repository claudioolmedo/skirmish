// Water W5 client tests: the Trade Cog (TRD-05/07/08 on the
// client) and the University ship pair (Careening 48, Clinker
// Construction 49). Source-text checks in the house style (the
// W2/W3/W3b client suites' discipline): the sim remains the
// only authority — these tests pin that every client surface
// the new unit and techs must touch actually names them.
//
// Note on tag 32: Water W4's Transport Ship was built on a
// sibling branch off this same base; the W5 tree reserved the
// slot in the dense client tables so the Trade Cog kept its
// true tag 33. On THIS merged tree (the W5-onto-W4 landing)
// slot 32 is the real Transport Ship — draw, verb and panel
// button included — and the Cog sits one Dock cell later
// (index 9, KeyV), exactly as the W5 handoff's merge notes
// prescribed.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';
import { PANEL_BUTTONS, PANEL_KEYS } from './hotkeys.js';
import { TECH_TREE } from './techtree.js';
import { validateCommand, applyCommand } from './netplay.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const netplay = readFileSync(new URL('./netplay.js', import.meta.url), 'utf8');
const wasmRs = readFileSync(new URL('../../sim/src/wasm.rs', import.meta.url), 'utf8');

test('the WASM sim exposes the Cog train verb; the techs ride the generic research', () => {
  assert.match(wasmRs, /pub fn push_train_trade_cog\(&mut self, building: u32\) -> bool/);
  // No dedicated research exports: Careening / Clinker ride
  // push_research_at by tag, like every placed-building tech.
  assert.doesNotMatch(wasmRs, /push_research_careening|push_research_clinker/);
  // Both command surfaces reach the Cog: the app.js dispatch
  // and the netplay table + validator.
  assert.match(app, /cmd\.k === 33 \? sim\.push_train_trade_cog\(cmd\.b\)/);
  assert.match(netplay, /33: 'push_train_trade_cog'/);
  assert.equal(validateCommand({ t: 'train_at', b: 1, k: 33 }), true);
  // Tag 32 (the Transport Ship) was NOT valid on the W5 tree —
  // on this merged tree W4 has landed, so it validates too.
  assert.equal(validateCommand({ t: 'train_at', b: 1, k: 32 }), true);
  let seen = null;
  const sim = { push_train_trade_cog: (b) => { seen = b; return true; } };
  applyCommand(sim, { t: 'train_at', b: 7, k: 33 });
  assert.equal(seen, 7);
  // The University ship pair passes the research validator.
  assert.equal(validateCommand({ t: 'research_at', b: 1, tech: 48 }), true);
  assert.equal(validateCommand({ t: 'research_at', b: 1, tech: 49 }), true);
  // Water W6: tags 50–52 (the Imperial trio) now validate too.
  assert.equal(validateCommand({ t: 'research_at', b: 1, tech: 50 }), true);
  assert.equal(validateCommand({ t: 'research_at', b: 1, tech: 52 }), true);
  assert.equal(validateCommand({ t: 'research_at', b: 1, tech: 53 }), false);
});

test('the Dock panel gains the Cog train button behind the Feudal gate', () => {
  assert.match(app, /const blockTCog = bk && context === 'dock' \? sim\.building_train_block\(bk\.id, 33\) : -1;/);
  assert.match(app, /blockFG, blockDR, blockHu, blockTS, blockTCog, age, techMask/);
  assert.match(app, /id: 'train-trade-cog', icon: 'i-trade-cog', label: 'Trade Cog',\s*costIcon: 'i-wood', cost: 100, cost2Icon: 'i-gold', cost2: 50,/);
  assert.match(app, /age < 1\s*\? t\('cmd\.buildGate', \{ name: KIND_NAME\[33\], gate: t\('gate\.feudal'\) \}\)/);
  assert.match(app, /costText\(\[\[100, 'wood'\], \[50, 'gold'\]\]\), secs: 36 \}\)/);
  // Attempt + refusal maps know the button beside the ships.
  assert.equal((app.match(/'train-trade-cog': 33,/g) || []).length, 2);
  assert.match(app, /\|\| id === 'train-transport-ship' \|\| id === 'train-trade-cog'\)/);
  // The Cog→Dock trade gesture mirrors the Cart→Market block.
  assert.match(app, /bT\.kind === 23 && bT\.complete && bT\.owner !== localOwner/);
  assert.match(app, /st\.kinds\[id\] !== 33\) continue;/);
});

test('panel order + letters: the Cog takes the V cell after the Transport Ship', () => {
  assert.deepEqual(PANEL_BUTTONS.dock,
    ['train-fishing-ship', 'research-fishinglines', 'research-gillnets',
      'train-galley', 'train-fire-galley', 'train-demolition-raft', 'train-hulk',
      'research-mediumwarships', 'train-transport-ship', 'train-trade-cog']);
  assert.deepEqual(PANEL_KEYS.dock['train-trade-cog'], ['KeyV', 'V']);
  assert.deepEqual(PANEL_KEYS.dock['train-transport-ship'], ['KeyC', 'C']);
  // The W2/W3a/W3b letters are untouched.
  assert.deepEqual(PANEL_KEYS.dock['train-hulk'], ['KeyZ', 'Z']);
  assert.deepEqual(PANEL_KEYS.dock['research-mediumwarships'], ['KeyX', 'X']);
});

test('the University gains the ship pair with the BigInt-safe done check', () => {
  // RB_TAGS reads both tags' blocks on the University panel.
  assert.equal((app.match(/university: \[20, 21, 22, 23, 24, 48, 49, 50, 51, 52\]/g) || []).length, 2); // W6 trio appended
  // The University techBtn reads the mask via techMaskHas —
  // `(1 << 48)` would wrap; the camp panels' lesson applied.
  assert.match(app, /const done = techMaskHas\(techMask, tag\);\s*const rb = rbB\[tag\];/);
  assert.match(app, /techBtn\('research-careening', 48, 'i-careening', 'cmd\.careeningTitle',\s*\[\[100, 'food'\], \[200, 'gold'\]\], 40, 5\)/);
  assert.match(app, /techBtn\('research-clinkerconstruction', 49, 'i-clinkerconstruction', 'cmd\.clinkerTitle',\s*\[\[150, 'food'\], \[100, 'wood'\]\], 40, 6\)/);
  // The research dispatch map knows both button ids.
  assert.match(app, /'research-careening': 48, 'research-clinkerconstruction': 49,/);
  // Panel order + letters: D and Z after the five.
  assert.deepEqual(PANEL_BUTTONS.university.slice(5, 7),
    ['research-careening', 'research-clinkerconstruction']); // W6 appends after these (waterw6client)
  assert.deepEqual(PANEL_KEYS.university['research-careening'], ['KeyD', 'D']);
  assert.deepEqual(PANEL_KEYS.university['research-clinkerconstruction'], ['KeyZ', 'Z']);
  assert.deepEqual(PANEL_KEYS.university['research-ballistics'], ['KeyQ', 'Q']);
});

test('the tech tree mirrors the University ship pair (tags 48/49)', () => {
  assert.equal(TECH_TREE.length, 53); // 50 here at W5; +the Imperial trio (W6)
  assert.deepEqual(TECH_TREE[48], {
    tag: 48, host: 16, age: 2, cost: [100, 0, 200, 0], secs: 40, requires: null,
  });
  assert.deepEqual(TECH_TREE[49], {
    tag: 49, host: 16, age: 2, cost: [150, 100, 0, 0], secs: 40, requires: null,
  });
});

test('rendering tables cover the Cog at 33 and the Transport Ship at 32', () => {
  assert.match(app, /'transportShip', 'tradeCog'\];/);
  assert.ok(app.includes("33: 'Trade Cog',"), 'KIND_NAME 33');
  assert.ok(app.includes("32: 'Transport Ship',"), 'KIND_NAME 32');
  assert.ok(app.includes("33: 'i-trade-cog',"), 'KIND_ICON 33');
  assert.ok(app.includes("32: 'i-transport-ship',"), 'KIND_ICON 32');
  assert.match(app, /drawTradeCog\]; \/\/ 33: Trade Cog \(Water W5, TRD-05\)/);
  assert.match(app, /function drawTradeCog\(i, wx, wy\)/);
  // The Cog's loaded leg reads the sim's trade state, like the
  // Cart's — never a client-side invention.
  assert.match(app, /const loaded = tradeStateOf\(i\) === 2;\s*const cloth = drawOwner === foeOwner\(\) \? ENEMY_CLOTH : '#31579e';\s*\/\/ Broad hull/);
});

test('names, titles and icons exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    assert.ok(STRINGS[loc]['unit.tradeCog']?.length > 0, `${loc}:unit.tradeCog present`);
    assert.ok(STRINGS[loc]['unit.transportShip']?.length > 0, `${loc}:unit.transportShip present`);
    assert.ok(STRINGS[loc]['tech.careening']?.length > 0, `${loc}:tech.careening present`);
    assert.ok(STRINGS[loc]['tech.clinkerconstruction']?.length > 0, `${loc}:tech.clinkerconstruction present`);
    assert.ok(STRINGS[loc]['cmd.careeningTitle']?.includes('D'), `${loc}:cmd.careeningTitle names the D hotkey`);
    assert.ok(STRINGS[loc]['cmd.clinkerTitle']?.includes('Z'), `${loc}:cmd.clinkerTitle names the Z hotkey`);
  }
  assert.equal(STRINGS.en['unit.tradeCog'], 'Trade Cog');
  assert.equal(STRINGS['pt-BR']['unit.tradeCog'], 'Coca de Comércio');
  assert.equal(STRINGS.es['unit.tradeCog'], 'Coca de Comercio');
  assert.equal(STRINGS.en['tech.careening'], 'Careening');
  assert.equal(STRINGS['pt-BR']['tech.careening'], 'Carenagem');
  assert.equal(STRINGS.en['tech.clinkerconstruction'], 'Clinker Construction');
  assert.equal(STRINGS['pt-BR']['tech.clinkerconstruction'], 'Construção em Trincado');
  for (const sym of ['i-trade-cog', 'i-careening', 'i-clinkerconstruction', 'i-transport-ship']) {
    assert.ok(html.includes(`<symbol id="${sym}"`), `${sym} symbol present`);
  }
});
