// Esc + Keyboard Lock regression pins (bot-loop, 2026-10-28).
//
// Front (3) of Claudio's automatic-improvement queue (design
// presented 2026-10-26; approved for automatic execution
// 2026-10-27): in fullscreen, Escape must serve the GAME, not
// exit fullscreen — navigator.keyboard.lock(["Escape"]) hands
// the key to the page, and one Esc press does exactly one step
// of this stack, in this order:
//   1. cancel an armed placement / targeting state,
//   2. close a transient panel / the chat input,
//   3. deselect.
// Esc NEVER opens the pause menu (Claudio, 2026-10-23 — the
// Menu button is the only door). Leaving fullscreen is by
// holding Esc ~2 s (Chrome warns) or by the fullscreen button.
// Where the Keyboard Lock API does not exist (Safari/Firefox),
// the previous behaviour stands and nothing may throw.
// The mouse twin of step 3: clicking the selected unit's
// portrait in the selection panel deselects it.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function escBlock() {
  const start = appJs.indexOf("if (e.code === 'Escape')");
  assert.ok(start >= 0, 'the Escape branch exists in the keydown handler');
  const end = appJs.indexOf('\n  }\n', start);
  assert.ok(end > start, 'the Escape branch is delimited');
  return appJs.slice(start, end);
}

test('fullscreen entry locks Escape through the Keyboard Lock API, feature-detected', () => {
  assert.ok(
    appJs.includes("kb.lock(['Escape'])"),
    'lockEscapeKey must call navigator.keyboard.lock(["Escape"])');
  assert.ok(
    appJs.includes("typeof kb.lock === 'function'"),
    'the lock call must be feature-detected so Safari/Firefox keep the old behaviour');
  const fsChange = appJs.indexOf('function onFullscreenChange()');
  assert.ok(fsChange >= 0, 'onFullscreenChange exists');
  const body = appJs.slice(fsChange, fsChange + 500);
  assert.ok(body.includes('if (isFullscreen()) lockEscapeKey();'),
    'entering fullscreen locks Escape');
  assert.ok(body.includes('else unlockEscapeKey();'),
    'leaving fullscreen releases the lock');
});

test('the Esc stack runs placement, then transient panel/chat, then deselect', () => {
  const block = escBlock();
  const placement = block.indexOf('if (placement)');
  const buildPage = block.indexOf('buildPage = null');
  const chat = block.indexOf('closeChatInput()');
  const help = block.indexOf("helpOverlay.style.display = 'none'");
  const deselect = block.indexOf('selectUnits(new Set())');
  assert.ok(placement >= 0, 'step 1 cancels placement');
  assert.ok(buildPage > placement, 'armed build page is part of step 1, after placement');
  assert.ok(chat > buildPage, 'step 2 closes the chat input after the armed states');
  assert.ok(help > buildPage, 'step 2 closes the help overlay after the armed states');
  assert.ok(deselect > chat && deselect > help,
    'step 3 deselects only after placement and transient panels are handled');
});

test('Esc never opens the pause menu', () => {
  const block = escBlock();
  assert.ok(!block.includes('setPaused'), 'the Esc branch must not touch pause state');
  assert.ok(!block.includes('pauseMenu'), 'the Esc branch must not touch the pause menu');
});

test('clicking the selection portrait deselects', () => {
  const wire = appJs.indexOf("selPortrait.addEventListener('click'");
  assert.ok(wire >= 0, 'the portrait carries a click listener');
  const body = appJs.slice(wire, wire + 300);
  assert.ok(body.includes('selectUnits(new Set())'),
    'the portrait click clears the selection through the selection helper');
});
