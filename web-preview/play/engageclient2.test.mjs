// Occluded-outline ORDER divergence, engageclient2 (bot-loop
// #486; follow-up to the engageclient scoping note,
// ~/workspace/skirmish-study/handoffs/engageclient-build.md).
//
// THE STAGED RESIDUAL (the scoping's §5/R1 + P3, the one
// configuration no occorder pin covered jointly):
//   - the selected Militia ITSELF is the frontmost outline
//     winner at the click pixel (its own INF-12 record);
//   - the foe under our Town Center holds NO record that
//     frame (record absence is a normal per-frame event —
//     occorder8's cliff: covered fraction < 0.15 or no
//     occluder drawn strictly in front at that instant);
//   - the click pixel is HIGH on the foe's figure — on the
//     TC's drawn ART, at body height 3.125, above the Scout's
//     cached body-box top (2.81, spriteBodyD2) and inside the
//     tall attack pose's reach (h ≈ 3.55–3.9, occorder4);
//   - the click's ground point is displaced north-west of
//     the foe: 4.42 wu away (past the 2.2 ground radius) and
//     OUTSIDE the TC footprint box (±8), so tcAt is null and
//     the ground-space TC arena (FIX #11) cannot arbitrate.
// On pristine ddcc27f every foe candidate misses in
// conjunction (outline/rect: no foe record; body: above the
// cached box top; ground: displacement > 2.2; arena: tcAt
// null), entityAt returns the own Militia, and orderAt
// reinterprets the click as a ground move — the unit walks
// to the displaced point and idles beside the foe.
//
// THE FIX (FIX #25, client entityAt only): the TC arena is
// restated in ART space — foeTcArtArenaAtPoint, consulted
// after foeTcArenaAtPoint at the same four stages (outline,
// record-rect, body, ground). The click must lie on OUR
// Town Center's drawn art (tcArtAt's FIX #22 content bands,
// computed inline from the TC centre so no new pick
// dependency is introduced), the foe must be alive, not
// garrisoned, cell-visible, within the arena's membership
// box (±tcHalf + 2 of the centre, FIX #14) and within one
// half-extent (tcHalf) of the click. Fog discipline is the
// arena's own; the target is not tFromOutline, so orderAt's
// fog gate still applies. Bounds below pin the limits: a
// far membership foe (d >= 8), a fog-hidden foe, a dead foe
// and the minimap path all keep their pristine shapes.
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
const unitAtSrc = sliceFn(appJs, 'function unitAt(wx, wy) {', '\n  // Left-click selection pick');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');
const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');
const occLogSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');

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
  return { occLog: r.occLog, entries: r.occLogEntries, on: r.OCCLOG_ON };
}

const CS = 4, HALF = 240, GW = 120, GH = 120;
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 }; // cavalry tall pose (occorder5/17)

function makeHarness(st, records, issued, occEntriesBundle, foePos) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = st.kinds;
  const { occLog, entries } = occEntriesBundle;
  const applyLocalOrder = (cmd) => {
    issued.push({ ...cmd, ok: true });
    return true;
  };
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: (x, y) => !(foePos && x === foePos[0] && y === foePos[1] && makeHarness.fogHidesFoe),
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [4],
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
    sim: { entity_kinds: () => kinds, positions: () => st.pos, tick_count: () => 0 },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
    isOwnUnit: (s, i) => s.owners[i] === 0 && s.kinds[i] !== 13 && s.hp[i * 2] >= 1
      && !(s.garrisoned && s.garrisoned[i]),
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder,
    occLog,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, unitAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, spriteBodyD2, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));
  return { fns, occEntries: entries };
}

function blankState(count) {
  return {
    kinds: new Array(count).fill(undefined),
    owners: new Array(count).fill(undefined),
    pos: new Array(count * 2).fill(0),
    hp: new Array(count * 2).fill(0),
    count, garrisoned: [],
  };
}
function put(st, i, kind, owner, x, y, h) {
  st.kinds[i] = kind; st.owners[i] = owner;
  st.pos[i * 2] = x; st.pos[i * 2 + 1] = y; st.hp[i * 2] = h; st.hp[i * 2 + 1] = h;
}

// The staged scene, TC-relative: TC 0 at the origin; the
// selected Militia 4 (kind 3) at (-5.5,-5.5), in front of
// the foe (depth -11 > -12), holding the frame's ONLY
// record; foe 28 (kind 2, owner 2) at foePos under the TC,
// holding NO record this frame.
function makeWorld({ foePos = [-6, -6], fogHidesFoe = false, foeDead = false } = {}) {
  makeHarness.fogHidesFoe = fogHidesFoe;
  const st = blankState(29);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 4, 3, 0, -5.5, -5.5, 40);
  put(st, 28, 2, 2, foePos[0], foePos[1], foeDead ? 0 : 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'), fogHidesFoe ? foePos : null);
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  const [sx, sy] = fns.worldToScreen(-5.5, -5.5);
  const rect = { x: sx + TALL_RECT.x0 * 8, y: sy + TALL_RECT.y0 * 8, w: TALL_RECT.w * 8, h: TALL_RECT.h * 8 };
  const res = occlusionFor({ rect, depth: -11, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
  assert.ok(res.occluded, 'staging: the Militia is occluded by the TC — its record exists');
  records.push({ id: 4, rect, cover: res.cover.map((o) => o.rect), depth: -11 });
  const pt = (px, py) => ({ screen: [px, py], world: fns.s2w(px, py) });
  return { ...fns, issued, records, occEntries, st, pt };
}

// The failing pixel: screen (960,467) — inside the
// Militia's outline (rect ∩ TC cover), high on the foe's
// figure (its tall rect spans screen y 460.8–493.2 at this
// column), and on the TC's drawn art (screen offset from
// the TC centre: ix = 0, iy = -9.125 — the upperworks band).
const CLICK = [960, 467];

// ------------------------------------------------------------------
// Wiring pin (fails on pristine).

test('FIX #25 wiring: entityAt carries the art-space TC arena candidate', () => {
  assert.match(entityAtSrc, /FIX #25 \(bot-loop engageclient2/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /foeTcArtArenaAtPoint/,
    'entityAt now resolves a foe through the TC art arena');
});

// ------------------------------------------------------------------
// The staged residual (fails on pristine: entityAt returns
// the own Militia 4 and orderAt issues a ground move).

test('staged residual post-fix: a HIGH outline click on a record-less foe under the TC attacks it', () => {
  const w = makeWorld({});
  const { world: [wx, wy] } = w.pt(...CLICK);
  assert.deepEqual(w.records.map((r) => r.id), [4],
    'staging: the foe holds NO record this frame — only the selected Militia has one');
  assert.equal(w.occludedUnitAt(wx, wy), 4,
    'staging: the frontmost outline winner at the pixel is our own selected Militia');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4,
    'staging: the record-rect winner is also our Militia — the foe has no rect to name it');
  assert.equal(w.spriteBodyD2(w.st, 28, wx, wy), null,
    'staging: the foe\'s static body box misses — the click sits at body height 3.125, above the cached box top 2.81 (the tall attack pose band)');
  const dFoe = Math.hypot(w.st.pos[28 * 2] - wx, w.st.pos[28 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2 && dFoe < 8,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click's ground point — past the ground radius, inside the arena's half-extent`);
  assert.equal(w.tcAt(wx, wy), null,
    'staging: the click\'s ground point is OUTSIDE the ±8 footprint box — the ground-space arena (FIX #11) cannot arbitrate');
  assert.equal(w.entityAt(wx, wy, true), 28,
    'post-fix the ORDER pick names the foe through the art-space arena — on pristine it returned our own Militia (4)');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 28, true]],
    'the Militia attacks 28 — on pristine the same click issued a ground move to the displaced point');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick && pick.picked === 28 && pick.afterGate === 28 && pick.tFromOutline === false,
    'the occLog pick line names the foe, not from the outline (the arena discipline — orderAt\'s fog gate still applies to it)');
});

// ------------------------------------------------------------------
// Control: a click at the visible foe's feet already
// attacked on pristine (body/ground geometry) — unchanged.

test('control: a click at the foe\'s feet attacks (already true on pristine)', () => {
  const w = makeWorld({});
  const { world: [wx, wy] } = w.pt(960, 500);
  assert.equal(w.entityAt(wx, wy, true), 28, 'the foe is named by its drawn body at its feet');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 28, true]],
    'the Militia attacks 28 from its feet pixel, as before the fix');
});

// ------------------------------------------------------------------
// Bounds.

test('bound: a far membership foe past the half-extent does not resolve through the art arena', () => {
  const w = makeWorld({ foePos: [9, -9] });
  const { world: [wx, wy] } = w.pt(...CLICK);
  const dFoe = Math.hypot(w.st.pos[28 * 2] - wx, w.st.pos[28 * 2 + 1] - wy);
  assert.ok(dFoe >= 8, `staging: the foe is ${dFoe.toFixed(2)} wu from the click — outside the arena distance gate`);
  assert.equal(w.entityAt(wx, wy, true), 4,
    'our own outline winner keeps the pick — the art arena is distance-gated like the ground arena');
  w.orderAt(wx, wy, false, true);
  assert.ok(!w.issued.some((c) => c.t === 'attack'), 'no attack is issued at distance');
});

test('bound: a fog-hidden foe names nothing through the art arena', () => {
  const w = makeWorld({ fogHidesFoe: true });
  const { world: [wx, wy] } = w.pt(...CLICK);
  assert.equal(w.entityAt(wx, wy, true), 4,
    'the arena fog discipline stands: a foe whose cell is not currently visible is never resolved');
  w.orderAt(wx, wy, false, true);
  assert.ok(!w.issued.some((c) => c.t === 'attack'), 'no attack on a fog-hidden foe');
});

test('bound: a dead foe names nothing — the own pick stands', () => {
  const w = makeWorld({ foeDead: true });
  const { world: [wx, wy] } = w.pt(...CLICK);
  assert.equal(w.entityAt(wx, wy, true), 4, 'no living foe under the roof: our Militia keeps the pick');
  w.orderAt(wx, wy, false, true);
  assert.ok(!w.issued.some((c) => c.t === 'attack'), 'no attack on a dead foe');
});

test('bound: the minimap pick is untouched — no arena, no art arena (SEL-16)', () => {
  const w = makeWorld({});
  const { world: [wx, wy] } = w.pt(...CLICK);
  assert.equal(w.entityAt(wx, wy, false), null,
    'with screenPick off, the pick is the plain ground pick — neither arena consults');
});
