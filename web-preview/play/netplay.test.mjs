// Unit tests for netplay.js (M8) — run from web-preview/play/:
//   node --test netplay.test.mjs
// The session tests drive two real sessions over a hand-delivered
// transport with a fake clock and a fake deterministic sim, the
// same way the live integration test drives two real WASM sims.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  TURN_TICKS, GAME_SPEED, TICK_MS, TURN_MS, advanceTicks,
  INITIAL_DELAY_TURNS, LOAD_TIMEOUT_MS,
  validateCommand, applyCommand, tcIdOf,
  encodeMessage, decodeMessage,
  createSession, createRecorder, isReplay, replayEvents,
  createRelayClient, probeRegionPing,
  makeRoomId, buildPlayUrl, parsePlayParams, linkJoinRoom,
  pickFastestRegion, REGIONS,
} from './netplay.js';

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
    { t: 'train_at', b: 3, k: 2 },
    { t: 'train_vil' },
    { t: 'research_tc', tech: 1 },
    { t: 'research_at', b: 3, tech: 5 },
    { t: 'rally_tc', x: 1, y: 2 },
    { t: 'rally_b', b: 3, x: 1, y: 2 },
    { t: 'force_dep', e: 7 },
  ];
  for (const c of ok) assert.equal(validateCommand(c), true, JSON.stringify(c));
  const bad = [
    null, 42, {}, { t: 'nope' },
    { t: 'move', e: -1, x: 0, y: 0 },
    { t: 'move', e: 1, x: NaN, y: 0 },
    { t: 'build', e: 1, k: 9, x: 0, y: 0 },
    { t: 'train_at', b: 1, k: 5 },
    { t: 'research_tc', tech: 2 },
    { t: 'gather', e: 1 },
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

test('M10 lobby messages decode: ready, the password handshake, cancel', () => {
  for (const m of [
    { type: 'ready', ready: true },
    { type: 'ready', ready: false },
    { type: 'pw', pw: 'swordfish' },
    { type: 'pw-req' },
    { type: 'pw-ok' },
    { type: 'pw-no' },
    { type: 'cancel' },
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
  applyCommand(sim, { t: 'train_at', b: 9, k: 3 }, 0, tcOf);
  // Town Center orders resolve the ISSUER's Town Center:
  applyCommand(sim, { t: 'train_vil' }, 2, tcOf);
  applyCommand(sim, { t: 'research_tc', tech: 0 }, 0, tcOf);
  applyCommand(sim, { t: 'rally_tc', x: 3, y: 4 }, 2, tcOf);
  assert.deepEqual(calls, [
    ['push_move', 5, 1, 2],
    ['push_move_queued', 5, 1, 2],
    ['push_build_barracks', 5, 1, 2],
    ['push_build_archery_range_queued', 5, 1, 2],
    ['push_train_militia', 9],
    ['push_train_villager_at', 40],
    ['push_research_tc_at', 0, 0],
    ['push_set_rally_tc_of', 40, 3, 4],
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

test('rebase: play resumes from the executed frontier after a drop', () => {
  const pair = linkedPair();
  pair.a.start(0);
  pair.b.start(0);
  run(pair, 0, 2000);
  const frontier = pair.a.state().turn;
  assert.equal(frontier, pair.b.state().turn);
  assert.ok(frontier > 20);
  // Both sides rebase to frontier + delay + 2 anchored "now" (the
  // resync outcome): the gap turns are empty on both sides and the
  // match continues in lockstep.
  const atTurn = frontier + INITIAL_DELAY_TURNS + 2;
  pair.a.rebase(atTurn, 5000);
  pair.b.rebase(atTurn, 5000);
  pair.a.localCommand({ t: 'move', e: 1, x: 5, y: 5 });
  run(pair, 5000, 8000);
  assert.ok(pair.a.state().turn > atTurn + 10, 'turns advanced past the rebase');
  assert.equal(pair.a.state().turn, pair.b.state().turn);
  assert.deepEqual(pair.simA.applied, pair.simB.applied);
  assert.equal(pair.eventsA.filter((e) => e.type === 'desync').length, 0);
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
