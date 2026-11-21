// Occluded-outline ORDER degradation, round 17 (bot-loop
// occorder17, 2026-11-20; follow-up to run #444, the directed
// live vigia on build f32240e / wasm f1923cd4, solo vs
// AI·Normal, client ?occlog=1 panel).
//
// THE CAPTURE (run #444, live): an enemy Scout Cavalry stood
// under the player's Town Center roof attacking it from
// minute 1 (TC 2400 → 1872 over the session). LEFT-clicks on
// its red outline DID select it repeatedly (HP 45/45). The
// diagnostic RIGHT-click — own Scout Cavalry selected,
// Aggressive stance confirmed, click on the tall red outline
// LOW at its feet — produced verbatim:
//   rclick world [-90.4721022950806, 68.54857042833213]
//   pick {selection:[4], picked:null, afterGate:null,
//     tFromOutline:false, outline:null, outlineRect:null,
//     records:5, recordIds:[2,56,4,1,3], pickedKind:null}
//   issue {t:'garrison_tc', e:4, tc:0} ok:false
//   issue {t:'move', e:4, x:-90.47…, y:68.54…}
// No enemy-guard line, no attack issue. The Scout never
// engaged; the foe left at full HP. (Run #431's capture was
// the downstream shape — pick resolved the foe via outline
// and the issue branch dropped it; here outline AND rect are
// both null: the pick itself never named the foe.)
//
// ------------------------------------------------------------------
// DISCRIMINATION (measured in this harness; pixel sweeps over
// every TC-footprint position class on the pristine build —
// foe at the centre disc (0,0)/(0.7,-2.9)/(2,2)/(3,0), the
// inside edges (±5,±8), the mid ring, and the outside strip
// (±9, ±10), records built exactly as occorder14's harness
// builds them):
//
//   - A foe INSIDE the client's symmetric ±tcHalf (8) box
//     around the TC entity position is fully covered: on its
//     outline, on its recorded rect, and in the whole feet
//     band below its rect, every inside-footprint click
//     resolves it (outline / rect / body / ground / the
//     FIX #11/#12 TC arena) — ZERO capture-signature pixels
//     (entityAt null while tcAt names the own TC). The disc
//     foe a TC attacker settles as (within melee reach 3.0
//     of the centre) is the most covered case of all. So the
//     live foe was NOT inside that box at click time.
//   - A foe whose anchor sits in the 9–10 wu strip BEHIND
//     the TC (west (-9,0) / north (0,-9) / NW (-9,-9) /
//     (-10,0)) still holds an INF-12 record — its sprite
//     overlaps the TC art (anchor up to ~2 wu past the art's
//     edge, the sprite's own reach paints the rest), its
//     outline shows through the roof, and clicks ON the
//     outline resolve it 100% (the left-click successes).
//     But the LOW feet band below its recorded rect (the
//     diagnostic click's home: ground point past the 2.2
//     ground radius, body box missed with h < 0, rect and
//     outline both missed below the rect's bottom edge)
//     falls to ONE remaining candidate — the TC arena — and
//     the arena's foe-membership gate is the symmetric
//     Chebyshev box |foe − TC| <= tcHalf (8) on both axes:
//     a strip foe fails it by 1–2 wu, so entityAt returns
//     null and orderAt reinterprets the click from its
//     ground point inside the footprint: garrison_tc
//     (refused — mounted) and a ground move. 104–127
//     capture-signature pixels measured per staging.
//   - The same membership gate excludes the east/south
//     edge strip ((10,0), (0,10), (9,-1), (-1,9) — inside
//     the SIM's asymmetric footprint, which spans −6..+10
//     around the entity position because the TC entity
//     stands on its footprint's (1,1) cell centre): those
//     foes hold no TC record (drawn in front of the art),
//     and the top band of their tall sprite (body height
//     3.75 in the click's decomposition, above the cached
//     body box's 2.81 top — the band FIX #5's record-rect
//     stage exists for, unreachable without a record)
//     converts to ground inside the footprint ≥ 5 wu from
//     the foe: 61–66 capture-signature pixels per staging.
//   - Fog, the centre-disc gate and the click-distance gate
//     are NOT implicated: the strip foe's record proves its
//     cell was fog-visible at draw time (occlusion.js's
//     fairness gate), and the failing clicks sit 2.2–5.4 wu
//     from the foe — inside the arena's click-distance gate
//     (one half-extent, 8). Membership is the one gate that
//     fails, in both arena candidates (entityAt's and
//     selectionAt's restated shadow).
//
// THE FIX (FIX #14, client pick layer only): the arena
// candidates' foe-membership box widens from tcHalf (8) to
// tcHalf + 2 (10) on both axes — the anchor strip whose
// sprite still overlaps the TC art / footprint (a cavalry
// sprite reaches ~2.1 wu past its anchor; foes past +10
// measured cannot hold a TC-occluded record at all: at
// (-11,0) the covered fraction is ~0.03, under the 0.15
// INF-12 threshold). Both distance gates are UNCHANGED
// (click within one half-extent of the foe, or the foe
// within 5.0 of the TC centre), so every earlier bound
// keeps its shape: their foes fail the distance gates, not
// membership (restaged below). selectionAt's arena shadow
// widens identically (selection parity, as FIX #11/#12
// mirrored). Raw picks, orderAt, tcAt and the sim are
// untouched; golden hash untouched by construction.
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
const selectionAtSrc = sliceFn(appJs, 'function selectionAt(wx, wy) {', '\n  // Left-click pick for an animal');
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
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 }; // cavalry tall pose (occorder5)
const VIL_RECT = { x0: -0.9, y0: -2.6, w: 1.8, h: 2.75 };   // villager box (occorder5)

function makeHarness(st, records, issued, occEntriesBundle, foePos) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = st.kinds;
  const { occLog, entries } = occEntriesBundle;
  // Records EVERY issue attempt with its verdict — run #444's
  // log shape (the refused garrison is an issued line too).
  const applyLocalOrder = (cmd) => {
    let ok = true;
    if (cmd.t === 'garrison_tc' || cmd.t === 'garrison_u') {
      ok = kinds[cmd.e] === 0 || kinds[cmd.e] === 3; // foot kinds only — a mounted Scout is refused
    }
    issued.push({ ...cmd, ok });
    return ok;
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
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, selectionAt, unitAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, spriteBodyD2, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
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

// The #444 scene, TC-relative (the harness TC stands at the
// origin, as in every occorder harness; live world coords
// are this frame translated to the generated TC cell).
// TC 0; villagers 1..3 at the start offsets; own Scout 4
// (the selected unit, kind 2) at its start (5,5) unless
// selPos says otherwise; foe Scout 56 (kind 2, owner 2) at
// foePos, HP 45/45 as left-clicked live.
function makeWorld({ foePos = [-9, 0], selKind = 2, selPos = [5, 5], fogHidesFoe = false, foeDead = false } = {}) {
  makeHarness.fogHidesFoe = fogHidesFoe;
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, 5, 0, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, 5, 50);
  put(st, 4, selKind, 0, selPos[0], selPos[1], 45);
  put(st, 56, 2, 2, foePos[0], foePos[1], foeDead ? 0 : 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'), fogHidesFoe ? foePos : null);
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  const addRecord = (id, box) => {
    const [sx, sy] = fns.worldToScreen(st.pos[id * 2], st.pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * 8, y: sy + box.y0 * 8, w: box.w * 8, h: box.h * 8 };
    const depth = st.pos[id * 2] + st.pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    if (!res.occluded) return null;
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  let foeRec = null;
  for (const [id, box] of [[1, VIL_RECT], [2, VIL_RECT], [3, VIL_RECT], [4, TALL_RECT], [56, TALL_RECT]]) {
    const r = addRecord(id, box);
    if (r) records.push(r);
    if (id === 56) foeRec = r;
  }
  const pt = (sx, sy) => ({ screen: [sx, sy], world: fns.s2w(sx, sy) });
  return { ...fns, issued, records, occEntries, st, tcCover, foeRec, pt };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #14 wiring: the widened arena membership lives in both arena candidates; consultation counts unchanged', () => {
  assert.match(entityAtSrc, /FIX #14 \(bot-loop occorder17/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /tcHalf\(\) \+ 2/,
    'entityAt\'s arena membership is tcHalf + 2 (the sprite-reach strip)');
  assert.equal((entityAtSrc.match(/foeTcArenaAtPoint\(\)/g) || []).length, 4,
    'the arena is still consulted at the same four sites (occorder13/14 pins)');
  assert.match(selectionAtSrc, /FIX #14 \(bot-loop occorder17/,
    'the selection mirror is marked in selectionAt');
  assert.match(selectionAtSrc, /tcHalf\(\) \+ 2/,
    'selectionAt\'s arena membership widens identically (selection parity)');
  assert.equal((selectionAtSrc.match(/foeTcArenaSelAtPoint\(\)/g) || []).length, 3,
    'the selection arena is consulted at its two shadow sites (occorder14 pin) plus the FIX #20 creator site (occorder22 supersession, count 2 -> 3)');
  assert.ok(!/foeTcArenaSelAtPoint|foeTcArenaAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
});

// ------------------------------------------------------------------
// Capture W: the west-behind strip foe (-9,0) — it holds a
// record (recordIds live included 56), its outline clicks
// resolve, and the LOW feet click is run #444's signature
// verbatim on pristine (entityAt null → garrison_tc refused
// → move). Post-fix the same click attacks.

test('capture W post-fix: a LOW feet click on a west-strip foe\'s outline attacks (FIX #14)', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  assert.ok(w.foeRec, 'staging: the strip foe holds an INF-12 record — its outline is painted through the TC art (live recordIds included 56)');
  assert.ok(w.records.some((r) => r.id === 56), 'staging: record 56 is in the frame\'s record set, as logged live');
  const { world: [wx, wy] } = w.pt(880, 518);
  assert.deepEqual([wx, wy], [-7.75, 2.25], 'staging: the click\'s ground point, LOW at the outline\'s feet (below the recorded rect)');
  assert.equal(w.occludedUnitAt(wx, wy), null, 'staging: no outline winner at the feet pixel — live outline:null');
  assert.equal(w.occludedRectUnitAt(wx, wy), null, 'staging: no record-rect winner either — live outlineRect:null');
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the ground point lies inside our own TC footprint — the garrison reinterpretation is armed');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2 && dFoe < 8,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click — past the ground radius, inside the arena's click-distance gate`);
  const cheb = Math.max(Math.abs(w.st.pos[56 * 2]), Math.abs(w.st.pos[56 * 2 + 1]));
  assert.ok(cheb === 9 && cheb > 8,
    'staging: the foe\'s anchor sits 1 wu past the pre-fix membership box (tcHalf 8) — the one gate that failed live');
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the strip foe by the widened TC arena, not nothing');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 56, true]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick && pick.picked === 56 && pick.afterGate === 56 && pick.outline === null && pick.outlineRect === null,
    'the occLog pick line post-fix reads picked:56 / afterGate:56 with outline still null — the arena, not a record stage, names the foe');
});

test('capture N post-fix: a LOW feet click on a north-strip foe\'s outline attacks (FIX #14)', () => {
  const w = makeWorld({ foePos: [0, -9] });
  assert.ok(w.foeRec, 'staging: the north-strip foe holds a record too');
  const { world: [wx, wy] } = w.pt(1024, 516);
  assert.deepEqual([wx, wy], [1, -7], 'staging: the feet click\'s ground point');
  assert.equal(w.occludedUnitAt(wx, wy), null, 'staging: no outline winner at the feet pixel');
  assert.equal(w.occludedRectUnitAt(wx, wy), null, 'staging: no record-rect winner');
  assert.equal(w.tcAt(wx, wy), 0, 'staging: inside our TC footprint');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2 && dFoe < 8, `staging: foe ${dFoe.toFixed(2)} wu away — ground misses, click gate passes`);
  assert.equal(w.entityAt(wx, wy, true), 56, 'entityAt resolves the north-strip foe by the widened arena');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 56, true]],
    'exactly one attack — no garrison_tc, no move');
});

test('capture E post-fix: the tall top band of an east-edge foe (10,0) attacks (FIX #14)', () => {
  const w = makeWorld({ foePos: [10, 0] });
  assert.equal(w.foeRec, null, 'staging: an east-edge foe holds NO TC record (drawn in front of the art) — the record-rect stage can never name it');
  const { world: [wx, wy] } = w.pt(1032, 550);
  assert.deepEqual([wx, wy], [5.75, -3.25], 'staging: the click on the foe sprite\'s tall top band, converted to ground inside the footprint');
  assert.equal(w.occludedUnitAt(wx, wy), null, 'staging: no outline winner');
  assert.equal(w.occludedRectUnitAt(wx, wy), null, 'staging: no record-rect winner (no record)');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'staging: the foe\'s static body box misses — the click sits at body height 3.75, above the cached box top 2.81 (the tall attack pose)');
  assert.equal(w.tcAt(wx, wy), 0, 'staging: inside our TC footprint — garrison armed on pristine');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2 && dFoe < 8, `staging: foe ${dFoe.toFixed(2)} wu away — ground misses, click gate passes`);
  assert.equal(w.entityAt(wx, wy, true), 56, 'entityAt resolves the east-edge foe by the widened arena');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 56, true]],
    'exactly one attack — no garrison_tc, no move');
});

// ------------------------------------------------------------------
// Controls: the outline itself always resolved (the live
// left-click successes), for selection and for orders —
// pre-fix and post-fix alike (regression pins, not captures).

test('control: a click ON the west-strip foe\'s painted outline selects and attacks it (the live left-click shape)', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  const { world: [wx, wy] } = w.pt(896, 490);
  assert.equal(w.occludedUnitAt(wx, wy), 56, 'staging: the pixel is on the painted outline (rect ∩ cover)');
  assert.equal(w.selectionAt(wx, wy), 56, 'selectionAt names the foe — the repeated live left-click success');
  assert.equal(w.entityAt(wx, wy, true), 56, 'entityAt names the foe through the outline stage');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 56, true]],
    'an outline click attacks, as it did live');
});

test('selection parity post-fix: the widened arena shadow names the strip foe over our own unit\'s ground win (FIX #14 mirror)', () => {
  const w = makeWorld({ foePos: [-9, 0], selPos: [-6, 2] });
  const { world: [wx, wy] } = { world: [-7.75, 2.25] };
  assert.equal(w.occludedUnitAt(wx, wy), null, 'staging: no outline winner at the feet point');
  assert.equal(w.unitAt(wx, wy), 4,
    'staging: the raw own-only fallback names our Scout by its ground radius — unitAt itself is unchanged');
  assert.equal(w.selectionAt(wx, wy), 56,
    'selectionAt shadows the own ground win with the widened-arena foe, exactly as entityAt resolves it');
});

// ------------------------------------------------------------------
// Sweep: over the staged strip frames, NO band pixel keeps
// the capture signature (fails on pristine: 104 / 127 / 62
// signature pixels measured for W / N / E).

test('sweep: no capture-signature pixels remain in the strip foes\' outline/rect/feet bands', () => {
  const inRect = (r, x, y) => r && x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
  for (const foePos of [[-9, 0], [0, -9], [10, 0]]) {
    const w = makeWorld({ foePos });
    const [ax, ay] = w.worldToScreen(foePos[0], foePos[1]);
    const rect = w.foeRec ? w.foeRec.rect
      : { x: ax + TALL_RECT.x0 * 8, y: ay + TALL_RECT.y0 * 8, w: TALL_RECT.w * 8, h: TALL_RECT.h * 8 };
    const sig = [];
    for (let sy = Math.floor(rect.y - 8); sy <= Math.ceil(rect.y + rect.h + 28); sy += 2) {
      for (let sx = Math.floor(rect.x - 8); sx <= Math.ceil(rect.x + rect.w + 8); sx += 2) {
        const inBand = inRect(rect, sx, sy)
          || (sx >= rect.x && sx < rect.x + rect.w && sy >= rect.y + rect.h && sy < rect.y + rect.h + 28);
        if (!inBand) continue;
        const [wx, wy] = w.s2w(sx, sy);
        if (w.entityAt(wx, wy, true) === null && w.tcAt(wx, wy) === 0) sig.push([sx, sy]);
      }
    }
    assert.deepEqual(sig, [],
      `foe at (${foePos}): no band pixel may degrade to the garrison/move reinterpretation`);
  }
});

// ------------------------------------------------------------------
// Bounds: the distance gates are untouched — widening
// membership alone must not turn the whole footprint, or a
// far click, into an attack.

test('bound: a far inside-footprint click past a strip foe still garrisons/moves — the distance gates stand', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  const [wx, wy] = [7, 7];
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  const cFoe = Math.hypot(w.st.pos[56 * 2], w.st.pos[56 * 2 + 1]);
  assert.ok(dFoe >= 8 && cFoe > 5.0,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click and ${cFoe.toFixed(2)} from the centre — outside BOTH distance gates`);
  assert.equal(w.entityAt(wx, wy, true), null, 'the widened membership alone does not resolve a far foe');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.ok]), [['garrison_tc', false], ['move', true]],
    'the mounted Scout\'s click keeps its pre-fix shape at distance: refused garrison, then a ground move — no attack');
});

test('bound: a west-edge foe outside both distance gates does not shadow our outline winner — the pick stands, the order falls through (occorder13/14 bound, restaged; occorder18)', () => {
  const w = makeWorld({ foePos: [-6, -1], selKind: 3, selPos: [9, -1] });
  // Our front Villager 1 restaged at (2,2) for this bound, as
  // in occorder14: move it and rebuild its record geometry
  // by hand (records were built at makeWorld time from the
  // start offsets, so restage the whole world instead).
  const w2 = makeWorldBound();
  const [wx, wy] = w2.pt(960, 555).world;
  assert.equal(w2.occludedUnitAt(wx, wy), 1, 'staging: only our Villager\'s outline is hit');
  const dFoe = Math.hypot(w2.st.pos[56 * 2] - wx, w2.st.pos[56 * 2 + 1] - wy);
  const cFoe = Math.hypot(w2.st.pos[56 * 2], w2.st.pos[56 * 2 + 1]);
  assert.ok(Math.abs(dFoe - 8.39) < 0.01 && dFoe >= 8 && cFoe > 5.0,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click and ${cFoe.toFixed(2)} from the centre — outside BOTH gates (membership was never its gate)`);
  assert.equal(w2.entityAt(wx, wy, true), 1, 'the foe preference does not fire — our Villager keeps the pick');
  w2.orderAt(wx, wy, false, true);
  // occorder18 (FIX #15) supersedes this bound's order half,
  // as in occorder13/14: the pick stands, the order is a move.
  assert.deepEqual(w2.issued.map((c) => [c.t, c.e, c.tc, c.ok]), [['move', 4, undefined, true]],
    'occorder18: an own-unit pick inside the footprint issues a move, never a garrison');
  void w;
});

// The occorder14 bound world: front Villager 1 at (2,2),
// selected Militia 4 at (9,-1), foe 56 at (-6,-1).
function makeWorldBound() {
  makeHarness.fogHidesFoe = false;
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, 2, 2, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, -5, 50);
  put(st, 4, 3, 0, 9, -1, 40);
  put(st, 56, 2, 2, -6, -1, 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'), null);
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  for (const [id, box] of [[1, VIL_RECT], [2, VIL_RECT], [3, VIL_RECT], [56, TALL_RECT]]) {
    const [sx, sy] = fns.worldToScreen(st.pos[id * 2], st.pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * 8, y: sy + box.y0 * 8, w: box.w * 8, h: box.h * 8 };
    const depth = st.pos[id * 2] + st.pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    if (res.occluded) records.push({ id, rect, cover: res.cover.map((o) => o.rect), depth });
  }
  const pt = (sx, sy) => ({ screen: [sx, sy], world: fns.s2w(sx, sy) });
  return { ...fns, issued, records, occEntries, st, pt };
}

test('bound: a TC-footprint click with no living foe still garrisons/moves as before', () => {
  const w = makeWorld({ foePos: [-9, 0], foeDead: true });
  const { world: [wx, wy] } = w.pt(880, 518);
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the point is inside the footprint');
  assert.equal(w.entityAt(wx, wy, true), null, 'no living foe: the widened arena names nothing');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.ok]), [['garrison_tc', false], ['move', true]],
    'the mounted Scout\'s click keeps its pre-fix shape — the arena needs a living foe in the strip');
});

test('bound: the minimap pick is untouched — no arena at the strip foe\'s feet (SEL-16)', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  const { world: [wx, wy] } = w.pt(880, 518);
  assert.equal(w.entityAt(wx, wy, false), null,
    'with screenPick off, the pick stays the plain ground pick — the widened arena never consults');
  w.orderAt(wx, wy, false, false);
  assert.deepEqual(w.issued.map((c) => [c.t, c.ok]), [['garrison_tc', false], ['move', true]],
    'the minimap order at the same world point keeps its pre-fix shape');
});

test('fog discipline: a strip foe whose cell is not visible names nothing in the widened arena', () => {
  const w = makeWorld({ foePos: [-9, 0], fogHidesFoe: true });
  const { world: [wx, wy] } = w.pt(880, 518);
  assert.equal(w.entityAt(wx, wy, true), null,
    'the foe\'s cell reads fog-hidden, so the widened arena names nothing at its feet — the click falls through as before');
});
