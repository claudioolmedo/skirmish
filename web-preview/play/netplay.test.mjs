// Unit tests for netplay.js (M8) — run from web-preview/play/:
//   node --test netplay.test.mjs
// The session tests drive two real sessions over a hand-delivered
// transport with a fake clock and a fake deterministic sim, the
// same way the live integration test drives two real WASM sims.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  TURN_TICKS, GAME_SPEED, GAME_SPEEDS, isGameSpeed, turnMsFor, TICK_MS, TURN_MS, advanceTicks,
  INITIAL_DELAY_TURNS, LOAD_TIMEOUT_MS,
  validateCommand, applyCommand, tcIdOf,
  encodeMessage, decodeMessage,
  createSession, createRecorder, isReplay, replayEvents,
  createRelayClient, probeRegionPing,
  makeRoomId, buildPlayUrl, parsePlayParams, linkJoinRoom,
  pickFastestRegion, REGIONS,
} from './netplay.js';
import * as netplay from './netplay.js';

// ---------------------------------------------------------------- codec

test('validateCommand accepts the full vocabulary and rejects junk', () => {
  const ok = [
    { t: 'move', e: 1, x: 2.5, y: -3 },
    { t: 'move', e: 1, x: 2, y: 3, q: true },
    { t: 'gather', e: 1, n: 9 },
    { t: 'attack', e: 1, tg: 2 },
    { t: 'attackb', e: 1, b: 4 },
    { t: 'build', e: 1, k: 0, x: 0, y: 0 },
    { t: 'build', e: 1, k: 8, x: 0, y: 0 },
    { t: 'build', e: 1, k: 9, x: 0, y: 0 },
    { t: 'build', e: 1, k: 10, x: 0, y: 0 },
    { t: 'build', e: 1, k: 12, x: 0, y: 0 },
    { t: 'build', e: 1, k: 13, x: 0, y: 0 },
    { t: 'build', e: 1, k: 14, x: 0, y: 0 },
    { t: 'build', e: 1, k: 15, x: 0, y: 0 },
    { t: 'build', e: 1, k: 16, x: 0, y: 0 },
    { t: 'mbuy', b: 3, r: 1 },
    { t: 'mbuy', b: 3, r: 2 },
    { t: 'msell', b: 3, r: 4 },
    { t: 'buildline', e: 1, x0: 0, y0: 0, x1: 5, y1: 5 },
    { t: 'buildline', e: 1, k: 14, x0: 0, y0: 0, x1: 5, y1: 5 },
    { t: 'buildline', e: 1, k: 9, x0: 0, y0: 0, x1: 5, y1: 5 },
    { t: 'train_at', b: 3, k: 2 },
    { t: 'train_at', b: 3, k: 15 },
    { t: 'train_vil' },
    { t: 'train_vil', tc: 4 },
    { t: 'research_tc', tech: 1 },
    { t: 'research_tc', tech: 14, tc: 4 },
    { t: 'research_tc', tech: 15 },
    // Imperial Age (tech 18) is a TC research (bot-loop rider,
    // 2026-10-30: the wire validator omitted it on the Imperial
    // tree, so netplay rejected what the TC panel issues).
    { t: 'research_tc', tech: 18 },
    // Package D: Caravan is tech 19 at the Market (renumbered
    // from the builder's 18 on rebase — 18 is Imperial).
    { t: 'research_at', b: 3, tech: 19 },
    { t: 'train_at', b: 3, k: 16 },
    { t: 'trade', e: 4, b: 9 },
    { t: 'rally_tc', x: 1, y: 2, tc: 4 },
    { t: 'research_at', b: 3, tech: 5 },
    { t: 'rally_tc', x: 1, y: 2 },
    { t: 'rally_b', b: 3, x: 1, y: 2 },
    { t: 'force_dep', e: 7 },
    // Castle P3b: the garrison vocabulary.
    { t: 'garrison_b', e: 1, b: 4 },
    { t: 'garrison_tc', e: 1, tc: 4 },
    { t: 'garrison_tc', e: 1 },
    { t: 'ungarrison_b', b: 4 },
    { t: 'ungarrison_tc', tc: 4 },
    { t: 'ungarrison_tc' },
    { t: 'town_bell', tc: 4 },
    { t: 'town_bell' },
    // Castle P4a: build kind 17 (Siege Workshop), train kind 17
    // (Battering Ram) and the Ram garrison vocabulary.
    { t: 'build', e: 1, k: 17, x: 0, y: 0 },
    { t: 'train_at', b: 3, k: 17 },
    { t: 'garrison_u', e: 1, host: 4 },
    { t: 'ungarrison_u', host: 4 },
    { t: 'auto_explore', e: 4, on: true },
    { t: 'auto_explore', e: 4, on: false },
    { t: 'attackmove', e: 4, x: 10, y: -2 },
    { t: 'attackmove', e: 4, x: 10, y: -2, q: true },
    { t: 'patrol', e: 4, x: 0, y: 0 },
    { t: 'stance', e: 4, s: 0 },
    { t: 'stance', e: 4, s: 3 },
    { t: 'stop', e: 4 },
    { t: 'delete', e: 4 },
    { t: 'deleteb', b: 3 },
    { t: 'cancel_tc', tc: 2, i: 0 },
    { t: 'cancel_tc', tc: 2, i: 15 },
    { t: 'cancel_b', b: 3, i: 2 },
  ];
  for (const c of ok) assert.equal(validateCommand(c), true, JSON.stringify(c));
  const bad = [
    null, 42, {}, { t: 'nope' },
    { t: 'move', e: -1, x: 0, y: 0 },
    { t: 'move', e: 1, x: NaN, y: 0 },
    { t: 'build', e: 1, k: 18, x: 0, y: 0 },
    { t: 'mbuy', b: 3, r: 3 },
    { t: 'mbuy', b: 3, r: 0 },
    { t: 'msell', r: 1 },
    { t: 'msell', b: 3, r: 1.5 },
    { t: 'buildline', e: 1, x0: 0, y0: 0, x1: 5 },
    { t: 'buildline', e: 1, k: 10, x0: 0, y0: 0, x1: 5, y1: 5 },
    { t: 'buildline', e: -1, x0: 0, y0: 0, x1: 5, y1: 5 },
    { t: 'buildline', e: 1, x0: NaN, y0: 0, x1: 5, y1: 5 },
    { t: 'train_at', b: 1, k: 5 },
    { t: 'research_tc', tech: 2 },
    { t: 'research_tc', tech: 9 },
    // Caravan (19) is not a TC research; Imperial (18) is not a
    // building research (Package D renumber pins, 2026-10-30).
    { t: 'research_tc', tech: 19 },
    { t: 'research_at', b: 3, tech: 18 },
    { t: 'trade', e: 4 },
    { t: 'trade', b: 9 },
    { t: 'train_vil', tc: -1 },
    { t: 'gather', e: 1 },
    { t: 'auto_explore', e: 4 },
    { t: 'auto_explore', e: 4, on: 1 },
    { t: 'attackmove', e: 4, x: 1 },
    { t: 'attackmove', e: -1, x: 1, y: 1 },
    { t: 'patrol', e: 4, x: NaN, y: 1 },
    { t: 'stance', e: 4 },
    { t: 'stance', e: 4, s: 4 },
    { t: 'stance', e: 4, s: 1.5 },
    { t: 'stop' },
    { t: 'delete', e: -1 },
    { t: 'deleteb' },
    { t: 'cancel_tc', tc: 2, i: 16 },
    { t: 'cancel_tc', tc: 2 },
    { t: 'cancel_b', b: 3, i: -1 },
    { t: 'garrison_b', e: 1 },
    { t: 'garrison_b', b: 4 },
    { t: 'garrison_tc', e: -1 },
    { t: 'ungarrison_b' },
    { t: 'ungarrison_tc', tc: -1 },
    { t: 'town_bell', tc: 1.5 },
    { t: 'garrison_u', e: 1 },
    { t: 'garrison_u', host: 4 },
    { t: 'garrison_u', e: 1, host: -1 },
    { t: 'ungarrison_u' },
    { t: 'ungarrison_u', host: 1.5 },
  ];
  for (const c of bad) assert.equal(validateCommand(c), false, JSON.stringify(c));
});

test('message codec round-trips bundles and rejects malformed traffic', () => {
  const msg = { type: 'bundle', turn: 12, cmds: [{ t: 'move', e: 1, x: 2, y: 3 }] };
  const dec = decodeMessage(encodeMessage(msg));
  assert.equal(dec.ok, true);
  assert.deepEqual(dec.msg, msg);
  assert.equal(decodeMessage('not json').ok, false);
  assert.equal(decodeMessage('{"type":"mystery"}').ok, false);
  assert.equal(decodeMessage('{"type":"bundle","turn":-1,"cmds":[]}').ok, false);
  assert.equal(decodeMessage('{"type":"bundle","turn":1,"cmds":[{"t":"move"}]}').ok, false);
  assert.equal(decodeMessage('{"type":"hash","turn":1}').ok, false);
});

test('rtc signaling decodes for the P2P chat channel — and only signaling shapes', () => {
  for (const m of [
    { type: 'rtc', kind: 'offer', sdp: 'v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\n' },
    { type: 'rtc', kind: 'answer', sdp: 'v=0\r\no=- 3 4 IN IP4 127.0.0.1\r\n' },
    { type: 'rtc', kind: 'ice', candidate: 'candidate:1 1 udp 1 192.0.2.1 9 typ host', sdpMid: '0', sdpMLineIndex: 0 },
    { type: 'rtc', kind: 'ice', candidate: '', sdpMid: null, sdpMLineIndex: null },
  ]) {
    const dec = decodeMessage(encodeMessage(m));
    assert.equal(dec.ok, true, JSON.stringify(m));
    assert.deepEqual(dec.msg, m);
  }
  // The envelope carries signaling, never content: other kinds,
  // missing/oversized payloads and off-shape candidates reject.
  assert.equal(decodeMessage('{"type":"rtc","kind":"text","text":"hi"}').ok, false);
  assert.equal(decodeMessage('{"type":"rtc","kind":"offer"}').ok, false);
  assert.equal(decodeMessage('{"type":"rtc","kind":"offer","sdp":""}').ok, false);
  assert.equal(decodeMessage(`{"type":"rtc","kind":"offer","sdp":"${'x'.repeat(16385)}"}`).ok, false);
  assert.equal(decodeMessage('{"type":"rtc","kind":"ice"}').ok, false);
  assert.equal(decodeMessage(`{"type":"rtc","kind":"ice","candidate":"${'x'.repeat(1025)}","sdpMid":null,"sdpMLineIndex":null}`).ok, false);
  assert.equal(decodeMessage('{"type":"rtc","kind":"ice","candidate":"c","sdpMid":4,"sdpMLineIndex":null}').ok, false);
  // A session never consumes signaling: receive() hands it back
  // to the app layer (the chat link), untouched by the sim.
  const s = createSession({
    localSlot: 0,
    send: () => {},
    applyBundle: () => {},
    tickOnce: () => '0',
  });
  assert.equal(s.receive({ type: 'rtc', kind: 'offer', sdp: 'v=0' }), false);
});

test('M10 lobby messages decode: ready, the password handshake, cancel', () => {
  for (const m of [
    { type: 'ready', ready: true },
    { type: 'ready', ready: false },
    { type: 'pw', pw: 'swordfish' },
    { type: 'pw-req' },
    { type: 'pw-ok' },
    { type: 'pw-no' },
    { type: 'cancel' },
    // Relay v6's originated frame: the peer's socket is gone.
    { type: 'peer-left' },
  ]) {
    const dec = decodeMessage(encodeMessage(m));
    assert.equal(dec.ok, true, JSON.stringify(m));
    assert.deepEqual(dec.msg, m);
  }
  // Malformed lobby traffic is rejected whole.
  assert.equal(decodeMessage('{"type":"ready"}').ok, false);
  assert.equal(decodeMessage('{"type":"ready","ready":"yes"}').ok, false);
  assert.equal(decodeMessage('{"type":"pw"}').ok, false);
  assert.equal(decodeMessage('{"type":"pw","pw":42}').ok, false);
  assert.equal(decodeMessage(`{"type":"pw","pw":"${'x'.repeat(129)}"}`).ok, false);
});

test('M10 link params: the private flag round-trips', () => {
  const priv = buildPlayUrl('https://skirmish.online/play/', 'abc123def4', 'wnam', { private: true });
  assert.equal(priv, 'https://skirmish.online/play/?room=abc123def4&region=wnam&private=1');
  assert.deepEqual(parsePlayParams('?room=abc123def4&region=wnam&private=1'),
    { room: 'abc123def4', region: 'wnam', online: false, private: true });
  const pub = buildPlayUrl('https://skirmish.online/play/', 'abc123def4', 'wnam');
  assert.equal(pub, 'https://skirmish.online/play/?room=abc123def4&region=wnam');
  assert.equal(parsePlayParams('?room=abc123def4&region=wnam').private, false);
});

test('removal: the in-game challenge is gone (Claudio, 2026-10-24)', () => {
  // The challenge chain was removed — his challenge design lives
  // on Instagram, not in the game. Pin the absence so it cannot
  // silently come back: no builder export, and a challenge-style
  // link parses as an ordinary room link (the stray params are
  // ignored, the room join itself is untouched).
  assert.equal('buildChallengeUrl' in netplay, false);
  const parsed = parsePlayParams('?room=abc123def4&region=wnam&challenge=1&from=Ana');
  assert.deepEqual(parsed,
    { room: 'abc123def4', region: 'wnam', online: false, private: false });
  const join = linkJoinRoom('?room=abc123def4&region=wnam&challenge=1&from=Ana', null);
  assert.equal(join.room, 'abc123def4');
  assert.equal('challenge' in join, false);
  assert.equal('from' in join, false);
});

test('the load timeout is the documented 90 s', () => {
  assert.equal(LOAD_TIMEOUT_MS, 90_000);
});

test('linkJoinRoom: a room link joins once per tab session', () => {
  const link = '?room=abc123def4&region=wnam';
  // Fresh tab (no mark): the boot joins the link's room.
  assert.deepEqual(linkJoinRoom(link, null),
    { room: 'abc123def4', region: 'wnam', online: false, private: false });
  // Reload in the same tab (mark matches): front door, no join.
  assert.equal(linkJoinRoom(link, 'abc123def4'), null);
  // Same tab, a DIFFERENT room's link: joins the new room.
  assert.equal(linkJoinRoom('?room=xyz987abcd&region=weur', 'abc123def4').room,
    'xyz987abcd');
  // No room in the URL: never a join, whatever the mark.
  assert.equal(linkJoinRoom('', null), null);
  assert.equal(linkJoinRoom('?online=1', 'abc123def4'), null);
  // The private flag rides along on the fresh join.
  assert.equal(linkJoinRoom('?room=abc123def4&private=1', null).private, true);
});

// ---------------------------------------------------------------- applier

test('applyCommand maps semantic commands to the per-seat sim calls', () => {
  const calls = [];
  const sim = new Proxy({}, {
    get: (_t, name) => (...args) => { calls.push([name, ...args]); return true; },
  });
  const tcOf = (owner) => (owner === 0 ? 0 : 40);
  applyCommand(sim, { t: 'move', e: 5, x: 1, y: 2 }, 0, tcOf);
  applyCommand(sim, { t: 'move', e: 5, x: 1, y: 2, q: true }, 0, tcOf);
  applyCommand(sim, { t: 'build', e: 5, k: 2, x: 1, y: 2 }, 0, tcOf);
  applyCommand(sim, { t: 'build', e: 5, k: 6, x: 1, y: 2, q: true }, 0, tcOf);
  applyCommand(sim, { t: 'build', e: 5, k: 9, x: 1, y: 2 }, 0, tcOf);
  applyCommand(sim, { t: 'build', e: 5, k: 12, x: 1, y: 2 }, 0, tcOf);
  applyCommand(sim, { t: 'build', e: 5, k: 12, x: 1, y: 2, q: true }, 0, tcOf);
  applyCommand(sim, { t: 'mbuy', b: 7, r: 1 }, 0, tcOf);
  applyCommand(sim, { t: 'msell', b: 7, r: 4 }, 0, tcOf);
  applyCommand(sim, { t: 'buildline', e: 5, x0: 1, y0: 2, x1: 9, y1: 2 }, 0, tcOf);
  applyCommand(sim, { t: 'train_at', b: 9, k: 3 }, 0, tcOf);
  // Town Center orders resolve the ISSUER's Town Center:
  applyCommand(sim, { t: 'train_vil' }, 2, tcOf);
  applyCommand(sim, { t: 'research_tc', tech: 0 }, 0, tcOf);
  applyCommand(sim, { t: 'rally_tc', x: 3, y: 4 }, 2, tcOf);
  // CMB-09/10/11: attack-move, patrol (direct + queued forms) and stance.
  applyCommand(sim, { t: 'attackmove', e: 5, x: 7, y: 8 }, 0, tcOf);
  applyCommand(sim, { t: 'attackmove', e: 5, x: 7, y: 8, q: true }, 0, tcOf);
  applyCommand(sim, { t: 'patrol', e: 5, x: 9, y: 10 }, 0, tcOf);
  applyCommand(sim, { t: 'patrol', e: 5, x: 9, y: 10, q: true }, 0, tcOf);
  applyCommand(sim, { t: 'stance', e: 5, s: 2 }, 0, tcOf);
  // Castle P3b: the garrison orders; the TC forms resolve the
  // issuer's TC when the command carries none.
  applyCommand(sim, { t: 'garrison_b', e: 5, b: 9 }, 0, tcOf);
  applyCommand(sim, { t: 'garrison_tc', e: 5, tc: 3 }, 0, tcOf);
  applyCommand(sim, { t: 'garrison_tc', e: 5 }, 2, tcOf);
  applyCommand(sim, { t: 'ungarrison_b', b: 9 }, 0, tcOf);
  applyCommand(sim, { t: 'ungarrison_tc' }, 2, tcOf);
  applyCommand(sim, { t: 'town_bell' }, 0, tcOf);
  assert.deepEqual(calls, [
    ['push_move', 5, 1, 2],
    ['push_move_queued', 5, 1, 2],
    ['push_build_barracks', 5, 1, 2],
    ['push_build_archery_range_queued', 5, 1, 2],
    ['push_build_palisade_wall', 5, 1, 2],
    ['push_build_market', 5, 1, 2],
    ['push_build_market_queued', 5, 1, 2],
    ['push_market_buy', 7, 1],
    ['push_market_sell', 7, 4],
    ['push_build_wall_line', 5, 1, 2, 9, 2],
    ['push_train_militia', 9],
    ['push_train_villager_at', 40],
    ['push_research_tc_at', 0, 0],
    ['push_set_rally_tc_of', 40, 3, 4],
    ['push_attack_move', 5, 7, 8],
    ['push_attack_move_queued', 5, 7, 8],
    ['push_patrol', 5, 9, 10],
    ['push_patrol_queued', 5, 9, 10],
    ['push_set_stance', 5, 2],
    ['push_garrison_building', 5, 9],
    ['push_garrison_tc', 5, 3],
    ['push_garrison_tc', 5, 40],
    ['push_ungarrison_building', 9],
    ['push_ungarrison_tc', 40],
    ['push_town_bell', 0],
  ]);
});

test('tcIdOf finds the owner\'s Town Center entity', () => {
  const sim = {
    entity_kinds: () => new Uint32Array([0, 1, 0, 1]),
    entity_owners: () => new Uint32Array([0, 0, 2, 2]),
  };
  assert.equal(tcIdOf(sim, 0), 1);
  assert.equal(tcIdOf(sim, 2), 3);
  assert.equal(tcIdOf(sim, 1), -1);
});

// ---------------------------------------------------------------- session harness

function fakeSim() {
  // Deterministic toy: the hash is a pure function of everything
  // applied so far — like the real sim's contract.
  let state = 1;
  return {
    applied: [],
    applyBundle(slot, cmds) {
      this.applied.push([slot, cmds]);
      for (const c of cmds) state = (state * 131 + c.e * 7 + slot) >>> 0;
    },
    tick() {
      state = (state * 31 + 7) >>> 0;
      return 'h' + state.toString(16);
    },
  };
}

function linkedPair(opts = {}) {
  const simA = fakeSim();
  const simB = fakeSim();
  const inboxA = [];
  const inboxB = [];
  const eventsA = [];
  const eventsB = [];
  const shared = { ...opts };
  const onTurnA = shared.onTurn;
  delete shared.onTurn;
  const mk = (slot, sim, inbox, events) => createSession({
    localSlot: slot,
    send: (m) => (slot === 0 ? inboxB : inboxA).push(m),
    applyBundle: (s, cmds) => sim.applyBundle(s, cmds),
    tickOnce: () => sim.tick(),
    onEvent: (e) => events.push(e),
    ...(slot === 0 && onTurnA ? { onTurn: onTurnA } : {}),
    ...shared,
  });
  const a = mk(0, simA, inboxA, eventsA);
  const b = mk(1, simB, inboxB, eventsB);
  const flush = () => {
    for (const m of inboxA.splice(0)) a.receive(m);
    for (const m of inboxB.splice(0)) b.receive(m);
  };
  return { a, b, simA, simB, eventsA, eventsB, inboxA, inboxB, flush };
}

/// Advance both sessions in 10 ms steps, flushing traffic each step.
function run(pair, fromMs, toMs, stepMs = 10) {
  for (let t = fromMs; t <= toMs; t += stepMs) {
    pair.a.pump(t);
    pair.b.pump(t);
    pair.flush();
  }
}

test('turn gating: nothing executes until BOTH bundles arrive', () => {
  const pair = linkedPair();
  pair.a.start(1000);
  pair.b.start(1000);
  // A plays alone for a while; B's transport is never flushed, so
  // A's bundles reach B but B's never reach A: A must stall.
  for (let t = 1000; t <= 1600; t += 10) {
    pair.a.pump(t);
    pair.b.pump(t);
    for (const m of pair.inboxB.splice(0)) pair.b.receive(m);
    // inboxA intentionally left undelivered
  }
  // Turns [0, delay) are pre-filled empty on both sides by
  // construction — A may execute exactly those, then must stall.
  assert.equal(pair.a.state().turn, INITIAL_DELAY_TURNS,
    'A ran past the pre-filled buffer without B\'s bundle');
  assert.equal(pair.a.state().waiting, true);
  // Now deliver B's bundles: A catches up.
  run(pair, 1610, 2200);
  assert.ok(pair.a.state().turn > INITIAL_DELAY_TURNS + 5, 'A caught up after delivery');
  assert.equal(pair.a.state().turn, pair.b.state().turn);
});

test('lockstep run: identical application order and hashes on both sides', () => {
  const pair = linkedPair();
  pair.a.start(0);
  pair.b.start(0);
  pair.a.localCommand({ t: 'move', e: 3, x: 10, y: 20 });
  pair.b.localCommand({ t: 'gather', e: 8, n: 4 });
  run(pair, 0, 3000);
  assert.equal(pair.a.state().turn, pair.b.state().turn);
  assert.ok(pair.a.state().turn >= 55, `turns executed: ${pair.a.state().turn}`);
  assert.deepEqual(pair.simA.applied, pair.simB.applied);
  assert.equal(pair.eventsA.filter((e) => e.type === 'desync').length, 0);
  // The first turns are empty by construction (input delay buffer).
  assert.deepEqual(pair.simA.applied[0], [0, []]);
  // The opening commands land at turn INITIAL_DELAY_TURNS.
  const stamped = pair.a.state().turn > INITIAL_DELAY_TURNS
    ? pair.simA.applied[INITIAL_DELAY_TURNS * 2]
    : null;
  assert.deepEqual(stamped, [0, [{ t: 'move', e: 3, x: 10, y: 20 }]]);
});

test('desync: a divergent hash stops both sides with the turn number', () => {
  const simB = fakeSim();
  const inboxA = [];
  const inboxB = [];
  const eventsA = [];
  const eventsB = [];
  const simA = fakeSim();
  const a = createSession({
    localSlot: 0,
    send: (m) => inboxB.push(m),
    applyBundle: (s, c) => simA.applyBundle(s, c),
    tickOnce: () => simA.tick(),
    onEvent: (e) => eventsA.push(e),
  });
  let ticksB = 0;
  const b = createSession({
    localSlot: 1,
    send: (m) => inboxA.push(m),
    applyBundle: (s, c) => simB.applyBundle(s, c),
    tickOnce: () => {
      ticksB++;
      // B's sim "diverges" during turn 7.
      return ticksB > 7 * TURN_TICKS && ticksB <= 8 * TURN_TICKS ? 'hdeadbeef' : simB.tick();
    },
    onEvent: (e) => eventsB.push(e),
  });
  a.start(0);
  b.start(0);
  const pair = { a, b, flush: () => {
    for (const m of inboxA.splice(0)) a.receive(m);
    for (const m of inboxB.splice(0)) b.receive(m);
  } };
  run(pair, 0, 2000);
  const dA = eventsA.find((e) => e.type === 'desync');
  const dB = eventsB.find((e) => e.type === 'desync');
  assert.ok(dA && dB, 'both sides detected the desync');
  assert.equal(dA.turn, 7);
  assert.equal(dB.turn, 7);
  assert.equal(a.state().phase, 'desync');
  assert.equal(b.state().phase, 'desync');
  assert.equal(a.state().turn, 8, 'execution stopped at the desync turn');
});

test('NET-04: a divergence on one tick of a turn is caught at that tick', () => {
  const simB = fakeSim();
  const inboxA = [];
  const inboxB = [];
  const eventsA = [];
  const simA = fakeSim();
  const a = createSession({
    localSlot: 0,
    send: (m) => inboxB.push(m),
    applyBundle: (s, c) => simA.applyBundle(s, c),
    tickOnce: () => simA.tick(),
    onEvent: (e) => eventsA.push(e),
  });
  let ticksB = 0;
  const b = createSession({
    localSlot: 1,
    send: (m) => inboxA.push(m),
    applyBundle: (s, c) => simB.applyBundle(s, c),
    tickOnce: () => {
      ticksB++;
      // B diverges on the SECOND tick of turn 5 only; the turn's
      // first and third tick hashes agree — per-turn comparison
      // would have missed this entirely.
      return ticksB === 5 * TURN_TICKS + 2 ? 'hdeadbeef' : simB.tick();
    },
    onEvent: () => {},
  });
  a.start(0);
  b.start(0);
  const pair = { a, b, flush: () => {
    for (const m of inboxA.splice(0)) a.receive(m);
    for (const m of inboxB.splice(0)) b.receive(m);
  } };
  run(pair, 0, 2000);
  const dA = eventsA.find((e) => e.type === 'desync');
  assert.ok(dA, 'the per-tick divergence was detected');
  assert.equal(dA.turn, 5);
  assert.equal(dA.tickInTurn, 1);
  assert.equal(dA.tick, 5 * TURN_TICKS + 1);
});

test('NET-04: the desync event names the first diverging SIM-06 subsystem', () => {
  const simB = fakeSim();
  const inboxA = [];
  const inboxB = [];
  const eventsA = [];
  const simA = fakeSim();
  let ticksA = 0;
  let ticksB = 0;
  const parts = (tickNo, economy) => ['rng' + tickNo, 'world' + tickNo, economy, 'targeting' + tickNo];
  const a = createSession({
    localSlot: 0,
    send: (m) => inboxB.push(m),
    applyBundle: (s, c) => simA.applyBundle(s, c),
    tickOnce: () => { ticksA++; return simA.tick(); },
    hashParts: () => parts(ticksA, 'eco-same'),
    onEvent: (e) => eventsA.push(e),
  });
  const b = createSession({
    localSlot: 1,
    send: (m) => inboxA.push(m),
    applyBundle: (s, c) => simB.applyBundle(s, c),
    tickOnce: () => {
      ticksB++;
      return ticksB === 5 * TURN_TICKS + 2 ? 'hdeadbeef' : simB.tick();
    },
    hashParts: () => parts(ticksB, ticksB === 5 * TURN_TICKS + 2 ? 'eco-DIVERGED' : 'eco-same'),
    onEvent: () => {},
  });
  a.start(0);
  b.start(0);
  const pair = { a, b, flush: () => {
    for (const m of inboxA.splice(0)) a.receive(m);
    for (const m of inboxB.splice(0)) b.receive(m);
  } };
  run(pair, 0, 2000);
  const dA = eventsA.find((e) => e.type === 'desync');
  assert.ok(dA, 'desync detected');
  assert.equal(dA.subsystem, 'economy');
});

test('NET-04: a legacy per-turn hash still compares against the last tick', () => {
  const pair = linkedPair();
  pair.a.start(0);
  pair.b.start(0);
  // Rewrite every hash message bound for A into the legacy
  // single-hash form (a cached older client): the match must run
  // clean — the legacy hash is the turn's last-tick hash.
  for (let t = 0; t <= 2000; t += 10) {
    pair.a.pump(t);
    pair.b.pump(t);
    for (const m of pair.inboxB.splice(0)) pair.b.receive(m);
    for (const m of pair.inboxA.splice(0)) {
      pair.a.receive(m.type === 'hash'
        ? { type: 'hash', turn: m.turn, hash: m.hashes[m.hashes.length - 1] }
        : m);
    }
  }
  assert.equal(pair.eventsA.filter((e) => e.type === 'desync').length, 0);
  assert.ok(pair.a.state().turn > 20, 'turns executed under legacy hashes');
});

test('NET-04 codec: per-tick hash messages validate strictly', () => {
  assert.equal(decodeMessage('{"type":"hash","turn":1,"hashes":["ha","hb","hc"]}').ok, true);
  assert.equal(decodeMessage('{"type":"hash","turn":1,"hashes":["ha"],"parts":[["r","w"]]}').ok, true);
  assert.equal(decodeMessage('{"type":"hash","turn":1,"hash":"hlast"}').ok, true);
  assert.equal(decodeMessage('{"type":"hash","turn":1,"hashes":"nope"}').ok, false);
  assert.equal(decodeMessage('{"type":"hash","turn":1,"hashes":["ha"],"parts":[["r"],["x"]]}').ok, false);
  assert.equal(decodeMessage('{"type":"hash","turn":1,"hashes":[]}').ok, false);
});

test('NET-07: exact duplicates within one bundle collapse', () => {
  const pair = linkedPair();
  pair.a.start(0);
  pair.b.start(0);
  const cmd = { t: 'move', e: 3, x: 1, y: 1 };
  pair.a.localCommand(cmd);
  pair.a.localCommand({ ...cmd });
  pair.a.localCommand({ t: 'move', e: 4, x: 1, y: 1 });
  run(pair, 0, 500);
  const bundle = pair.simB.applied.find(([s, cmds]) => s === 0 && cmds.length > 0);
  assert.equal(bundle[1].length, 2, 'duplicate dropped, distinct kept');
  assert.equal(pair.a.state().droppedDupes, 1);
});

test('delay adaptation: a lagging link raises the input delay for both', () => {
  // B's bundles are delivered 400 ms late throughout: turns execute
  // late persistently, so the host proposes a raise and both agree.
  const pair = linkedPair();
  pair.a.start(0);
  pair.b.start(0);
  const held = [];
  for (let t = 0; t <= 30000; t += 10) {
    pair.a.pump(t);
    pair.b.pump(t);
    for (const m of pair.inboxB.splice(0)) pair.b.receive(m);
    for (const m of pair.inboxA.splice(0)) held.push([t + 400, m]);
    for (let i = held.length - 1; i >= 0; i--) {
      if (held[i][0] <= t) {
        pair.a.receive(held[i][1]);
        held.splice(i, 1);
      }
    }
  }
  assert.ok(pair.a.state().delay > INITIAL_DELAY_TURNS,
    `host delay adapted: ${pair.a.state().delay}`);
  assert.equal(pair.b.state().delay, pair.a.state().delay, 'guest applied the same delay');
  assert.equal(pair.eventsA.filter((e) => e.type === 'desync').length, 0);
});

test('rejoin: rebase (remaining peer) vs startAt (rejoiner) keep identical tick streams', () => {
  // REGRESSION for F2 (2026-10-22 live exercise). The production
  // resume is ASYMMETRIC — the remaining peer rebase()es and
  // EXECUTES the gap turns [frontier, atTurn), while the rejoiner
  // rebuilds from the recorded stream and startAt()s at atTurn.
  // The old test called rebase() on BOTH sides with sims whose
  // ticks it never accounted, so it could not see that startAt
  // skipped the gap: the rejoiner's sim stayed delay + 2 turns
  // behind and the first resumed hash exchange desynced. Here
  // the fake sims' hashes are tick-sensitive, the rejoiner is a
  // genuinely fresh sim + session rebuilt from the recorder
  // stream (rejoinFromResync's shape), and the resumed match
  // must compare equal hashes for 120+ turns with zero desyncs.
  const rec = createRecorder({ seed: 9, modeTag: 3, seats: [0, 2] });
  const hashesA = new Map();
  const pair = linkedPair({
    onTurn: (turn, c0, c1, hash) => {
      rec.addTurn(turn, c0, c1);
      hashesA.set(turn, hash);
    },
  });
  pair.a.start(0);
  pair.b.start(0);
  pair.a.localCommand({ t: 'train_vil' });
  pair.b.localCommand({ t: 'train_vil' });
  run(pair, 0, 2000);
  const frontier = pair.a.state().turn;
  assert.equal(frontier, pair.b.state().turn);
  assert.ok(frontier > 20);
  const snapshot = rec.finish({ ticks: frontier * TURN_TICKS });

  // The drop: B's session and sim are gone, and whatever traffic
  // was in flight is eaten by the disconnect.
  pair.inboxA.length = 0;
  pair.inboxB.length = 0;

  // The rejoiner rebuilds: a fresh sim fast-forwarded through the
  // transferred stream — each event's bundles applied at their
  // tick, in slot order, ticking in between (exactly
  // rejoinFromResync). It lands on the frontier, NOT on atTurn.
  const simB2 = fakeSim();
  const byTick = new Map();
  for (const e of snapshot.events) byTick.set(e.tick, e);
  for (let t = 0; t < snapshot.ticks; t++) {
    const e = byTick.get(t);
    if (e) {
      simB2.applyBundle(0, e.s0);
      if (e.s1) simB2.applyBundle(1, e.s1);
    }
    simB2.tick();
  }
  const eventsB2 = [];
  const hashesB2 = new Map();
  const b2 = createSession({
    localSlot: 1,
    send: (m) => pair.inboxA.push(m),
    applyBundle: (s, cmds) => simB2.applyBundle(s, cmds),
    tickOnce: () => simB2.tick(),
    onEvent: (e) => eventsB2.push(e),
    onTurn: (turn, c0, c1, hash) => hashesB2.set(turn, hash),
  });
  const flushAB2 = () => {
    for (const m of pair.inboxA.splice(0)) pair.a.receive(m);
    for (const m of pair.inboxB.splice(0)) b2.receive(m);
  };
  const runAB2 = (fromMs, toMs) => {
    for (let t = fromMs; t <= toMs; t += 10) {
      pair.a.pump(t);
      b2.pump(t);
      flushAB2();
    }
  };

  // The resume handshake: the remaining peer rebases, the
  // rejoiner startAts — both to frontier + delay + 2, and the
  // rejoiner hands startAt its rebuilt tick count so the gap is
  // ticked, not skipped (the F2 invariant).
  const atTurn = frontier + INITIAL_DELAY_TURNS + 2;
  pair.a.rebase(atTurn, 5000);
  b2.startAt(atTurn, 5000, INITIAL_DELAY_TURNS, snapshot.ticks);
  pair.a.localCommand({ t: 'move', e: 1, x: 5, y: 5 });
  b2.localCommand({ t: 'move', e: 2, x: 6, y: 6 });
  runAB2(5000, 5000 + 140 * TURN_MS + 500);

  assert.ok(pair.a.state().turn > atTurn + 120,
    `host advanced past the resume: ${pair.a.state().turn}`);
  assert.equal(b2.state().turn, pair.a.state().turn,
    'both sessions executed the same turns');
  assert.equal(pair.eventsA.filter((e) => e.type === 'desync').length, 0,
    'host desynced after the resume');
  assert.equal(eventsB2.filter((e) => e.type === 'desync').length, 0,
    'rejoiner desynced after the resume');
  let compared = 0;
  for (const [turn, h] of hashesB2) {
    if (turn < atTurn) continue;
    assert.equal(hashesA.get(turn), h, `hash mismatch at resumed turn ${turn}`);
    compared++;
  }
  assert.ok(compared >= 120, `resumed hashes actually compared: ${compared}`);
});

test('startAt: rebuiltTicks ticks the empty gap before opening', () => {
  const mkCounting = () => {
    const sim = fakeSim();
    const counter = { ticks: 0 };
    const session = createSession({
      localSlot: 1,
      send: () => {},
      applyBundle: (s, cmds) => sim.applyBundle(s, cmds),
      tickOnce: () => { counter.ticks++; return sim.tick(); },
      onEvent: () => {},
    });
    return { session, counter };
  };
  // Rebuilt to tick 282 (= the turn-94 frontier); the resume
  // opens at turn 100: startAt must tick the (100 - 94) turns of
  // command-free gap itself — the F2 invariant, counted.
  const { session, counter } = mkCounting();
  session.startAt(100, 5000, INITIAL_DELAY_TURNS, 282);
  assert.equal(counter.ticks, 100 * TURN_TICKS - 282);
  assert.equal(session.state().tick, 100 * TURN_TICKS);
  // Without rebuiltTicks the legacy contract stands: the caller
  // asserts the sim already stands at the frontier; nothing ticks.
  const legacy = mkCounting();
  legacy.session.startAt(100, 5000, INITIAL_DELAY_TURNS);
  assert.equal(legacy.counter.ticks, 0);
});

// ---------------------------------------------------------------- recorder / replay

test('recorder: an online run records a replay that re-executes identically', () => {
  const rec = createRecorder({ seed: 42, modeTag: 3, region: 'wnam', seats: [0, 2] });
  const pair = linkedPair({
    onTurn: (turn, c0, c1) => rec.addTurn(turn, c0, c1),
  });
  pair.a.start(0);
  pair.b.start(0);
  pair.a.localCommand({ t: 'train_vil' });
  pair.b.localCommand({ t: 'train_vil' });
  run(pair, 0, 1500);
  const replay = rec.finish({ result: 'ongoing', ticks: pair.a.state().tick });
  assert.equal(isReplay(replay), true);
  const events = replayEvents(replay);
  assert.ok(events.length >= 1);
  // Re-execution: feed the events through a fresh pair of fake sims
  // in slot order at their ticks — the application log must match
  // the original run's exactly.
  const simC = fakeSim();
  for (const e of events) {
    simC.applyBundle(0, e.s0);
    if (e.s1) simC.applyBundle(1, e.s1);
  }
  const original = pair.simA.applied.filter(([, cmds]) => cmds.length > 0);
  assert.deepEqual(simC.applied, original);
  assert.equal(isReplay({ kind: 'nope' }), false);
  assert.equal(replayEvents({ kind: 'skirmish2-replay', v: 1, seed: 1, modeTag: 3, events: [{ tick: 0, s0: [{ t: 'bogus' }] }] }), null);
});

test('recorder: local commands group by tick', () => {
  const rec = createRecorder({ seed: 7, modeTag: 2, seats: [0] });
  rec.addLocal(10, { t: 'move', e: 1, x: 0, y: 0 });
  rec.addLocal(10, { t: 'move', e: 2, x: 0, y: 0 });
  rec.addLocal(25, { t: 'gather', e: 1, n: 3 });
  const r = rec.finish({});
  assert.equal(r.events.length, 2);
  assert.equal(r.events[0].s0.length, 2);
});

// ---------------------------------------------------------------- transport / probe

test('relay client encodes sends and dispatches decoded messages', () => {
  const sent = [];
  const got = [];
  const handlers = {};
  const fakeWs = {
    readyState: 1,
    addEventListener: (ev, fn) => { handlers[ev] = fn; },
    send: (t) => sent.push(t),
    close: () => {},
  };
  const client = createRelayClient({
    socketFactory: () => fakeWs,
    onMessage: (m) => got.push(m),
  });
  client.connect('ws://example/room/x');
  client.send({ type: 'hello', v: 1 });
  assert.deepEqual(JSON.parse(sent[0]), { type: 'hello', v: 1 });
  handlers.message({ data: '{"type":"hash","turn":3,"hash":"habc"}' });
  handlers.message({ data: 'garbage' });
  assert.deepEqual(got, [{ type: 'hash', turn: 3, hash: 'habc' }]);
});

function fakeSocketFactory(script) {
  // script: { openDelay, echoes: [rtt1, rtt2] | null }
  return (url) => {
    const handlers = {};
    const ws = {
      url,
      readyState: 0,
      addEventListener: (ev, fn) => { handlers[ev] = fn; },
      send: () => {
        if (script.echoes) {
          const rtt = script.echoes.shift();
          if (rtt !== undefined) {
            setTimeout(() => handlers.message && handlers.message({ data: 'probe' }), rtt);
          }
        }
      },
      close: () => {},
    };
    setTimeout(() => {
      ws.readyState = 1;
      handlers.open && handlers.open();
    }, script.openDelay);
    return ws;
  };
}

test('region ping reports the second (warm) echo round trip', async () => {
  const res = await probeRegionPing({
    relayHttp: 'https://relay.example',
    region: 'weur',
    socketFactory: fakeSocketFactory({ openDelay: 5, echoes: [120, 40] }),
    rand: 'testrand',
  });
  assert.equal(res.via, 'echo');
  assert.ok(res.ms >= 35 && res.ms <= 60, `warm echo ~40ms, got ${res.ms}`);
});

test('region ping falls back to handshake timing when echoes never return', async () => {
  const res = await probeRegionPing({
    relayHttp: 'https://relay.example',
    region: 'weur',
    socketFactory: fakeSocketFactory({ openDelay: 30, echoes: null }),
    timeoutMs: 200,
    rand: 'testrand',
  });
  assert.equal(res.via, 'handshake');
  assert.ok(res.ms >= 25 && res.ms <= 80, `handshake ~30ms, got ${res.ms}`);
});

// ---------------------------------------------------------------- lobby helpers

test('lobby helpers: room ids, links, fastest region', () => {
  const id = makeRoomId(() => 0.5);
  assert.equal(id.length, 10);
  assert.match(id, /^[a-z2-9]+$/);
  const url = buildPlayUrl('https://skirmish.online/play/', 'abc123xyz9', 'wnam');
  assert.equal(url, 'https://skirmish.online/play/?room=abc123xyz9&region=wnam');
  const parsed = parsePlayParams('?room=abc123xyz9&region=wnam');
  assert.deepEqual(parsed, { room: 'abc123xyz9', region: 'wnam', online: false, private: false });
  assert.equal(parsePlayParams('?online=1').online, true);
  assert.equal(pickFastestRegion({ wnam: 80, weur: 25, sam: null }), 'weur');
  assert.equal(pickFastestRegion({}), null);
  assert.equal(REGIONS.length, 7);
});

test('startAt: a session opens at the frontier with an empty delay buffer', () => {
  const sim = fakeSim();
  const sent = [];
  const s = createSession({
    localSlot: 1,
    send: (m) => sent.push(m),
    applyBundle: (sl, cmds) => sim.applyBundle(sl, cmds),
    tickOnce: () => sim.tick(),
    onEvent: () => {},
  });
  s.startAt(100, 5000, INITIAL_DELAY_TURNS);
  // A stale bundle from before the frontier is ignored entirely.
  s.receive({ type: 'bundle', turn: 50, cmds: [{ t: 'move', e: 1, x: 0, y: 0 }] });
  // Turns [100, 100+delay) are pre-filled: they execute with no
  // traffic at all once their targets pass.
  for (let t = 5000; t <= 5300; t += 10) s.pump(t);
  assert.ok(s.state().turn >= 100 + INITIAL_DELAY_TURNS,
    `advanced through the buffer: ${s.state().turn}`);
  assert.deepEqual(sim.applied[0], [0, []]);
  // The first bundle this session SENDS is for turn 100+delay.
  const first = sent.find((m) => m.type === 'bundle');
  assert.equal(first.turn, 100 + INITIAL_DELAY_TURNS);
  // localCommand stamps for the open turn, delay turns ahead.
  const stamped = s.localCommand({ t: 'train_vil' });
  assert.ok(stamped >= 100 + INITIAL_DELAY_TURNS);
});

// ------------------------------------------------------- game speed
// Gap 2 (2026-10-19 study fix): one constant, GAME_SPEED = 1.7
// (DE Normal), drives BOTH places real time becomes ticks —
// the local frame loop's accumulator (advanceTicks) and the
// netplay turn pacing (TURN_MS). The sim never sees it.
// SIM-11 (bot-loop, 2026-10-28): the constant became a SETTING —
// the same conversion takes the match's chosen speed (GAME_SPEEDS),
// sessions pace their turns from it, and the default stays 1.7.

test('game speed setting: the DE lobby speeds, standard 1.7', () => {
  assert.deepEqual(GAME_SPEEDS, [1.0, 1.5, 1.7, 2.0]);
  for (const s of GAME_SPEEDS) assert.ok(isGameSpeed(s), `${s} is a lobby speed`);
  for (const bad of [0, 1.2, 3, '1.7', null, undefined, NaN]) {
    assert.ok(!isGameSpeed(bad), `${bad} is not a lobby speed`);
  }
  // Turn wall time scales inversely with the speed; the default
  // export stays the standard-speed value existing callers pin.
  assert.equal(turnMsFor(1.7), TURN_MS);
  assert.ok(Math.abs(turnMsFor(1.0) - 50) < 1e-9, 'at 1.0x a turn is 50 wall ms');
  assert.ok(Math.abs(turnMsFor(2.0) - 25) < 1e-9, 'at 2.0x a turn is 25 wall ms');
  assert.equal(turnMsFor(9), TURN_MS, 'an unknown speed falls back to standard');
});

test('game speed setting: advanceTicks honours the match speed', () => {
  // One real second at 1.0x runs exactly one game second (60 ticks).
  let acc = 0; let total = 0;
  for (let i = 0; i < 60; i++) { const r = advanceTicks(acc, 1000 / 60, 1.0); acc = r.accMs; total += r.ticks; }
  assert.ok(Math.abs(total - 60) <= 1, `1.0x ticks over 60 frames: ${total}`);
  // At 2.0x the same wall second runs two game seconds (120 ticks).
  acc = 0; total = 0;
  for (let i = 0; i < 60; i++) { const r = advanceTicks(acc, 1000 / 60, 2.0); acc = r.accMs; total += r.ticks; }
  assert.ok(Math.abs(total - 120) <= 1, `2.0x ticks over 60 frames: ${total}`);
  // No speed argument = the pre-setting behaviour, bit for bit.
  assert.deepEqual(advanceTicks(0, 50), advanceTicks(0, 50, 1.7));
  assert.deepEqual(advanceTicks(0, 50, 9), advanceTicks(0, 50), 'unknown speed falls back');
});

test('game speed setting: two sessions at 2.0x pace turns at 2.0x, in lockstep', () => {
  const fast = linkedPair({ speed: 2.0 });
  fast.a.start(0);
  fast.b.start(0);
  run(fast, 0, 1500);
  const std = linkedPair();
  std.a.start(0);
  std.b.start(0);
  run(std, 0, 1500);
  assert.equal(fast.a.state().turn, fast.b.state().turn, 'fast pair stays in lockstep');
  assert.equal(fast.eventsA.filter((e) => e.type === 'desync').length, 0);
  // 1500 wall ms hold 1500/25 = 60 turns at 2.0x and 1500/29.41 ≈ 51
  // at the standard 1.7x: the executed-turn counts track the speeds.
  assert.ok(Math.abs(fast.a.state().turn - 60) <= 2, `2.0x turns: ${fast.a.state().turn}`);
  assert.ok(Math.abs(std.a.state().turn - 51) <= 2, `1.7x turns: ${std.a.state().turn}`);
  assert.ok(fast.a.state().turn > std.a.state().turn,
    'the faster setting executes more turns in the same wall time');
});

test('game speed: 1.7x drives the turn interval math', () => {
  assert.equal(GAME_SPEED, 1.7, 'DE Normal speed');
  assert.equal(TICK_MS, 1000 / 60, 'a tick is 1/60 s of game time');
  // A turn is TURN_TICKS ticks of game time, compressed by the
  // speed into wall time: 3 ticks x 16.67 ms / 1.7 ≈ 29.41 ms.
  assert.ok(Math.abs(TURN_MS - (TURN_TICKS * TICK_MS) / GAME_SPEED) < 1e-9);
  assert.ok(Math.abs(TURN_MS - 29.4118) < 0.001, `TURN_MS=${TURN_MS}`);
  // 34 turns per real second; one game second per ~588 wall ms.
  assert.ok(Math.abs(1000 / TURN_MS - 34) < 0.01);
});

test('game speed: advanceTicks converts wall time to ticks at 1.7x', () => {
  // One real second runs 102 ticks (1.7 game seconds at 60 tps).
  // Float note: TICK_MS is a hair over 50/3, so a single 1000 ms
  // step can floor to 101 with a near-full-tick remainder — the
  // remainder carries, so the long-run rate is exact (below).
  const s1 = advanceTicks(0, 1000);
  assert.ok(Math.abs(s1.ticks - 102) <= 1, `ticks ${s1.ticks}`);
  assert.ok(s1.accMs >= 0 && s1.accMs < TICK_MS, `remainder ${s1.accMs}`);
  // A 50 ms frame step runs 5 ticks and keeps the 0.1-tick rest.
  const s2 = advanceTicks(0, 50);
  assert.equal(s2.ticks, 5);
  assert.ok(Math.abs(s2.accMs - (85 - 5 * TICK_MS)) < 1e-9);
  // The remainder carries: two half-steps equal one whole step.
  const a = advanceTicks(0, 25);
  const b = advanceTicks(a.accMs, 25);
  assert.equal(a.ticks + b.ticks, s2.ticks);
  // A render frame at 60 Hz advances 1.7 ticks on average:
  // over 60 frames that is 102 ticks (±1 for float accumulation
  // in the remainder, the same property the pre-1.7x accumulator
  // had).
  let acc = 0;
  let total = 0;
  for (let i = 0; i < 60; i++) {
    const r = advanceTicks(acc, 1000 / 60);
    acc = r.accMs;
    total += r.ticks;
  }
  assert.ok(Math.abs(total - 102) <= 1, `ticks over 60 frames: ${total}`);
});
