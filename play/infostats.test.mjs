// INF-05 regression pins (bot-loop, 2026-10-28).
//
// Rulebook INF-05: a single selected unit/building shows its
// portrait, HP bar, attack/armor stats, and (for production
// buildings) queue and rally. Everything shipped except the
// attack/armor stats — the panel showed name + HP only. The stats
// must come from the sim's own effective rows (tech patches and
// age steps included), never from a client-side table that could
// drift from combat: the client reads them through the
// entity_display_stats / building_display_stats wasm getters,
// which wrap Sim::stats_for / Sim::building_weapon + the building
// armor rows.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');
const libRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'lib.rs'), 'utf8');
const wasmRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'wasm.rs'), 'utf8');

test('the sim exposes effective display stats for entities and buildings', () => {
  assert.ok(libRs.includes('pub fn entity_display_stats'), 'Sim::entity_display_stats exists');
  assert.ok(libRs.includes('pub fn building_display_stats'), 'Sim::building_display_stats exists');
  assert.ok(libRs.includes('self.unit_stats(idx)'),
    'entity stats read through unit_stats (the combat row)');
  assert.ok(libRs.includes('building_weapon(b.owner, b.kind)'),
    'building attack reads through building_weapon (tech patches included)');
  assert.ok(wasmRs.includes('pub fn entity_display_stats'), 'wasm entity_display_stats exported');
  assert.ok(wasmRs.includes('pub fn building_display_stats'), 'wasm building_display_stats exported');
});

test('the selection panel reads its stats through the sim getters', () => {
  assert.ok(appJs.includes('sim.entity_display_stats(selUnits[0])'),
    'single unit stats come from the sim');
  assert.ok(appJs.includes('sim.entity_display_stats(selectedBuilding.id)'),
    'Town Center stats come from the sim (the TC is an entity)');
  assert.ok(appJs.includes('sim.building_display_stats(bk.id)'),
    'placed-building stats come from the sim');
  assert.ok(appJs.includes("t('sel.statsAtk'") && appJs.includes("t('sel.statsNoAtk'"),
    'the panel renders the stats line for armed and unarmed selections');
});

test('the stats join the panel signature so research refreshes them', () => {
  assert.ok(appJs.includes("selStats ? selStats.join(',') : ''"),
    'selStats is part of the render signature');
});

test('the stats strings exist in every locale', () => {
  for (const key of ['sel.statsAtk', 'sel.statsNoAtk']) {
    const hits = i18nJs.match(new RegExp(`'${key.replace('.', '\\.')}':`, 'g')) || [];
    assert.equal(hits.length, 3, `${key} is defined for EN, PT and ES`);
  }
  const atk = [...i18nJs.matchAll(/'sel\.statsAtk':\s*'([^']+)'/g)].map((m) => m[1]);
  assert.ok(atk[0].includes('Attack'), 'EN stats line names the attack');
  assert.ok(atk[1].includes('Ataque'), 'PT stats line names the attack');
  assert.ok(atk[2].includes('Ataque'), 'ES stats line names the attack');
});
