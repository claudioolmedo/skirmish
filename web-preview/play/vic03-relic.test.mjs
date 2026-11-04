// VIC-03 landing pins: the client wiring for relic victory.
// Package C's settings object gains the relicVictory toggle —
// these pins keep it flowing through every path the sibling
// conditions take (normalize → flags → buildMatchSim, the
// prematch controls, the host's create panel, the localized
// label), plus the top-bar countdown readout the condition
// shows while an all-relics countdown runs. A path that drops
// the flag silently turns a relic match into Conquest (and, in
// netplay, desyncs it: the settings are hashed off-default).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');
const indexHtml = readFileSync(join(here, 'index.html'), 'utf8');

function fnBody(sig) {
  const start = appJs.indexOf(sig);
  assert.ok(start >= 0, `${sig} exists`);
  const end = appJs.indexOf('\n}\n', start);
  assert.ok(end > start, `${sig} is delimited`);
  return appJs.slice(start, end);
}

test('the relic toggle rides the settings object end to end', () => {
  assert.ok(appJs.includes('relicVictory: false'),
    'VICTORY_DEFAULT carries relicVictory off');
  assert.ok(fnBody('function normalizeVictory(').includes('relicVictory: v.relicVictory === true'),
    'normalizeVictory keeps the toggle');
  assert.ok(fnBody('function victoryIsDefault(').includes('!n.relicVictory'),
    'a relic match is not default (it must build via new_match_victory)');
  assert.ok(fnBody('function victoryFromPick(').includes('relicVictory: relic === true'),
    'the prematch pick sets the toggle');
  assert.ok(fnBody('function victoryPickOf(').includes('relicVictory: n.relicVictory'),
    'the inverse pick reflects the toggle into the controls');
});

test('the WASM flags packing adds bit 2 without moving bits 0/1', () => {
  const body = fnBody('function victoryFlags(');
  assert.ok(body.includes('(v.regicide ? 1 : 0)'), 'bit 0 stays Regicide');
  assert.ok(body.includes('(v.suddenDeath ? 2 : 0)'), 'bit 1 stays Sudden Death');
  assert.ok(body.includes('(v.relicVictory ? 4 : 0)'), 'bit 2 is Relic victory');
});

test('the label names the relic condition', () => {
  assert.ok(fnBody('function victoryLabel(').includes("label += ' · ' + t('victory.relic')"),
    'victoryLabel appends the relic suffix');
});

test('prematch relic controls exist for solo and for the create panel', () => {
  for (const id of ['vic-relic', 'vic-relic-txt', 'create-victory-relic', 'set-victory-relic-name']) {
    assert.ok(indexHtml.includes(`id="${id}"`), `#${id} exists in the page`);
  }
  assert.ok(appJs.includes("document.getElementById('vic-relic')"),
    'the pause menu checkbox is read');
  assert.ok(appJs.includes('vicRelicInput ? vicRelicInput.checked : false'),
    'readVictoryInputs feeds the checkbox into chosenVictory');
  assert.ok(appJs.includes('netUi.createVictoryRelic ? netUi.createVictoryRelic.checked : false'),
    'the host create handler reads the relic pick');
});

test('relic strings exist in all three client languages', () => {
  for (const key of ['victory.relic', 'victory.relicCountdown', 'victory.relicCountdownFoe']) {
    const hits = i18nJs.split(`'${key}':`).length - 1;
    assert.equal(hits, 3, `${key} is defined for en + pt-BR + es`);
  }
});

test('the top bar carries the countdown readout', () => {
  assert.ok(indexHtml.includes('id="relic-countdown-chip"'), 'the chip exists in the top bar');
  assert.ok(indexHtml.includes('id="relic-countdown"'), 'the chip value exists');
  const body = fnBody('function updateTopBar(');
  assert.ok(body.includes('sim.relic_countdown_holder()'), 'the readout reads the holder getter');
  assert.ok(body.includes('sim.relic_countdown_ticks_left()'), 'the readout reads the ticks-left getter');
  assert.ok(body.includes("'victory.relicCountdown'"), 'own countdown uses the own string');
  assert.ok(body.includes("'victory.relicCountdownFoe'"), 'foe countdown uses the foe string');
});
