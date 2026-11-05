// Teams T2 landing pins (2026-11-05): the client wiring for
// VIC-12 (Allied Victory). The settings object gains the
// alliedVictory checkbox field, the WASM flags packing gains
// bit 7 (the sim's last free flags bit), both prematch panels
// (pause menu + create-room settings box) carry the checkbox
// beside the Package C / Package I opt-ins, and the label
// suffix names it in all three languages. A path that dropped
// the field would silently turn an allied-victory match into
// a per-player one (and, in netplay, desync it: non-default
// victory settings are hashed).
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

test('victoryFlags packs allied victory at bit 7 (functional)', () => {
  const body = fnBody('function victoryFlags(');
  const victoryFlags = new Function(`${body}\n}; return victoryFlags;`)();
  assert.equal(victoryFlags({}), 0);
  assert.equal(victoryFlags({ alliedVictory: true }), 128);
  assert.equal(victoryFlags({ regicide: true, alliedVictory: true }), 129);
  assert.equal(
    victoryFlags({ regicide: true, suddenDeath: true, relicVictory: true, deathMatch: true, empireWars: true, kingOfTheHill: true, alliedVictory: true }),
    191,
    'bit 7 composes with every client-packed bit (0-5) and collides with none; bit 6 is the sim-side Wonder bit the prematch surface does not pack',
  );
});

test('the settings object carries alliedVictory end to end', () => {
  assert.ok(appJs.includes('alliedVictory: false'), 'VICTORY_DEFAULT has the field off');
  assert.ok(fnBody('function normalizeVictory(').includes('alliedVictory: v.alliedVictory === true'),
    'normalizeVictory keeps the field (handshake/replay safe)');
  assert.ok(fnBody('function victoryIsDefault(').includes('!n.alliedVictory'),
    'an allied-only pick is non-default and rides the victory paths');
  assert.ok(fnBody('function victoryFromPick(').includes('alliedVictory: m.alliedVictory === true'),
    'victoryFromPick reads the checkbox from the modes bag');
  assert.ok(fnBody('function victoryPickOf(').includes('alliedVictory: n.alliedVictory'),
    'victoryPickOf reflects the field back into the controls');
  assert.ok(fnBody('function victoryLabel(').includes("if (n.alliedVictory) label += ' · ' + t('victory.alliedVictory');"),
    'the room config label names the setting');
});

test('buildMatchSim passes the packed flags on both construction paths', () => {
  const body = fnBody('function buildMatchSim(');
  assert.ok(body.includes('victoryFlags(v), packTeams(t)'),
    'the teams export receives the flags word (allied + same-team matches)');
  assert.ok(body.includes('v.scoreTarget, victoryFlags(v))'),
    'the victory export receives the flags word (allied + distinct teams)');
});

test('both prematch panels carry the allied victory checkbox', () => {
  for (const id of ['vic-allied', 'vic-allied-txt', 'create-victory-allied', 'set-victory-allied-name']) {
    assert.ok(indexHtml.includes(`id="${id}"`), `#${id} exists in the page`);
  }
  assert.ok(appJs.includes("document.getElementById('vic-allied')"), 'the pause-menu checkbox is wired');
  assert.ok(appJs.includes("document.getElementById('create-victory-allied')"), 'the create-panel checkbox is wired');
  assert.ok(appJs.includes('vicAlliedInput ? vicAlliedInput.checked : false'),
    'the pause-menu pick feeds chosenVictory');
  assert.ok(appJs.includes('netUi.createVictoryAllied ? netUi.createVictoryAllied.checked : false'),
    'the create-panel pick feeds the host victory settings');
  assert.ok(appJs.includes("setText('vic-allied-txt', 'victory.alliedVictory');"), 'the pause label localizes');
  assert.ok(appJs.includes("setText('set-victory-allied-name', 'victory.alliedVictory');"), 'the create label localizes');
});

test('the allied victory name exists in all three client languages', () => {
  const hits = i18nJs.split("'victory.alliedVictory':").length - 1;
  assert.equal(hits, 3, 'victory.alliedVictory is defined for en + pt-BR + es');
});
