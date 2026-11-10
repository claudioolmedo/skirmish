// Volume slider arrow-key regression pins (bot-loop, 2026-11-10).
//
// Defect (live-confirmed in proofs #240 and #241 on the real
// client, the second with a directed focus test): in the pause
// menu, the volume slider IGNORED ArrowLeft/ArrowRight — with
// explicit focus on the slider, after clicking its thumb, and
// after dragging it. Only dragging changed the value. A keyboard
// player could not adjust the volume at all.
//
// Root cause (code): the app's global keydown handler (the pan /
// hotkey listener) ran `if (e.code.startsWith('Arrow'))
// e.preventDefault();` unconditionally, for every target.
// preventDefault on a keydown kills the native stepping of a
// focused <input type="range"> (and a <select>'s arrow choice,
// and a text field's caret moves) — so the slider never received
// its arrow steps, and no 'input' event ever fired from the keys.
//
// Fix (app.js only; sim/wasm untouched): arrow keys whose target
// is inside a form control (input, textarea, select,
// contenteditable) return from the global handler untouched, so
// the control handles them natively; arrows aimed anywhere else
// keep the game's preventDefault (camera pan must not scroll the
// page). These pins keep the exemption in place, in the right
// order, and keep the slider wired so a native step — which fires
// 'input' — actually changes the volume.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const playIndex = readFileSync(join(here, 'index.html'), 'utf8');

function panHandlerHead() {
  const start = appJs.indexOf('const keys = new Set();');
  assert.ok(start >= 0, 'the pan key set exists in app.js');
  const blanket = appJs.indexOf(
    "if (e.code.startsWith('Arrow')) e.preventDefault();", start);
  assert.ok(blanket > start, 'the blanket arrow preventDefault exists');
  return { head: appJs.slice(start, blanket), blanket };
}

test('arrow keys aimed at a form control are exempted before the blanket preventDefault', () => {
  const { head } = panHandlerHead();
  const exempt = head.indexOf("e.target.closest('input, textarea, select");
  assert.ok(exempt >= 0,
    'the global keydown handler exempts form controls (closest input/textarea/select) before preventDefault');
  const blockStart = head.lastIndexOf("if (e.code.startsWith('Arrow')", exempt);
  assert.ok(blockStart >= 0, 'the exemption is itself scoped to arrow keys');
  const block = head.slice(blockStart, exempt + 160);
  assert.match(block, /return;/, 'the exemption returns, leaving the event untouched');
});

test('arrows aimed at the game still get the preventDefault (camera pan does not scroll the page)', () => {
  const { head, blanket } = panHandlerHead();
  assert.ok(blanket > 0, 'the blanket preventDefault is still present after the exemption');
  assert.ok(head.includes("if (chatInputOpen()) return;"),
    'the chat-input guard still precedes everything');
});

test('the volume control is a native range input, so exempted arrows step it', () => {
  assert.match(playIndex,
    /<input id="audio-volume" type="range"[^>]*>/,
    'index.html carries the volume slider as <input type="range">');
});

test('a native slider step actually changes the volume (input listener wired to setVolume)', () => {
  assert.ok(appJs.includes("audioVolumeInput.addEventListener('input'"),
    'the slider has an input listener');
  assert.ok(appJs.includes('gameAudio.setVolume(Number(audioVolumeInput.value) / 100)'),
    'the input listener feeds the slider value to the audio engine');
});
