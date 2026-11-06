// Water W4 CLIENT SLICE wiring pins (GAR-09, Transport Ship)
// — run from web-preview/play/:
//   node --test waterw4client.test.mjs
//
// The W4 sim half landed the Transport Ship (kind 32) with its
// cargo list on the ship entity, the LoadTransport /
// UnloadTransport command pair (codec 54/55) and the
// death routing that kills the cargo with the ship. This
// suite pins the client half end to end, the way
// waterw3client.test.mjs pinned W3a's: the WASM exports, both
// dispatch paths (local + netplay), the Dock panel's train
// button, the smart-order load gestures, the Unload button,
// the kind render tables, the i18n names and keys in all
// three locales, and the SVG icon.

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

test('the WASM exports train and work the Transport Ship', () => {
  assert.match(wasmRs, /pub fn push_train_transport_ship\(&mut self, building: u32\) -> bool \{\s*self\.push_train_at\(building, EntityKind::TransportShip\)/);
  assert.match(wasmRs, /pub fn push_load_transport\(&mut self, entity: u32, ship: u32\) -> bool/);
  assert.match(wasmRs, /pub fn push_unload_transport\(&mut self, ship: u32\) -> bool/);
  assert.match(wasmRs, /pub fn transport_cargo_count\(&self, ship: u32\) -> u32/);
});

test('both dispatch paths route the transport commands', () => {
  assert.match(app, /cmd\.k === 32 \? sim\.push_train_transport_ship\(cmd\.b\)/);
  assert.match(netplay, /32: 'push_train_transport_ship'/);
  assert.equal(validateCommand({ t: 'train_at', b: 3, k: 32 }), true);
  // The upgrade-only Castle tiers stay off the wire.
  assert.equal(validateCommand({ t: 'train_at', b: 3, k: 28 }), false);
  // Load / unload validate and dispatch on both paths.
  assert.equal(validateCommand({ t: 'load_t', e: 5, ship: 9 }), true);
  assert.equal(validateCommand({ t: 'load_t', e: 5 }), false);
  assert.equal(validateCommand({ t: 'unload_t', ship: 9 }), true);
  assert.match(app, /case 'load_t': return sim\.push_load_transport\(cmd\.e, cmd\.ship\);/);
  assert.match(app, /case 'unload_t': return sim\.push_unload_transport\(cmd\.ship\);/);
  assert.match(netplay, /case 'load_t': return sim\.push_load_transport\(cmd\.e, cmd\.ship\);/);
  assert.match(netplay, /case 'unload_t': return sim\.push_unload_transport\(cmd\.ship\);/);
  // The attemptTrain / refusal maps and the runCommandButton
  // train list name the button.
  assert.match(app, /'train-transport-ship': 32,/);
  assert.ok(app.includes(`id === 'train-transport-ship'`), 'train-transport-ship in runCommandButton');
});

test('the Dock panel gains the Transport train button (Dark Age, no gate)', () => {
  assert.match(app, /const blockTS = bk && context === 'dock' \? sim\.building_train_block\(bk\.id, 32\) : -1;/);
  assert.match(app, /id: 'train-transport-ship', icon: 'i-transport-ship', label: 'Transport Ship',\s*costIcon: 'i-wood', cost: 125,/);
  assert.match(app, /costText\(\[\[125, 'wood'\]\]\), secs: 46 \}\)/);
  // Panel order + letter: the button completes the positional
  // grid's third row at C; every W2/W3 letter is untouched.
  assert.deepEqual(PANEL_BUTTONS.dock,
    ['train-fishing-ship', 'research-fishinglines', 'research-gillnets',
      'train-galley', 'train-fire-galley', 'train-demolition-raft', 'train-hulk',
      'research-mediumwarships', 'train-transport-ship', 'train-trade-cog']);
  // (The Trade Cog appends after the Transport Ship on the
  // merged tree — Water W5 merge; it takes KeyV, see the W5
  // client suite. The Transport Ship's own cell is unchanged.)
  assert.deepEqual(PANEL_KEYS.dock['train-transport-ship'], ['KeyC', 'C']);
  assert.deepEqual(PANEL_KEYS.dock['research-mediumwarships'], ['KeyX', 'X']);
});

test('the human load/unload paths ride the smart order and the panel', () => {
  // Right-click an own Transport with land units selected.
  assert.match(app, /st\.kinds\[t\] === 32\) \{\s*let any = false;/);
  assert.match(app, /issue\(\{ t: 'load_t', e: id, ship: t \}\)/);
  // The panel's Unload button + its dispatch.
  assert.match(app, /id: 'unload-transport', icon: 'i-ungarrison', label: t\('cmd\.unloadTransport'\)/);
  assert.match(app, /issue\(\{ t: 'unload_t', ship: selT\[0\] \}\)/);
  // The selection info line carries the cargo count (cap 5).
  assert.match(app, /t\('sel\.cargoLine', \{ n: cN, cap: 5 \}\)/);
});

test('rendering tables cover the Transport Ship', () => {
  assert.ok(app.includes(`32: 'Transport Ship',`), 'KIND_NAME 32');
  assert.ok(app.includes(`32: 'i-transport-ship',`), 'KIND_ICON 32');
  assert.match(app, /'transportShip', 'tradeCog'\];/); // merged tail: the Cog (33) follows the Transport Ship
  assert.match(app, /function drawTransportShip\(i, wx, wy\)/);
  assert.match(app, /drawTransportShip, \/\/ 32: Transport Ship/); // no longer the tail: the Cog's draw follows (W5 merge)
  assert.match(app, /const PROD_KINDS = \[2, 3, 4, 5, 6, 7, 8, 12, 15, 16, 17, 20, 23, 24, 25, 26, 27, 32\];/);
});

test('names and transport keys exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    assert.ok(STRINGS[loc]['unit.transportShip']?.length > 0, `${loc}:unit.transportShip present`);
    assert.ok(STRINGS[loc]['cmd.unloadTransport']?.length > 0, `${loc}:cmd.unloadTransport present`);
    assert.ok(STRINGS[loc]['cmd.unloadTransportTitle']?.length > 0, `${loc}:cmd.unloadTransportTitle present`);
    assert.ok(STRINGS[loc]['sel.cargoLine']?.length > 0, `${loc}:sel.cargoLine present`);
    assert.ok(STRINGS[loc]['flash.loaded']?.length > 0, `${loc}:flash.loaded present`);
  }
  assert.equal(STRINGS.en['unit.transportShip'], 'Transport Ship');
  // PT-BR (Brazilian forms — Claudio's correction of 2026-11-04).
  assert.equal(STRINGS['pt-BR']['unit.transportShip'], 'Navio de Transporte');
  assert.equal(STRINGS['pt-BR']['cmd.unloadTransport'], 'Desembarcar');
  assert.equal(STRINGS.es['unit.transportShip'], 'Barco de Transporte');
});

test('the transport icon is defined', () => {
  assert.ok(indexHtml.includes('<symbol id="i-transport-ship"'), 'i-transport-ship symbol exists');
});
