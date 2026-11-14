// TECH_NAME completeness pins (bot-loop run #345, 2026-11-14).
//
// Live defect found in run #344: three University cards in the
// Tech Tree's Imperial column showed costs and "Requires" but
// NO name. Root cause, proven in code: refreshNames() rebuilds
// TECH_NAME from the i18n table at load, and its rebuild stopped
// at tag 47 (Medium Warships) — tags 48-52 (Careening, Clinker
// Construction, Dry Dock, Carvel Hull, Shipwright, the W5/W6
// University ship techs) were never added, so at runtime
// TECH_NAME[48..52] was undefined and the tree render
// (`TECH_NAME[e.tag] || ''`) painted an empty name; the Requires
// lines of Dry Dock / Carvel Hull and the queue/end-screen name
// lookups lost the names the same way. The static fallback table
// had the mirror-image gap: tags 37-44 (Redemption, Heresy,
// Block Printing, Illumination, Theocracy, Faith, Bow Saw,
// Two-Man Saw) missing. The i18n strings themselves always
// existed in all three locales — only the two TECH_NAME tables
// were incomplete. These tests pin BOTH tables against the
// TECH_TREE mirror so no future tech can ship nameless again.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';
import { TECH_TREE } from './techtree.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');

/** Extract the object-literal body of the FIRST `marker { ... };` block. */
function tableBody(marker, from = 0) {
  const a = app.indexOf(marker, from);
  assert.ok(a >= 0, `marker found: ${marker}`);
  const open = app.indexOf('{', a);
  const close = app.indexOf('};', open);
  assert.ok(open >= 0 && close > open, `block bounds for: ${marker}`);
  return app.slice(open + 1, close);
}

/** The set of numeric keys present in a TECH_NAME literal body. */
function literalKeys(body) {
  return new Set([...body.matchAll(/(?:^|,)\s*(\d+):/g)].map((m) => Number(m[1])));
}

test('the static TECH_NAME fallback names every TECH_TREE tag', () => {
  const keys = literalKeys(tableBody('let TECH_NAME ='));
  for (const e of TECH_TREE) {
    assert.ok(keys.has(e.tag), `static TECH_NAME missing tag ${e.tag}`);
  }
});

test('refreshNames() rebuilds a name for every TECH_TREE tag', () => {
  const fnAt = app.indexOf('function refreshNames()');
  assert.ok(fnAt >= 0, 'refreshNames found');
  const body = tableBody('TECH_NAME =', fnAt);
  const keys = literalKeys(body);
  for (const e of TECH_TREE) {
    assert.ok(keys.has(e.tag), `refreshNames TECH_NAME missing tag ${e.tag}`);
  }
  // Every i18n key the rebuild references must resolve to a
  // non-empty string in all three locales (the tree renders in
  // whichever locale is active).
  for (const m of body.matchAll(/t\('([^']+)'\)/g)) {
    for (const loc of ['en', 'pt-BR', 'es']) {
      assert.ok(STRINGS[loc][m[1]]?.length > 0, `${loc}:${m[1]} present`);
    }
  }
});

test('the W5/W6 University ship techs keep their names after refreshNames', () => {
  const fnAt = app.indexOf('function refreshNames()');
  const body = tableBody('TECH_NAME =', fnAt);
  for (const [tag, key] of [
    [48, 'tech.careening'], [49, 'tech.clinkerconstruction'],
    [50, 'tech.drydock'], [51, 'tech.carvelhull'], [52, 'tech.shipwright'],
  ]) {
    assert.ok(body.includes(`${tag}: t('${key}')`), `refreshNames names tag ${tag} via ${key}`);
  }
  assert.equal(STRINGS.en['tech.drydock'], 'Dry Dock');
  assert.equal(STRINGS.en['tech.carvelhull'], 'Carvel Hull');
  assert.equal(STRINGS.en['tech.shipwright'], 'Shipwright');
});
