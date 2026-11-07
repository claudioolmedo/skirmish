// Naval & Transport guide section pins (bot-loop builder package).
//
// The How to Play guide (help.i1..i16, rendered positionally by
// app.js onto the .howto <li>s in index.html) gained a compact
// naval section at help.i13–i15 — Dock / Fishing Ships / fish
// traps, the Transport Ship, warships by age, the Trade Cog and
// the University ship technologies — with Esc renumbered to
// help.i16, still last. Presentation only: these pins keep the
// section present, localized and fact-true in all three locales
// and in the index.html fallback markup.
// Run from web-preview/: node --test play/navalguide.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';

const indexHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const appJs = readFileSync(new URL('../app.js', import.meta.url), 'utf8');

const LOCALES = ['en', 'pt-BR', 'es'];
const HEADINGS = {
  en: '<strong>Naval &amp; Transport — the Dock:</strong>',
  'pt-BR': '<strong>Naval e Transporte — o Porto:</strong>',
  es: '<strong>Naval y Transporte — el Muelle:</strong>',
};

test('the naval section exists at help.i13-i15 in every locale, Esc last at i16', () => {
  for (const loc of LOCALES) {
    for (const n of [13, 14, 15, 16]) {
      const v = STRINGS[loc][`help.i${n}`];
      assert.equal(typeof v, 'string', `${loc} help.i${n} missing`);
      assert.ok(v.length > 0, `${loc} help.i${n} is empty`);
    }
    assert.ok(
      STRINGS[loc]['help.i13'].startsWith(HEADINGS[loc]),
      `${loc} help.i13 must open with the Naval & Transport heading`);
  }
  // Esc stays the last item, renumbered i13 -> i16, text unchanged.
  assert.equal(STRINGS.en['help.i16'], '<strong>Esc</strong> pauses.');
  assert.equal(STRINGS['pt-BR']['help.i16'], '<strong>Esc</strong> pausa.');
  assert.equal(STRINGS.es['help.i16'], '<strong>Esc</strong> pausa.');
});

test('help.i13 covers the Dock, Fishing Ships and fish traps with localized names', () => {
  for (const loc of LOCALES) {
    const s = STRINGS[loc]['help.i13'];
    for (const key of ['building.dock', 'unit.fishingShip', 'building.fishtrap',
      'tech.fishinglines', 'tech.gillnets']) {
      assert.ok(s.includes(STRINGS[loc][key]),
        `${loc} help.i13 must name ${key} (${STRINGS[loc][key]})`);
    }
    assert.ok(s.includes('150'), `${loc} help.i13 keeps the Dock's 150-wood cost`);
    assert.ok(s.includes('75'), `${loc} help.i13 keeps the Fishing Ship's 75-wood cost`);
    assert.ok(s.includes('100'), `${loc} help.i13 keeps the Fish Trap's 100-wood cost`);
  }
});

test('help.i14 covers the Transport Ship (5 aboard, load + Unload) and warships by age', () => {
  for (const loc of LOCALES) {
    const s = STRINGS[loc]['help.i14'];
    for (const key of ['unit.transportShip', 'unit.galley', 'unit.fireGalley',
      'unit.demolitionRaft', 'unit.hulk', 'tech.mediumwarships',
      'unit.warGalley', 'unit.fireShip', 'unit.demolitionShip', 'unit.warHulk']) {
      assert.ok(s.includes(STRINGS[loc][key]),
        `${loc} help.i14 must name ${key} (${STRINGS[loc][key]})`);
    }
    assert.ok(s.includes('<strong>5</strong>'),
      `${loc} help.i14 keeps the Transport Ship's 5-unit capacity`);
    assert.ok(s.includes('125'), `${loc} help.i14 keeps the Transport Ship's 125-wood cost`);
    assert.ok(s.includes(STRINGS[loc]['cmd.unloadTransport']),
      `${loc} help.i14 names the Unload command (${STRINGS[loc]['cmd.unloadTransport']})`);
    // Feudal line first, Castle upgrade after it.
    assert.ok(
      s.indexOf(STRINGS[loc]['unit.galley']) < s.indexOf(STRINGS[loc]['unit.warGalley']),
      `${loc} help.i14 presents the Feudal warships before their Castle upgrades`);
  }
});

test('help.i15 covers the Trade Cog / foreign Dock and the University ship technologies', () => {
  for (const loc of LOCALES) {
    const s = STRINGS[loc]['help.i15'];
    for (const key of ['unit.tradeCog', 'building.dock', 'building.university',
      'tech.careening', 'tech.clinkerconstruction', 'tech.drydock',
      'tech.carvelhull', 'tech.shipwright', 'unit.tradeCart']) {
      assert.ok(s.includes(STRINGS[loc][key]),
        `${loc} help.i15 must name ${key} (${STRINGS[loc][key]})`);
    }
    // Castle pair before the Imperial trio.
    assert.ok(
      s.indexOf(STRINGS[loc]['tech.careening']) < s.indexOf(STRINGS[loc]['tech.drydock']),
      `${loc} help.i15 presents the Castle University pair before the Imperial trio`);
  }
});

test('index.html fallback carries the same naval items, 16 guide items in all', () => {
  const howto = indexHtml.match(/<div class="howto">([\s\S]*?)<\/div>/);
  assert.ok(howto, 'the .howto block exists in play/index.html');
  const items = [...howto[1].matchAll(/<li>([\s\S]*?)<\/li>/g)].map((m) => m[1]);
  assert.equal(items.length, 16, 'the guide now lists 16 items (13 + the 3 naval items)');
  // The new items' fallback markup matches the EN strings exactly,
  // so a no-JS / pre-localization render shows the same section.
  assert.equal(items[12], STRINGS.en['help.i13']);
  assert.equal(items[13], STRINGS.en['help.i14']);
  assert.equal(items[14], STRINGS.en['help.i15']);
  assert.equal(items[15], STRINGS.en['help.i16']);
});

test('app.js still renders the guide positionally (help.i + index)', () => {
  assert.match(appJs, /document\.querySelectorAll\('\.howto li'\)\.forEach\(\(li, i\) => \{\s*li\.innerHTML = t\('help\.i' \+ \(i \+ 1\)\);/);
});
