// Occluded-outline ORDER regression pins (FIX #2b; bot-loop
// runs #417–#419, 7+ live confirmations on published 0ce201d).
//
// FIX #2 made the hidden unit left-click selectable through
// its INF-12 outline (proven live, runs #418/#419: 4/4 precise
// left clicks selected the enemy). But the RIGHT-click attack
// on the same red outline failed 7+/7: the ordered Scout /
// Militia walked to the outline point and stood IDLE, the
// enemy's HP never moved, the TC kept draining ~0.5 HP/s.
//
// Diagnosis (code read + this harness, which executes the REAL
// orderAt / entityAt / occludedUnitAt sliced out of app.js):
// orderAt applies a post-pick fog gate —
//
//   if (tgt != null && st.owners[tgt] !== localOwner
//       && !cellVisibleW(st.pos[tgt*2], st.pos[tgt*2+1])) tgt = null;
//
// — to EVERY entityAt result, including a pick that came from
// the occluded-outline record. The outline record is already
// fog-gated at draw time (occlusion.js's fairness gate: no
// current visibility, no record), so the post-pick gate can
// only ever null a pick the player is literally looking at
// whenever the two fog reads disagree (the record is the last
// frame's; the gate re-reads the live cell). A nulled target
// degrades the order silently: the attack block is skipped,
// the own-TC garrison block refuses a military unit, and the
// click lands as a plain ground move to the click point —
// the unit walks to the outline and idles, exactly as lived.
// The left-click path has no such gate: it trusts the record.
//
// These tests stage that scene on the real functions: an
// occluded-outline record for an enemy Scout under the
// player's Town Center, a selected Scout of ours, and a fog
// function whose answer for the enemy's cell is controlled
// per scenario. On pristine code the "record present, gate
// disagrees" scenario issues a ground MOVE instead of the
// attack; the fix exempts outline-sourced picks from the
// post-pick gate (the gate still applies to ground picks).
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

const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');

// ------------------------------------------------------------------
// The staged scene (indices are entity ids, as in the client):
//   0  our Town Center (entity) at (100, 100)
//   1  our Scout Cavalry at (84, 116) — the ordered attacker
//   7  enemy Scout Cavalry at (98, 98) — hidden behind the TC,
//      standing where a TC attacker settles (within melee reach
//      of the TC centre), visible only as its red outline.
// The outline record is exactly what drawOccludedOutlines keeps
// for it: its screen rect ∩ the TC's rect. worldToScreen is the
// identity here (the harness works in screen == world units);
// the click point is the outline's centre.
const TC_POS = [100, 100];
const ENEMY_POS = [98, 98];
const CLICK = [99, 99]; // ground conversion of a click on the outline

function makeWorld({ enemyVisible }) {
  const kinds = [];
  const owners = [];
  const pos = [];
  const hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y;
    hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, TC_POS[0], TC_POS[1], 2400); // KIND_TC = 13
  put(1, 2, 0, 84, 116, 45); // our Scout (kind 2)
  put(7, 2, 2, ENEMY_POS[0], ENEMY_POS[1], 45); // enemy Scout
  const st = { kinds, owners, pos, hp, count: 8, garrisoned: [] };
  const issued = [];
  const outlineRecord = {
    id: 7,
    rect: { x: 90, y: 90, w: 24, h: 30 },
    cover: [{ x: 60, y: 40, w: 110, h: 100 }],
    depth: 50,
  };
  const env = {
    // scene state
    entityState: () => st,
    lastOccludedOutlines: [outlineRecord],
    worldToScreen: (wx, wy) => [wx, wy],
    occludedPickAt,
    // fog: the enemy's cell answer is the scenario's variable;
    // everything else reads visible/explored.
    cellVisibleW: (wx, wy) => (wx === ENEMY_POS[0] && wy === ENEMY_POS[1] ? enemyVisible : true),
    cellExploredW: () => true,
    // picks orderAt consults before/around entityAt
    tcAt: (wx, wy) => (Math.abs(wx - TC_POS[0]) <= 8 && Math.abs(wy - TC_POS[1]) <= 8 ? 0 : null),
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    // selection + order sink
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
    // Faithful to the sim's gates for THIS scene: the attack is
    // accepted; a Scout Cavalry is mounted, and a Town Center
    // admits foot-garrisonable kinds only (garrison.rs's
    // can_garrison), so the own-TC garrison fallthrough refuses
    // it — the click then degrades to the ground move.
    issue: (cmd) => {
      issued.push(cmd);
      return cmd.t !== 'garrison_tc';
    },
    // misc orderAt plumbing
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
    sim: {},
    OBST: [],
    GW: 0,
    worldToCell: () => [0, 0],
    KIND_RAM: 17,
    KIND_TC: 13,
    KIND_SHEEP: 5,
    KIND_RELIC: 21,
    RAM_BODY_H: 3.0,
    RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\nreturn { orderAt, entityAt, occludedUnitAt };`,
  );
  const fns = factory(...Object.values(env));
  return { ...fns, issued };
}

test('staging: the outline record names the enemy at the click point', () => {
  const w = makeWorld({ enemyVisible: true });
  assert.equal(w.occludedUnitAt(CLICK[0], CLICK[1]), 7);
  assert.equal(w.entityAt(CLICK[0], CLICK[1], true), 7,
    'entityAt returns the outline candidate ahead of the ground pick');
});

test('right-click on the outline issues the attack when the fog gate agrees', () => {
  const w = makeWorld({ enemyVisible: true });
  w.orderAt(CLICK[0], CLICK[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'exactly one order: the attack');
  assert.equal(w.issued[0].tg, 7, 'the attack names the hidden enemy');
  assert.equal(w.issued[0].e, 1, 'the attack comes from our selected Scout');
});

test('FIX #2b: an outline-sourced pick is NOT nulled by the post-pick fog gate', () => {
  // The record exists (draw-time fog gate passed — the player
  // sees the red outline), but the post-pick gate's re-read of
  // the enemy's cell disagrees. On pristine code this issued a
  // ground 'move' to the click point — the live failure.
  const w = makeWorld({ enemyVisible: false });
  w.orderAt(CLICK[0], CLICK[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'the outline pick still issues the attack (no silent ground-move degradation)');
  assert.equal(w.issued[0].tg, 7);
});

test('the fog gate still applies to a plain ground pick (no outline record)', () => {
  // Same scene with NO outline record: entityAt's ground pick
  // can still name the enemy (the click converts within its
  // 2.2 radius), and a fog-hidden ground pick must still
  // degrade to a ground move — M7's rule is untouched for
  // picks that did not come through the outline.
  const w2 = makeWorldNoRecord();
  w2.orderAt(CLICK[0], CLICK[1], false, true);
  assert.ok(!w2.issued.some((c) => c.t === 'attack'),
    'a fog-hidden ground pick never becomes an attack');
  assert.ok(w2.issued.some((c) => c.t === 'move'),
    'it degrades to the ground move, as a miss does');
});

function makeWorldNoRecord() {
  // Rebuild the world with an empty outline table by reusing
  // makeWorld's factory through a visible-world and clearing
  // the record list is not possible from outside (the env is
  // captured), so this variant is built by the same factory
  // with lastOccludedOutlines = [].
  const kinds = []; const owners = []; const pos = []; const hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, TC_POS[0], TC_POS[1], 2400);
  put(1, 2, 0, 84, 116, 45);
  put(7, 2, 2, ENEMY_POS[0], ENEMY_POS[1], 45);
  const st = { kinds, owners, pos, hp, count: 8, garrisoned: [] };
  const issued = [];
  const env = {
    entityState: () => st,
    lastOccludedOutlines: [],
    worldToScreen: (wx, wy) => [wx, wy],
    occludedPickAt,
    cellVisibleW: () => false,
    cellExploredW: () => true,
    tcAt: (wx, wy) => (Math.abs(wx - TC_POS[0]) <= 8 && Math.abs(wy - TC_POS[1]) <= 8 ? 0 : null),
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
    issue: (cmd) => { issued.push(cmd); return cmd.t !== 'garrison_tc'; },
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
    sim: {},
    OBST: [],
    GW: 0,
    worldToCell: () => [0, 0],
    KIND_RAM: 17,
    KIND_TC: 13,
    KIND_SHEEP: 5,
    KIND_RELIC: 21,
    RAM_BODY_H: 3.0,
    RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\nreturn { orderAt, entityAt, occludedUnitAt };`,
  );
  return { ...factory(...Object.values(env)), issued };
}

test('wiring: orderAt marks the outline-sourced target before the fog gate', () => {
  const body = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');
  const gate = body.indexOf('cellVisibleW(st.pos[tgt * 2], st.pos[tgt * 2 + 1])) tgt = null;');
  assert.ok(gate >= 0, 'the post-pick fog gate is present');
  const exempt = body.indexOf('occludedUnitAt(wx, wy) === tgt');
  assert.ok(exempt >= 0 && exempt < gate,
    'the outline source of the pick is established before the gate and exempts it');
});
