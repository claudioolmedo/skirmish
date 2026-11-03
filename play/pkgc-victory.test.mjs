// Package C landing pins (2026-11-02): the client + netplay
// wiring for VIC-05/06/07. The sim package (fc1584b) shipped
// WasmSim.new_match_victory with NO client call site; these pins
// keep every construction path building under the match's
// victory settings — a path that falls back to plain new_match
// silently turns a Regicide / Sudden Death / timed match into
// Conquest (and, in netplay, desyncs it: the settings are hashed
// whenever non-default).
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

test('buildMatchSim routes non-default settings to new_match_victory', () => {
  const body = fnBody('function buildMatchSim(');
  assert.ok(body.includes('WasmSim.new_match_victory(seed, modeTag, v.timeLimitTicks, v.scoreTarget, victoryFlags(v))'),
    'the victory export is called with the ABI packing');
  assert.ok(body.includes('WasmSim.new_match(seed, modeTag)'),
    'default settings keep the plain construction');
});

test('every match construction goes through buildMatchSim', () => {
  for (const sig of ['function resetSim(', 'async function hostStartMatch(',
    'async function guestHandleStart(', 'async function hostStartAiMatch(',
    'function restartAiMatch()', 'async function rejoinFromResync(',
    'async function startPlayback(']) {
    assert.ok(fnBody(sig).includes('buildMatchSim('), `${sig} builds via buildMatchSim`);
  }
  // No bare new_match call sites remain outside buildMatchSim.
  const bare = appJs.match(/WasmSim\.new_match\(/g) || [];
  assert.equal(bare.length, 1, 'only buildMatchSim calls WasmSim.new_match directly');
});

test('the start handshake carries the host victory settings', () => {
  const body = fnBody('async function hostStartMatch(');
  assert.ok(body.includes('victory: { ...matchVictory }'), "the 'start' message carries victory");
  const guest = fnBody('async function guestHandleStart(');
  assert.ok(guest.includes('normalizeVictory(msg.victory)'), 'the guest adopts msg.victory');
});

test('resync carries the settings and the rebuild uses them', () => {
  assert.ok(fnBody('function sendResync()').includes('victory: { ...matchVictory }'),
    'sendResync ships matchVictory');
  assert.ok(fnBody('async function rejoinFromResync(').includes('normalizeVictory(msg.victory)'),
    'the rejoiner adopts the resync victory');
});

test('replays record and restore non-default victory settings', () => {
  assert.ok(fnBody('function victoryMeta(').includes('victoryIsDefault'),
    'Conquest recordings keep the pre-Package-C shape');
  assert.ok(fnBody('async function startPlayback(').includes('normalizeVictory(rec.victory)'),
    'playback rebuilds under rec.victory');
});

test('the King kind is wired into the client tables', () => {
  assert.ok(appJs.includes("22: 'King'"), 'KIND_NAME carries the King at tag 22');
  assert.ok(appJs.includes("22: 'i-king'"), 'KIND_ICON carries the King portrait');
  assert.ok(appJs.includes("'monk', 'relic', 'king']"), 'KIND_I18N ends with king at index 22 (relic keeps 21)');
  assert.ok(appJs.includes('const KIND_KING = 22;'), 'the KIND_KING constant matches the sim tag');
  assert.ok(indexHtml.includes('id="i-king"'), 'the crown symbol exists in the page');
});

test('the King has a sprite-cached draw (INF-12 occlusion can see him)', () => {
  const body = fnBody('function drawKing(');
  assert.ok(body.includes('kingSprite(cloth)'), 'drawKing blits the cached sprite');
  assert.ok(body.includes('ctx.drawImage(spr,'), 'the draw records drawImage calls');
  assert.ok(fnBody('function paintKingFigure(').includes('#d8b34a'),
    'the figure carries the gold crown/sceptre colour');
});

test('victory names exist in all three client languages', () => {
  for (const key of ['unit.king', 'victory.regicide', 'victory.sudden', 'victory.conquest']) {
    const hits = i18nJs.split(`'${key}':`).length - 1;
    assert.equal(hits, 3, `${key} is defined for en + pt-BR + es`);
  }
});

test('prematch victory controls exist for solo and for the create panel', () => {
  for (const id of ['victory-row', 'vic-time', 'vic-score',
    'create-victory', 'create-victory-time', 'create-victory-score']) {
    assert.ok(indexHtml.includes(`id="${id}"`), `#${id} exists in the page`);
  }
  assert.ok(appJs.includes("netUi.createVictory ? netUi.createVictory.value : 'conquest'"),
    'the create handler reads the victory pick');
});
