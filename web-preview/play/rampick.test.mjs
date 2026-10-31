// Ram body-pick regression pins (bot-loop defect, 2026-10-30;
// live sessions b57befa2 + fa0c2b59: right-clicking a Militia or
// villager onto the Battering Ram — even precisely on its centre —
// never garrisoned anyone; each unit walked up and stood beside
// it, and the Ram itself could not be selected at all when it
// stood overlapped by building footprints).
//
// Root cause: drawBatteringRam draws an A-frame ~3.0 world units
// tall and ~3.1 wide rising straight up the screen from the Ram's
// ground point, but unitAt/entityAt only tested ground-plane
// distance to that point (radii 1.9 / 2.2). Screen-up is the
// (-d, -d) world direction, so a click on the visible body lands
// up to ~4.2 units north-west of the ground point — outside both
// radii — and a bystander unit standing beside the Ram won
// entityAt's nearest test, degrading the order to a ground move.
// Same family as the tree-canopy pick (canopypick.test.mjs).
//
// The fix is a client-only body pick, ramBodyD2: the click is
// decomposed against the Ram's ground point — h = height up the
// drawn sprite, dx = screen-x offset from its column — and a
// click inside the drawn box names the Ram ahead of the ground
// pick in both unitAt (selection) and entityAt (order target).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('the Ram kind and body-box constants exist', () => {
  assert.match(appJs, /const KIND_RAM = 17;/, 'KIND_RAM matches the sim tag 17');
  assert.match(appJs, /const RAM_BODY_H = 3\.0;/, 'body height matches drawBatteringRam (~3.0)');
  assert.match(appJs, /const RAM_BODY_HALF_W = 1\.6;/, 'body half width matches the drawn ~1.55');
});

test('ramBodyD2 decomposes the click against the drawn body', () => {
  assert.match(appJs, /function ramBodyD2\(st, i, wx, wy\)/, 'the body pick exists');
  assert.match(appJs, /const h = -\(dxw \+ dyw\) \/ 2;/, 'h is the screen-up decomposition');
  assert.match(appJs, /const dx = dxw - dyw;/, 'dx is the screen-x decomposition');
  assert.match(
    appJs,
    /if \(h < 0 \|\| h > RAM_BODY_H \|\| Math\.abs\(dx\) > RAM_BODY_HALF_W\) return null;/,
    'the candidate window is exactly the drawn sprite box',
  );
});

test('the body pick runs ahead of the ground pick in both picks', () => {
  const unitBody = appJs.indexOf('if (bodyBest !== null) return bodyBest;');
  assert.ok(unitBody >= 0, 'unitAt returns a body hit');
  const entityBody = appJs.indexOf('if (bodyBest !== null) return bodyBest;', unitBody + 1);
  assert.ok(entityBody > unitBody, 'entityAt also returns a body hit');
});

// The pick geometry, re-implemented from the same formulas the
// pins above lock into app.js, exercised on a Ram at (100, 100).
const RAM_BODY_H = 3.0;
const RAM_BODY_HALF_W = 1.6;
function ramBodyD2(x, y) {
  const dxw = x - 100;
  const dyw = y - 100;
  const h = -(dxw + dyw) / 2;
  const dx = dxw - dyw;
  if (h < 0 || h > RAM_BODY_H || Math.abs(dx) > RAM_BODY_HALF_W) return null;
  return (h - RAM_BODY_H / 2) ** 2 + dx ** 2;
}
const groundDist = (x, y) => Math.hypot(x - 100, y - 100);

test('geometry: body clicks miss both ground radii but name the Ram', () => {
  // Ground point: the ground pick already works (unchanged).
  assert.ok(groundDist(100, 100) < 1.9);
  assert.notEqual(ramBodyD2(100, 100), null, 'the feet are inside the body box too');
  // Body centre (~1.5 up the sprite, straight up-screen): the
  // exact clicks the live sessions saw degraded to ground moves.
  const cx = 100 - 1.5;
  const cy = 100 - 1.5;
  assert.ok(groundDist(cx, cy) > 1.9, 'body centre is outside unitAt radius 1.9');
  assert.notEqual(ramBodyD2(cx, cy), null, 'body centre names the Ram');
  // Roof apex (~2.95 up): outside even entityAt's 2.2.
  const ax = 100 - 2.95;
  const ay = 100 - 2.95;
  assert.ok(groundDist(ax, ay) > 2.2, 'apex is outside entityAt radius 2.2');
  assert.notEqual(ramBodyD2(ax, ay), null, 'apex names the Ram');
  // A bystander beside the Ram (ground distance ~1 from the body
  // centre click) would win the old nearest test; the body pick
  // is checked first, so the Ram wins regardless.
});

test('geometry: clicks the body does not cover stay ground clicks', () => {
  assert.equal(ramBodyD2(100 - 3.5, 100 - 3.5), null, 'above the roof');
  assert.equal(ramBodyD2(100 - 1.5 + 2.5, 100 - 1.5 - 2.5), null, 'beside the body (dx too wide)');
  assert.equal(ramBodyD2(103, 103), null, 'ground south-east of the Ram');
});
