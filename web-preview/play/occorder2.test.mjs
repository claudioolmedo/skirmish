// Occluded-outline ORDER degradation, round 2 (bot-loop
// occorder2, 2026-11-18; follow-up to run #423, where the
// occorder fix 5413df1 FAILED its directed live proof).
//
// Sweep 2 (play/occordersweep2.mjs, the REAL orderAt / entityAt /
// occludedUnitAt / unitAt through the REAL isometric projection,
// record creation through the REAL occlusionFor) measured:
//
//  * Record PRESENT + a friendly unit coincident with the hidden
//    enemy: right-clicks on the outline still attack 100% —
//    entityAt already prefers the occluded pick over the ground
//    pick (H1 does not break the right-click path by itself).
//    LEFT-clicks, however, select the own coincident unit on
//    44% of outline clicks, because the canvas handler runs
//    unitAt (own units, ground radius 1.9) BEFORE the occluded
//    pick — exactly the live fact that left-clicking the red
//    figure selected our own Scout. (Measured, not changed:
//    left-click precedence is selection, not an order.)
//  * Record ABSENT (any cause — see the H3 audit in the handoff:
//    Spearman/Skirmisher vector fallbacks record no drawImage
//    at all) or NO record because the enemy at the TC's east
//    porch is beside the art TC's rect (covered fraction 0.000
//    under the art cover model): 100% of the enemy sprite's
//    clicks degraded to a ground move. entityAt split 51.6%
//    our own coincident Scout (nearest, winning every equal-
//    distance tie as the lower id) / 48.4% null (the sprite's
//    upper half converts to ground 4.23 wu from the unit,
//    outside the 2.2 pick radius). Inside the TC footprint the
//    attempts ran garrison_tc (refused — sim can_garrison_tc:
//    mounted units cannot garrison in a TC) and then move:
//    the H4 fall-through, matching live attempts 1-2 (the Scout
//    walked to the TC tile and stood).
//
// The fix (FIX #2c, in entityAt's ground pick): a right-click
// pick never names our OWN unit while a non-own unit is also
// in pick range — our own unit is never the target of a
// right-click order there, while the non-own unit is the
// actionable reading (attack / convert / inspect). Battering
// Ram (17) and Transport Ship (32) hosts keep their win: they
// ARE right-click targets (garrison / load). Post-fix sweep:
// the coincident scenes go from 100% move to 51.6% attack /
// 48.4% move; the remaining move half is the sprite's upper
// half (ent=null — no ground pick can name anyone there; it
// would need a unit sprite-body pick, a larger change, left
// as a proposal in the handoff note).
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
const CAVALRY_BOX = { x0: -1.4, y0: -3.05, w: 3.5, h: 3.2 };
const ENEMY = [6, -6]; // the TC east porch (footprint edge, NE side)

// Scene: our TC at the origin; our Scout (id 1, the selected /
// ordered unit) standing ON the enemy's tile, as in run #423;
// the enemy (id 7) at the east porch. `record` controls whether
// the INF-12 outline record for the enemy exists.
function makeWorld({ record = false, friendlyPos = ENEMY, extra = null } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 1909);
  put(1, 2, 0, friendlyPos[0], friendlyPos[1], 45);
  put(7, 2, 2, ENEMY[0], ENEMY[1], 45);
  if (extra) put(extra.i, extra.kind, extra.owner, extra.x, extra.y, extra.h);
  const count = extra ? extra.i + 1 : 8;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const issued = [];
  const attempts = [];
  const records = [];
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: () => true,
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
    // Faithful sim verdicts: mounted units cannot garrison in
    // a TC (sim/src/garrison.rs can_garrison_tc), so the
    // garrison attempt is refused and orderAt falls through.
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
    `return { orderAt, entityAt, occludedUnitAt, worldToScreen };`,
  );
  const fns = factory(...Object.values(env));
  if (record) {
    const [esx, esy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    const [tsx, tsy] = fns.worldToScreen(0, 0);
    records.push({
      id: 7,
      rect: { x: esx + CAVALRY_BOX.x0 * camS, y: esy + CAVALRY_BOX.y0 * camS, w: CAVALRY_BOX.w * camS, h: CAVALRY_BOX.h * camS },
      cover: [{ x: tsx - 16 * camS, y: tsy - 10.2 * camS, w: 32 * camS, h: 18.2 * camS }],
      depth: ENEMY[0] + ENEMY[1],
    });
  }
  return { ...fns, issued, attempts, records };
}

// ------------------------------------------------------------------
// Wiring pin (fails on pristine).

test('entityAt tracks a non-own candidate alongside the ground pick (FIX #2c)', () => {
  assert.match(entityAtSrc, /let foeBest = null;/,
    'the ground pick tracks the best non-own candidate');
  assert.match(entityAtSrc, /st\.owners\[best\] === localOwner/,
    'the preference fires only when the ground pick named our own unit');
  assert.match(entityAtSrc, /st\.kinds\[best\] !== KIND_RAM && st\.kinds\[best\] !== 32/,
    'Ram / Transport Ship hosts keep their win (garrison / load targets)');
  assert.match(entityAtSrc, /return foeBest;/,
    'the non-own candidate is returned instead of our own bystander');
});

// ------------------------------------------------------------------
// Behavioural pins (fail on pristine, pass with the fix).

test('coincident friendly, no record: entityAt names the enemy, not our Scout', () => {
  const w = makeWorld({ record: false });
  assert.equal(w.entityAt(ENEMY[0], ENEMY[1], true), 7,
    'a right-click pick on the shared tile names the enemy');
  assert.equal(w.entityAt(ENEMY[0], ENEMY[1], false), 7,
    'the rule lives in the ground pick (minimap picks inherit it)');
});

test('coincident friendly, no record: right-click on the enemy tile issues the attack', () => {
  const w = makeWorld({ record: false });
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'exactly one order: the attack (pristine issued a move after a refused garrison_tc)');
  assert.equal(w.issued[0].tg, 7, 'the attack names the enemy');
  assert.equal(w.issued[0].e, 1, 'the attack comes from our selected Scout');
});

test('coincident friendly, no record: nearby points in pick range of both also attack', () => {
  for (const [x, y] of [[5.5, -5.5], [6.5, -6.2], [5.2, -6.8]]) {
    const w = makeWorld({ record: false });
    w.orderAt(x, y, false, true);
    assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
      `click at (${x},${y}) attacks instead of moving`);
    assert.equal(w.issued[0].tg, 7);
  }
});

// ------------------------------------------------------------------
// Non-regression pins (pass before and after).

test('record present + coincident friendly: the outline pick still wins (unchanged path)', () => {
  const w = makeWorld({ record: true });
  const [sx, sy] = w.worldToScreen(ENEMY[0], ENEMY[1]);
  // Click the centre of the painted outline via the real
  // projection round trip used by the sweep: use the enemy's
  // ground point, which the record also covers in this scene.
  void sx; void sy;
  assert.equal(w.occludedUnitAt(ENEMY[0], ENEMY[1]), 7);
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack']);
  assert.equal(w.issued[0].tg, 7);
});

test('our own unit alone in pick range still names itself (heal / garrison paths)', () => {
  const w = makeWorld({ record: false, friendlyPos: ENEMY, extra: null });
  // Move the enemy out of range by staging a second world: the
  // enemy is at the porch; click our Scout's tile in a world
  // where the enemy stands far away is makeWorld's friendlyPos
  // scene — instead click a point in range of our Scout only:
  // our Scout is coincident with the enemy here, so use a
  // dedicated scene below.
  void w;
  const w2 = makeWorld({ record: false, friendlyPos: [-20, 20] });
  // Enemy still at the porch (6,-6); our Scout far SW. A click
  // on our Scout's own tile names our Scout (no non-own unit
  // in range to prefer).
  assert.equal(w2.entityAt(-20, 20, true), 1,
    'with no non-own unit in range, the ground pick still names our own unit');
});

test('an own Battering Ram in range keeps its win over a coincident enemy', () => {
  const w = makeWorld({
    record: false,
    friendlyPos: [20, 20],
    extra: { i: 9, kind: 17, owner: 0, x: ENEMY[0], y: ENEMY[1], h: 150 },
  });
  // The Ram stands on the enemy's tile: its drawn body is a
  // garrison target, never shadowed by the foe preference.
  assert.equal(w.entityAt(ENEMY[0], ENEMY[1], true), 9,
    'the Ram (body pick / host exemption) still names the Ram');
});
