// Wall-preview endpoint regression pins (bot-loop, 2026-11-11).
//
// Defect (live-directed, runs #272/#273): on a slow Palisade drag
// the preview label read "3 segments · 9 wood" in the last instant
// before release, but the release placed 5 foundations and charged
// 15 wood. Root cause: the placement ghost renders the wall line
// from `mouseWorld`, which was updated ONLY by the canvas
// 'mousemove' listener — the top/bottom bars float over the canvas
// and swallow mousemove for their bands, so a drag ending over a
// bar froze the ghost at the bar's edge. The release handler
// (window 'mouseup') instead computes its endpoint from the
// release event itself (eventWorld), committing cells the player
// never saw previewed.
//
// Fix: `mouseWorld` also tracks in the window-level 'mousemove'
// listener (which sees every move — 'mousemove' is never stopped
// by the pointer-lock router), and the window 'mouseup' handler
// syncs `mouseWorld` to the release point before committing, so
// the previewed endpoint and the committed endpoint are the same
// point by construction.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('mouseWorld tracks at the window level, not only on the canvas', () => {
  const anchor = appJs.indexOf('mouseClientPt = pointerLocked');
  assert.ok(anchor >= 0, 'the window-level mousemove listener exists');
  const start = appJs.lastIndexOf("addEventListener('mousemove', (e) => {", anchor);
  assert.ok(start >= 0, 'the window-level mousemove listener exists');
  const block = appJs.slice(start, appJs.indexOf('});', start));
  assert.ok(block.includes('mouseWorld = eventWorld(e);'),
    'window mousemove refreshes mouseWorld from the event itself');
});

test('the release syncs mouseWorld to the release point before committing', () => {
  const start = appJs.indexOf("window.addEventListener('mouseup', (ev) => {");
  assert.ok(start >= 0, 'the window mouseup handler exists');
  const block = appJs.slice(start, appJs.indexOf('attemptPlaceWallLine', start));
  assert.ok(block.includes('const [wx, wy] = eventWorld(ev);'),
    'the release endpoint still comes from the release event');
  assert.ok(block.includes('mouseWorld = [wx, wy];'),
    'mouseWorld is synced to the release point in the same handler');
  assert.ok(
    block.indexOf('mouseWorld = [wx, wy];') > block.indexOf('const [wx, wy] = eventWorld(ev);'),
    'the sync happens after the endpoint is computed');
});

test('the wall ghost still renders from mouseWorld and the release endpoint', () => {
  assert.ok(appJs.includes('const cells = wallLineCells(wallAnchor[0], wallAnchor[1], cx, cy);'),
    'the ghost expands the line from the mouseWorld cell');
  assert.ok(appJs.includes('attemptPlaceWallLine(wallAnchor[0], wallAnchor[1], ex, ey);'),
    'the release commits the line to the release cell');
});
