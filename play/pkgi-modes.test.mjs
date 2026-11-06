// Package I landing pins (2026-11-04): the client + netplay
// wiring for VIC-09 (Death Match), VIC-10 (Empire Wars) and
// VIC-08 + AI-09 (King of the Hill). The sim package (64905bb)
// extended VictorySettings and the new_match_victory flags word
// with NO client call site; these pins keep the three modes
// selectable in both prematch surfaces, packed at the landed
// ABI bits, and readable in-match — a mode the client cannot
// set silently plays as Conquest (and, in netplay, desyncs:
// the settings are hashed whenever non-default).
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

test('victoryFlags packs the Package I modes at bits 3/4/5', () => {
  const body = fnBody('function victoryFlags(');
  assert.ok(body.includes('(v.deathMatch ? 8 : 0)'), 'Death Match rides bit 3');
  assert.ok(body.includes('(v.empireWars ? 16 : 0)'), 'Empire Wars rides bit 4');
  assert.ok(body.includes('(v.kingOfTheHill ? 32 : 0)'), 'King of the Hill rides bit 5');
  assert.ok(body.includes('(v.relicVictory ? 4 : 0)'), 'VIC-03 keeps bit 2');
});

test('normalizeVictory and victoryIsDefault carry the three modes', () => {
  const norm = fnBody('function normalizeVictory(');
  for (const f of ['deathMatch', 'empireWars', 'kingOfTheHill']) {
    assert.ok(norm.includes(`${f}: v.${f} === true`), `normalizeVictory reads ${f}`);
  }
  const def = fnBody('function victoryIsDefault(');
  assert.ok(def.includes('!n.deathMatch && !n.empireWars && !n.kingOfTheHill'),
    'a mode pick is never the default (plain new_match) path');
});

test('the pick round-trips through victoryFromPick / victoryPickOf', () => {
  const from = fnBody('function victoryFromPick(');
  assert.ok(from.includes('deathMatch: m.deathMatch === true'), 'FromPick reads the modes bag');
  assert.ok(from.includes('kingOfTheHill: m.kingOfTheHill === true'), 'FromPick reads KotH');
  const of = fnBody('function victoryPickOf(');
  assert.ok(of.includes('deathMatch: n.deathMatch'), 'PickOf reflects Death Match');
  assert.ok(of.includes('empireWars: n.empireWars'), 'PickOf reflects Empire Wars');
  assert.ok(of.includes('kingOfTheHill: n.kingOfTheHill'), 'PickOf reflects KotH');
});

test('both prematch surfaces offer the three modes', () => {
  for (const id of ['vic-deathmatch', 'vic-empirewars', 'vic-koth',
    'create-victory-deathmatch', 'create-victory-empirewars', 'create-victory-koth']) {
    assert.ok(indexHtml.includes(`id="${id}"`), `index.html has #${id}`);
    assert.ok(appJs.includes(`getElementById('${id}')`), `app.js binds #${id}`);
  }
  assert.ok(fnBody('function readVictoryInputs()').includes('kingOfTheHill: vicKothInput'),
    'the pause-menu reader feeds chosenVictory');
});

test('the Monument has client tables at tag 21 and a draw routine', () => {
  assert.ok(appJs.includes("'monastery', 'monument', 'wonder', 'dock', 'fishtrap']"), 'BUILD_I18N names tag 21 (the Wonder follows at 22, VIC-02; the Dock appends at 23, Water W1; the Fish Trap at 24, Water W2b)');
  assert.ok(appJs.includes("'Monastery', 'Monument', 'Wonder', 'Dock', 'Fish Trap']"), 'BUILD_NAME names tag 21');
  assert.ok(appJs.includes("'i-monastery', 'i-monument', 'i-wonder', 'i-dock', 'i-fish-trap']"), 'BUILD_ICON has the monument symbol');
  assert.ok(appJs.includes("'monastery.png', 'monument.png', 'wonder.png', 'dock.png', 'fish-trap.png']"), 'BUILD_PNG has the monument portrait');
  assert.ok(appJs.includes('} else if (b.kind === 21) {\n    drawMonument(b);'),
    'drawBuilding routes kind 21 to drawMonument');
  assert.ok(fnBody('function drawMonument(').includes('b.owner === foeOwner() ? ENEMY_CLOTH'),
    'the pennant reads the holder');
});

test('the KotH countdown chip reads the sim readout pair', () => {
  assert.ok(indexHtml.includes('id="koth-countdown-chip"'), 'the chip exists in the top bar');
  assert.ok(appJs.includes("sim.koth_owner()"), 'the chip reads koth_owner');
  assert.ok(appJs.includes("sim.koth_ticks_left()"), 'the chip reads koth_ticks_left');
  assert.ok(appJs.includes("'victory.kothCountdown'"), 'own-hold label');
  assert.ok(appJs.includes("'victory.kothCountdownFoe'"), 'foe-hold label');
});

test('the Wonder countdown chip reads the sim readout pair (VIC-02)', () => {
  assert.ok(indexHtml.includes('id="wonder-countdown-chip"'), 'the chip exists in the top bar');
  assert.ok(appJs.includes("sim.wonder_countdown_holder()"), 'the chip reads wonder_countdown_holder');
  assert.ok(appJs.includes("sim.wonder_countdown_ticks_left()"), 'the chip reads wonder_countdown_ticks_left');
  assert.ok(appJs.includes("'victory.wonderCountdown'"), 'own-hold label');
  assert.ok(appJs.includes("'victory.wonderCountdownFoe'"), 'foe-hold label');
  for (const key of ['building.wonder', 'victory.wonderCountdown', 'victory.wonderCountdownFoe']) {
    const hits = i18nJs.split(`'${key}':`).length - 1;
    assert.equal(hits, 3, `${key} is translated in EN/PT/ES`);
  }
});

test('the mode labels exist in all three locales', () => {
  for (const key of ['victory.deathMatch', 'victory.empireWars', 'victory.kingOfTheHill',
    'victory.kothCountdown', 'victory.kothCountdownFoe', 'building.monument']) {
    const hits = i18nJs.split(`'${key}':`).length - 1;
    assert.equal(hits, 3, `${key} is translated in EN/PT/ES`);
  }
  const label = fnBody('function victoryLabel(');
  assert.ok(label.includes("t('victory.kingOfTheHill')"), 'the room config line names KotH');
});
