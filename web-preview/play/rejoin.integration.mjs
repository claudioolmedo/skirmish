// Headless rejoin integration test (2026-10-22, rejoin fix F1+F2).
// NOT part of `node --test` (it needs the live relay + the built
// WASM pkg). Run from web-preview/play/:
//   node rejoin.integration.mjs
//
// REGRESSION for the 2026-10-22 live exercise, which proved the
// shipped rejoin broken at two points (full evidence:
// ~/workspace/skirmish-study/rejoin-exercise-2026-10-22.md):
//   F1 — nothing told the remaining client its peer's socket had
//        closed, so the grace window never opened and the
//        returning peer was answered room-full. Fixed relay-side
//        (v6 originates {"type":"peer-left"} on socket close) —
//        this test asserts the host learns of the drop from the
//        relay alone, with NO operator intervention.
//   F2 — the resume desynced on the first resumed turn: rebase()
//        EXECUTES the gap turns [frontier, atTurn) while the old
//        startAt() SKIPPED them, leaving the rejoiner delay+2
//        turns behind. Fixed in netplay.js startAt (it ticks the
//        command-free gap when handed the rebuilt tick count) —
//        this test asserts 150+ compared turns, 0 mismatches,
//        0 desyncs after EACH of two full drop/rejoin cycles.
//
// The app-level orchestration (hello / start / resync /
// resync-done / resume, sendResync, rejoinFromResync, the phase
// machine) replicates web-preview/app.js the same way the
// exercise harness did, so what passes here is what a real
// player's browser does. The one simplification, shared with
// the exercise: the host starts without the both-ready gate.

import { readFileSync } from 'node:fs';
import init, { WasmSim } from '../pkg/skirmish_sim.js';
import {
  TURN_TICKS, applyCommand, tcIdOf, createSession, createRecorder,
  createRelayClient, probeRegionPing,
} from './netplay.js';

const RELAY = 'https://skirmish-relay.hoardserdure7.workers.dev';
const REGION = 'wnam';
const MODE_TAG = 3; // MODE_TAG.online in app.js
const SEED = 777;
const PRE_DROP_TURN = 60;
const WATCH_TURNS = 150;

const fail = (msg) => {
  console.error('REJOIN INTEGRATION FAIL:', msg);
  process.exit(1);
};
const now = () => performance.now();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

await init(readFileSync(new URL('../pkg/skirmish_sim_bg.wasm', import.meta.url)));

const probe = await probeRegionPing({ relayHttp: RELAY, region: REGION });
console.log('probe wnam:', JSON.stringify(probe));
if (!probe || typeof probe.ms !== 'number') fail('region probe returned no measurement');

// ------------------------------------------------------------ client
// A faithful headless replica of app.js's `net` object + handlers
// (post-fix: 'peer-left' comes from the relay; startAt receives
// the rebuilt sim's tick count).
class Client {
  constructor(role, room, label) {
    this.role = role;
    this.label = label;
    this.slot = role === 'host' ? 0 : 1;
    this.owner = role === 'host' ? 0 : 2;
    this.room = room;
    this.phase = 'lobby'; // lobby|starting|playing|peer-left|rebuilding|ended
    this.peerPresent = false;
    this.peerGone = false;
    this.peerLeftViaRelay = false;
    this.helloReplied = false;
    this.hellosSeen = 0;
    this.sim = null;
    this.pendingSim = null;
    this.session = null;
    this.recorder = null;
    this.seed = null;
    this.resyncDelay = null;
    this.hashes = new Map();
    this.desyncs = [];
    this.roomFullReceived = 0;
    this.lastResyncMeta = null;
    this.resumeInfo = null;
    this.resyncFrontier = null;
    const url = `${RELAY.replace(/^http/, 'ws')}/room/${room}?region=${REGION}`
      + (role === 'host' ? '&host=1' : '');
    this.client = createRelayClient({
      onOpen: () => this.client.send({ type: 'hello', v: 1, role }),
      onClose: () => {},
      onMessage: (msg) => this.onMessage(msg),
    });
    this.client.connect(url);
  }

  makeSession() {
    const self = this;
    return createSession({
      localSlot: this.slot,
      send: (m) => self.client.send(m),
      applyBundle: (slot, cmds) => {
        const o = slot === 0 ? 0 : 2;
        for (const c of cmds) applyCommand(self.sim, c, o, (oo) => tcIdOf(self.sim, oo));
      },
      tickOnce: () => {
        self.sim.tick();
        return self.sim.state_hash();
      },
      onEvent: (e) => {
        if (e.type === 'desync') self.desyncs.push(e);
      },
      onTurn: (turn, c0, c1, hash) => {
        if (self.recorder) self.recorder.addTurn(turn, c0, c1);
        self.hashes.set(turn, hash);
      },
    });
  }

  beginMatch(t0) {
    this.sim = this.pendingSim;
    this.pendingSim = null;
    this.recorder = createRecorder({
      seed: this.seed, modeTag: MODE_TAG, region: REGION, seats: [0, 2],
    });
    this.session = this.makeSession();
    this.phase = 'playing';
    this.session.start(t0);
  }

  // app.js sendResync(), verbatim payload.
  sendResync() {
    if (!this.session || !this.recorder) return;
    const snapshot = this.recorder.finish({ result: 'ongoing', ticks: this.sim.tick_count() });
    this.resyncFrontier = {
      turn: this.session.state().turn,
      hash: this.hashes.get(this.session.state().turn - 1),
    };
    this.client.send({
      type: 'resync',
      seed: this.seed,
      ticks: this.sim.tick_count(),
      delay: this.session.state().delay,
      yourSlot: this.role === 'host' ? 1 : 0,
      events: snapshot.events,
    });
  }

  // app.js rejoinFromResync(): rebuild + fast-forward, resync-done.
  async rejoinFromResync(msg) {
    this.phase = 'rebuilding';
    this.seed = msg.seed;
    const slot = typeof msg.yourSlot === 'number' ? msg.yourSlot : 1;
    this.role = slot === 0 ? 'host' : 'guest';
    this.slot = slot;
    this.owner = slot === 0 ? 0 : 2;
    this.sim = WasmSim.new_match(msg.seed, MODE_TAG);
    const byTick = new Map();
    for (const e of msg.events || []) byTick.set(e.tick, e);
    const total = msg.ticks || 0;
    for (let t = 0; t < total; t++) {
      const e = byTick.get(t);
      if (e) {
        for (const c of e.s0) applyCommand(this.sim, c, 0, (o) => tcIdOf(this.sim, o));
        if (e.s1) for (const c of e.s1) applyCommand(this.sim, c, 2, (o) => tcIdOf(this.sim, o));
      }
      this.sim.tick();
      if (t % 600 === 599) await new Promise((r) => setTimeout(r, 0));
    }
    this.recorder = createRecorder({
      seed: this.seed, modeTag: MODE_TAG, region: REGION, seats: [0, 2],
    });
    this.recorder.seed(msg.events || []);
    this.session = this.makeSession();
    this.resyncDelay = msg.delay || 4;
    this.lastResyncMeta = {
      ticks: total, events: (msg.events || []).length,
      hashAfterFF: this.sim.state_hash(),
    };
    this.client.send({ type: 'resync-done' });
  }

  onMessage(msg) {
    if (this.session && this.session.receive(msg)) return;
    switch (msg.type) {
      case 'peer-left':
        // Relay v6's originated frame — app.js: peerGone + onPeerGone().
        if (this.phase === 'playing' || this.phase === 'starting'
          || this.phase === 'lobby') {
          this.peerGone = true;
          this.peerLeftViaRelay = true;
          if (this.phase === 'playing') this.phase = 'peer-left';
        }
        break;
      case 'hello': {
        if (!this.helloReplied) {
          this.helloReplied = true;
          this.client.send({ type: 'hello', v: 1, role: this.role });
        }
        const wasGone = this.peerGone;
        this.hellosSeen++;
        this.peerPresent = true;
        this.peerGone = false;
        if (this.phase === 'peer-left' || (this.phase === 'playing' && wasGone)) {
          this.phase = 'playing';
          this.sendResync();
        } else if (this.phase === 'playing' && !wasGone && this.hellosSeen > 2) {
          this.client.send({ type: 'room-full' });
        }
        break;
      }
      case 'room-full':
        this.roomFullReceived++;
        break;
      case 'start': {
        if (this.role === 'guest' && this.phase === 'lobby') {
          this.phase = 'starting';
          const candidate = WasmSim.new_match(msg.seed, MODE_TAG);
          if (candidate.state_hash() !== msg.hash0) {
            this.client.send({ type: 'start-abort', reason: 'build-mismatch' });
            fail('construction hash mismatch');
          }
          this.seed = msg.seed;
          this.pendingSim = candidate;
          this.client.send({ type: 'start-ack' });
        }
        break;
      }
      case 'start-ack': {
        if (this.role === 'host' && this.phase === 'starting' && this.pendingSim) {
          this.client.send({ type: 'go', inMs: 600 });
          this.beginMatch(now() + 600);
        }
        break;
      }
      case 'go': {
        if (this.role === 'guest' && this.pendingSim) {
          this.beginMatch(now() + (msg.inMs || 600));
        }
        break;
      }
      case 'resync': {
        if (this.phase === 'lobby' || this.phase === 'peer-left') {
          this.rejoinFromResync(msg);
        }
        break;
      }
      case 'resync-done': {
        if (this.session && this.phase === 'playing') {
          const st = this.session.state();
          const atTurn = st.turn + st.delay + 2;
          this.client.send({ type: 'resume', atTurn });
          this.session.rebase(atTurn, now() + 700);
        }
        break;
      }
      case 'resume': {
        if (this.session && this.phase === 'rebuilding') {
          this.resumeInfo = { atTurn: msg.atTurn };
          // app.js (post-F2-fix): the rebuilt tick count goes to
          // startAt, which ticks the empty gap turns itself.
          this.session.startAt(msg.atTurn, now() + 700,
            this.resyncDelay || undefined, this.sim.tick_count());
          this.phase = 'playing';
        }
        break;
      }
      default:
        break;
    }
  }

  hardDrop() {
    // The tab is gone: stop pumping, close the socket, send nothing.
    this.phase = 'ended';
    if (this.session) this.session.stop();
    this.client.close();
  }

  pump() {
    if (this.session && (this.phase === 'playing' || this.phase === 'peer-left')) {
      this.session.pump(now());
    }
  }
}

// ------------------------------------------------------------ driver
async function drive(clients, { until, timeoutMs = 30000, label = '' }) {
  const start = Date.now();
  for (;;) {
    for (const c of clients) c.pump();
    if (until()) return true;
    if (Date.now() - start > timeoutMs) {
      console.error(`TIMEOUT waiting: ${label}`);
      return false;
    }
    await sleep(4);
  }
}

function compareHashes(a, b, fromTurn) {
  let compared = 0;
  const mismatches = [];
  for (const [turn, ha] of a.hashes) {
    if (turn < fromTurn) continue;
    const hb = b.hashes.get(turn);
    if (hb === undefined) continue;
    compared++;
    if (ha !== hb) mismatches.push({ turn, a: ha, b: hb });
  }
  return { compared, mismatches };
}

// ------------------------------------------------------------ match
const room = 'rejoinfix' + Math.random().toString(36).slice(2, 8);
console.log('room:', room);
const host = new Client('host', room, 'host');
let guest = new Client('guest', room, 'guest1');

if (!await drive([host, guest], {
  until: () => host.peerPresent && guest.peerPresent,
  timeoutMs: 20000, label: 'presence',
})) fail('presence handshake timed out');

host.phase = 'starting';
host.seed = SEED;
host.pendingSim = WasmSim.new_match(SEED, MODE_TAG);
host.client.send({ type: 'start', seed: SEED, hash0: host.pendingSim.state_hash() });
if (!await drive([host, guest], {
  until: () => host.phase === 'playing' && guest.phase === 'playing',
  timeoutMs: 20000, label: 'match start',
})) fail('match did not start');

// Scripted play, then to the pre-drop turn.
let scripted = false;
if (!await drive([host, guest], {
  until: () => {
    const turn = host.session.state().turn;
    if (!scripted && turn >= 2) {
      scripted = true;
      host.session.localCommand({ t: 'train_vil' });
      guest.session.localCommand({ t: 'train_vil' });
      guest.session.localCommand({ t: 'research_tc', tech: 0 }); // Loom
    }
    return turn >= PRE_DROP_TURN;
  },
  timeoutMs: 60000, label: `play to turn ${PRE_DROP_TURN}`,
})) fail(`did not reach turn ${PRE_DROP_TURN}`);
{
  const pre = compareHashes(host, guest, 0);
  console.log(`pre-drop: compared=${pre.compared} mismatches=${pre.mismatches.length}`);
  if (pre.mismatches.length) fail('pre-drop hashes diverged');
}

// ------------------------------------------------------------ cycles
for (let cycle = 1; cycle <= 2; cycle++) {
  const dropTurn = host.session.state().turn;
  guest.hardDrop();
  console.log(`cycle ${cycle}: guest hard-dropped at host turn ${dropTurn}`);

  // F1: the relay alone must tell the host — no operator, no timer
  // trickery in this client (the stall fallback lives in app.js's
  // UI layer and is not replicated here; the relay frame is the
  // signal under test).
  if (!await drive([host], {
    until: () => host.phase === 'peer-left' && host.peerLeftViaRelay,
    timeoutMs: 15000, label: `cycle ${cycle}: relay peer-left`,
  })) fail(`cycle ${cycle}: host never learned of the drop from the relay (F1)`);
  console.log(`cycle ${cycle}: host in peer-left via relay notice; `
    + `host turn=${host.session.state().turn} waiting=${host.session.state().waiting}`);

  guest = new Client('guest', room, `guest-rejoin-${cycle}`);
  if (!await drive([host, guest], {
    until: () => !!guest.resumeInfo,
    timeoutMs: 30000, label: `cycle ${cycle}: resync + resume`,
  })) fail(`cycle ${cycle}: rejoin did not reach resume (roomFull=${guest.roomFullReceived})`);
  if (guest.roomFullReceived) fail(`cycle ${cycle}: returning peer got room-full (F1)`);
  const frontierHash = host.resyncFrontier.hash;
  console.log(`cycle ${cycle}: resync ticks=${guest.lastResyncMeta.ticks} `
    + `events=${guest.lastResyncMeta.events} atTurn=${guest.resumeInfo.atTurn} `
    + `frontierTurn=${host.resyncFrontier.turn}`);
  if (guest.lastResyncMeta.hashAfterFF !== frontierHash) {
    fail(`cycle ${cycle}: rebuilt hash ${guest.lastResyncMeta.hashAfterFF} `
      + `!= host frontier hash ${frontierHash}`);
  }
  console.log(`cycle ${cycle}: rebuilt hash == host frontier hash (${frontierHash})`);

  // F2: the resumed match must compare clean for WATCH_TURNS turns.
  // The watch must wait for BOTH peers to reach atTurn + WATCH_TURNS:
  // the guest anchors its pacing clock when it RECEIVES the resume
  // frame, which is after the host anchored its own at rebase() time,
  // so at the instant the host alone reaches the target the guest is
  // deterministically 2–3 turns behind (TURN_MS ≈ 29.4 ms vs the
  // relay round-trip). Stopping on the host alone therefore compared
  // only 147–148 turns with no turn missing and no mismatch — a test
  // accounting artefact, proven by instrumented runs on 2026-10-28
  // (bot-loop): after a catch-up wait the same cycles compare the
  // full 150 turns, 0 mismatches, 0 desyncs.
  const atTurn = guest.resumeInfo.atTurn;
  if (!await drive([host, guest], {
    until: () => (host.session.state().turn >= atTurn + WATCH_TURNS
        && guest.session.state().turn >= atTurn + WATCH_TURNS)
      || host.desyncs.length > 0 || guest.desyncs.length > 0,
    timeoutMs: 120000, label: `cycle ${cycle}: post-resume watch`,
  })) fail(`cycle ${cycle}: post-resume watch timed out`);
  const post = compareHashes(host, guest, atTurn);
  console.log(`cycle ${cycle}: post-resume compared=${post.compared} `
    + `mismatches=${post.mismatches.length} desyncs host=${host.desyncs.length} `
    + `guest=${guest.desyncs.length}`);
  if (post.mismatches.length) fail(`cycle ${cycle}: post-resume hash mismatch ${JSON.stringify(post.mismatches[0])}`);
  if (host.desyncs.length || guest.desyncs.length) fail(`cycle ${cycle}: desync after resume (F2)`);
  if (post.compared < WATCH_TURNS) fail(`cycle ${cycle}: only ${post.compared} turns compared (< ${WATCH_TURNS})`);
}

host.client.close();
guest.client.close();
console.log('REJOIN INTEGRATION PASS');
process.exit(0);
