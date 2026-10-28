// Replay fog choice regression pins (bot-loop front 7, 2026-10-28).
//
// NET-08: a replay's observer controls include a PER-PLAYER FOG
// CHOICE — VIS states are derived, never stored, so the viewer can
// re-derive the fog for either seat (or reveal the whole map) at any
// moment of the playback. Before this front the playback always
// rendered the recording seat's fog and offered no choice, which the
// rulebook recorded as DIFFERS.
//
// These pins keep the choice wired end to end: the playback bar
// carries the fog button, starting a playback seeds the choice from
// the recording's own view seat, the cycle walks every seat in the
// recording then the revealed map, myFog (the single fog source for
// the main view AND the minimap) derives playback fog from the
// choice, switching views drops the previous view's explored-memory,
// and the labels exist in every shipped locale.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');
const html = readFileSync(join(here, 'index.html'), 'utf8');

test('the playback bar carries a fog-choice button', () => {
  assert.ok(html.includes('id="playback-bar"'), 'playback bar exists');
  assert.ok(html.includes('id="pb-fog"'), 'the fog button is in the playback bar');
  assert.ok(
    appJs.includes("pbFog: document.getElementById('pb-fog')"),
    'netUi wires the fog button');
  assert.ok(
    appJs.includes("netUi.pbFog.addEventListener('click', cyclePlaybackFog)"),
    'the fog button cycles the choice');
});

test('playback seeds its fog choice from the recording view seat', () => {
  assert.ok(
    appJs.includes("fogView: typeof rec.view === 'number' ? rec.view : owners[0]"),
    'fogView starts at the recording seat, like view itself');
  assert.ok(
    appJs.includes('netUi.pbFog.textContent = playbackFogLabel()'),
    'the button is labelled when playback starts');
});

test('the fog cycle walks every recorded seat, then the revealed map', () => {
  const start = appJs.indexOf('function cyclePlaybackFog()');
  assert.ok(start >= 0, 'cyclePlaybackFog exists');
  const body = appJs.slice(start, appJs.indexOf('\n}\n', start));
  assert.ok(
    body.includes("const choices = [...playback.owners, 'all']"),
    'choices are the recording seats plus the revealed map');
  assert.ok(
    body.includes('seenBuildings.clear()'),
    'switching views drops the previous view explored-memory');
});

test('myFog derives playback fog from the choice, per seat or revealed', () => {
  const start = appJs.indexOf('const myFog = () => {');
  assert.ok(start >= 0, 'myFog is the derived-fog function');
  const body = appJs.slice(start, appJs.indexOf('\n};', start));
  assert.ok(
    body.includes("if (playback.fogView === 'all') return revealedFog();"),
    'the revealed choice renders the whole map');
  assert.ok(
    body.includes('return sim.fog_for(playback.fogView);'),
    'a seat choice derives that seat fog from the sim (never stored)');
  assert.ok(
    body.indexOf('if (playback)') < body.indexOf('seatAware()'),
    'the playback branch precedes the live-play branch');
});

test('the fog labels exist in every locale, with the seat number slot', () => {
  const players = [...i18nJs.matchAll(/'pb\.fogPlayer':\s*'([^']+)'/g)]
    .map((m) => m[1]);
  assert.equal(players.length, 3, 'pb.fogPlayer is defined for EN, PT and ES');
  for (const v of players) assert.ok(v.includes('{n}'), 'label carries the seat number');
  const alls = i18nJs.match(/'pb\.fogAll':/g) || [];
  assert.equal(alls.length, 3, 'pb.fogAll is defined for EN, PT and ES');
});
