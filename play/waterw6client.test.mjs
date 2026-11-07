// Water W6 client tests: the Imperial University ship trio
// (Dry Dock 50, Carvel Hull 51, Shipwright 52). Source-text
// checks in the house style (the W5 client suite's
// discipline): the sim remains the only authority — these
// tests pin that every client surface the new techs must
// touch actually names them. Costs/times are the W6 Passo 0
// VERIFIED pins (wiki Technology table, fetched 2026-11-06):
// Dry Dock 200 F / 400 G / 60 s, Carvel Hull 350 F / 200 W /
// 60 s, Shipwright 1 000 F / 300 G / 60 s.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';
import { PANEL_BUTTONS, PANEL_KEYS } from './hotkeys.js';
import { TECH_TREE } from './techtree.js';
import { validateCommand } from './netplay.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const wasmRs = readFileSync(new URL('../../sim/src/wasm.rs', import.meta.url), 'utf8');

test('the trio rides the generic research path; the validator ceiling is 52', () => {
  // No dedicated research exports: the trio rides
  // push_research_at by tag, like every placed-building tech.
  assert.doesNotMatch(wasmRs, /push_research_dry_dock|push_research_carvel|push_research_shipwright/);
  for (const tech of [50, 51, 52]) {
    assert.equal(validateCommand({ t: 'research_at', b: 1, tech }), true, `tech ${tech}`);
  }
  assert.equal(validateCommand({ t: 'research_at', b: 1, tech: 53 }), false);
});

test('the University gains the Imperial trio after the W5 pair', () => {
  // RB_TAGS reads all three tags' blocks on the University panel.
  assert.equal((app.match(/university: \[20, 21, 22, 23, 24, 48, 49, 50, 51, 52\]/g) || []).length, 2);
  assert.match(app, /techBtn\('research-drydock', 50, 'i-drydock', 'cmd\.drydockTitle',\s*\[\[200, 'food'\], \[400, 'gold'\]\], 60, 7\)/);
  assert.match(app, /techBtn\('research-carvelhull', 51, 'i-carvelhull', 'cmd\.carvelhullTitle',\s*\[\[350, 'food'\], \[200, 'wood'\]\], 60, 8\)/);
  assert.match(app, /techBtn\('research-shipwright', 52, 'i-shipwright', 'cmd\.shipwrightTitle',\s*\[\[1000, 'food'\], \[300, 'gold'\]\], 60, 9\)/);
  // The research dispatch map knows all three button ids.
  assert.match(app, /'research-drydock': 50, 'research-carvelhull': 51, 'research-shipwright': 52,/);
  // TECH_NAME and the semantic techName map carry the trio.
  assert.ok(app.includes("50: 'Dry Dock', 51: 'Carvel Hull', 52: 'Shipwright',"), 'TECH_NAME trio');
  assert.ok(app.includes('drydock: 50, carvelhull: 51, shipwright: 52'), 'techName trio');
  // Panel order + letters: X, C, V after the W5 pair's D, Z.
  assert.deepEqual(PANEL_BUTTONS.university.slice(7),
    ['research-drydock', 'research-carvelhull', 'research-shipwright']);
  assert.deepEqual(PANEL_KEYS.university['research-drydock'], ['KeyX', 'X']);
  assert.deepEqual(PANEL_KEYS.university['research-carvelhull'], ['KeyC', 'C']);
  assert.deepEqual(PANEL_KEYS.university['research-shipwright'], ['KeyV', 'V']);
  // The W5 pair's cells are untouched.
  assert.deepEqual(PANEL_KEYS.university['research-careening'], ['KeyD', 'D']);
  assert.deepEqual(PANEL_KEYS.university['research-clinkerconstruction'], ['KeyZ', 'Z']);
});

test('the tech tree mirrors the Imperial trio (tags 50/51/52)', () => {
  assert.equal(TECH_TREE.length, 53);
  assert.deepEqual(TECH_TREE[50], {
    tag: 50, host: 16, age: 3, cost: [200, 0, 400, 0], secs: 60, requires: 48,
  });
  assert.deepEqual(TECH_TREE[51], {
    tag: 51, host: 16, age: 3, cost: [350, 200, 0, 0], secs: 60, requires: 49,
  });
  assert.deepEqual(TECH_TREE[52], {
    tag: 52, host: 16, age: 3, cost: [1000, 0, 300, 0], secs: 60, requires: null,
  });
});

test('names, titles and icons exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    for (const k of ['tech.drydock', 'tech.carvelhull', 'tech.shipwright']) {
      assert.ok(STRINGS[loc][k]?.length > 0, `${loc}:${k} present`);
    }
    assert.ok(STRINGS[loc]['cmd.drydockTitle']?.includes('X'), `${loc}:cmd.drydockTitle names the X hotkey`);
    assert.ok(STRINGS[loc]['cmd.carvelhullTitle']?.includes('C'), `${loc}:cmd.carvelhullTitle names the C hotkey`);
    assert.ok(STRINGS[loc]['cmd.shipwrightTitle']?.includes('V'), `${loc}:cmd.shipwrightTitle names the V hotkey`);
  }
  assert.equal(STRINGS.en['tech.drydock'], 'Dry Dock');
  assert.equal(STRINGS.en['tech.carvelhull'], 'Carvel Hull');
  assert.equal(STRINGS.en['tech.shipwright'], 'Shipwright');
  // PT-BR names are translations (recorded in the W6 handoff).
  assert.equal(STRINGS['pt-BR']['tech.drydock'], 'Dique Seco');
  assert.equal(STRINGS['pt-BR']['tech.carvelhull'], 'Casco Liso');
  assert.equal(STRINGS['pt-BR']['tech.shipwright'], 'Construtor Naval');
  for (const sym of ['i-drydock', 'i-carvelhull', 'i-shipwright']) {
    assert.ok(html.includes(`<symbol id="${sym}"`), `${sym} symbol present`);
  }
});
