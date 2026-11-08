// Esc label truthfulness pins (bot-loop, 2026-11-07, proof #182).
//
// Defect (live-confirmed in proof #182 on the real client): three
// player-facing texts claimed Escape PAUSES / opens the menu —
//   * the top-bar Menu button's title, "Menu (Esc)" (all locales);
//   * the How to Play guide's last bullet, "Esc pauses.";
//   * the landing page hint, "Esc pauses.".
// In the same live session, pressing Esc with a villager selected
// ONLY deselected: the pause menu did not open. That is the design
// (Claudio, 2026-10-23): Escape NEVER shows the pause menu — the
// Menu button is the only door — and one Esc press does exactly one
// step of the stack: cancel an armed state, close a transient
// panel, deselect (see esclock.test.mjs). The labels contradicted
// the behaviour a player actually gets, which is exactly the class
// of menu confusion Claudio reported on 2026-10-24.
//
// Fix (presentation/text only; sim, wasm and the Esc stack itself
// untouched): the title is just "Menu" / "Menú", and every Esc
// sentence now describes the real stack: cancels, closes panels
// and deselects. These pins keep the false claim from returning
// in any locale or fallback surface.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS } from './i18n.js';

const playIndex = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const landingIndex = readFileSync(new URL('../index.html', import.meta.url), 'utf8');

const MENU_TITLE = { en: 'Menu', 'pt-BR': 'Menu', es: 'Menú' };
const ESC_LINE = {
  en: '<strong>Esc</strong> cancels, closes panels and deselects.',
  'pt-BR': '<strong>Esc</strong> cancela, fecha painéis e desseleciona.',
  es: '<strong>Esc</strong> cancela, cierra paneles y deselecciona.',
};

test('the Menu button title never promises Esc, in any locale', () => {
  for (const loc of Object.keys(MENU_TITLE)) {
    assert.equal(STRINGS[loc]['top.menuTitle'], MENU_TITLE[loc],
      `${loc} top.menuTitle must be the plain Menu title`);
    assert.ok(!STRINGS[loc]['top.menuTitle'].includes('Esc'),
      `${loc} top.menuTitle must not mention Esc`);
  }
  assert.ok(playIndex.includes('id="pause-btn" class="btn btn-ghost btn-sm" type="button" title="Menu"'),
    'the index.html fallback pause button carries title="Menu"');
  assert.ok(!playIndex.includes('Menu (Esc)'),
    'the index.html fallback must not carry the old false title');
});

test('the guide Esc bullet describes the real Esc stack, in any locale', () => {
  for (const loc of Object.keys(ESC_LINE)) {
    assert.equal(STRINGS[loc]['help.i16'], ESC_LINE[loc],
      `${loc} help.i16 must describe cancel/close/deselect, not pausing`);
  }
  assert.ok(playIndex.includes(`<li>${ESC_LINE.en}</li>`),
    'the index.html fallback guide carries the same Esc bullet');
});

test('no player-facing surface claims Esc pauses', () => {
  for (const loc of Object.keys(ESC_LINE)) {
    for (const [key, value] of Object.entries(STRINGS[loc])) {
      assert.ok(!(value.includes('Esc') && /paus/i.test(value) && key !== 'help.escNote'),
        `${loc} ${key} must not claim Esc pauses: ${value}`);
    }
  }
  assert.ok(!playIndex.includes('Esc</strong> pauses'),
    'the play fallback must not claim Esc pauses');
  assert.ok(landingIndex.includes('Esc cancels, closes panels and deselects.'),
    'the landing hint describes the real Esc behaviour');
  assert.ok(!landingIndex.includes('Esc pauses'),
    'the landing hint must not claim Esc pauses');
});
