// Silent order-drop logging in issue(), engageclient4
// (bot-loop, 2026-11-24; follow-up to engageclient3's code
// evidence, items 1–2).
//
// THE GAP (not a semantics bug — an observability hole):
// issue() had two early returns that dropped an order with
// NO occlog line at all:
//   1. `if (playback) return false;` — any order attempted
//      during replay playback vanished without a trace.
//   2. the net branch's `if (!net.session ||
//      net.phase !== 'playing') return false;` — in an
//      online match whose phase is not exactly 'playing',
//      a right-click produced rclick + pick lines and then
//      nothing, indistinguishable from "order never issued".
// Both now emit {ev:'issue-drop', reason, cmd, ...} through
// occLog before returning false. occLog is inert without
// ?occlog=1, so the drops stay totally silent with the flag
// off, and both points still return false / issue nothing —
// logging only, no order semantics change, sim untouched.
// These pins hold that contract.
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
const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');

function makeLog(search) {
  const env = {
    params: new URLSearchParams(search),
    window: {},
    performance: { now: () => 0 },
  };
  const factory = new Function(
    ...Object.keys(env),
    `${occLogSrc}\nreturn { occLog, occLogEntries, OCCLOG_ON };`,
  );
  const r = factory(...Object.values(env));
  return { occLog: r.occLog, entries: r.occLogEntries, on: r.OCCLOG_ON };
}

// Build the REAL issue() from app.js against a stubbed
// client environment. `calls` records whether anything was
// actually issued (local apply / net localCommand).
function makeIssue(search, over = {}) {
  const log = makeLog(search);
  const calls = { local: [], net: [] };
  const env = {
    playback: null,
    net: null,
    playing: false,
    localRecorder: null,
    sim: { tick_count: () => 0 },
    applyLocalOrder: (cmd) => { calls.local.push(cmd); return true; },
    occLog: log.occLog,
    occAttackWatchStart: () => false,
    ...over,
  };
  const factory = new Function(...Object.keys(env), `${issueSrc}\nreturn { issue };`);
  const { issue } = factory(...Object.values(env));
  return { issue, entries: log.entries, on: log.on, calls };
}

const dropLines = (entries) => entries.filter((e) => e.ev === 'issue-drop');
const issueLines = (entries) => entries.filter((e) => e.ev === 'issue');
const CMD = { t: 'attack', e: 4, tg: 28 };

// ------------------------------------------------------------------
// Wiring pins.

test('wiring: both silent drop points in issue() now log an issue-drop line', () => {
  assert.match(issueSrc, /ev: 'issue-drop', reason: 'playback'/,
    'the playback return logs its drop');
  assert.match(issueSrc, /ev: 'issue-drop', reason: 'net-not-playing'/,
    'the net not-playing return logs its drop');
  assert.match(issueSrc, /ISSUEDROP \(bot-loop engageclient4\)/,
    'the change is marked');
});

// ------------------------------------------------------------------
// Emission under occlog.

test('occlog on: an order during playback is dropped, returns false, and logs reason playback', () => {
  const { issue, entries, calls } = makeIssue('occlog=1', { playback: { id: 1 } });
  assert.equal(issue(CMD), false, 'the refused order still returns false');
  assert.equal(calls.local.length, 0, 'nothing was issued locally');
  const drops = dropLines(entries);
  assert.equal(drops.length, 1, 'exactly one drop line');
  assert.equal(drops[0].reason, 'playback', 'the reason names the playback case');
  assert.deepEqual(drops[0].cmd, CMD, 'the line carries the command attempted');
  assert.equal(issueLines(entries).length, 0,
    'no {ev:issue} verdict line — the drop line replaces the old silence, distinctly');
});

test('occlog on: a net order before phase playing is dropped and logs phase + session state', () => {
  const session = { localCommand: (cmd) => { throw new Error('must not be called'); } };
  const { issue, entries } = makeIssue('occlog=1', {
    net: { session, phase: 'lobby' },
  });
  assert.equal(issue(CMD), false, 'the refused order still returns false');
  const drops = dropLines(entries);
  assert.equal(drops.length, 1, 'exactly one drop line');
  assert.equal(drops[0].reason, 'net-not-playing', 'the reason names the net case');
  assert.equal(drops[0].phase, 'lobby', 'the line carries the actual net phase');
  assert.equal(drops[0].hasSession, true, 'the line says a session existed (phase was the blocker)');
  assert.deepEqual(drops[0].cmd, CMD, 'the line carries the command attempted');
  assert.equal(issueLines(entries).length, 0, 'no {ev:issue} verdict line for the drop');
});

test('occlog on: a net order with no session logs hasSession false', () => {
  const { issue, entries } = makeIssue('occlog=1', {
    net: { session: null, phase: 'playing' },
  });
  assert.equal(issue(CMD), false, 'the refused order still returns false');
  const drops = dropLines(entries);
  assert.equal(drops.length, 1, 'exactly one drop line');
  assert.equal(drops[0].reason, 'net-not-playing', 'same reason — one net drop case');
  assert.equal(drops[0].hasSession, false, 'the line says the missing session was the blocker');
  assert.equal(drops[0].phase, 'playing', 'the phase is still reported verbatim');
});

// ------------------------------------------------------------------
// Silence without the flag.

test('occlog off: both drop points stay totally silent and still return false', () => {
  const pb = makeIssue('', { playback: { id: 1 } });
  assert.equal(pb.on, false, 'staging: the flag is off');
  assert.equal(pb.issue(CMD), false, 'playback drop still returns false');
  assert.equal(pb.entries.length, 0, 'no log footprint for the playback drop');

  const net = makeIssue('', { net: { session: null, phase: 'lobby' } });
  assert.equal(net.issue(CMD), false, 'net drop still returns false');
  assert.equal(net.entries.length, 0, 'no log footprint for the net drop');
});

// ------------------------------------------------------------------
// Undisturbed paths: logging added ONLY at the drops.

test('occlog on: an accepted local order issues normally with no drop line', () => {
  const { issue, entries, calls } = makeIssue('occlog=1');
  assert.equal(issue(CMD), true, 'the local order is accepted as before');
  assert.equal(calls.local.length, 1, 'the order reached the local apply path');
  assert.equal(dropLines(entries).length, 0, 'no drop line on the happy path');
  assert.equal(issueLines(entries).length, 1, 'the usual {ev:issue} verdict line is intact');
});

test('occlog on: a net order in phase playing sends normally with no drop line', () => {
  const sent = [];
  const { issue, entries } = makeIssue('occlog=1', {
    net: { session: { localCommand: (cmd) => { sent.push(cmd); return 0; } }, phase: 'playing' },
  });
  assert.equal(issue(CMD), true, 'the net order is sent as before');
  assert.equal(sent.length, 1, 'the command reached the session');
  assert.equal(dropLines(entries).length, 0, 'no drop line on the happy path');
  assert.equal(issueLines(entries).length, 1, 'the usual net {ev:issue} verdict line is intact');
});
