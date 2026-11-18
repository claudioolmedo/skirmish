// Tech Tree overlay leak across match boundaries — regression
// pins (bot-loop runs #418/#419, live on the real client).
//
// Defect: the Technology Tree is a live, non-pausing panel
// (INF-06: "a view, not a panel — never pauses"), toggled only
// by its top-bar button, its ✕ and the Esc stack. Nothing closed
// it when a match ENDED or a new match STARTED:
//  - showMatchEnd() raised the Defeat/Victory card over it and
//    left it display:flex behind the card (it peered out around
//    the smaller end card);
//  - openLobby() (Back to menu) left it flex behind the Lobby
//    Browser the same way;
//  - resetSim() / adoptSim() / adoptLoadedSim() — the three
//    per-match adopters every start path funnels through (Play
//    solo, Restart, online 'go', AI room, rejoin, playback,
//    loaded save) — reset the end card and the score history
//    but never the live panels, so the tree was still open
//    INSIDE the next match at 00:03 (closed by hand, live).
// The Resign path did NOT reproduce live (run #420) only
// because the open tree covers the Menu button — the tester
// had to close the tree before resigning was reachable at
// all. In code the resign exits land in the same showMatchEnd
// / netMatchEnd, so they share the leak and the same fix.
// The sibling live panels — Score (INF-11) and Diplomacy
// (Package K) — share the exact "live panel, never a pause"
// design and the same three setters, so they leaked the same
// way and are closed by the same helper.
//
// Fix: one helper, closeLivePanels(), closing all three live
// panels through their own setters, called at every match
// boundary: showMatchEnd (natural end + local resign sweep),
// netMatchEnd (online end incl. the desync early return),
// openLobby (Back to menu / Play online), and the three
// per-match adopters resetSim / adoptSim / adoptLoadedSim
// (every new-match start). In-match behaviour is untouched:
// the panels still toggle live and never pause.
//
// Why source pins: app.js is the browser shell (WASM + DOM,
// not importable in node); the neighbouring regression suites
// for this exact surface (diploclose.test.mjs, esclock.test.mjs,
// resignvis.test.mjs) pin at the same level — the boundary
// functions' own bodies must contain the close.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

/** Slice one top-level function's body: from its declaration
 * to the next named top-level function (exclusive). */
function fnBody(name, nextName) {
  const start = appJs.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `function ${name} exists in app.js`);
  const end = appJs.indexOf(`function ${nextName}(`, start + 10);
  assert.ok(end > start, `function ${nextName} delimits ${name}`);
  return appJs.slice(start, end);
}

test('closeLivePanels closes all three live panels via their setters', () => {
  const body = fnBody('closeLivePanels', 'resetScoreHistory');
  assert.ok(body.includes('setDiplo(false)'), 'closes Diplomacy via setDiplo(false)');
  assert.ok(body.includes('setScore(false)'), 'closes Score via setScore(false)');
  assert.ok(body.includes('setTechTree(false)'), 'closes Tech Tree via setTechTree(false)');
});

test('match end closes the live panels (natural defeat/victory + local resign sweep)', () => {
  const body = fnBody('showMatchEnd', 'renderEndScore');
  assert.ok(body.includes('closeLivePanels()'),
    'showMatchEnd closes the live panels as the end card rises');
  assert.ok(body.includes("endOverlay.style.display = 'flex'"),
    'the end card still rises (the close does not replace it)');
});

test('online match end closes the live panels, including the desync exit', () => {
  const body = fnBody('netMatchEnd', 'onDesync');
  assert.ok(body.includes('closeLivePanels()'),
    'netMatchEnd closes the live panels (its desync return bypasses showMatchEnd)');
});

test('Back to menu / opening the lobby closes the live panels', () => {
  const start = appJs.indexOf('function openLobby(');
  assert.ok(start >= 0, 'openLobby exists in app.js');
  const end = appJs.indexOf('async function loadRegionsAndPings(', start + 10);
  assert.ok(end > start, 'loadRegionsAndPings delimits openLobby');
  const body = appJs.slice(start, end);
  assert.ok(body.includes('closeLivePanels()'),
    'openLobby closes the live panels so none peers behind the Lobby Browser');
});

test('every new-match adopter closes the live panels', () => {
  for (const [name, next] of [
    ['resetSim', 'fitCanvas'],
    ['adoptSim', 'makeNetSession'],
    ['adoptLoadedSim', 'setText'],
  ]) {
    const body = fnBody(name, next);
    assert.ok(body.includes('closeLivePanels()'),
      `${name} closes the live panels (no panel survives into the fresh match)`);
    assert.ok(body.includes("endOverlay.style.display = 'none'"),
      `${name} still hides the end card beside the panel close`);
  }
});

test('in-match behaviour is untouched: the panels still toggle live', () => {
  // The setters keep their toggle semantics (open = flex +
  // refresh, close = none) — the fix closes at boundaries,
  // it does not disable or pause the panels.
  const tt = fnBody('setTechTree', 'resetScoreHistory');
  assert.ok(tt.includes("techtreeOverlay.style.display = open ? 'flex' : 'none'"),
    'setTechTree still toggles the overlay display');
  assert.ok(tt.includes('if (open) refreshTechTree();'),
    'opening the tree still refreshes it live');
  assert.ok(appJs.includes("setTechTree(techtreeOverlay.style.display === 'none')"),
    'the top-bar Tech Tree button still toggles the panel');
  assert.ok(appJs.includes("setScore(scoreOverlay.style.display === 'none')"),
    'the top-bar Score button still toggles the panel');
  assert.ok(appJs.includes("setDiplo(diploOverlay.style.display === 'none')"),
    'the top-bar Diplomacy button still toggles the panel');
});
