// Occluded-outline ORDER degradation, round 6 (bot-loop
// occorder6, 2026-11-19; follow-up to run #430's directed live
// session on the occorder5 build 4da0054).
//
// occorder5 built the instrument: with ?occlog=1 the client
// records every right-click, pick resolution, guard firing and
// issued command into window.__occlog. In run #430 the scenario
// occurred on cue and the attack failed again — but the log
// could not be READ: the automated browser environment cannot
// open DevTools (Ctrl+Shift+J/I do nothing, there is no Inspect
// context menu), so the capture stayed locked behind a reader
// that does not exist in that environment. The instrument was
// fine; the READER was the blocked half.
//
// This round fixes the reader, minimally, in the page itself:
// with ?occlog=1 (and only then) app.js adds a small fixed
// on-page panel that live-updates with the log entries as
// plain text — monospace, scrollable, readable in an ordinary
// screenshot by a mouse-only agent — with Hide (collapse) and
// ✕ (close; window.__occlogShow() reopens) controls so it never
// blocks play, plus Copy / Download buttons that hand over the
// full JSON. Placement is top-right under the resource bar:
// top-left is already the event feed's and the chat log's
// column (ui.css), and the 38vh height cap keeps the panel
// clear of the bottom command panel and the minimap. The
// recording logic is untouched: the panel only reads
// occLogEntries, at the one hook at the end of occLog.
//
// This file pins: (1) the panel wiring in app.js, including
// that the occorder5 recording sites are unchanged; (2) the
// panel exists under the flag and live-updates as entries are
// pushed through the REAL occLog; (3) the Hide / close /
// reopen and Copy controls work; (4) inertness without the
// flag — no panel, no DOM additions, no window hooks; and
// (5) the block still runs with no document at all (the
// occorder5 slicing harness's shape), so the older harnesses
// keep working unchanged.
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

// The whole OCCLOG block — flag, log and panel together — is
// what app.js runs at startup and what occorder5 slices.
const occBlockSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');

// ----------------------------------------------------------------
// A minimal fake DOM: just the surface the panel code uses
// (createElement, body.appendChild, appendChild, textContent,
// style, addEventListener, click). Elements are plain objects;
// findById walks the tree the code actually built.
function makeFakeDocument() {
  function makeEl(tag) {
    return {
      tagName: String(tag).toUpperCase(),
      children: [],
      style: {},
      listeners: {},
      textContent: '',
      id: '',
      type: '',
      scrollTop: 0,
      scrollHeight: 0,
      appendChild(child) { this.children.push(child); return child; },
      addEventListener(type, fn) { (this.listeners[type] ||= []).push(fn); },
      click() { for (const fn of this.listeners.click || []) fn({}); },
    };
  }
  const body = makeEl('body');
  return { createElement: (tag) => makeEl(tag), body };
}

function findById(el, id) {
  if (!el) return null;
  if (el.id === id) return el;
  for (const c of el.children || []) {
    const hit = findById(c, id);
    if (hit) return hit;
  }
  return null;
}

// Build the REAL occ block under a chosen query string and a
// chosen environment. `withDocument:false` reproduces the
// occorder5 harness's shape (no document global at all).
function makeOccBlock(search, { withDocument = true } = {}) {
  const document = withDocument ? makeFakeDocument() : undefined;
  const copied = { text: null };
  const env = {
    params: new URLSearchParams(search),
    window: {},
    performance: { now: () => 0 },
    navigator: { clipboard: { writeText: (t) => { copied.text = t; } } },
  };
  if (withDocument) env.document = document;
  const factory = new Function(
    ...Object.keys(env),
    `${occBlockSrc}\nreturn { occLog, occLogEntries, OCCLOG_ON, win: window, occPanelEnsure, occPanelUpdate };`,
  );
  const r = factory(...Object.values(env));
  return { ...r, entries: r.occLogEntries, on: r.OCCLOG_ON, document, copied };
}

// ------------------------------------------------------------------
// Wiring pins.

test('occorder6 wiring: the panel is built inside the occ block, flag-gated, top-right under the bar', () => {
  assert.match(occBlockSrc, /panel\.id = 'occlog-panel';/,
    'the panel element carries the occlog-panel id a reader can find');
  assert.match(occBlockSrc, /position:fixed;top:calc\(var\(--beta-h, 30px\) \+ 52px\);right:12px;/,
    'the panel is fixed top-right, just under the resource bar (top-left is the event feed / chat column)');
  assert.match(occBlockSrc, /max-height:38vh;/,
    'the height cap keeps the panel clear of the bottom command panel and the minimap');
  assert.match(occBlockSrc, /body\.id = 'occlog-panel-body';/,
    'the entries render into a dedicated text body');
  assert.match(occBlockSrc, /mkBtn\('Copy', 'occlog-copy'\)/,
    'a Copy button hands over the JSON');
  assert.match(occBlockSrc, /mkBtn\('Download', 'occlog-download'\)/,
    'a Download button hands over the JSON as a file');
  assert.match(occBlockSrc, /mkBtn\('Hide', 'occlog-collapse'\)/,
    'a Hide button collapses the panel so it does not block play');
  assert.match(occBlockSrc, /mkBtn\('✕', 'occlog-close'\)/,
    'a ✕ button closes the panel');
  // The flag gates, in source: every entry point checks first.
  assert.match(occBlockSrc, /function occPanelEnsure\(\) \{\s*if \(!OCCLOG_ON\) return null;/,
    'panel creation is gated on the flag before any DOM access');
  assert.match(occBlockSrc, /function occPanelUpdate\(\) \{\s*if \(!OCCLOG_ON\) return;/,
    'panel updates are gated on the flag');
  assert.match(occBlockSrc, /if \(OCCLOG_ON\) occPanelEnsure\(\);/,
    'the startup creation only runs under the flag');
  // The one hook into the recording path — at the END of
  // occLog, after the entry is recorded; the record sites and
  // the recording logic itself are untouched.
  assert.match(occBlockSrc, /occLogEntries\.push\(e\);\s*console\.info\('\[occlog\]', e\);\s*occPanelUpdate\(\);/,
    'occLog records exactly as occorder5 left it, then refreshes the panel');
});

test('occorder6 wiring: the occorder5 recording sites are unchanged', () => {
  assert.match(appJs, /occLog\(\{ ev: 'rclick', screen: eventScreen\(e\), world: \[wx, wy\], shift: e\.shiftKey \}\)/,
    'the canvas right-click record is as occorder5 left it');
  assert.match(appJs, /occLog\(\{ ev: 'rclick-minimap', world: \[w\[0\], w\[1\]\], shift: ev\.shiftKey \}\)/,
    'the minimap right-click record is as occorder5 left it');
  assert.match(appJs, /ev: 'pick', wx, wy, fromCanvas, selection: targets\.slice\(\)/,
    'the pick-resolution record is as occorder5 left it');
  assert.match(appJs, /ev: 'enemy-guard', tgt/,
    'the FIX #5 guard record is as occorder5 left it');
  const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');
  assert.match(issueSrc, /occLog\(\{ ev: 'issue', cmd, ok \}\)/,
    'issue() still records every locally-applied command');
  assert.match(issueSrc, /occLog\(\{ ev: 'issue', cmd, ok: sent, net: true \}\)/,
    'issue() still records net-stamped commands');
});

// ------------------------------------------------------------------
// Behaviour under the flag: the panel exists and live-updates.

test('panel exists under the flag and live-updates as entries are pushed', () => {
  const { occLog, entries, document, win } = makeOccBlock('occlog=1');
  const panel = findById(document.body, 'occlog-panel');
  assert.ok(panel, 'the panel is in the page from startup under ?occlog=1');
  assert.equal(document.body.children.length, 1, 'the panel is the one top-level addition');
  assert.equal(win.__occlogPanel, panel, 'the panel is reachable as window.__occlogPanel');
  assert.equal(typeof win.__occlogShow, 'function', 'the reopen hook is exposed under the flag');
  const title = findById(panel, 'occlog-panel-title');
  const body = findById(panel, 'occlog-panel-body');
  assert.ok(title && body, 'the panel has its title and text body');
  assert.equal(title.textContent, 'occlog · 0');
  assert.equal(body.textContent, '', 'no entries yet, empty body');

  occLog({ ev: 'rclick', screen: [640, 300], world: [7, -1], shift: false });
  assert.equal(title.textContent, 'occlog · 1', 'the count live-updates');
  assert.match(body.textContent, /"ev":"rclick"/, 'the entry text is visible in the page');
  assert.match(body.textContent, /"world":\[7,-1\]/);

  occLog({ ev: 'pick', picked: 7, afterGate: 7, outline: 7 });
  occLog({ ev: 'issue', cmd: { t: 'attack', tg: 7 }, ok: true });
  assert.equal(entries.length, 3);
  assert.equal(title.textContent, 'occlog · 3');
  const lines = body.textContent.split('\n');
  assert.equal(lines.length, 3, 'one visible line per entry');
  assert.deepEqual(lines.map((l) => JSON.parse(l).ev), ['rclick', 'pick', 'issue'],
    'the visible text is the log itself, in order, parseable back');
  assert.equal(JSON.parse(lines[2]).cmd.tg, 7, 'the discriminating field survives into the visible text');
});

test('panel controls: Hide collapses and restores, ✕ closes, __occlogShow reopens, Copy hands over the JSON', () => {
  const { occLog, document, win, copied } = makeOccBlock('occlog=1');
  occLog({ ev: 'rclick', world: [7, -1] });
  const panel = findById(document.body, 'occlog-panel');
  const body = findById(panel, 'occlog-panel-body');

  const collapseBtn = findById(panel, 'occlog-collapse');
  assert.ok(collapseBtn, 'the Hide button exists');
  collapseBtn.click();
  assert.equal(body.style.display, 'none', 'Hide collapses the entries out of the way');
  assert.equal(collapseBtn.textContent, 'Show', 'the button now offers to show them again');
  collapseBtn.click();
  assert.equal(body.style.display, '', 'a second click restores the entries');
  assert.equal(collapseBtn.textContent, 'Hide');

  const copyBtn = findById(panel, 'occlog-copy');
  assert.ok(copyBtn, 'the Copy button exists');
  copyBtn.click();
  assert.deepEqual(JSON.parse(copied.text).map((e) => e.ev), ['rclick'],
    'Copy hands the clipboard the full JSON of the entries');

  const dlBtn = findById(panel, 'occlog-download');
  assert.ok(dlBtn, 'the Download button exists');
  assert.doesNotThrow(() => dlBtn.click(), 'Download never throws into the page');

  const closeBtn = findById(panel, 'occlog-close');
  assert.ok(closeBtn, 'the ✕ button exists');
  closeBtn.click();
  assert.equal(panel.style.display, 'none', '✕ closes the panel so play is unobstructed');
  win.__occlogShow();
  assert.equal(panel.style.display, '', '__occlogShow() brings the panel back');
});

// ------------------------------------------------------------------
// Inertness without the flag — the pin the task demands.

test('inert without the flag: no panel, no DOM additions, no window hooks', () => {
  for (const search of ['', 'seed=7', 'occlog=0', 'occlog=true']) {
    const { occLog, entries, on, document, win, occPanelEnsure } = makeOccBlock(search);
    assert.equal(on, false, `flag off for ?${search || '(empty)'}`);
    assert.equal(document.body.children.length, 0, 'startup added nothing to the page');
    assert.equal(occPanelEnsure(), null, 'panel creation refuses without the flag');
    assert.equal(document.body.children.length, 0, 'even a direct ensure call adds nothing');
    occLog({ ev: 'issue', cmd: { t: 'attack', tg: 7 }, ok: true });
    assert.deepEqual(entries, [], 'nothing is recorded without the flag (occorder5 pin, restated)');
    assert.equal(document.body.children.length, 0, 'recording attempts add nothing either');
    assert.equal(win.__occlogPanel, undefined, 'no panel hook is exposed');
    assert.equal(win.__occlogShow, undefined, 'no reopen hook is exposed');
    assert.equal(findById(document.body, 'occlog-panel'), null, 'no panel exists anywhere in the tree');
  }
});

test('the occ block still runs with no document at all (the occorder5 harness shape)', () => {
  const { occLog, entries } = makeOccBlock('occlog=1', { withDocument: false });
  assert.doesNotThrow(() => occLog({ ev: 'rclick', world: [7, -1] }),
    'occLog records without a document — the panel hook degrades to a no-op');
  assert.equal(entries.length, 1, 'the entry is still recorded for window.__occlog readers');
  assert.equal(entries[0].ev, 'rclick');
});
