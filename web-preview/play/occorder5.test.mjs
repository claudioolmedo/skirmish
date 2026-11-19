// Occluded-outline ORDER degradation, round 5 (bot-loop
// occorder5, 2026-11-18; follow-up to run #428 session 2,
// where the directed live watch on the PUBLISHED FIX #5 build
// 2b3f113 failed again: enemy Scout Cavalry outline-only under
// the Town Center roof, left-click SELECTED it (45/45), own
// Scout (Aggressive) right-clicked onto the outline — walked
// to the enemy's exact point and stood IDLE; the enemy's HP
// never dropped; when the enemy later fled, our Scout did not
// pursue).
//
// State of the evidence entering this round (handoffs
// occorder/2/3/4, stanceatk, amoveprobe): every staged path
// engages — the client's order path whenever the click RESOLVES
// to the enemy (occorder4: 100% attack over the whole sprite in
// every record-present porch scene, post-FIX #5), and the sim
// whenever an explicit Attack arrives (stanceatk: identical
// engagement in all stances; amoveprobe: footprint targets
// engage, and its one real find — five kinds missing from
// auto-acquisition — does not cover a Scout with an explicit
// order). What no staging has ever captured is the LIVE click
// itself: which pixel, what entityAt returned, what command
// left the client. occorder4's handoff asked for exactly that
// instrument. This round builds it:
//
//   ?occlog=1 — a permanent, inert-unless-flagged debug log in
//   app.js. It records, per canvas right-click: the screen
//   pixel and world point (ev 'rclick'), orderAt's pick
//   resolution (ev 'pick': entityAt's result before and after
//   the fog gate, the outline pick, the record-rect pick, the
//   record ids present that frame, the picked unit's kind /
//   owner / fog verdict), the FIX #5 guard firing
//   (ev 'enemy-guard'), and every command issue() emits with
//   the sim's verdict (ev 'issue'). window.__occlog holds the
//   entries for the session to dump.
//
// This file pins: (1) the instrumentation wiring; (2) the
// task's headline behaviour — a right-click resolving to an
// occluded enemy emits an Attack command ON THAT ENTITY (not
// move / garrison / attack-move), on both the outline region
// and the record-rect-only region, with the occLog entries to
// prove the stages; (3) occLog's inertness without the flag;
// (4) two measured BOUNDS, asserted as current behaviour so a
// future change must move them consciously: the fog gate
// nulling a record-rect-named enemy whose cell reads
// fog-hidden (a state the real draw path cannot produce —
// records and the FOG snapshot are written by the same draw
// pass, so a record exists only for a cell the same snapshot
// calls visible), and the frontmost-record doctrine naming an
// OWN unit whose outline record overlaps the enemy's at the
// clicked pixel (occlusion.js's documented pick rule).
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
const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');
const occLogSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');

// Build the REAL occLog from its slice, under a chosen query
// string. Returns { occLog, entries, on }.
function makeOccLog(search) {
  const env = {
    params: new URLSearchParams(search),
    window: {},
    performance: { now: () => 0 },
  };
  const factory = new Function(
    ...Object.keys(env),
    `${occLogSrc}\nreturn { occLog, occLogEntries, OCCLOG_ON, win: window };`,
  );
  const r = factory(...Object.values(env));
  assert.equal(r.win.__occlog, r.occLogEntries,
    'window.__occlog is the live entries array a session can dump');
  return { occLog: r.occLog, entries: r.occLogEntries, on: r.OCCLOG_ON };
}

const CS = 4, HALF = 60, GW = 30, GH = 30;
const ENEMY = [7, -1]; // inside the TC footprint, NE edge (the east porch)
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 };

// Run #428's scene, occorder4's staging: our TC at the origin,
// our Scout (id 1) selected, the enemy Scout (id 7) at the east
// porch with a real INF-12 record built through the REAL
// occlusionFor against the TC art's rect. `fogVisible=false`
// models the fog gate reading the enemy's cell as not
// currently visible while the record stands (see the bound
// pin below for why the real draw path cannot produce it).
// issue() is the REAL one, over a stub applyLocalOrder whose
// verdicts are faithful to the sim for this scene (attack
// accepted; garrison_tc refuses a mounted Scout).
function makeWorld({ fogVisible = true, withVillager = false } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 2092);
  put(1, 2, 0, -17.4, 17.4, 45);
  put(7, 2, 2, ENEMY[0], ENEMY[1], 45);
  if (withVillager) put(3, 0, 0, 7.5, -0.5, 50); // our Villager beside the enemy, slightly toward the viewer
  const count = 8;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const issued = [];
  const records = [];
  const { occLog, entries } = makeOccLog('occlog=1');
  const applyLocalOrder = (cmd) => {
    let ok = true;
    if (cmd.t === 'garrison_tc' || cmd.t === 'garrison_u') ok = false; // mounted Scout
    if (ok) issued.push(cmd);
    return ok;
  };
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: () => fogVisible,
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
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
    sim: { entity_kinds: () => kinds, positions: () => pos, tick_count: () => 0 },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
    // issue()'s own environment (the real issue, sliced):
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder,
    occLog,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, occludedUnitAt, occludedRectUnitAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));
  const [esx, esy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const rect = {
    x: esx + TALL_RECT.x0 * camS, y: esy + TALL_RECT.y0 * camS,
    w: TALL_RECT.w * camS, h: TALL_RECT.h * camS,
  };
  const cover = { x: tsx - 8.96 * camS, y: tsy - 13.52 * camS, w: 17.92 * camS, h: 17.92 * camS };
  const res = occlusionFor(
    { rect, depth: ENEMY[0] + ENEMY[1], fogVisible: true },
    [{ rect: cover, depth: 8 }]);
  assert.ok(res.occluded, 'staging: the enemy is occluded by the TC art (a record exists)');
  records.push({ id: 7, rect, cover: res.cover.map((o) => o.rect), depth: ENEMY[0] + ENEMY[1] });
  let sharedPixelWorld = null;
  if (withVillager) {
    const [vsx, vsy] = fns.worldToScreen(7.5, -0.5);
    const vrect = { x: vsx - 0.9 * camS, y: vsy - 2.6 * camS, w: 1.8 * camS, h: 2.75 * camS };
    const vres = occlusionFor(
      { rect: vrect, depth: 7.5 + -0.5, fogVisible: true },
      [{ rect: cover, depth: 8 }]);
    assert.ok(vres.occluded, 'staging: the villager is occluded too (it has a record)');
    records.push({ id: 3, rect: vrect, cover: vres.cover.map((o) => o.rect), depth: 7 });
    // A screen pixel inside BOTH records' rect ∩ cover (the
    // enemy anchor's screen point, nudged toward the villager
    // and up the figure): the overlap the bound pin probes.
    const px = esx + 4, py = esy - 8;
    for (const r of records) {
      const hit = r.cover.some((c) => {
        const ix = Math.max(r.rect.x, c.x), iy = Math.max(r.rect.y, c.y);
        const ix2 = Math.min(r.rect.x + r.rect.w, c.x + c.w);
        const iy2 = Math.min(r.rect.y + r.rect.h, c.y + c.h);
        return px >= ix && px < ix2 && py >= iy && py < iy2;
      });
      assert.ok(hit, `staging: the shared pixel is inside record ${r.id}'s outline`);
    }
    sharedPixelWorld = fns.s2w(px, py);
  }
  const clickAt = (dx, h) => {
    const [sx, sy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    return fns.s2w(sx + dx * camS, sy - h * camS);
  };
  return { ...fns, issued, records, clickAt, occEntries: entries, sharedPixelWorld };
}

// ------------------------------------------------------------------
// Wiring pins.

test('occlog wiring: the flag, the log, and the call sites exist', () => {
  assert.match(appJs, /const OCCLOG_ON = params\.get\('occlog'\) === '1';/,
    'the ?occlog=1 flag is parsed from the query string');
  assert.match(appJs, /window\.__occlog = occLogEntries;/,
    'the entries are exposed on window.__occlog for a session to dump');
  assert.match(issueSrc, /occLog\(\{ ev: 'issue', cmd, ok \}\)/,
    'issue() records every locally-applied command with its verdict');
  assert.match(issueSrc, /occLog\(\{ ev: 'issue', cmd, ok: sent, net: true \}\)/,
    'issue() records net-stamped commands too');
  assert.match(orderAtSrc, /ev: 'pick', wx, wy, fromCanvas, selection: targets\.slice\(\)/,
    'orderAt records the pick resolution');
  assert.match(orderAtSrc, /pickedFog:/,
    'the pick record carries the fog gate\u2019s verdict on the picked unit');
  assert.match(orderAtSrc, /ev: 'enemy-guard', tgt/,
    'the FIX #5 guard records when it ends a click');
  assert.match(appJs, /occLog\(\{ ev: 'rclick', screen: eventScreen\(e\), world: \[wx, wy\], shift: e\.shiftKey \}\)/,
    'the canvas right-click handler records the pixel and the world point');
  assert.match(appJs, /occLog\(\{ ev: 'rclick-minimap', world: \[w\[0\], w\[1\]\], shift: ev\.shiftKey \}\)/,
    'the minimap right-click records too (its pick is the ground pick)');
  // The sliced-function discipline: orderAt's log calls are
  // typeof-guarded so the older slicing harnesses (occorder,
  // occludedorder, ordershadow, ...) keep running unchanged.
  assert.match(orderAtSrc, /if \(typeof occLog === 'function'\) occLog\(\{\s*ev: 'pick'/,
    'the pick log is typeof-guarded for the slicing harnesses');
});

test('occlog is inert without the flag', () => {
  const { occLog, entries, on } = makeOccLog('');
  assert.equal(on, false);
  occLog({ ev: 'issue', cmd: { t: 'attack', e: 1, tg: 7 }, ok: true });
  assert.deepEqual(entries, [], 'no flag, no records — and no behaviour change');
});

// ------------------------------------------------------------------
// The headline pin: a right-click resolving to an occluded
// enemy emits an Attack command ON THAT ENTITY.

test('outline click: the right-click emits Attack on the occluded enemy (and occLog proves the stages)', () => {
  const w = makeWorld({});
  // The enemy's own ground point re-projects inside rect ∩
  // cover — a click on the painted outline itself.
  w.orderAt(ENEMY[0], ENEMY[1], false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'exactly one command: the attack — not move, not garrison_tc, not attackmove');
  assert.equal(w.issued[0].tg, 7, 'the attack names the occluded enemy entity');
  assert.equal(w.issued[0].e, 1, 'the attack comes from our selected Scout');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution');
  assert.equal(pick.picked, 7, 'entityAt named the enemy');
  assert.equal(pick.afterGate, 7, 'the fog gate kept it');
  assert.equal(pick.outline, 7, 'the outline pick named it');
  assert.equal(pick.tFromOutline, true);
  assert.deepEqual(pick.recordIds, [7], 'the frame\u2019s only record is the enemy\u2019s');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 7, true]],
    'occLog\u2019s issue record matches what the sim accepted');
});

test('record-rect-only click (visible sliver above the roof): the right-click emits Attack on the occluded enemy', () => {
  const w = makeWorld({});
  const [wx, wy] = w.clickAt(1.5, 3.3); // occorder4's tall click: outside rect ∩ cover
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: the outline pick misses this pixel');
  assert.equal(w.occludedRectUnitAt(wx, wy), 7,
    'staging: the record-rect pick names the enemy');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'exactly one command: the attack — not move, not garrison_tc, not attackmove');
  assert.equal(w.issued[0].tg, 7);
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.equal(pick.picked, 7);
  assert.equal(pick.afterGate, 7);
  assert.equal(pick.outline, null);
  assert.equal(pick.outlineRect, 7, 'occLog shows the record-rect pick carried this click');
  assert.equal(pick.tFromOutline, false,
    'the fog exemption did not apply — the gate passed on the cell\u2019s own visibility');
});

// ------------------------------------------------------------------
// Bounds (asserted as CURRENT behaviour — see the header).

test('BOUND: a record-rect-named enemy whose cell reads fog-hidden is nulled by the fog gate', () => {
  // entityAt names the enemy through its record (the record
  // pick has no fog check of its own), tFromOutline is false
  // (the click is outside rect ∩ cover, so FIX #2b's exemption
  // does not apply), and the M7 fog gate nulls the target: the
  // click degrades to a ground move. In the real client this
  // state cannot arise from the draw path — draw() refreshes
  // the FOG snapshot and builds the frame's records from that
  // same snapshot, and a non-own unit in a non-visible cell is
  // not even drawn, let alone recorded — so this pin documents
  // the gate's precedence, it does not report a live route.
  // The ?occlog=1 capture settles it empirically either way:
  // its pick entries carry pickedFog alongside the picks.
  const w = makeWorld({ fogVisible: false });
  const [wx, wy] = w.clickAt(1.5, 3.3);
  assert.equal(w.entityAt(wx, wy, true), 7, 'staging: the pick still names the enemy');
  w.orderAt(wx, wy, false, true);
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.equal(pick.picked, 7);
  assert.equal(pick.afterGate, null, 'the fog gate nulled the record-rect-named target');
  assert.equal(pick.pickedFog, false);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'current behaviour: the click degrades to a ground move (documented bound)');
});

test('BOUND: an overlapping OWN outline record wins the pick at a shared pixel (frontmost doctrine)', () => {
  // occlusion.js's documented rule: among overlapping
  // outlines the frontmost unit (greatest painter depth) wins.
  // Our own Villager stands beside the enemy, slightly toward
  // the viewer, also occluded by the TC; at a pixel inside
  // both records' rect ∩ cover the pick names the Villager —
  // its outline is the one painted on top there — and the
  // order degrades (the selected Scout cannot garrison; the
  // click ends as a ground move). Whether live clicks ever
  // land in such an overlap is one of the things the
  // ?occlog=1 capture answers (recordIds + picked in the pick
  // entry: picked = an own unit while the enemy's id is also
  // in recordIds).
  const w = makeWorld({ withVillager: true });
  const [wx, wy] = w.sharedPixelWorld;
  assert.equal(w.occludedUnitAt(wx, wy), 3,
    'staging: the outline pick names the frontmost record — our Villager');
  w.orderAt(wx, wy, false, true);
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.equal(pick.picked, 3, 'the click resolved to our own Villager, not the enemy');
  assert.deepEqual(pick.recordIds, [7, 3],
    'both records were present that frame — the enemy\u2019s among them');
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'current behaviour: an own-named click inside the TC footprint degrades to a ground move (documented bound)');
});
