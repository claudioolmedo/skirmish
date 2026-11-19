// Occluded-outline ORDER degradation, round 4 (bot-loop
// occorder4, 2026-11-18; follow-up to run #427, where the full
// FIX #2/#2b/#2c/#3 + selpick stack FAILED its directed live
// proof on build 3d40245).
//
// Run #427's exact scene, tested three ways in the real client:
// an enemy Scout Cavalry (HP 45/45) stands INSIDE our Town
// Center's footprint, under the east porch roof, visible only
// as its red INF-12 outline, draining the TC. Left-click now
// SELECTS the enemy (selpick holds). But every right-click
// attack attempt failed:
//   1. own Scout, Aggressive, right-click on the outline:
//      walked to the enemy and stopped IDLE beside it;
//   2. own Militia, Aggressive, right-click on the outline:
//      GARRISONED into the TC instead ("Garrisoned." toast);
//   3. Attack Move past the point: walked past without
//      engaging (a ground order by construction — its
//      engagement is the sim's auto-acquisition, not this
//      layer; see the handoff note).
//
// Static trace of the order path on a32b999 (orderAt is
// byte-identical since cfda53c; the stanceatk probe already
// proved a true Attack order engages in any stance):
//
//  * When a click resolves to the enemy, the attack branch
//    fires and returns — the path is clean THERE. The live
//    failures are the clicks that do NOT resolve, and what
//    orderAt does with them next: it reinterprets the SAME
//    click from its raw ground point, which — the enemy
//    standing inside the footprint — lies inside our own TC,
//    so the garrison block (tcT computed from the click point
//    alone, never asking what the click was ON) swallows it:
//    accepted for a foot Militia (can_garrison_tc: foot kinds
//    only) -> attempt 2; refused for a mounted Scout -> the
//    ground-move tail -> attempt 1 (the Scout walks to the
//    displaced ground point, up to ~4.2 wu NW of the enemy,
//    and idles; auto-acquisition is LoS-bounded and the move
//    order itself suppresses it en route).
//  * Two resolution gaps feed the swallow. (a) The outline
//    pick names only rect ∩ cover — the painted outline. The
//    VISIBLE part of the same sprite (beside/above the
//    occluder's rect) fell to the static body pick, whose
//    per-kind heights are the CACHED sprites' content bounds
//    (Scout 2.81): a taller draw path's pixels (the DIR
//    scout's attack pose reaches h ≈ 3.55–3.9 — and the live
//    attacker is always in its attack pose) named nothing.
//    occorder3's honesty bound flagged exactly this slice.
//    (b) Even when the attack branch IS reached, a refusal at
//    issue time (any unit whose attack the sim declines)
//    fell through to the same garrison/move reinterpretation
//    instead of ending the click.
//
// The fix (FIX #5, client order layer only):
//  a. entityAt gains the record-rect pick (occludedRectUnitAt):
//     a unit that HAS an INF-12 record is named by a click
//     anywhere on its recorded rect — the measured sprite as
//     drawn this frame, pose included — not only on the
//     outline sliver. It runs after the outline pick and
//     ahead of the static body pick, canvas-only like every
//     record pick, with the outline pick's revalidation.
//  b. orderAt gains the enemy guard: once a click has
//     resolved to an enemy unit (tgt names the foe's unit)
//     and the attack block did not return (every attack
//     refused at issue time), the click ENDS — it is never
//     reinterpreted as garrison / drop-off / ground move.
//     The branches that legitimately reuse an enemy-named
//     click (Monk convert, relic pickup, the building
//     readings) all run above the guard.
//
// Presentation/order layer only; sim and golden hash untouched.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { occludedPickAt, occlusionFor } from './occlusion.js';

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
const ENEMY = [7, -1]; // inside the TC footprint, NE edge (the east porch)
// The enemy sprite's rect as the DIR attack pose draws it:
// the cavalry dest box widened upward to the pose's reach
// (top 3.9 wu against the cached figure's 3.05 dest / 2.81
// content). x0/w match the cavalry dest box.
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 };
const DEST_RECT = { x0: -1.4, y0: -3.05, w: 3.5, h: 3.2 };

// Scene: our TC at the origin; our selected unit (id 1); the
// enemy Scout (id 7) at the east porch. `rectModel` chooses
// the staged INF-12 record's sprite rect; the record is built
// through the REAL occlusionFor against the TC art's rect
// (17.92 wu wide, base planted 4.4 wu toward the front edge —
// drawTC's own contract), exactly as drawOccludedOutlines
// builds it. `refuseAttack` models an issue-time refusal of
// the attack order (the sim's can_attack gate declining at
// apply time); a Monk's attack is always refused (its spec
// is NO_ATTACKS — the convert branch is its right-click).
function makeWorld({ selKind = 2, selPos = [-17.4, 17.4], rectModel = 'tall', withRecord = true, refuseAttack = false, enemyHp = 45 } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 2092);
  put(1, selKind, 0, selPos[0], selPos[1], 45);
  put(7, 2, 2, ENEMY[0], ENEMY[1], enemyHp);
  const count = 8;
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
    // Faithful sim verdicts for this scene: attack accepted
    // unless the scenario refuses it (or the attacker is a
    // Monk — NO_ATTACKS); garrison_tc admits foot kinds only
    // (Militia 3 / Villager 0 — the Scout is mounted and the
    // sim refuses it, can_garrison_tc); everything else the
    // scene can reach is accepted.
    issue: (cmd) => {
      attempts.push(cmd);
      let ok = true;
      if (cmd.t === 'attack') ok = !refuseAttack && selKind !== 20;
      if (cmd.t === 'garrison_tc' || cmd.t === 'garrison_u') ok = selKind === 3 || selKind === 0;
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
  if (withRecord) {
    const [esx, esy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    const [tsx, tsy] = fns.worldToScreen(0, 0);
    const B = rectModel === 'tall' ? TALL_RECT : DEST_RECT;
    const rect = { x: esx + B.x0 * camS, y: esy + B.y0 * camS, w: B.w * camS, h: B.h * camS };
    const cover = { x: tsx - 8.96 * camS, y: tsy - 13.52 * camS, w: 17.92 * camS, h: 17.92 * camS };
    const res = occlusionFor(
      { rect, depth: ENEMY[0] + ENEMY[1], fogVisible: true },
      [{ rect: cover, depth: 8 }]);
    assert.ok(res.occluded, 'staging: the enemy is occluded by the TC art (a record exists)');
    records.push({ id: 7, rect, cover: res.cover.map((o) => o.rect), depth: ENEMY[0] + ENEMY[1] });
  }
  // The ground point a click at sprite offset (dx, h) from the
  // enemy converts to, through the REAL projection (as in
  // occorder3): screen point = the enemy's screen anchor +
  // (dx * camS, -h * camS), converted back with s2w.
  const clickAt = (dx, h) => {
    const [sx, sy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    return fns.s2w(sx + dx * camS, sy - h * camS);
  };
  return { ...fns, issued, attempts, records, clickAt };
}

// The tall-slice click: on the enemy figure's upper body as
// the DIR attack pose paints it (h = 3.3, above the cached
// content top 2.81), at dx = 1.5 — screen-east of the TC art's
// rect, so the click is on the VISIBLE part of the sprite,
// outside the painted outline's rect ∩ cover region.
const TALL_CLICK = [1.5, 3.3];

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('the record-rect pick is wired into entityAt (FIX #5a)', () => {
  assert.match(appJs, /function occludedRectUnitAt\(wx, wy\) \{/,
    'the record-rect pick exists beside occludedUnitAt');
  assert.match(entityAtSrc, /occludedRectUnitAt\(wx, wy\)/,
    'entityAt consults the record-rect pick');
  const occ = entityAtSrc.indexOf('if (occ !== null) return occ;');
  const rect = entityAtSrc.indexOf('if (occRect !== null) return occRect;');
  const body = entityAtSrc.indexOf('if (sBest !== null) return sBest;');
  assert.ok(occ >= 0 && rect > occ, 'the record-rect pick runs after the outline pick');
  assert.ok(body > rect, 'the record-rect pick runs ahead of the static body pick');
});

test('the enemy guard is wired into orderAt ahead of the garrison block (FIX #5b)', () => {
  // occorder5: the guard now records its firing through the
  // typeof-guarded occLog before returning — the pin's shape
  // follows it; the guard's position and effect are unchanged.
  assert.match(orderAtSrc, /if \(tgt != null && st\.owners\[tgt\] === foeOwner\(\)\) \{\s*if \(typeof occLog === 'function'\) occLog\(\{ ev: 'enemy-guard', tgt \}\);\s*return;\s*\}/,
    'an enemy-named target ends the click instead of falling through');
  const guard = orderAtSrc.indexOf('st.owners[tgt] === foeOwner()) {');
  const convert = orderAtSrc.indexOf("t: 'convert'");
  const garrison = orderAtSrc.indexOf("t: 'garrison_tc'");
  assert.ok(convert >= 0 && guard > convert,
    'the guard runs after the Monk convert block (its legitimate reuse of an enemy click)');
  assert.ok(garrison >= 0 && guard < garrison,
    'the guard runs before the TC garrison block it exists to disarm');
});

// ------------------------------------------------------------------
// Behavioural pins (fail on pristine, pass with the fix).

test('tall pose, visible part: entityAt names the enemy through its recorded rect', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(...TALL_CLICK);
  assert.ok(Math.hypot(wx - ENEMY[0], wy - ENEMY[1]) > 2.2,
    'staging: the ground conversion sits outside the ground-pick radius');
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: the click is outside the painted outline (rect ∩ cover) — the outline pick misses it');
  assert.equal(w.entityAt(wx, wy, true), 7,
    'the record-rect pick names the enemy (pristine named nothing: body box top 2.81 < click height 3.3)');
});

test('tall pose, visible part: the right-click issues the attack, not a move', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(...TALL_CLICK);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'exactly one order: the attack (pristine issued a ground move to the displaced point)');
  assert.equal(w.issued[0].tg, 7, 'the attack names the hidden enemy');
  assert.equal(w.issued[0].e, 1, 'the attack comes from our selected Scout');
});

test('attack refused at issue time: a Militia no longer garrisons into the TC', () => {
  // The click is the enemy's own ground point — inside the
  // outline record on both versions, so tgt names the enemy
  // either way; the scenario's variable is the sim declining
  // the attack at issue time. Pristine fell through to the
  // own-TC garrison block and the Militia garrisoned (run
  // #427, attempt 2). With the guard the click ends: the
  // attack was attempted, nothing else is issued.
  const w = makeWorld({ selKind: 3, rectModel: 'dest', refuseAttack: true });
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.ok(w.attempts.some((c) => c.t === 'attack' && c.tg === 7),
    'staging: the attack on the enemy was attempted');
  assert.deepEqual(w.issued, [],
    'no order is issued — in particular no garrison_tc (pristine issued exactly that)');
});

test('attack refused at issue time: a Scout no longer degrades to a ground move', () => {
  // Run #427, attempt 1's shape with the refusal made
  // explicit: pristine attempted the refused garrison (a
  // mounted unit cannot garrison in a TC) and landed on the
  // ground-move tail — the Scout walked to the click point
  // and idled. With the guard the click ends.
  const w = makeWorld({ selKind: 2, rectModel: 'dest', refuseAttack: true });
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.ok(w.attempts.some((c) => c.t === 'attack' && c.tg === 7),
    'staging: the attack on the enemy was attempted');
  assert.deepEqual(w.issued, [],
    'no order is issued — in particular no move (pristine issued exactly that)');
});

// ------------------------------------------------------------------
// Non-regression pins (pass before and after).

test('record-rect pick stays canvas-only: a minimap pick at the same point names nothing', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(...TALL_CLICK);
  assert.equal(w.entityAt(wx, wy, false), null,
    'the minimap draws no sprites and keeps no records — its pick stays the plain ground pick (SEL-16)');
});

test('a dead unit\u2019s record names nothing (revalidation)', () => {
  const w = makeWorld({ enemyHp: 0 });
  const [wx, wy] = w.clickAt(...TALL_CLICK);
  assert.equal(w.entityAt(wx, wy, true), null,
    'the record is re-validated against the live state, like the outline pick');
});

test('the outline click itself still attacks (unchanged path)', () => {
  const w = makeWorld({ rectModel: 'dest' });
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack']);
  assert.equal(w.issued[0].tg, 7);
});

test('a click on empty TC footprint still garrisons a foot unit (no enemy named)', () => {
  // The guard fires only when the click RESOLVED to an enemy.
  // A click deep inside our own TC, away from any unit, keeps
  // its DE meaning: the selected Militia garrisons.
  const w = makeWorld({ selKind: 3, withRecord: false });
  w.orderAt(-4, 4, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['garrison_tc'],
    'the ordinary garrison gesture is untouched');
});

test('a Monk\u2019s convert still reuses an enemy-named click (guard runs after it)', () => {
  // The Monk cannot attack (NO_ATTACKS), so its attack attempt
  // is refused on both versions and the click must still
  // reach the convert block below the attack block — and
  // above the guard.
  const w = makeWorld({ selKind: 20, rectModel: 'dest' });
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['convert'],
    'the monk converts the enemy instead of attacking or garrisoning');
  assert.equal(w.issued[0].tg, 7);
});
