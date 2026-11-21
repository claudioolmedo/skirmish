// Endgame-UI regression pins (bot-loop runs #449/#451).
//
// Two client-only defects on the end-of-match flow, both
// live-confirmed on the real client:
//
// (a) SCORE GRAPH EMPTY — the Defeat screen's Score graph box
// rendered as an empty cream rectangle while the score TABLE
// above it was filled. Mechanism (proven by recorded canvas
// calls in endgameui.smoke.mjs on pristine 43a16e5): the
// timeline samples are per-minute and the decision tick's
// totals were never sampled, so a match ending inside its last
// minute reached drawScoreTimeline with zero or one points —
// and a one-point series strokes a lone moveTo, which paints
// nothing. Fix: renderEndStats appends the final totals once
// (sampleScoreFinal) before drawing, and drawScoreTimeline
// marks every sample with a dot so lone points render too.
//
// (b) RESIGN LEAK — Resign -> end screen -> "Back to menu" ->
// Lobby Browser -> close (x) revealed the resigned match still
// sitting paused underneath (frozen clock, pop 0/5). Mechanism:
// Back to menu called openLobby with matchEnded still standing,
// so openLobby derived lobbyReturnToEnded — the finished-match
// return that belongs ONLY to the Play-online path over a
// finished match (View map -> Menu -> Play online, pinned by
// lobbytrap.smoke.mjs). Fix: the end card's Back-to-menu
// handler clears the match-standing flags (playing /
// matchEnded / paused) BEFORE openLobby derives its return
// flags, durably — a later room-leave inside the lobby
// re-derives them and must not resurrect the dead return.
//
// The behavioral proof is endgameui.smoke.mjs (real app.js +
// real WASM, recorded canvas calls, clicked buttons); these
// pins keep the wiring from silently dying in a future edit,
// in the w1left.test.mjs idiom.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function fnBody(name, nextMarker) {
  const start = appJs.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `${name} exists in app.js`);
  const end = appJs.indexOf(nextMarker, start);
  assert.ok(end > start, `${name} body bounded by ${nextMarker}`);
  return appJs.slice(start, end);
}

test('(a) the end card appends the final totals before drawing the timeline', () => {
  const fin = fnBody('sampleScoreFinal', '/// Fill the end card');
  assert.ok(fin.includes('sim.score_for(localOwner)[4]'),
    'the final sample reads the local total');
  assert.ok(fin.includes('sim.score_for(foeOwner())[4]'),
    'the final sample reads the foe total');
  assert.ok(fin.includes('scoreHistory.push(cur)'),
    'the final sample is appended to the history');
  assert.ok(fin.includes('last[0] !== cur[0] || last[1] !== cur[1]'),
    'no duplicate when the last sample already is the final totals');
  const stats = fnBody('renderEndStats', '/// The timeline itself');
  const callAt = stats.indexOf('sampleScoreFinal();');
  const drawAt = stats.indexOf('drawScoreTimeline();');
  assert.ok(callAt >= 0 && drawAt > callAt,
    'renderEndStats samples the final totals before it draws');
});

test('(a) the timeline marks every sample so a lone point renders', () => {
  const draw = fnBody('drawScoreTimeline', "/// INF-11's \"View map\"");
  assert.ok(draw.includes('ctx.arc('), 'samples are marked with dots');
  assert.ok(draw.includes('ctx.fill()'), 'the markers are filled');
  // The polyline itself is untouched: both series still stroke.
  assert.ok(draw.includes("line(0, '#2c4f9e')"), 'local series still drawn');
  assert.ok(draw.includes("line(1, '#b03a2e')"), 'foe series still drawn');
});

test('(b) Back to menu clears the match-standing flags before openLobby', () => {
  const start = appJs.indexOf("endMenuBtn.addEventListener('click'");
  assert.ok(start >= 0, 'the end-menu handler exists');
  const block = appJs.slice(start, appJs.indexOf('// End-of-match feedback', start));
  const clearPlaying = block.indexOf('playing = false;');
  const clearEnded = block.indexOf('matchEnded = false;');
  const clearPaused = block.indexOf('paused = false;');
  const lobbyAt = block.indexOf('openLobby(null)');
  assert.ok(clearPlaying >= 0, 'Back to menu clears `playing`');
  assert.ok(clearEnded >= 0, 'Back to menu clears `matchEnded`');
  assert.ok(clearPaused >= 0, 'Back to menu clears `paused`');
  assert.ok(lobbyAt > clearEnded && lobbyAt > clearPlaying,
    'the flags clear BEFORE the handler opens the lobby');
  // The clearing sits after the online branch: leaveNetMatch
  // owns the net exit and replaces the match itself.
  const netAt = block.indexOf("leaveNetMatch('abandoned')");
  assert.ok(netAt >= 0 && netAt < clearEnded,
    'the net branch still leaves through leaveNetMatch first');
});

test('(b) the Play-online finished-match return itself is NOT removed', () => {
  // Deleting lobbyReturnToEnded would "fix" the leak by
  // re-trapping the player of lobbytrap.smoke.mjs (#405-#412).
  // The return must still be derived in openLobby and still be
  // honoured by the lobby close handler.
  const open = fnBody('openLobby', 'async function loadRegionsAndPings');
  assert.ok(open.includes('lobbyReturnToEnded = !!(playing && matchEnded && !join);'),
    'openLobby still derives the finished-match return');
  assert.ok(open.includes('lobbyReturnToMatch = !!(playing && paused && !matchEnded && !join);'),
    'openLobby still derives the live-match return');
  assert.ok(appJs.includes('if (lobbyReturnToEnded) {'),
    'the lobby close handler still honours the finished-match return');
  assert.ok(appJs.includes('const canClose = lobbyReturnToMatch || lobbyReturnToEnded'),
    'the close button visibility still counts both returns');
});
