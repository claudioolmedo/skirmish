// Headless two-client integration test for M8 lockstep netplay.
// NOT part of `node --test` (it needs the live relay + the built
// WASM pkg). Run from web-preview/play/:
//   node netplay.integration.mjs
//
// Two real WasmSim instances in one Node process, two real
// WebSocket connections to the LIVE relay (region wnam), the
// production netplay.js session on both sides. They handshake,
// play scripted commands for >= 1,800 ticks, and must agree on the
// state hash at EVERY turn. The recorded replay is then re-executed
// through a third fresh sim and must reproduce every per-turn hash.
// Exit code 0 = pass; the summary line is the report evidence.

import { readFileSync } from 'node:fs';
import init, { WasmSim } from '../pkg/skirmish_sim.js';
import {
  TURN_TICKS, applyCommand, tcIdOf, createSession, createRecorder,
  createRelayClient, probeRegionPing,
} from './netplay.js';

const RELAY = 'https://skirmish-relay.hoardserdure7.workers.dev';
const REGION = 'wnam';
const SEED = 1234;
const TARGET_TICKS = 1800;
const TARGET_TURNS = TARGET_TICKS / TURN_TICKS;

const fail = (msg) => {
  console.error('INTEGRATION FAIL:', msg);
  process.exit(1);
};

await init(readFileSync(new URL('../pkg/skirmish_sim_bg.wasm', import.meta.url)));

// -- Region probe against the live relay (also validates echo mode)
const probe = await probeRegionPing({ relayHttp: RELAY, region: REGION });
console.log('probe wnam:', JSON.stringify(probe));
if (!probe || typeof probe.ms !== 'number') fail('region probe returned no measurement');

const room = 'itest' + Math.random().toString(36).slice(2, 10);
const roomUrl = (role) =>
  `${RELAY.replace(/^http/, 'ws')}/room/${room}?region=${REGION}`;

// ------------------------------------------------------------ one side
function makeSide(role) {
  const slot = role === 'host' ? 0 : 1;
  const owner = slot === 0 ? 0 : 2;
  const side = {
    role, slot, owner,
    sim: null,
    session: null,
    hashes: [],
    desyncs: [],
    peerPresent: false,
    closed: false,
    recorder: role === 'host'
      ? createRecorder({ seed: SEED, modeTag: 3, region: REGION, seats: [0, 2] })
      : null,
  };
  side.client = createRelayClient({
    onOpen: () => side.client.send({ type: 'hello', v: 1, role }),
    onClose: () => { side.closed = true; },
    onMessage: (msg) => {
      if (side.session && side.session.receive(msg)) return;
      if (msg.type === 'hello') {
        side.peerPresent = true;
        return;
      }
      if (role === 'guest' && msg.type === 'start') {
        side.sim = WasmSim.new_match(msg.seed, 3);
        const h = side.sim.state_hash().toString(16);
        if (h !== msg.hash0) {
          side.client.send({ type: 'start-abort', reason: 'build-mismatch' });
          fail(`construction hash mismatch: guest ${h} vs host ${msg.hash0}`);
        }
        side.client.send({ type: 'start-ack' });
        return;
      }
      if (role === 'host' && msg.type === 'start-ack') {
        side.client.send({ type: 'go', inMs: 600 });
        side.begin(Date.now() + 600);
        return;
      }
      if (role === 'guest' && msg.type === 'go') {
        side.begin(Date.now() + (msg.inMs || 600));
        return;
      }
    },
  });
  side.begin = (t0) => {
    if (!side.sim) side.sim = WasmSim.new_match(SEED, 3);
    const sim = side.sim;
    side.session = createSession({
      localSlot: slot,
      send: (m) => side.client.send(m),
      applyBundle: (s, cmds) => {
        const o = s === 0 ? 0 : 2;
        for (const c of cmds) applyCommand(sim, c, o, (oo) => tcIdOf(sim, oo));
      },
      tickOnce: () => {
        sim.tick();
        return sim.state_hash().toString(16);
      },
      onEvent: (e) => { if (e.type === 'desync') side.desyncs.push(e); },
      onTurn: (turn, c0, c1, hash) => {
        side.hashes[turn] = hash;
        if (side.recorder) side.recorder.addTurn(turn, c0, c1);
      },
    });
    side.session.start(t0);
  };
  return side;
}

const host = makeSide('host');
const guest = makeSide('guest');
host.client.connect(roomUrl('host'));
guest.client.connect(roomUrl('guest'));

// Wait for presence both ways, then the host starts the match.
const deadline = Date.now() + 20000;
while (!(host.peerPresent && guest.peerPresent)) {
  if (Date.now() > deadline) fail('presence handshake timed out');
  await new Promise((r) => setTimeout(r, 25));
}
host.sim = WasmSim.new_match(SEED, 3);
host.client.send({
  type: 'start', seed: SEED, hash0: host.sim.state_hash().toString(16),
});

// ------------------------------------------------------------ play
const t0wall = Date.now();
let scriptStep = 0;
const scriptTimer = setInterval(() => {
  if (!host.session || !guest.session) return;
  const turn = host.session.state().turn;
  if (scriptStep === 0 && turn >= 2) {
    scriptStep++;
    host.session.localCommand({ t: 'train_vil' });
    guest.session.localCommand({ t: 'train_vil' });
  } else if (scriptStep === 1 && turn >= 30) {
    scriptStep++;
    guest.session.localCommand({ t: 'research_tc', tech: 0 }); // Loom
    host.session.localCommand({ t: 'rally_tc', x: 0, y: -60 });
  } else if (scriptStep === 2 && turn >= 80) {
    scriptStep++;
    const kinds = host.sim.entity_kinds();
    const owners = host.sim.entity_owners();
    const pos = host.sim.positions();
    for (let i = 0; i < kinds.length; i++) {
      if (kinds[i] === 0 && owners[i] === 0) {
        host.session.localCommand({ t: 'move', e: i, x: pos[i * 2] + 12, y: pos[i * 2 + 1] });
        break;
      }
    }
    for (let i = 0; i < kinds.length; i++) {
      if (kinds[i] === 0 && owners[i] === 2) {
        guest.session.localCommand({ t: 'move', e: i, x: pos[i * 2] - 12, y: pos[i * 2 + 1] });
        break;
      }
    }
  }
}, 50);

while (true) {
  const now = Date.now();
  if (host.session) host.session.pump(now);
  if (guest.session) guest.session.pump(now);
  const turn = host.session ? host.session.state().turn : 0;
  if (turn >= TARGET_TURNS) break;
  if (Date.now() - t0wall > 120000) fail(`timed out at turn ${turn}`);
  if (host.desyncs.length || guest.desyncs.length) {
    fail(`desync: ${JSON.stringify(host.desyncs[0] || guest.desyncs[0])}`);
  }
  await new Promise((r) => setTimeout(r, 5));
}
clearInterval(scriptTimer);

// Let the guest execute the final turns too (its last bundles and
// hashes are still in flight when the host crosses the line).
{
  const catchUpDeadline = Date.now() + 15000;
  host.session.stop();
  while (guest.session.state().turn < TARGET_TURNS && Date.now() < catchUpDeadline) {
    guest.session.pump(Date.now());
    await new Promise((r) => setTimeout(r, 5));
  }
  guest.session.stop();
}

const turns = host.session.state().turn;
let compared = 0;
let mismatches = 0;
for (let t = 0; t < turns; t++) {
  if (host.hashes[t] !== undefined && guest.hashes[t] !== undefined) {
    compared++;
    if (host.hashes[t] !== guest.hashes[t]) mismatches++;
  }
}
console.log(`played: ${turns} turns = ${turns * TURN_TICKS} ticks`);
console.log(`hashes compared: ${compared}, mismatches: ${mismatches}`);
if (mismatches > 0) fail('per-turn hashes diverged');
if (compared < TARGET_TURNS) fail('not all turns were hash-compared');

// Both clients' sims agree on the state hash at the last compared
// turn (each turn hash IS the sim's state hash after that turn's
// final tick — the strongest form of the check).
const finalHost = host.hashes[TARGET_TURNS - 1];
const finalGuest = guest.hashes[TARGET_TURNS - 1];
if (finalHost !== finalGuest) fail('final turn hash differs');
console.log(`hash at turn ${TARGET_TURNS - 1} (both clients):`, finalHost);

// ------------------------------------------------------------ replay check
const replay = host.recorder.finish({ result: 'abandoned', ticks: turns * TURN_TICKS });
const simC = WasmSim.new_match(replay.seed, replay.modeTag);
const byTick = new Map();
for (const e of replay.events) byTick.set(e.tick, e);
let replayMismatches = 0;
for (let tick = 0; tick < turns * TURN_TICKS; tick++) {
  const e = byTick.get(tick);
  if (e) {
    for (const c of e.s0) applyCommand(simC, c, 0, (o) => tcIdOf(simC, o));
    if (e.s1) for (const c of e.s1) applyCommand(simC, c, 2, (o) => tcIdOf(simC, o));
  }
  simC.tick();
  if ((tick + 1) % TURN_TICKS === 0) {
    const turn = (tick + 1) / TURN_TICKS - 1;
    if (simC.state_hash().toString(16) !== host.hashes[turn]) replayMismatches++;
  }
}
console.log(`replay re-execution: ${turns} turn hashes checked, mismatches: ${replayMismatches}`);
if (replayMismatches > 0) fail('replay does not reproduce the match');

// ------------------------------------------------------------ telemetry + clean disconnect
const tel = await fetch(`${RELAY}/telemetry`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    region: REGION,
    durationTicks: turns * TURN_TICKS,
    result: 'abandoned',
    desyncCount: 0,
    buildVersion: 'm8-integration-test',
  }),
});
console.log('telemetry POST:', tel.status);
const agg = await (await fetch(`${RELAY}/telemetry`)).json();
console.log('telemetry aggregate:', JSON.stringify(agg));

host.client.close();
guest.client.close();
await new Promise((r) => setTimeout(r, 300));
console.log(`clean disconnect: host closed=${host.closed}, guest closed=${guest.closed}`);
console.log('INTEGRATION PASS');
process.exit(0);
