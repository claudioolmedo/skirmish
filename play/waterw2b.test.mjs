// Water W2b CLIENT wiring pins (ECO-15's Fish Trap clause) —
// run from web-preview/play/:
//   node --test waterw2b.test.mjs
//
// The W2b sim half lands the Fish Trap: building kind 24, built
// BY a Fishing Ship on one water cell, completing into the
// 715-food node kind 9, plus the Dock's trap-reseed desk. This
// suite pins the client half end to end, the way
// waterw2client.test.mjs pinned W2a's: the WASM exports, both
// dispatch paths (local + netplay), the ship panel's Build Fish
// Trap button and the placement flow a human runs (panel arm ->
// ghost mirror -> attemptPlace's ship branch), the right-click
// gather/resume pairings, the Dock desk, the render tables, the
// i18n keys in all three locales, and the SVG icon.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { NODE_COLORS } from './minimap.js';
import { STRINGS } from './i18n.js';
import { validateCommand } from './netplay.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const netplay = readFileSync(new URL('./netplay.js', import.meta.url), 'utf8');
const wasmRs = readFileSync(new URL('../../sim/src/wasm.rs', import.meta.url), 'utf8');
const indexHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8');

test('the WASM exports build the trap and run the Dock desk', () => {
  assert.match(wasmRs, /pub fn push_build_fish_trap\(&mut self, entity: u32, x: f64, y: f64\) -> bool \{\s*self\.push_build\(entity, BuildingKind::FishTrap, x, y\)/);
  assert.match(wasmRs, /pub fn push_build_fish_trap_queued\(&mut self, entity: u32, x: f64, y: f64\) -> bool \{\s*self\.push_build_queued\(entity, BuildingKind::FishTrap, x, y\)/);
  assert.match(wasmRs, /pub fn push_queue_trap_reseed\(&mut self, building: u32\) -> bool/);
  assert.match(wasmRs, /pub fn push_dequeue_trap_reseed\(&mut self, building: u32\) -> bool/);
  assert.match(wasmRs, /pub fn push_set_auto_trap_reseed\(&mut self, building: u32, on: bool\) -> bool/);
  assert.match(wasmRs, /pub fn trap_reseed_for\(&self, owner: u32\) -> Vec<u32>/);
});

test('the build tables carry kind 24 at the SPEC row', () => {
  // Extract the flat tables the way panels.test.mjs does.
  const table = (name) => {
    const m = app.match(new RegExp(`const ${name} = \\[([^\\]]+)\\];`));
    assert.ok(m, `${name} table found`);
    return m[1].split(',').map((s) => s.trim());
  };
  assert.equal(table('BUILD_COST')[24], '100');
  assert.equal(table('BUILD_COST_STONE')[24], '0');
  assert.equal(table('BUILD_COST_GOLD')[24], '0');
  assert.equal(table('BUILD_SIZE')[24], '1');
  assert.equal(table('BUILD_COST').length, 25);
  assert.match(app, /'i-fish-trap'\];?\s*$/m);
  assert.match(app, /'dock', 'fishtrap'\]/);
  assert.match(app, /'Dock', 'Fish Trap'\]/);
  // The local dispatch tables reach the new export.
  assert.match(app, /PUSH_BUILD\[24\] = 'push_build_fish_trap';/);
  assert.match(app, /PUSH_BUILD_QUEUED\[24\] = 'push_build_fish_trap_queued';/);
});

test('both dispatch paths route trap builds and the desk', () => {
  // Netplay: the build table reaches tag 24 on both the plain
  // and queued dispatch, and the wire validator takes 24 —
  // while every other kind keeps its pre-W2b wire legality
  // exactly (19/20 dispatchable but junk on the wire, 21 the
  // unconstructible Monument, 22/23 local-dispatch only, 25
  // past the end).
  assert.match(netplay, /BUILD_FNS\[24\] = 'push_build_fish_trap';/);
  assert.equal(validateCommand({ t: 'build', e: 1, k: 24, x: 3, y: 4 }), true);
  assert.equal(validateCommand({ t: 'build', e: 1, k: 19, x: 3, y: 4 }), false);
  assert.equal(validateCommand({ t: 'build', e: 1, k: 21, x: 3, y: 4 }), false);
  assert.equal(validateCommand({ t: 'build', e: 1, k: 23, x: 3, y: 4 }), false);
  assert.equal(validateCommand({ t: 'build', e: 1, k: 25, x: 3, y: 4 }), false);
  // The desk trio validates and applies on the wire.
  assert.equal(validateCommand({ t: 'queue_trap_reseed', b: 3 }), true);
  assert.equal(validateCommand({ t: 'dequeue_trap_reseed', b: 3 }), true);
  assert.equal(validateCommand({ t: 'set_auto_trap_reseed', b: 3, on: true }), true);
  assert.equal(validateCommand({ t: 'set_auto_trap_reseed', b: 3, on: 1 }), false);
  assert.match(netplay, /case 'queue_trap_reseed': return typeof sim\.push_queue_trap_reseed === 'function'/);
  assert.match(netplay, /case 'dequeue_trap_reseed': return typeof sim\.push_dequeue_trap_reseed === 'function'/);
  assert.match(netplay, /case 'set_auto_trap_reseed': return typeof sim\.push_set_auto_trap_reseed === 'function'/);
  // The local applier carries the same three cases, guarded.
  assert.match(app, /case 'queue_trap_reseed': return typeof sim\.push_queue_trap_reseed === 'function'/);
  assert.match(app, /case 'dequeue_trap_reseed': return typeof sim\.push_dequeue_trap_reseed === 'function'/);
  assert.match(app, /case 'set_auto_trap_reseed': return typeof sim\.push_set_auto_trap_reseed === 'function'/);
});

test('a human can build a trap: panel button, ghost mirror, ship placement', () => {
  // The ship panel: our own Fishing Ships in the selection get
  // the Build Fish Trap button, gated on Feudal + 100 wood.
  assert.match(app, /const selShips = context === 'units'\s*\? selUnits\.filter\(\(id\) => st\.kinds\[id\] === 23 && st\.owners\[id\] === localOwner\) : \[\];/);
  assert.match(app, /id: 'build-fish-trap', icon: BUILD_ICON\[24\], label: BUILD_NAME\[24\],/);
  assert.match(app, /disabled: trapGate \|\| lacksWood,/);
  // Its click arms placement of kind 24 (its own branch — the
  // numeric build-* parser cannot read this id).
  assert.match(app, /id === 'build-fish-trap'\) \{\s*\/\/ Water W2b: arm \(or disarm\) Fish Trap placement/);
  assert.match(app, /placement = placement && placement\.kind === 24 \? null : \{ kind: 24 \};/);
  // The ghost mirror: the water rule, hooked into mirrorValid.
  assert.match(app, /if \(kind === 24\) return mirrorTrapValid\(cx, cy\);/);
  assert.match(app, /function mirrorTrapValid\(cx, cy\) \{/);
  assert.match(app, /if \(myAge\(\) < 1\) return false; \/\/ the Fish Trap is Feudal-born/);
  assert.match(app, /if \(t < 1\) return false; \/\/ water-side only \(deep water or shallows\)/);
  // attemptPlace picks the nearest selected FISHING SHIP for
  // kind 24 (villagers for everything else) and flashes the
  // ship-specific refusal when none is selected.
  assert.match(app, /const wantKind = placement\.kind === 24 \? 23 : 0;/);
  assert.match(app, /flash\(t\(wasTrap \? 'flash\.buildSelectShip' : 'flash\.buildSelectVil'\)\);/);
});

test('right-click flows task ships onto traps and trap foundations', () => {
  // Gather pairing: a ship right-clicking a trap NODE (kind 9)
  // is sent to gather, like the fish spots (7/8).
  assert.match(app, /st\.kinds\[id\] === 23 && \(n\.kind === 7 \|\| n\.kind === 8 \|\| n\.kind === 9\)/);
  // Build-resume: an own incomplete trap takes selected ships
  // as builders; other foundations stay villager-only.
  assert.match(app, /if \(st\.kinds\[id\] !== 0 && !\(bT\.kind === 24 && st\.kinds\[id\] === 23\)\) continue;/);
});

test('the Dock panel carries the trap-reseed desk', () => {
  // Buttons ride the Dock context, complete Docks only, fed by
  // the trap_reseed_for read; the desk state joins the panel
  // signature so the counts/toggle repaint.
  assert.match(app, /if \(bk\.complete && typeof sim\.trap_reseed_for === 'function'\) \{/);
  assert.match(app, /id: 'trap-reseed-queue', icon: 'i-reseed', label: t\('dock\.trapReseedQueue'\),/);
  assert.match(app, /id: 'trap-reseed-auto', icon: 'i-croprotation', label: t\('dock\.trapReseedAuto'\),/);
  assert.match(app, /context === 'dock' && typeof sim\.trap_reseed_for === 'function'\s*\? sim\.trap_reseed_for\(localOwner\)\.join\('\.'\) : '',/);
  // Clicks issue the desk commands; right-click on the queue
  // refunds (the ECO-14 gesture, shared listener).
  assert.match(app, /issue\(\{ t: 'queue_trap_reseed', b: selectedBuilding\.id \}\)/);
  assert.match(app, /issue\(\{ t: 'set_auto_trap_reseed', b: selectedBuilding\.id, on: auto !== 1 \}\)/);
  assert.match(app, /\{ t: 'dequeue_trap_reseed', b: selectedBuilding\.id \}/);
});

test('the render tables draw the trap node and its foundation', () => {
  // Node kind 9: draw entry, full amount 715, food carry dot,
  // minimap color (the minimap table grows to 10).
  assert.match(app, /drawShoreFish, drawDeepFish, drawFishTrap\];/);
  assert.match(app, /const NODE_MAX = \[100, 125, 800, 350, 175, 340, 100, 200, 250, 715\];/);
  assert.equal(NODE_COLORS.length, 10);
  assert.equal(NODE_COLORS[9], '#5ec8a8');
  // A completed trap is drawn as its node (skipped in the
  // building pass); a foundation gets its own draw; destroyed
  // traps leave no rubble.
  assert.match(app, /if \(b\.kind === 24 && b\.complete\) continue; \/\/ drawn as a fish trap node \(Water W2b\)/);
  assert.match(app, /function drawFishTrapFoundation\(b\) \{/);
  assert.match(app, /if \(b\.kind === 1 \|\| b\.kind === 24 \|\| !b\.complete\) continue; \/\/ farms and fish traps leave no rubble/);
});

test('the i18n keys exist in all three locales', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    const s = STRINGS[loc];
    assert.ok(s['building.fishtrap'], `${loc} building.fishtrap`);
    assert.ok(s['flash.buildSelectShip'], `${loc} flash.buildSelectShip`);
    assert.ok(s['dock.trapReseedQueue'], `${loc} dock.trapReseedQueue`);
    assert.ok(s['dock.trapReseedTitle']?.includes('{n}'), `${loc} dock.trapReseedTitle names the count`);
    assert.ok(s['dock.trapReseedAuto'], `${loc} dock.trapReseedAuto`);
    assert.ok(s['dock.trapReseedAutoTitle']?.includes('{state}'), `${loc} dock.trapReseedAutoTitle names the state`);
    assert.ok(s['sel.fishingShipInfo']?.includes('Fish Trap')
      || s['sel.fishingShipInfo']?.includes('Armadilha')
      || s['sel.fishingShipInfo']?.includes('Trampa'), `${loc} ship info names the trap`);
  }
  assert.equal(STRINGS.en['building.fishtrap'], 'Fish Trap');
  assert.equal(STRINGS['pt-BR']['building.fishtrap'], 'Armadilha de Peixes');
  assert.equal(STRINGS.es['building.fishtrap'], 'Trampa para Peces');
});

test('the fish trap icon symbol exists', () => {
  assert.match(indexHtml, /<symbol id="i-fish-trap" viewBox="0 0 24 24">/);
});
