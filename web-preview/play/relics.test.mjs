// Relics client pins (Castle P5d, 2026-11-02) — the client half
// of MNK-09/ECO-16: the relic kind (tag 21) is named, iconed and
// drawn; a carried relic rides as its monk's prop and a
// garrisoned one leaves the map; the Monastery panel shows the
// store count and offers the retrieve; the right-click paths
// issue pickup/deposit through the ordinary order vocabulary,
// mirrored in netplay. These pins read the sources the way
// monasteryclient.test.mjs does — a dropped case or a missing
// table entry fails here, not live.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const netplayJs = readFileSync(join(here, 'netplay.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');
const wasmRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'wasm.rs'), 'utf8');

test('the relic kind rides tag 21 in every client table', () => {
  assert.match(appJs, /const KIND_RELIC = 21;/, 'KIND_RELIC constant');
  assert.match(appJs, /'monk', 'relic', 'king'\];/, 'KIND_I18N[21] is relic (Package C appended king at 22)');
  assert.match(appJs, /20: 'Monk', 21: 'Relic',/, 'KIND_NAME[21]');
  assert.match(appJs, /20: 'i-monk', 21: 'i-gold',/, 'KIND_ICON[21] (portrait fallback)');
});

test('the draw dispatch draws relics and hides the off-ground ones', () => {
  assert.match(appJs, /drawRelic, \/\/ 21: Relic \(Castle P5d\)/, 'UNIT_DRAW[21] (Package C appended drawKing at 22)');
  assert.match(appJs, /function drawRelic\(i, wx, wy\)/, 'drawRelic exists');
  assert.match(appJs, /kinds\[i\] === KIND_RELIC && relicStateOf\(i\) && relicStateOf\(i\)\.state !== 0/,
    'the draw loop skips carried/garrisoned relics at their stored position');
  assert.match(appJs, /if \(carriedRelicOf\(i\) != null\) \{\s*paintRelicFigure/,
    'drawMonk props the reliquary over a carrying monk');
});

test('the relic side channel is read through the cached reader', () => {
  assert.match(appJs, /function refreshRelicCache\(\)/, 'the per-tick cache exists');
  assert.match(appJs, /sim\.relic_states\(\)/, 'reads the wasm side channel');
  assert.match(appJs, /function firstGarrisonedRelic\(buildingId\)/, 'retrieve lookup exists');
});

test('orders dispatch through the ordinary vocabulary, both seats', () => {
  assert.match(appJs, /case 'pickup': return sim\.push_pickup_relic\(cmd\.e, cmd\.tg\);/,
    'applyLocalOrder pickup');
  assert.match(appJs, /case 'deposit': return sim\.push_deposit_relic\(cmd\.e, cmd\.b\);/,
    'applyLocalOrder deposit');
  assert.match(netplayJs, /case 'pickup': return isInt\(c\.e\) && isInt\(c\.tg\);/,
    'netplay validates pickup');
  assert.match(netplayJs, /case 'deposit': return isInt\(c\.e\) && isInt\(c\.b\);/,
    'netplay validates deposit');
  assert.match(netplayJs, /case 'pickup': return sim\.push_pickup_relic\(cmd\.e, cmd\.tg\);/,
    'netplay applies pickup');
  assert.match(netplayJs, /case 'deposit': return sim\.push_deposit_relic\(cmd\.e, cmd\.b\);/,
    'netplay applies deposit');
});

test('the right-click paths issue pickup before attack, deposit at an own Monastery', () => {
  const pickupAt = appJs.indexOf("st.kinds[t] === KIND_RELIC");
  const attackAt = appJs.indexOf("st.owners[t] === 1 || st.owners[t] === foeOwner()");
  assert.ok(pickupAt > 0 && attackAt > pickupAt,
    'the relic-pickup block precedes the generic Gaia-attack block');
  assert.match(appJs, /issue\(\{ t: 'pickup', e: id, tg: t \}\)/, 'pickup order issued');
  assert.match(appJs, /issue\(\{ t: 'deposit', e: id, b: bT\.id \}\)/, 'deposit order issued');
  assert.match(appJs, /issue\(\{ t: 'pickup', e: id, tg: rid \}\)/, 'retrieve order issued');
});

test('the panels surface the carry, the relic and the store count', () => {
  assert.match(appJs, /t\('sel\.relicInfo', \{ hp: hpTxt \}\)/, 'relic selection line');
  assert.match(appJs, /t\('sel\.carryingRelic'\)/, 'monk carrying suffix');
  assert.match(appJs, /sim\.monastery_relic_count\(bk\.id\)/, 'Monastery panel reads the count');
  assert.match(appJs, /t\('sel\.relicLine', \{ n: rN \}\)/, 'Monastery info line shows the count');
  assert.match(appJs, /id: 'retrieve-relic'/, 'retrieve button in the Monastery grid');
  assert.match(appJs, /id === 'retrieve-relic'/, 'retrieve button handler');
});

test('the wasm exports the client calls all exist', () => {
  for (const name of ['push_pickup_relic', 'push_deposit_relic', 'carrying_relic',
    'monastery_relic_count', 'relic_states']) {
    assert.match(wasmRs, new RegExp(`pub fn ${name}\\(`), `wasm export ${name}`);
  }
});

test('the new strings exist in all three locales', () => {
  for (const key of ['unit.relic', 'sel.relicInfo', 'sel.carryingRelic', 'sel.relicLine',
    'cmd.retrieveRelic', 'cmd.retrieveRelicTitle', 'flash.noFreeMonk']) {
    const n = i18nJs.split(`'${key}':`).length - 1;
    assert.equal(n, 3, `${key} present in EN, PT-BR and ES (found ${n})`);
  }
});

test('the ground relic draws on explored ground, not only while visible (2026-11-03 fix)', () => {
  assert.match(appJs, /kinds\[i\] === KIND_RELIC && st\.hp\[i \* 2\] >= 1/,
    'the draw loop has a dedicated ground-relic branch');
  assert.match(appJs, /if \(!cellExploredW\(wx, wy\)\) continue;/,
    'the ground relic is gated on explored, not on currently visible');
  assert.match(appJs, /21: '#e8c95a'/, 'the minimap plots the relic in relic gold');
  assert.match(appJs, /if \(f < 1 \|\| \(rs && rs\.state !== 0\)\) continue;/,
    'the minimap relic plots once explored and only while on the ground');
});
