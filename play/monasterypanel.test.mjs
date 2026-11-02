// Monastery panel + tech-tag boundary pins (bot-loop, 2026-11-01)
// — regression for the live Train Monk defect family found on the
// P5a/P5e builds: training a Monk deducted 100 gold and consumed
// a population slot, but the Monastery panel showed NO queue /
// progress line and NO cancel chip, so the click read as "nothing
// happened" (the same complaint family as the Market queue line,
// bot-loop 2026-10-30, and the Siege Workshop queue line,
// b57befa2). Root causes pinned here:
//   1. updateSelPanel's PROD_KINDS — the list that earns a
//      building the shared production info shape (queue line +
//      cancel chips) — was never extended to kind 20 when the
//      Monastery landed in Castle P5a, so it fell into the plain
//      'building' shape whose info line is empty.
//   2. The command panel's GAR_PANEL list (which hosts get the
//      Ungarrison button) likewise omitted 'monastery', so a
//      rally-garrisoned Monk (GAR-08) could never be ejected from
//      the panel of his own Monastery.
//   3. Tech tags >= 32 (Sanctity 32, Fervor 33, Herbal Medicine
//      34) were truncated at the wasm boundary (tech_mask /
//      tech_mask_for returned u32; researched_techs_for scanned
//      only tags 0..31, dropping Crop Rotation 31 too) and the
//      client's `mask & (1 << tag)` checks aliased them onto the
//      low bits — Fervor (33) read "researched" off the Feudal
//      bit in two independent live sessions that never researched
//      it. The mask now crosses as f64 and the bit tests are
//      arithmetic (techMaskHas in app.js, techDone here).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { techDone } from './techtree.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const wasmRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'wasm.rs'), 'utf8');

test('Monastery (kind 20) earns the production panel shape', () => {
  const m = appJs.match(/const PROD_KINDS = \[([^\]]*)\]/);
  assert.ok(m, 'PROD_KINDS found in app.js');
  const kinds = m[1].split(',').map((x) => Number(x.trim()));
  assert.ok(kinds.includes(20),
    'PROD_KINDS includes 20 — a selected Monastery shows its training queue line + cancel chips');
});

test('Monastery panel carries the Ungarrison button', () => {
  const m = appJs.match(/const GAR_PANEL = \[([^\]]*)\]/);
  assert.ok(m, 'GAR_PANEL found in app.js');
  assert.ok(m[1].includes("'monastery'"),
    "GAR_PANEL includes 'monastery' — a garrisoned Monk can be ejected from the Monastery panel");
});

test('Monastery research faces test the mask arithmetically', () => {
  const block = appJs.match(/context === 'monastery'[\s\S]*?\n  \} else if \(context === 'blacksmith'\)/);
  assert.ok(block, 'monastery command block found');
  assert.match(block[0], /const done = techMaskHas\(techMask, tag\)/,
    'monastery techBtn reads done via techMaskHas (no 32-bit shift alias)');
});

test('wasm tech masks cross the boundary untruncated', () => {
  assert.match(wasmRs, /pub fn tech_mask\(&self\) -> f64/,
    'tech_mask returns f64 (full u64 mask), not u32');
  assert.match(wasmRs, /pub fn tech_mask_for\(&self, owner: u32\) -> f64/,
    'tech_mask_for returns f64 (full u64 mask), not u32');
  const rtf = wasmRs.match(/pub fn researched_techs_for[\s\S]*?\n    \}/);
  assert.ok(rtf, 'researched_techs_for found');
  assert.match(rtf[0], /\(0\.\.64u32\)/,
    'researched_techs_for scans the full tag range (31 Crop Rotation + 32..34 Monastery techs included)');
});

test('techDone reads high tags off the f64 mask, never the aliased low bits', () => {
  const feudalOnly = 2; // bit 1 — the Feudal Age advancement bit
  assert.equal(techDone(33, feudalOnly, 2), false,
    'Fervor (33) is NOT done off the Feudal bit alone (the live phantom)');
  assert.equal(techDone(33, 2 ** 33, 2), true, 'Fervor (33) done when its own bit is set');
  assert.equal(techDone(32, 1, 2), false, 'Sanctity (32) is NOT done off the Loom bit alone');
  assert.equal(techDone(32, 2 ** 32, 2), true, 'Sanctity (32) done when its own bit is set');
  assert.equal(techDone(34, 2 ** 34, 2), true, 'Herbal Medicine (34) done when its own bit is set');
  assert.equal(techDone(31, 2 ** 31, 2), true, 'Crop Rotation (31) done when its own bit is set');
  assert.equal(techDone(0, 1, 0), true, 'Loom (0) still reads off bit 0');
  assert.equal(techDone(1, 0, 1), true, 'Feudal advancement still reads off the age, not a bit');
});

test('Castle P5b: Monastery panel carries Atonement + Devotion at tags 35/36', () => {
  assert.match(appJs, /techBtn\('research-atonement', 35, 'i-atonement', 'cmd\.atonementTitle', 40, 325, 4\)/,
    'Atonement button rides tag 35 at 325 gold / 40 s');
  assert.match(appJs, /id: 'research-devotion'[\s\S]*?cost: 100, cost2Icon: 'i-gold', cost2: 200/,
    'Devotion button carries its mixed cost (100 food / 200 gold)');
  assert.match(appJs, /'research-atonement': 35, 'research-devotion': 36/,
    'research dispatch maps both buttons to tags 35/36');
  assert.match(appJs, /35: 'Atonement', 36: 'Devotion'/,
    'TECH_NAME carries both new techs');
});

test('Castle P5b: Convert order path — dispatch, smart order, arm, faith line', () => {
  assert.match(appJs, /case 'convert': return sim\.push_convert\(cmd\.e, cmd\.tg\)/,
    'applyCommand dispatches convert to the sim');
  assert.match(appJs, /issue\(\{ t: 'convert', e: id, tg: t \}\)/,
    'right-click smart order issues convert for selected monks');
  assert.match(appJs, /id: 'convert', icon: 'i-convert'/,
    'the monk panel carries the Convert button');
  assert.match(appJs, /sim\.monk_faith\(id\)/,
    'the monk selection line reads faith from the sim');
  assert.match(wasmRs, /pub fn push_convert/,
    'the wasm boundary exports push_convert');
  assert.match(wasmRs, /pub fn monk_faith/,
    'the wasm boundary exports monk_faith');
});
