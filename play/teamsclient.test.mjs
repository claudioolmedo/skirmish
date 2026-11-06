// Teams T1 client landing pins (2026-11-05): the client half of
// T1 — the WASM teams constructor + shared fog read, and the
// plumbing that carries the host's team assignment through
// buildMatchSim, the room team pick, the handshake, the resync
// and the replay header. The sim core (37aeae0, landed as
// 3565377) shipped TeamSettings + visibility_shared with NO
// client path to either; these pins keep every construction
// path building under the match's teams — a path that falls
// back to a teams-less constructor silently turns an allied
// match into distinct teams (and, in netplay, desyncs it: the
// settings are hashed whenever non-default).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');
const indexHtml = readFileSync(join(here, 'index.html'), 'utf8');
const wasmRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'wasm.rs'), 'utf8');

function fnBody(sig) {
  const start = appJs.indexOf(sig);
  assert.ok(start >= 0, `${sig} exists`);
  const end = appJs.indexOf('\n}\n', start);
  assert.ok(end > start, `${sig} is delimited`);
  return appJs.slice(start, end);
}

test('wasm: the teams constructor export carries the packed u32 scheme', () => {
  assert.ok(wasmRs.includes('pub fn new_match_victory_teams('),
    'the new constructor export exists');
  assert.ok(wasmRs.includes('teams_from_packed(teams)'),
    'the export unpacks the packed word into TeamSettings');
  assert.ok(wasmRs.includes('Sim::with_settings_teams('),
    'the export builds through with_settings_teams');
  // The old export keeps its exact signature/meaning — a caller
  // built against it must never read a missing argument as
  // teams [0, 0] (which would silently ally the seats).
  assert.ok(wasmRs.includes('pub fn new_match_victory('),
    'the pre-T1 victory export still exists');
});

test('wasm: fog_for serves the shared (union) grid', () => {
  const start = wasmRs.indexOf('pub fn fog_for(');
  assert.ok(start >= 0, 'fog_for exists');
  const body = wasmRs.slice(start, wasmRs.indexOf('\n    }', start));
  assert.ok(body.includes('visibility_shared(owner as u8)'),
    'fog_for returns visibility_shared, not the per-player grid');
});

test('buildMatchSim routes non-default teams to the teams export, with fallback', () => {
  const body = fnBody('function buildMatchSim(');
  assert.ok(body.includes("typeof WasmSim.new_match_victory_teams === 'function'"),
    'the stale-wasm guard checks for the new export');
  assert.ok(body.includes('WasmSim.new_match_victory_teams(seed, modeTag, v.timeLimitTicks, v.scoreTarget, victoryFlags(v), packTeamsWithLock(t, u))'),
    'the teams export is called with the packed word (T3b: packTeamsWithLock — the pair bytes plus bit 16 while unlocked)');
  assert.ok(body.includes('WasmSim.new_match_victory(seed, modeTag, v.timeLimitTicks, v.scoreTarget, victoryFlags(v))'),
    'default teams keep the pre-T1 victory construction');
  assert.ok(body.includes('WasmSim.new_match(seed, modeTag)'),
    'all-default settings keep the plain construction');
});

test('every match construction goes through buildMatchSim with matchTeams', () => {
  for (const sig of ['function resetSim(', 'async function hostStartMatch(',
    'async function guestHandleStart(', 'async function hostStartAiMatch(',
    'function restartAiMatch()', 'async function rejoinFromResync(',
    'async function startPlayback(']) {
    assert.ok(fnBody(sig).includes('buildMatchSim('), `${sig} builds via buildMatchSim`);
    assert.ok(fnBody(sig).includes('matchTeams'), `${sig} passes the match teams`);
  }
});

test('the start handshake and the resync carry the team assignment', () => {
  assert.ok(fnBody('async function hostStartMatch(').includes('teams: [...matchTeams]'),
    "the 'start' message carries teams");
  assert.ok(fnBody('async function guestHandleStart(').includes('normalizeTeams(msg.teams)'),
    'the guest adopts msg.teams (junk -> defaults)');
  assert.ok(fnBody('function sendResync()').includes('teams: [...matchTeams]'),
    'sendResync ships matchTeams');
  assert.ok(fnBody('async function rejoinFromResync(').includes('normalizeTeams(msg.teams)'),
    'the rejoiner adopts the resync teams');
  // The host's hello advertises the assignment for the room
  // summary (display only), like the victory settings.
  assert.ok(appJs.includes('teams: [...normalizeTeams(net.teams)]'),
    "the host's hello carries teams");
});

test('replays record and restore non-default team assignments', () => {
  assert.ok(fnBody('function teamMeta(').includes('teamsIsDefault'),
    'distinct-team recordings keep the pre-T1 shape');
  assert.ok(fnBody('async function startPlayback(').includes('normalizeTeams(rec.teams)'),
    'playback rebuilds under rec.teams');
});

test('the room team pick surface exists (host, human guest seat only)', () => {
  assert.ok(indexHtml.includes('id="guest-team-select"'), 'the team select is in the room screen');
  assert.ok(indexHtml.includes('id="team-control-row"'), 'the team control row exists');
  assert.ok(appJs.includes("roomGuestTeam: document.getElementById('guest-team-select')"),
    'netUi wires the team select');
  assert.ok(fnBody('function setTeamPick(').includes('teamsForPick(teamPickOf(value))'),
    'setTeamPick normalizes the pick into the pair (junk -> opponent)');
  const render = fnBody('function renderRoom()');
  assert.ok(render.includes('teamPickForTeams(net.teams)'),
    'renderRoom reflects the current assignment in the pick + config line');
  assert.ok(render.includes("slotIsAi(slot) || slot === 'closed'"),
    'the pick hides for AI/closed seats (an AI seat is an opponent by construction)');
});

test('team strings exist in all three client languages', () => {
  for (const key of ['set.guestTeam', 'team.opponent', 'team.ally',
    'team.opponentOption', 'team.allyOption', 'room.teamAlly']) {
    const hits = i18nJs.split(`'${key}':`).length - 1;
    assert.equal(hits, 3, `${key} is defined for en + pt-BR + es`);
  }
});
