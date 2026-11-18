// Occluded-outline ORDER degradation, round 3 (bot-loop
// occorder3, 2026-11-18): the generalised unit sprite-body pick.
//
// occorder2's honest bound (its handoff, sweep 2): after FIX #2c,
// the no-record scenes still degraded 48.4% of the enemy sprite's
// clicks to a ground move — the sprite's upper half converts to
// a ground point up to 4.23 wu from the unit, outside entityAt's
// 2.2 ground radius, and no sprite-body pick existed for non-Ram
// units. FIX #3 generalises the Ram-body / tree-canopy pattern:
// spriteBodyD2 decomposes the click against per-kind body boxes
// (UNIT_BODY_BOX — conservative content bounds from the draw
// code) and entityAt consults it on canvas picks, after the Ram
// body and the occluded outline, ahead of the ground pick, with
// FIX #2c's ownership classes (own Transport Ship host keeps its
// win; a non-own unit beats our own bystander) and the draw
// loop's fog gate (a non-own unit is body-pickable only while
// its cell is visible — it is not drawn otherwise).
//
// Sweep 3 (play/occordersweep3.mjs, the real orderAt/entityAt
// through the real projection): whole-sprite attack fraction in
// the no-record scenes 51.6% -> 85.7% (S5/S6/S8/S10) and 79.1%
// (S9); record-present scenes unchanged at 100%. The residual
// is the blit boxes' transparent margin (the Scout dest box is
// [-1.4, 2.1] x top 3.05; the figure's content is [-1.17, 1.85]
// x top 2.81) — clicks there paint nothing in the cached-sprite
// path and still degrade, as the bound pin below documents.
//
// Presentation/order layer only; sim and golden hash untouched.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { occludedPickAt } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

const w2sSrc = sliceFn(appJs, 'function w2s(wx, wy, s, ox, oy) {', '\nfunction s2w');
const s2wSrc = sliceFn(appJs, 'function s2w(sx, sy, s, ox, oy) {', '\nfunction centerCamera');
const worldToCellSrc = sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost');
const heightAtWorldSrc = sliceFn(appJs, 'function heightAtWorld(wx, wy) {', '\nfunction fogAtWorld');
const worldToScreenSrc = sliceFn(appJs, 'const worldToScreen = (wx, wy) => {', '\n};') + '\n};';
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');

const CS = 4, HALF = 60, GW = 30, GH = 30;
const ENEMY = [6, -6]; // the TC east porch, as in occorder2
const FAR = [-17.4, 17.4];

// Scene: our TC at the origin; our Scout (id 1, the selected /
// ordered unit); the enemy (id 7) at the east porch. No outline
// record ever exists here — that is the scene FIX #3 is for.
function makeWorld({ enemyKind = 2, friendlyPos = FAR, extra = null, enemyVisible = true } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 1909);
  put(1, 2, 0, friendlyPos[0], friendlyPos[1], 45);
  put(7, enemyKind, 2, ENEMY[0], ENEMY[1], 45);
  if (extra) put(extra.i, extra.kind, extra.owner, extra.x, extra.y, extra.h);
  const count = extra ? extra.i + 1 : 8;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const issued = [];
  const attempts = [];
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: [],
    occludedPickAt,
    cellVisibleW: (wx, wy) => (wx === ENEMY[0] && wy === ENEMY[1] ? enemyVisible : true),
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
    issue: (cmd) => {
      attempts.push(cmd);
      const ok = cmd.t !== 'garrison_tc' && cmd.t !== 'garrison_u';
      if (ok) issued.push(cmd);
      return ok;
    },
    localOwner: 0,
    foeOwner: () => 2,
    formationApplies: () => false,
    formationChoice: { tag: () => 0 },
    ackClassForKinds: () => null,
    playAck: () => {},
    playAudioCue: () => {},
    flash: () => {},
    t: (k) => k,
    carryKind: new Set(),
    clickMarkers: { add: () => {} },
    firstGarrisonedRelic: () => null,
    sim: { entity_kinds: () => kinds, positions: () => pos },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, occludedUnitAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));
  // The ground point a click at sprite offset (dx, h) from the
  // enemy converts to, through the REAL projection: screen point
  // = the enemy's screen anchor + (dx * camS, -h * camS).
  const clickAt = (dx, h) => {
    const [sx, sy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    return fns.s2w(sx + dx * camS, sy - h * camS);
  };
  return { ...fns, issued, attempts, clickAt };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('the generalised sprite-body pick is wired into entityAt (FIX #3)', () => {
  assert.match(appJs, /const UNIT_BODY_BOX = \{/,
    'the per-kind body-box table exists');
  assert.match(appJs, /function spriteBodyD2\(st, i, wx, wy\) \{/,
    'the body decomposition exists beside ramBodyD2');
  assert.match(entityAtSrc, /spriteBodyD2\(st, i, wx, wy\)/,
    'entityAt consults the body pick');
  assert.match(entityAtSrc, /if \(screenPick\) \{\s*let sBest = null;/,
    'the body pick is canvas-only (screenPick), like the outline pick');
  const boxSrc = sliceFn(appJs, 'const UNIT_BODY_BOX = {', '\n  };');
  assert.ok(!/^\s+1:/m.test(boxSrc), 'no Town Center entry (kind 1 is a building pick)');
  assert.ok(!/^\s+17:/m.test(boxSrc), 'no Battering Ram entry (ramBodyD2 owns kind 17)');
});

test('the body pick runs after the outline pick and ahead of the ground pick', () => {
  const occ = entityAtSrc.indexOf('if (occ !== null) return occ;');
  const body = entityAtSrc.indexOf('if (sBest !== null) return sBest;');
  const ground = entityAtSrc.indexOf('let best = null;');
  assert.ok(occ >= 0 && body > occ, 'the outline pick keeps precedence over the body pick');
  assert.ok(ground > body, 'the body pick runs ahead of the ground pick');
});

// ------------------------------------------------------------------
// Behavioural pins (fail on pristine, pass with the fix).

test('no record, friendly far: a click on the enemy sprite upper body attacks', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(0.35, 2.0); // rider torso/head height
  assert.ok(Math.hypot(wx - ENEMY[0], wy - ENEMY[1]) > 2.2,
    'staging: the ground conversion sits outside the ground-pick radius');
  assert.equal(w.entityAt(wx, wy, true), 7, 'the body pick names the enemy');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack']);
  assert.equal(w.issued[0].tg, 7);
});

test('no record, friendly coincident: the upper-body click still names the enemy, not our Scout', () => {
  const w = makeWorld({ friendlyPos: ENEMY });
  const [wx, wy] = w.clickAt(0.35, 2.0);
  assert.equal(w.entityAt(wx, wy, true), 7,
    'FIX #2c ownership classes carry into the body pick');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack']);
  assert.equal(w.issued[0].tg, 7);
});

test('the generalisation reaches foot kinds: an enemy Militia upper body attacks', () => {
  const w = makeWorld({ enemyKind: 3 });
  const [wx, wy] = w.clickAt(0, 2.0); // above the ground radius, inside the militia box (H 2.6)
  assert.equal(w.entityAt(wx, wy, true), 7);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack']);
  assert.equal(w.issued[0].tg, 7);
});

test('an own Transport Ship body click keeps its host win over a coincident enemy', () => {
  const w = makeWorld({
    friendlyPos: FAR,
    extra: { i: 9, kind: 32, owner: 0, x: ENEMY[0], y: ENEMY[1], h: 200 },
  });
  const [wx, wy] = w.clickAt(0, 2.5); // the transport's mast/sail height, the enemy scout's box too
  assert.equal(w.entityAt(wx, wy, true), 9,
    'the own Transport Ship (load target) is not shadowed by the foe preference');
});

test('our own unit alone: a click on its upper body names it (heal / convert paths)', () => {
  const w = makeWorld({ friendlyPos: FAR });
  // Click 2.0 wu up our own Scout's sprite at FAR (the enemy is
  // at the porch, far out of any box): the class-2 (own) reading.
  const [sx, sy] = w.worldToScreen(FAR[0], FAR[1]);
  const [wx, wy] = w.s2w(sx, sy - 2.0 * 8);
  assert.equal(w.entityAt(wx, wy, true), 1,
    'an own unit with no rival candidate is named by its body');
});

// ------------------------------------------------------------------
// Non-regression / bound pins (pass before and after).

test('minimap picks stay ground picks: the same upper-body click names nothing off-canvas', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(0.35, 2.0);
  assert.equal(w.entityAt(wx, wy, false), null,
    'screenPick=false (minimap) never consults the body pick (SEL-16)');
});

test('fog gate: a body box reaching into fog names nothing', () => {
  const w = makeWorld({ enemyVisible: false });
  const [wx, wy] = w.clickAt(0.35, 2.0);
  assert.equal(w.entityAt(wx, wy, true), null,
    'a non-own unit whose cell is not visible is not body-pickable (it is not drawn)');
});

test('bound: above the figure content top, the click still degrades (transparent dest margin)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(0.35, 2.93); // dest-box top strip; cached figure content ends at 2.81
  assert.equal(w.entityAt(wx, wy, true), null,
    'the conservative content box does not claim the blit margin');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'documented residual: margin clicks still degrade to a ground move');
});

test('an own Battering Ram on the enemy tile still wins by its own body pick', () => {
  const w = makeWorld({
    friendlyPos: FAR,
    extra: { i: 9, kind: 17, owner: 0, x: ENEMY[0], y: ENEMY[1], h: 150 },
  });
  const [wx, wy] = w.clickAt(0, 1.5);
  assert.equal(w.entityAt(wx, wy, true), 9,
    'Ram precedence is untouched (ramBodyD2 returns before the generalised pick)');
});
