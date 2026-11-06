// Teams T3b CLIENT wiring pins (NET-11 panel + INF-04 flares) —
// run from web-preview/play/:
//   node --test teams-t3b.test.mjs
//
// T3a landed the sim half of diplomacy (the directed stance
// table, Command::SetDiplomacy, the WASM exports stance_for /
// push_set_diplomacy, packed teams bit 16 = unlocked) with no
// player-facing surface. This suite pins the client half end to
// end, the way teamsclient.test.mjs pinned T1's and
// teams-t2.test.mjs pinned T2's:
//   * the diplomacy wire form is {t:'diplomacy'} — never
//     {t:'stance'} (CMB-09's per-unit stance) — validated and
//     applied through BOTH dispatch paths (netplay's
//     validateCommand/applyCommand and app.js's applyLocalOrder);
//   * the unlocked-teams toggle (pause prematch, create panel,
//     room screen) carries bit 16 of the packed teams word
//     through buildMatchSim, the handshake, the resync and the
//     replay meta;
//   * the diplomacy panel paints both stance rows from the ABI
//     and enables its selector only in unlocked matches;
//   * flares travel as the non-command {type:'flare'} envelope
//     (NET-10) — never in a bundle, never to the sim — and paint
//     through the INF-01 alert painter in the sender's color.
// INF-04's rulebook Status does NOT flip on this package: it is
// gated on a live two-client proof the bot-loop runs at landing.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  validateCommand, applyCommand, decodeMessage, createSession,
  packTeams, packTeamsWithLock, normalizeTeamsUnlocked,
  teamsUnlockedFromPacked, TEAMS_UNLOCKED_BIT,
} from './netplay.js';
import { roomConfigText } from './room.js';
import { createMinimap, renderMinimap } from './minimap.js';
import { STRINGS } from './i18n.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const netplayJs = readFileSync(join(here, 'netplay.js'), 'utf8');
const indexHtml = readFileSync(join(here, 'index.html'), 'utf8');
const wasmRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'wasm.rs'), 'utf8');

function fnBody(sig) {
  const start = appJs.indexOf(sig);
  assert.ok(start >= 0, `${sig} exists`);
  const end = appJs.indexOf('\n}\n', start);
  assert.ok(end > start, `${sig} is delimited`);
  return appJs.slice(start, end);
}

// ------------------------------------------------------------ wire form

test('the diplomacy wire form is {t:\'diplomacy\'}, validated strictly', () => {
  assert.equal(validateCommand({ t: 'diplomacy', to: 0, s: 0 }), true, 'Ally toward seat 0');
  assert.equal(validateCommand({ t: 'diplomacy', to: 2, s: 1 }), true, 'Neutral toward seat 2');
  assert.equal(validateCommand({ t: 'diplomacy', to: 2, s: 2 }), true, 'Enemy toward seat 2');
  assert.equal(validateCommand({ t: 'diplomacy', to: 2, s: 3 }), false, 'tag 3 is a unit-stance tag, not a diplomacy tag');
  assert.equal(validateCommand({ t: 'diplomacy', to: 1, s: 0 }), false, 'Gaia is not a diplomacy target');
  assert.equal(validateCommand({ t: 'diplomacy', to: 2 }), false, 'missing stance');
  assert.equal(validateCommand({ t: 'diplomacy', s: 1 }), false, 'missing target');
  assert.equal(validateCommand({ t: 'diplomacy', to: 2, s: 1.5 }), false, 'fractional tag');
  // The two stance vocabularies must never be confusable: the
  // CMB-09 form names an entity, the diplomacy form a seat.
  assert.equal(validateCommand({ t: 'stance', e: 7, s: 3 }), true, 'CMB-09 unit stance keeps its own form');
  assert.equal(validateCommand({ t: 'stance', to: 2, s: 1 }), false, 'a diplomacy-shaped stance command is junk');
});

test('netplay applyCommand routes diplomacy to push_set_diplomacy with the bundle owner as sender', () => {
  const calls = [];
  const sim = {
    push_set_diplomacy(player, target, stance) {
      calls.push([player, target, stance]);
      return true;
    },
  };
  assert.equal(applyCommand(sim, { t: 'diplomacy', to: 2, s: 1 }, 0, () => -1), true);
  assert.deepEqual(calls, [[0, 2, 1]], 'sender is the bundle owner, target/stance ride the command');
  assert.equal(applyCommand(sim, { t: 'diplomacy', to: 0, s: 0 }, 2, () => -1), true);
  assert.deepEqual(calls[1], [2, 0, 0], 'the guest seat sends as owner 2');
  // A stale wasm without the T3a export refuses cleanly.
  assert.equal(applyCommand({}, { t: 'diplomacy', to: 2, s: 1 }, 0, () => -1), false);
});

test('the local dispatch path routes diplomacy the same way (applyLocalOrder)', () => {
  const body = fnBody('function applyLocalOrder(');
  assert.ok(body.includes("case 'diplomacy': return typeof sim.push_set_diplomacy === 'function'"),
    'applyLocalOrder guards the T3a export like netplay does');
  assert.ok(body.includes('? sim.push_set_diplomacy(localOwner, cmd.to, cmd.s)'),
    'locally the sender is this client\'s seat');
  // And the T3a ABI the client consumes is the one that exists.
  assert.ok(wasmRs.includes('pub fn push_set_diplomacy(&mut self, player: u32, target: u32, stance: u32) -> bool'),
    'wasm export push_set_diplomacy(player, target, stance)');
  assert.ok(wasmRs.includes('pub fn stance_for(&self, viewer: u32, other: u32) -> i32'),
    'wasm export stance_for(viewer, other)');
  assert.ok(netplayJs.includes("case 'diplomacy': return typeof sim.push_set_diplomacy === 'function'"),
    'netplay applyCommand guards the export identically');
});

// ------------------------------------------------------------ packed lock bit

test('bit 16 of the packed teams word is the unlocked flag (functional)', () => {
  assert.equal(TEAMS_UNLOCKED_BIT, 1 << 16);
  assert.equal(normalizeTeamsUnlocked(true), true);
  assert.equal(normalizeTeamsUnlocked(false), false);
  assert.equal(normalizeTeamsUnlocked('true'), false, 'junk never unlocks');
  assert.equal(normalizeTeamsUnlocked(1), false);
  assert.equal(normalizeTeamsUnlocked(undefined), false);
  assert.equal(packTeamsWithLock([1, 2], false), packTeams([1, 2]), 'locked packs exactly as before T3b');
  assert.equal(packTeamsWithLock([1, 2], true), 513 | (1 << 16));
  assert.equal(packTeamsWithLock([1, 1], true), 257 | (1 << 16));
  assert.equal(packTeamsWithLock('junk', true), 513 | (1 << 16), 'junk pair still defaults, flag still honored');
  assert.equal(teamsUnlockedFromPacked(513 | (1 << 16)), true);
  assert.equal(teamsUnlockedFromPacked(513), false, 'every pre-T3a packed word reads locked');
  assert.equal(teamsUnlockedFromPacked(-1), false);
  // The sim side of the same bit (T3a): set means locked_teams = false.
  assert.ok(wasmRs.includes('locked_teams: packed & (1 << 16) == 0'),
    'wasm teams_from_packed reads bit 16 as unlocked');
});

test('buildMatchSim routes unlocked matches to the teams export with the lock bit packed', () => {
  const body = fnBody('function buildMatchSim(');
  assert.ok(body.includes('teamsUnlocked = false'), 'the lock posture is a construction input');
  assert.ok(body.includes('(!teamsIsDefault(t) || u)'),
    'unlocked-with-default-ids is non-default state and must not fall through to the teams-less exports');
  assert.ok(body.includes('packTeamsWithLock(t, u)'), 'the packed word carries bit 16');
  assert.ok(fnBody('function teamMeta(').includes('teamsUnlocked: true'),
    'replay meta carries the flag while unlocked');
  assert.ok(fnBody('function teamMeta(').includes('teamsIsDefault(t) && !u'),
    'locked distinct-team recordings keep the pre-T1 shape');
});

test('the unlocked flag rides every construction path (handshake, resync, replay, restart)', () => {
  assert.ok(fnBody('async function hostStartMatch(').includes('matchTeamsUnlocked = normalizeTeamsUnlocked(net.teamsUnlocked)'),
    'the host builds under its room toggle');
  assert.ok(fnBody('async function hostStartMatch(').includes('teamsUnlocked: matchTeamsUnlocked'),
    "the 'start' handshake carries the binding copy");
  assert.ok(fnBody('async function guestHandleStart(').includes('normalizeTeamsUnlocked(msg.teamsUnlocked)'),
    'the guest adopts the host\'s posture (junk -> locked)');
  assert.ok(fnBody('async function rejoinFromResync(').includes('normalizeTeamsUnlocked(msg.teamsUnlocked)'),
    'the rejoiner rebuilds under the match\'s posture');
  assert.ok(fnBody('function sendResync()').includes('teamsUnlocked: matchTeamsUnlocked'),
    'the resync carries the posture');
  assert.ok(fnBody('async function startPlayback(').includes('normalizeTeamsUnlocked(rec.teamsUnlocked)'),
    'playback rebuilds under the recording\'s posture');
  assert.ok(fnBody('function restartAiMatch()').includes('normalizeTeamsUnlocked(aiRoom.teamsUnlocked)'),
    'an AI-room restart keeps its (locked) posture');
  assert.ok(fnBody('async function hostStartAiMatch(').includes('matchTeamsUnlocked = false'),
    'an AI-room match always builds locked, like its team pick');
  for (const sig of ['function resetSim(', 'async function hostStartMatch(',
    'async function guestHandleStart(', 'async function hostStartAiMatch(',
    'function restartAiMatch()', 'async function rejoinFromResync(']) {
    assert.ok(fnBody(sig).includes('matchTeamsUnlocked'), `${sig} threads the posture into its build`);
  }
});

// ------------------------------------------------------------ prematch / room surfaces

test('the unlocked toggle exists on all three prematch surfaces', () => {
  for (const id of ['vic-unlocked', 'vic-unlocked-txt', 'create-teams-unlocked',
    'set-teams-unlocked-name', 'unlocked-control-row', 'unlocked-control-txt',
    'room-teams-unlocked']) {
    assert.ok(indexHtml.includes(`id="${id}"`), `#${id} exists in the page`);
  }
  assert.ok(appJs.includes("document.getElementById('vic-unlocked')"), 'the pause-menu checkbox is wired');
  assert.ok(appJs.includes("createTeamsUnlocked: document.getElementById('create-teams-unlocked')"),
    'the create-panel checkbox is wired');
  assert.ok(appJs.includes("roomTeamsUnlocked: document.getElementById('room-teams-unlocked')"),
    'the room toggle is wired');
  assert.ok(fnBody('function createGame()').includes('net.teamsUnlocked = !!(netUi.createTeamsUnlocked && netUi.createTeamsUnlocked.checked)'),
    'createGame reads the create-panel pick into the room state');
  assert.ok(appJs.includes('net.teamsUnlocked = value === true;'),
    'setTeamsUnlocked only ever unlocks on the literal true');
  // The pause-menu pick is its own state, applied on Restart.
  assert.ok(appJs.includes('chosenTeamsUnlocked = vicUnlockedInput ? vicUnlockedInput.checked : false'),
    'the pause pick feeds chosenTeamsUnlocked');
  assert.ok(fnBody('function resetSim(').includes("matchTeamsUnlocked = mode === 'sandbox' ? false : chosenTeamsUnlocked"),
    'a local Restart builds under the pause pick (Sandbox records locked — its plain constructor is locked)');
  // The room toggle's visibility/lock discipline mirrors the
  // team pick's: host only, human guest seat only, frozen while
  // a guest is present.
  const render = fnBody('function renderRoom()');
  assert.ok(render.includes('netUi.unlockedControlRow.hidden = !isHost || slotIsAi(slot) || slot === \'closed\''),
    'the room toggle shows only for a host with a human guest seat');
  assert.ok(render.includes('netUi.roomTeamsUnlocked.disabled = net.phase !== \'lobby\''),
    'the room toggle freezes outside the lobby / with a guest present');
  assert.ok(render.includes('teamsUnlocked: normalizeTeamsUnlocked(net.teamsUnlocked)'),
    'the room config line receives the posture');
});

test('roomConfigText names an unlocked room and nothing else changes', () => {
  const locked = roomConfigText({ guestSlot: 'open' });
  assert.equal(roomConfigText({ guestSlot: 'open', teamsUnlocked: false }), locked,
    'locked rooms read exactly as before');
  assert.equal(roomConfigText({ guestSlot: 'open', teamsUnlocked: 'yes' }), locked,
    'junk never unlocks the summary');
  assert.equal(roomConfigText({ guestSlot: 'open', teamsUnlocked: true }),
    `${locked} · Unlocked teams`);
  assert.equal(roomConfigText({ guestSlot: 'open', teamPick: 'ally', teamsUnlocked: true }),
    roomConfigText({ guestSlot: 'open', teamPick: 'ally' }) + ' · Unlocked teams',
    'the unlocked segment composes after the allied segment');
  assert.equal(
    roomConfigText({ guestSlot: 'open', teamsUnlocked: true }, { teamsUnlocked: 'Times destravados' }).endsWith(' · Times destravados'),
    true, 'the segment localizes through the caller\'s strings');
});

// ------------------------------------------------------------ diplomacy panel

test('the diplomacy panel paints both stance rows from the ABI', () => {
  for (const id of ['diplo-col-incoming', 'diplo-you-incoming', 'diplo-foe-incoming',
    'diplo-foe-stance-select', 'diplo-stance-opt-0', 'diplo-stance-opt-1',
    'diplo-stance-opt-2', 'diplo-lock-note']) {
    assert.ok(indexHtml.includes(`id="${id}"`), `#${id} exists in the page`);
  }
  const body = fnBody('function refreshDiplo()');
  assert.ok(body.includes('sim.stance_for(localOwner, foeOwner())'), 'my row toward the foe reads the ABI');
  assert.ok(body.includes('sim.stance_for(foeOwner(), localOwner)'), 'the incoming row reads the ABI');
  assert.ok(body.includes("set('diplo-foe-incoming', diploStanceName(incoming))"),
    'the incoming cell paints the foe\'s stance toward me (DE shows it)');
  assert.ok(body.includes('matchTeamsUnlocked && !playback'),
    'the selector arms only in an unlocked live match — never in playback');
  assert.ok(body.includes('stanceSel.disabled = !diploUnlocked'),
    'locked matches render the selector disabled');
  assert.ok(body.includes("t(diploUnlocked ? 'diplo.unlockedNote' : 'diplo.lockedNote')"),
    'the panel states the lock posture either way');
  // The selector issues through the ordinary command path.
  assert.ok(appJs.includes("issue({ t: 'diplomacy', to: foeOwner(), s: tag })"),
    'a selector change issues {t:\'diplomacy\'} via issue()');
  // applyLocale must not resurrect the hardcoded placeholder
  // over the live cell (refreshDiplo owns it now).
  assert.ok(!appJs.includes("setText('diplo-foe-stance', 'diplo.enemy');"),
    'the pre-T3b hardcoded Enemy paint is gone from applyLocale');
});

// ------------------------------------------------------------ flares

test('flares decode as a bounded non-command envelope and never enter a bundle', () => {
  const ok = decodeMessage(JSON.stringify({ type: 'flare', x: 12.5, y: -40 }));
  assert.equal(ok.ok, true, 'a well-formed flare decodes');
  assert.equal(decodeMessage(JSON.stringify({ type: 'flare', x: '12', y: 0 })).ok, false, 'non-numeric x is junk');
  assert.equal(decodeMessage(JSON.stringify({ type: 'flare', x: 5000, y: 0 })).ok, false, 'off-world x is junk');
  assert.equal(decodeMessage(JSON.stringify({ type: 'flare', x: 1 })).ok, false, 'missing y is junk');
  assert.equal(validateCommand({ t: 'flare', x: 1, y: 2 }), false,
    'a flare is not a command — the sim vocabulary refuses it');
  const bundle = decodeMessage(JSON.stringify({
    type: 'bundle', turn: 3, cmds: [{ t: 'flare', x: 1, y: 2 }],
  }));
  assert.equal(bundle.ok, false, 'a flare inside a bundle rejects the bundle whole (NET-10)');
  // The session does not consume flares: receive() returns
  // false so the app layer paints them.
  const s = createSession({
    localSlot: 0, send: () => {}, applyBundle: () => {}, tickOnce: () => '0',
  });
  assert.equal(s.receive({ type: 'flare', x: 1, y: 2 }), false,
    'the lockstep session never consumes a flare');
});

test('the flare flow: button arms, click sends the envelope, peer paints it', () => {
  assert.ok(indexHtml.includes('id="flare-btn"'), 'the Signal Allies button sits beside the minimap');
  assert.ok(fnBody('function sendFlare(').includes("net.client.send({ type: 'flare', x, y })"),
    'sending rides the room socket envelope, not the session');
  assert.ok(fnBody('function sendFlare(').includes('showFlare(x, y, localOwner, false)'),
    'the sender sees their own marker (presentation only)');
  assert.ok(!fnBody('function sendFlare(').includes('localCommand'),
    'a flare never enters the command stream');
  assert.ok(appJs.includes("case 'flare':"), 'onNetMessage handles the envelope');
  assert.ok(appJs.includes('showFlare(msg.x, msg.y, foeOwner(), true)'),
    'a received flare paints in the peer\'s (sender\'s) seat color, with the cue');
  assert.ok(appJs.includes("if (flareArm) {\n      setFlareArm(false); // Teams T3b: Esc disarms an armed flare"),
    'Esc disarms an armed flare first, like placement');
  const render = appJs.slice(appJs.indexOf('// INF-04 (Teams T3b): ally flares'));
  assert.ok(render.includes('flareMarkers.filter((f) => alertFresh(f, now))'),
    'the minimap paint prunes by the alert lifetime (6 s adopted)');
  assert.ok(render.includes('on: alertFlashOn(f, now)'),
    'the minimap paint reuses the alert blink phase');
});

test('renderMinimap paints flare Xs in their sender color, only while lit', () => {
  const g = createMinimap({ worldHalf: 100, width: 220, height: 124, pad: 2 });
  const mkCtx = () => {
    const strokes = [];
    const ctx = {
      fillStyle: '', strokeStyle: '', lineWidth: 1, strokes,
      beginPath() {}, moveTo() {}, lineTo() {}, closePath() {},
      fill() {}, clip() {}, save() {}, restore() {}, arc() {}, fillRect() {}, strokeRect() {},
      stroke() { strokes.push(this.strokeStyle); },
    };
    return ctx;
  };
  const base = {
    gridW: 50, gridH: 50, cellSize: 4, obstacles: new Uint8Array(50 * 50),
    nodes: [], buildings: [], entities: [], cam: null, alert: null,
  };
  const plain = mkCtx();
  renderMinimap(plain, g, { ...base, flares: [] });
  const lit = mkCtx();
  renderMinimap(lit, g, {
    ...base,
    flares: [
      { x: 10, y: -20, on: true, color: '#4da3ff' }, // seat 0 blue
      { x: -30, y: 40, on: false, color: '#e0483e' }, // dark phase: unpainted
    ],
  });
  assert.equal(lit.strokes.length, plain.strokes.length + 1, 'one lit flare = one X stroke');
  assert.ok(lit.strokes.includes('#4da3ff'), 'the X paints in the sender\'s player color, not the alert red');
  const both = mkCtx();
  renderMinimap(both, g, {
    ...base,
    alert: { x: 0, y: 0, on: true },
    flares: [{ x: 10, y: -20, on: true, color: '#e0483e' }],
  });
  assert.ok(both.strokes.includes('#e8402f') && both.strokes.includes('#e0483e'),
    'alert and flare Xs coexist in their own colors');
});

// ------------------------------------------------------------ i18n

test('the T3b strings exist in all three locales, PT-BR in Brazilian forms', () => {
  for (const loc of ['en', 'pt-BR', 'es']) {
    for (const key of ['set.unlockedTeams', 'room.teamsUnlocked',
      'diplo.ally', 'diplo.neutral', 'diplo.incoming',
      'diplo.lockedNote', 'diplo.unlockedNote',
      'flare.button', 'flare.title', 'flare.armed']) {
      assert.ok(STRINGS[loc][key]?.length > 0, `${loc}:${key} present`);
    }
  }
  assert.equal(STRINGS.en['diplo.ally'], 'Ally');
  assert.equal(STRINGS.en['diplo.neutral'], 'Neutral');
  // PT-BR (Brazilian forms — você, gerund vocabulary, "tela"
  // family; Claudio's standing correction of 2026-11-04/05).
  assert.equal(STRINGS['pt-BR']['diplo.incoming'], 'Postura em relação a você');
  assert.ok(STRINGS['pt-BR']['diplo.lockedNote'].includes('você'), 'PT-BR locked note uses você');
  assert.ok(STRINGS['pt-BR']['flare.armed'].startsWith('Clique'), 'PT-BR flare hint is an imperative to the player');
  assert.equal(STRINGS['pt-BR']['room.teamsUnlocked'], 'Times destravados');
  assert.equal(STRINGS.es['diplo.neutral'], 'Neutral');
});
