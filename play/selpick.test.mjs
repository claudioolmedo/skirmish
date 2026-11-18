// Left-click SELECTION parity (bot-loop selpick, 2026-11-18;
// follow-up to occorder2/occorder3, which fixed the RIGHT-click
// order path and explicitly left selection untouched).
//
// occordersweep2 measured the standing defect: with the INF-12
// record present and a friendly unit coincident with the hidden
// enemy, LEFT-clicks on the painted outline selected our own
// unit on 44.0% of clicks — the canvas handler ran unitAt (own
// units only, ground radius 1.9) BEFORE occludedUnitAt, so the
// occluded pick was only a fallback when unitAt missed. And no
// non-Ram unit could be SELECTED by its upper body at all:
// FIX #3's generalised sprite-body pick lived only in entityAt
// (orders), so a click on a visible unit's torso/head — whose
// ground conversion lands up to ~4.2 wu north-west of the unit,
// outside every ground radius — named nothing (or the Town
// Center whose footprint contains the displaced point).
//
// The fix (FIX #4, client presentation layer only; sim and
// golden hash untouched):
//  * unitAt gains the own-unit sprite-body pick (spriteBodyD2)
//    after the Ram body, ahead of its ground pick — own units
//    are selectable by their visible body (double-click, which
//    keeps using unitAt, inherits exactly this and nothing
//    else: unitAt stays own-only by contract).
//  * a new selectionAt is the plain-click handler's candidate:
//    Ram body (any owner) → occluded outline → sprite body with
//    FIX #2c/#3 ownership classes (own Transport Ship host
//    class 0, non-own class 1, own bystander class 2) and the
//    draw loop's fog gate for non-own units → unitAt's own
//    ground pick. The handler routes an own hit to selectUnits
//    and anyone else's to the information selection
//    (selectAnimal), as the outline path already did.
//  * Nothing else moves: the minimap never selects (its left
//    click jumps the camera), animalAt/tcAt/buildingAt remain
//    the fallbacks when selectionAt names nothing, and the
//    Relic takes no part in the body pick.
//
// The leftClick model below replicates the handler's decision
// verbatim (selectionAt → own? selectUnits : selectAnimal →
// occluded fallback → animalAt → tcAt → clear). On a pristine
// tree selectionAt does not exist and the model falls back to
// the pristine handler (unitAt first, occluded only on a miss),
// so the behavioural pins read as before/after pairs.
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
function trySlice(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  if (a < 0) return null;
  const b = src.indexOf(endMarker, a + startMarker.length);
  return b > a ? src.slice(a, b) : null;
}

const w2sSrc = sliceFn(appJs, 'function w2s(wx, wy, s, ox, oy) {', '\nfunction s2w');
const s2wSrc = sliceFn(appJs, 'function s2w(sx, sy, s, ox, oy) {', '\nfunction centerCamera');
const worldToCellSrc = sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost');
const heightAtWorldSrc = sliceFn(appJs, 'function heightAtWorld(wx, wy) {', '\nfunction fogAtWorld');
const worldToScreenSrc = sliceFn(appJs, 'const worldToScreen = (wx, wy) => {', '\n};') + '\n};';
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const unitAtSrc = sliceFn(appJs, 'function unitAt(wx, wy) {', '\n  // Left-click pick for an animal');
const animalAtSrc = sliceFn(appJs, 'function animalAt(wx, wy) {', '\n  // Occluded-outline pick');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const selectionAtSrc = trySlice(appJs, 'function selectionAt(wx, wy) {', '\n  // Left-click pick for an animal');

const CS = 4, HALF = 60, GW = 30, GH = 30;
const CAVALRY_BOX = { x0: -1.4, y0: -3.05, w: 3.5, h: 3.2 };
const ENEMY = [6, -6]; // the TC east porch, as in occorder2/3
const FAR = [-17.4, 17.4];
const KIND_TC = 1, KIND_RAM = 17, KIND_SHEEP = 5, KIND_RELIC = 21;

// Scene: our TC (id 0, kind 1) at the origin; our Scout (id 1);
// the enemy (id 7) at the east porch. `record` stages the INF-12
// outline record for the enemy (vector cover, as sweep 2).
function makeWorld({ enemyKind = 2, friendlyPos = FAR, extra = null, record = false, enemyVisible = true } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, KIND_TC, 0, 0, 0, 1909);
  put(1, 2, 0, friendlyPos[0], friendlyPos[1], 45);
  put(7, enemyKind, 2, ENEMY[0], ENEMY[1], 45);
  if (extra) put(extra.i, extra.kind, extra.owner, extra.x, extra.y, extra.h);
  const count = extra ? extra.i + 1 : 8;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const records = [];
  const isOwnUnit = (s, i) =>
    s.owners[i] === 0 && s.kinds[i] !== KIND_TC && s.hp[i * 2] >= 1
    && !(s.garrisoned && s.garrisoned[i]);
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    isOwnUnit,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: (wx, wy) => (wx === ENEMY[0] && wy === ENEMY[1] ? enemyVisible : true),
    localOwner: 0,
    sim: { entity_kinds: () => kinds, positions: () => pos },
    tcHalf: () => 8,
    KIND_RAM, KIND_TC, KIND_SHEEP, KIND_RELIC,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${animalAtSrc}\n${occSrc}\n${selectionAtSrc || ''}\n` +
    `return { unitAt, animalAt, occludedUnitAt, tcAt, worldToScreen, ` +
    `selectionAt: typeof selectionAt === 'function' ? selectionAt : null, ` +
    `s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
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
  // The ground point a click at sprite offset (dx, h) from the
  // enemy converts to, through the REAL projection (as sweep 3).
  const clickAt = (dx, h) => {
    const [sx, sy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    return fns.s2w(sx + dx * camS, sy - h * camS);
  };
  const clickAtPos = (px, py, dx, h) => {
    const [sx, sy] = fns.worldToScreen(px, py);
    return fns.s2w(sx + dx * camS, sy - h * camS);
  };
  // The plain-click handler's decision, replicated verbatim.
  // With FIX #4 the candidate is selectionAt; on pristine it is
  // unitAt, with the occluded pick only as a miss fallback.
  const leftClick = (wx, wy, shift = false) => {
    if (fns.selectionAt) {
      const v = fns.selectionAt(wx, wy);
      if (v != null && isOwnUnit(st, v)) return { type: 'units', id: v };
      if (!shift) {
        if (v != null) return { type: 'animal', id: v };
        const occ = fns.occludedUnitAt(wx, wy);
        if (occ != null) return isOwnUnit(st, occ) ? { type: 'units', id: occ } : { type: 'animal', id: occ };
        const a = fns.animalAt(wx, wy);
        if (a != null) return { type: 'animal', id: a };
        const tc = fns.tcAt(wx, wy);
        if (tc != null) return { type: 'tc', id: tc };
        return { type: 'none' };
      }
      return { type: 'none' };
    }
    const v = fns.unitAt(wx, wy);
    if (v != null) return { type: 'units', id: v };
    if (!shift) {
      const occ = fns.occludedUnitAt(wx, wy);
      if (occ != null) return isOwnUnit(st, occ) ? { type: 'units', id: occ } : { type: 'animal', id: occ };
      const a = fns.animalAt(wx, wy);
      if (a != null) return { type: 'animal', id: a };
      const tc = fns.tcAt(wx, wy);
      if (tc != null) return { type: 'tc', id: tc };
      return { type: 'none' };
    }
    return { type: 'none' };
  };
  return { ...fns, leftClick, clickAt, clickAtPos, st };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('selectionAt is wired with entityAt precedence (FIX #4)', () => {
  assert.ok(selectionAtSrc, 'selectionAt exists beside unitAt');
  assert.match(appJs, /const v = selectionAt\(wx, wy\);/,
    'the plain-click handler takes its candidate from selectionAt');
  const ram = selectionAtSrc.indexOf('ramBodyD2(st, i, wx, wy)');
  const occ = selectionAtSrc.indexOf('occludedUnitAt(wx, wy)');
  const body = selectionAtSrc.indexOf('spriteBodyD2(st, i, wx, wy)');
  const ground = selectionAtSrc.indexOf('return unitAt(wx, wy);');
  assert.ok(ram >= 0 && occ > ram, 'Ram body first, occluded outline second');
  assert.ok(body > occ, 'sprite body after the outline');
  assert.ok(ground > body, 'unitAt ground pick last');
  assert.match(selectionAtSrc, /st\.kinds\[i\] === 32 \? 0 : 2\) : 1/,
    'FIX #2c/#3 ownership classes: own Transport host 0, non-own 1, own 2');
  assert.match(selectionAtSrc, /!cellVisibleW\(st\.pos\[i \* 2\]/,
    'the fog gate restated for non-own body candidates');
  assert.match(selectionAtSrc, /st\.kinds\[i\] === KIND_RELIC\) continue;/,
    'the Relic takes no part in the selection body pick');
});

test('unitAt selects own units by their sprite body (FIX #4)', () => {
  assert.match(unitAtSrc, /spriteBodyD2\(st, i, wx, wy\)/,
    'unitAt consults the generalised body pick for own units');
  const ram = unitAtSrc.indexOf('if (bodyBest !== null) return bodyBest;');
  const body = unitAtSrc.indexOf('if (sBest !== null) return sBest;');
  const ground = unitAtSrc.indexOf('let best = null;');
  assert.ok(ram >= 0 && body > ram && ground > body,
    'Ram body, then sprite body, then the ground pick');
});

// ------------------------------------------------------------------
// Behavioural pins (fail on pristine, pass with the fix).

test('record present + coincident friendly: an outline click info-selects the enemy, not our Scout', () => {
  const w = makeWorld({ record: true, friendlyPos: ENEMY });
  const [wx, wy] = w.clickAt(0.35, 0.7); // low on the outline: ground conversion still inside unitAt's 1.9 of the shared tile
  assert.deepEqual(w.leftClick(wx, wy), { type: 'animal', id: 7 },
    'the outline names the enemy ahead of the coincident own ground pick (sweep 2: 44% named our unit)');
});

test('no record, coincident friendly: an upper-body click info-selects the enemy', () => {
  const w = makeWorld({ friendlyPos: ENEMY });
  const [wx, wy] = w.clickAt(0.35, 2.0);
  assert.deepEqual(w.leftClick(wx, wy), { type: 'animal', id: 7 },
    'the sprite-body pick carries FIX #2c classes: non-own beats the coincident own bystander');
});

test('no record, friendly far: a visible enemy upper body is selectable', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(0.35, 2.0);
  assert.deepEqual(w.leftClick(wx, wy), { type: 'animal', id: 7 },
    'pristine named the Town Center (the displaced point sits in its footprint)');
});

test('the generalisation reaches foot kinds: an enemy Militia upper body info-selects', () => {
  const w = makeWorld({ enemyKind: 3 });
  const [wx, wy] = w.clickAt(0, 2.0);
  assert.deepEqual(w.leftClick(wx, wy), { type: 'animal', id: 7 });
});

test('an enemy Battering Ram is info-selectable by its body', () => {
  const w = makeWorld({ enemyKind: 17 });
  const [wx, wy] = w.clickAt(0, 1.5);
  assert.deepEqual(w.leftClick(wx, wy), { type: 'animal', id: 7 },
    'Ram-body precedence is ownership-blind in selectionAt, as in entityAt');
});

test('our own unit is selectable by its upper body', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAtPos(FAR[0], FAR[1], 0, 2.0);
  assert.deepEqual(w.leftClick(wx, wy), { type: 'units', id: 1 },
    'pristine unitAt missed: the conversion sits 2.83 wu from the ground point');
});

test('an own Transport Ship body click keeps its host win over a coincident enemy', () => {
  const w = makeWorld({
    extra: { i: 9, kind: 32, owner: 0, x: ENEMY[0], y: ENEMY[1], h: 200 },
  });
  const [wx, wy] = w.clickAt(0, 2.5); // inside both the transport's and the scout's body boxes
  assert.deepEqual(w.leftClick(wx, wy), { type: 'units', id: 9 },
    'class 0 (own Transport host) is not shadowed by the foe preference');
});

// ------------------------------------------------------------------
// Non-regression pins (pass before and after).

test('our own Battering Ram body click still selects the Ram', () => {
  const w = makeWorld({
    extra: { i: 9, kind: 17, owner: 0, x: ENEMY[0], y: ENEMY[1], h: 150 },
  });
  const [wx, wy] = w.clickAt(0, 1.5);
  assert.deepEqual(w.leftClick(wx, wy), { type: 'units', id: 9 },
    'Ram precedence in unitAt/selectionAt is untouched');
});

test('own ground click still selects our unit; the TC still selects on its own ground', () => {
  const w = makeWorld({});
  assert.deepEqual(w.leftClick(FAR[0], FAR[1]), { type: 'units', id: 1 });
  assert.deepEqual(w.leftClick(2, 2), { type: 'tc', id: 0 },
    'a click on the TC away from any unit body still selects the TC');
});

test('fog gate: an enemy body reaching into fog info-selects nothing', () => {
  const w = makeWorld({ enemyVisible: false });
  const [wx, wy] = w.clickAt(0.35, 1.8); // ground conversion (4.35, -7.98): inside the TC footprint
  assert.deepEqual(w.leftClick(wx, wy), { type: 'tc', id: 0 },
    'the non-own unit is not drawn in fog, so its body names nothing — the TC pick stands');
});

test('double-click keeps the own-only unitAt contract; the minimap never selects', () => {
  const dbl = sliceFn(appJs, "canvas.addEventListener('dblclick'", "\n  // Zoom toward the cursor");
  assert.match(dbl, /const v = unitAt\(wx, wy\);/,
    'double-click still picks through own-only unitAt (same-kind selection)');
  const mm = sliceFn(appJs, "minimapCanvas.addEventListener('pointerdown'", "minimapCanvas.addEventListener('pointermove'");
  assert.ok(!mm.includes('selectionAt') && !mm.includes('unitAt'),
    'the minimap left click only jumps the camera — no selection pick (SEL-16)');
});
