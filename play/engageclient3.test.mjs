// Attack-order follow-up instrumentation, engageclient3
// (bot-loop, 2026-11-24; follow-up to engageclient2 / FIX #25).
//
// THE DEFECT THIS INSTRUMENTS (not fixes): live sessions
// repeatedly show a Militia ordered — by a real right-click,
// through this client — onto an enemy hidden under the Town
// Center roof dealing exactly 1 hit and then going idle
// beside the foe, or arriving and idling without hitting.
// Every sim-harness staging failed to reproduce it, so the
// suspect is the client order path. The pre-existing
// ?occlog=1 instrument proved the click, the pick and the
// issue verdict ({ev:'issue', cmd, ok}) — and then went
// blind: nothing recorded what the attacker did AFTER the
// accepted attack order, so "dropped by the client after
// acceptance", "reached the sim but the unit went idle" and
// "kept attacking but no damage landed" were one shape.
//
// THE INSTRUMENT (client-only, ?occlog=1 only, no sim or
// order-semantics change): issue() registers a watch on
// every ACCEPTED {t:'attack'} (occAttackWatchStart), which
// logs {ev:'attack-watch', phase:'issue', e, tg, ...} at
// once, then occAttackWatchSampleAll emits a sample every
// ~1 s for up to ~10 s — plus immediately on a state-key
// change (idle flip / death / garrison) — carrying the
// tick, both HPs, both positions, the distance, the sim's
// idle_military verdict for the attacker, its order-route
// and path lengths and its garrison flag, ending with
// phase:'end' (reason target-dead / attacker-dead /
// timeout). These pins hold the emission contract: the
// lines exist under occlog, are absent without it, refused
// and non-attack orders register nothing, and issue()
// itself starts the watch on an accepted attack only.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

const occLogSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');
const watchSrc = sliceFn(appJs, '// ATTACKWATCH (bot-loop engageclient3', '\n// SEL-05:');
const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');

function makeWatch(search) {
  const env = {
    params: new URLSearchParams(search),
    window: {},
    performance: { now: () => 0 },
  };
  const factory = new Function(
    ...Object.keys(env),
    `${occLogSrc}\n${watchSrc}\nreturn { occLog, occLogEntries, OCCLOG_ON, occAttackWatchStart, occAttackWatchSampleAll, watches: () => occAttackWatches };`,
  );
  const r = factory(...Object.values(env));
  return { ...r, entries: r.occLogEntries, on: r.OCCLOG_ON };
}

// A client-view snapshot of attacker 4 vs target 28, as
// occAttackRead would build it live; tests mutate the copy.
function snap(over = {}) {
  return {
    tick: 600, aHp: 40, tHp: 45,
    aPos: [0, 0], tPos: [2, 0],
    idle: false, garrisoned: false, routeLen: 0, pathLen: 1,
    ...over,
  };
}
const watchLines = (w) => w.entries.filter((e) => e.ev === 'attack-watch');

// ------------------------------------------------------------------
// Wiring pins.

test('wiring: the attack-watch block and both issue() hooks are in app.js', () => {
  assert.match(watchSrc, /occAttackWatchStart/, 'the watch registrar exists');
  assert.match(watchSrc, /occAttackWatchSampleAll/, 'the sampler exists');
  assert.equal((appJs.match(/occAttackWatchStart\(cmd, (sent|ok)\)/g) || []).length, 2,
    'issue() starts a watch in BOTH branches (net and local)');
  assert.match(issueSrc, /ATTACKWATCH \(engageclient3\)/,
    'the issue() hook is marked');
});

// ------------------------------------------------------------------
// Emission under occlog.

test('occlog on: an accepted attack logs the issue line, then sampled follow-ups', () => {
  const w = makeWatch('occlog=1');
  const s = snap();
  assert.equal(w.occAttackWatchStart({ t: 'attack', e: 4, tg: 28 }, true, 0), true,
    'an accepted attack registers a watch');
  // occAttackRead has no live sim in this harness, so the
  // issue line carries null readings — the pair is named.
  const issue = watchLines(w).find((e) => e.phase === 'issue');
  assert.ok(issue && issue.e === 4 && issue.tg === 28 && issue.ok === true,
    'the issue line names attacker and target');
  const read = () => s;
  assert.equal(w.occAttackWatchSampleAll(0, read), 1, 'the first sample is immediate');
  assert.equal(w.occAttackWatchSampleAll(500, read), 0, 'no sample before the ~1 s cadence');
  assert.equal(w.occAttackWatchSampleAll(1000, read), 1, 'a sample at ~1 s');
  const sample = watchLines(w).find((e) => e.phase === 'sample');
  assert.ok(sample && sample.tHp === 45 && sample.aHp === 40 && sample.dist === 2
    && sample.idle === false && sample.state === 'in-range',
    'the sample carries target HP, attacker HP, distance, idle verdict and state — the discriminating fields');
});

test('occlog on: the 1-hit-then-idle shape is visible — idle flip samples at once, HP frozen', () => {
  const w = makeWatch('occlog=1');
  const s = snap();
  w.occAttackWatchStart({ t: 'attack', e: 4, tg: 28 }, true, 0);
  w.occAttackWatchSampleAll(0, () => s); // attacking, in range
  s.tHp = 40; // exactly one hit landed
  s.idle = true; s.pathLen = 0; // then the unit went idle
  assert.equal(w.occAttackWatchSampleAll(400, () => s), 1,
    'an idle flip emits immediately, ahead of the 1 s cadence (state-key change)');
  const idleLine = watchLines(w).at(-1);
  assert.ok(idleLine.state === 'idle' && idleLine.tHp === 40 && idleLine.idle === true,
    'the log now says, in one line: order accepted, one hit (45->40), attacker idle, target alive');
  assert.equal(w.occAttackWatchSampleAll(10400, () => s), 1, 'the watch closes at the ~10 s cap');
  const end = watchLines(w).at(-1);
  assert.ok(end.phase === 'end' && end.reason === 'timeout' && end.state === 'idle',
    'a still-idle attacker at the cap ends as timeout/idle — not a silent disappearance');
  assert.equal(w.watches().length, 0, 'the finished watch is dropped');
});

test('occlog on: target death ends the watch early with its reason', () => {
  const w = makeWatch('occlog=1');
  const s = snap();
  w.occAttackWatchStart({ t: 'attack', e: 4, tg: 28 }, true, 0);
  w.occAttackWatchSampleAll(0, () => s);
  s.tHp = 0;
  w.occAttackWatchSampleAll(300, () => s);
  const end = watchLines(w).at(-1);
  assert.ok(end.phase === 'end' && end.reason === 'target-dead',
    'a killed target closes the watch as target-dead (the healthy outcome)');
  assert.equal(w.watches().length, 0, 'no samples are emitted after the end');
  assert.equal(w.occAttackWatchSampleAll(2000, () => s), 0, 'the sampler stays silent once closed');
});

test('occlog on: a re-issued attack replaces the attacker\'s watch (one watch per attacker)', () => {
  const w = makeWatch('occlog=1');
  w.occAttackWatchStart({ t: 'attack', e: 4, tg: 28 }, true, 0);
  w.occAttackWatchStart({ t: 'attack', e: 4, tg: 31 }, true, 100);
  assert.equal(w.watches().length, 1, 'the second order replaces the first watch');
  assert.equal(w.watches()[0].tg, 31, 'the live watch follows the newest target');
});

// ------------------------------------------------------------------
// Silence without the flag / without an accepted attack.

test('occlog off: no watch, no lines, no state', () => {
  const w = makeWatch('');
  assert.equal(w.on, false, 'staging: the flag is off');
  assert.equal(w.occAttackWatchStart({ t: 'attack', e: 4, tg: 28 }, true, 0), false,
    'registration is refused without the flag');
  assert.equal(w.watches().length, 0, 'no watch state is kept');
  assert.equal(w.occAttackWatchSampleAll(0, () => snap()), 0, 'the sampler emits nothing');
  assert.equal(w.entries.length, 0, 'the occlog stays empty — zero footprint without the flag');
});

test('occlog on: refused attacks and non-attack orders register nothing', () => {
  const w = makeWatch('occlog=1');
  assert.equal(w.occAttackWatchStart({ t: 'attack', e: 4, tg: 28 }, false, 0), false,
    'an attack the sim refused (issue ok=false) is not watched — the issue line already carries that verdict');
  assert.equal(w.occAttackWatchStart({ t: 'move', e: 4, x: 1, y: 2 }, true, 0), false,
    'a move order is not watched');
  assert.equal(w.occAttackWatchStart({ t: 'attackb', e: 4, b: 9 }, true, 0), false,
    'a building attack is out of this instrument\'s scope (unit target only)');
  assert.equal(w.watches().length, 0, 'nothing was registered');
  assert.equal(watchLines(w).length, 0, 'no attack-watch lines were emitted');
});

// ------------------------------------------------------------------
// issue() integration: the real issue function starts the
// watch on an accepted attack — and only there.

test('issue(): an accepted attack starts a watch; a move does not', () => {
  const w = makeWatch('occlog=1');
  const env = {
    playback: null, net: null, playing: false, localRecorder: null,
    localOwner: 0, sim: { tick_count: () => 0 },
    applyLocalOrder: () => true,
    occLog: w.occLog, occAttackWatchStart: w.occAttackWatchStart,
  };
  const factory = new Function(...Object.keys(env), `${issueSrc}\nreturn { issue };`);
  const { issue } = factory(...Object.values(env));
  assert.equal(issue({ t: 'attack', e: 4, tg: 28 }), true, 'the attack is accepted');
  assert.equal(w.watches().length, 1, 'issue() registered the follow-up watch');
  assert.equal(issue({ t: 'move', e: 4, x: 1, y: 2 }), true, 'the move is accepted');
  assert.equal(w.watches().length, 1, 'the move added no watch (and the attack watch stands)');
  const lines = watchLines(w);
  assert.equal(lines.filter((e) => e.phase === 'issue').length, 1,
    'exactly one attack-watch issue line — for the attack, not the move');
});
