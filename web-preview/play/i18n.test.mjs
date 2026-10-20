// i18n unit tests: lookup, interpolation, EN fallback, locale
// resolution order, and the en/pt-BR/es KEY-PARITY gate (the
// suite fails if any locale's key set differs from English).
// Run from web-preview/: node --test play/i18n.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  STRINGS, LOCALES, tIn, resolveLocale, genderOf, costText,
} from './i18n.js';

test('key parity: en, pt-BR and es carry identical key sets', () => {
  const en = Object.keys(STRINGS.en).sort();
  assert.ok(en.length > 250, `expected a full table, got ${en.length} keys`);
  for (const loc of ['pt-BR', 'es']) {
    const keys = Object.keys(STRINGS[loc]).sort();
    assert.deepEqual(keys, en, `${loc} key set differs from en`);
  }
  assert.deepEqual(LOCALES, ['en', 'pt-BR', 'es']);
});

test('no empty or untranslated-EN values in pt-BR / es for the core vocabulary', () => {
  for (const loc of ['pt-BR', 'es']) {
    for (const [key, value] of Object.entries(STRINGS[loc])) {
      assert.equal(typeof value, 'string', `${loc}:${key} not a string`);
      assert.ok(value.length > 0, `${loc}:${key} is empty`);
    }
  }
  // The DE-official vocabulary the milestone is anchored on.
  assert.equal(STRINGS['pt-BR']['unit.villager'], 'Aldeão');
  assert.equal(STRINGS['pt-BR']['unit.townCenter'], 'Centro da Cidade');
  assert.equal(STRINGS['pt-BR']['age.feudal'], 'Idade Feudal');
  assert.equal(STRINGS['pt-BR']['tech.loom'], 'Tear');
  assert.equal(STRINGS['pt-BR']['end.victory'], 'Vitória!');
  assert.equal(STRINGS.es['unit.villager'], 'Aldeano');
  assert.equal(STRINGS.es['unit.townCenter'], 'Centro Urbano');
  assert.equal(STRINGS.es['age.feudal'], 'Edad Feudal');
  assert.equal(STRINGS.es['tech.loom'], 'Telar');
  assert.equal(STRINGS.es['end.victory'], '¡Victoria!');
});

test('lookup + interpolation in each locale', () => {
  assert.equal(tIn('en', 'cmd.trainVilBlocked', { block: 'X.' }), 'Train Villager — X.');
  assert.equal(tIn('pt-BR', 'lobby.titleAuto', { region: 'América do Sul' }), '1v1 Arábia — América do Sul');
  assert.equal(tIn('es', 'net.ping', { ms: 42 }), 'ping 42 ms');
  assert.equal(tIn('en', 'end.detailVictory', { time: '12:34' }),
    'The enemy has been eliminated. Match time 12:34.');
  // A repeated placeholder interpolates at every site.
  assert.equal(tIn('pt-BR', 'cmd.feudalDone', { age: 'Idade Feudal' }),
    'Idade Feudal — você está na Idade Feudal.');
});

test('fallback: missing locale -> en; missing key -> the key itself', () => {
  assert.equal(tIn('fr', 'unit.villager'), 'Villager');
  assert.equal(tIn('pt-BR', 'no.such.key'), 'no.such.key');
  // A placeholder with no var stays visible rather than crashing.
  assert.equal(tIn('en', 'net.ping'), 'ping {ms} ms');
});

test('locale resolution order: stored wins, then navigator language', () => {
  assert.equal(resolveLocale({ stored: 'es', nav: 'pt-BR' }), 'es');
  assert.equal(resolveLocale({ stored: 'pt-BR', nav: 'en-US' }), 'pt-BR');
  assert.equal(resolveLocale({ stored: 'bogus', nav: 'es-MX' }), 'es');
  assert.equal(resolveLocale({ nav: 'pt-PT' }), 'pt-BR');
  assert.equal(resolveLocale({ nav: 'pt' }), 'pt-BR');
  assert.equal(resolveLocale({ nav: 'es' }), 'es');
  assert.equal(resolveLocale({ nav: 'en-GB' }), 'en');
  assert.equal(resolveLocale({ nav: 'de-DE' }), 'en');
  assert.equal(resolveLocale({}), 'en');
  assert.equal(resolveLocale({ nav: null }), 'en');
});

test('gender tables drive the feed participles', () => {
  assert.equal(genderOf('pt-BR', 'building', 'house'), 'f');
  assert.equal(genderOf('pt-BR', 'building', 'barracks'), 'm');
  assert.equal(genderOf('es', 'building', 'range'), 'f');
  assert.equal(genderOf('es', 'unit', 'militia'), 'f');
  assert.equal(genderOf('en', 'building', 'house'), 'm'); // unused in en
  assert.equal(tIn('pt-BR', 'feed.built.f', { name: 'Casa' }), 'Casa construída');
  assert.equal(tIn('es', 'feed.created.f', { name: 'Milicia' }), 'Milicia creada');
  assert.equal(tIn('pt-BR', 'feed.created.m', { name: 'Aldeão' }), 'Aldeão criado');
});

test('costText joins localized resource parts', () => {
  assert.equal(costText([[60, 'food'], [20, 'gold']], 'en'), '60 food · 20 gold');
  assert.equal(costText([[25, 'wood']], 'pt-BR'), '25 de madeira');
  assert.equal(costText([[100, 'food'], [50, 'gold']], 'es'), '100 de alimento · 50 de oro');
});
