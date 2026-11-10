// Resign-button visibility regression pins (bot-loop, 2026-11-04).
//
// Defect: VIC-11 wired the pause menu's "Resign match" button for
// local matches (setPaused unhides it when a non-Sandbox local match
// is undecided), but the button element lived INSIDE the #net-status
// container — and that container is only ever unhidden by
// updateNetStatus() for an ONLINE match with a room. In local play
// the ancestor stayed hidden, so the button could never appear: a
// live session on the published combo build (main 79b9ed4) listed
// every pause-menu entry twice (before and after a stale-build
// reload cycle) and found no Resign control at all.
//
// Fix: the button lives in the main pause actions, outside
// #net-status. Its own `hidden` flag is still owned by the same two
// paths as before — setPaused for local matches, updateNetStatus
// (by net phase) online, teardownNet hides it — these pins keep the
// element out of the online-only container and keep both owners.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const indexHtml = readFileSync(join(here, 'index.html'), 'utf8');
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const playCss = readFileSync(join(here, 'play.css'), 'utf8');
const siteCss = readFileSync(join(here, '..', 'site.css'), 'utf8');

test('the Resign button exists once and outside #net-status', () => {
  const occurrences = indexHtml.split('id="btn-resign"').length - 1;
  assert.equal(occurrences, 1, 'exactly one #btn-resign element');
  const btn = indexHtml.indexOf('id="btn-resign"');
  const netStatus = indexHtml.indexOf('id="net-status"');
  assert.ok(netStatus >= 0, '#net-status container still exists');
  assert.ok(btn >= 0 && btn < netStatus,
    'the button precedes #net-status (it is not nested inside it)');
  const netBlock = indexHtml.slice(netStatus, indexHtml.indexOf('</div>', netStatus));
  assert.ok(!netBlock.includes('btn-resign'),
    'the #net-status block no longer contains the Resign button');
});

test('setPaused still owns local Resign visibility (not Sandbox, not decided)', () => {
  const start = appJs.indexOf('function setPaused(v) {');
  assert.ok(start >= 0, 'setPaused exists in app.js');
  const block = appJs.slice(start, appJs.indexOf('// --- M8:', start));
  assert.ok(block.includes('if (v && !net'),
    'the local branch runs when the menu opens offline');
  assert.ok(block.includes("netUi.resignBtn.hidden = !(builtMode !== 'sandbox' && !matchEnded)"),
    'local visibility = non-Sandbox match that has not ended');
});

test('online visibility is still owned by updateNetStatus and teardownNet', () => {
  assert.ok(appJs.includes("netUi.resignBtn.hidden = !(net.phase === 'playing' || net.phase === 'peer-left')"),
    'updateNetStatus shows Resign only while playing / peer-left');
  assert.ok(appJs.includes('if (netUi.resignBtn) netUi.resignBtn.hidden = true;'),
    'teardownNet hides the button');
});

test('the hidden flag actually hides the button (CSS guard)', () => {
  // Why this pin exists (bot-loop runs #245/#246, 2026-11-10): in a
  // Sandbox match the pause menu showed a live-looking "Resign match"
  // button; clicking it did nothing (the sim ignores resign in
  // Sandbox). setPaused had set hidden=true correctly — but site.css
  // `.btn { display: inline-block }` is an author rule and beats the
  // UA stylesheet's [hidden] rule, so the button rendered anyway.
  // play.css already guards [hidden] with !important for other
  // components (.playback-bar/.net-wait/.pw-row, #lock-cursor,
  // #capture-overlay); #btn-resign must be guarded the same way.
  assert.ok(/\.btn\s*\{[^}]*display:\s*inline-block/.test(siteCss),
    'precondition: site.css .btn sets display (the override being guarded against)');
  assert.ok(/#btn-resign\[hidden\]\s*\{[^}]*display:\s*none\s*!important/.test(playCss)
    || /[^{}]*#btn-resign\[hidden\][^{}]*\{[^}]*display:\s*none\s*!important/.test(playCss),
    'play.css must force display:none !important for #btn-resign[hidden]');
});
