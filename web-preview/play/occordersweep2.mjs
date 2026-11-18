// OCCORDER SWEEP 2 — follow-up diagnosis for bot-loop run #423
// (2026-11-18), where the occorder fix (main 5413df1) FAILED its
// directed live proof even though sweep 1 measured 100% attack on
// the painted outline.
//
// What sweep 1 did NOT stage (the gaps this sweep closes):
//  * No FRIENDLY unit stood anywhere near the hidden enemy. In
//    the live scene our own Scout stood ON (nearly) the hidden
//    enemy's tile — LEFT-clicking the red figure selected our
//    own Scout. Pick precedence with a coincident friendly is
//    hypothesis H1.
//  * The enemy sat fully inside the TC cover rect. In the live
//    scene it sat at the TC's EAST PORCH — partly inside the
//    footprint / cover, partly visible. Partial occlusion is
//    hypothesis H2: clicks on the unit's VISIBLE part are not
//    outline clicks (occludedPickAt only hits rect ∩ cover), so
//    they fall to the ground pick — where a coincident friendly
//    can win.
//  * The TC-centre branches of orderAt (H4): when the click's
//    ground point lies inside our own TC footprint and the
//    target degrades to our own unit / null, orderAt tries
//    garrison_tc (refused for mounted units in the sim —
//    can_garrison_tc: mounted may use the Castle only), then
//    falls through to a plain ground MOVE. Measured here via
//    the attempts list.
//
// Method: the REAL orderAt / entityAt / occludedUnitAt / unitAt
// sliced verbatim from app.js (same slicing as sweep 1), the
// REAL isometric projection (w2s/s2w/worldToScreen/eventWorld),
// and the REAL occlusionFor / occludedPickAt from occlusion.js
// deciding whether a record exists and what it covers. The TC
// cover is modelled two ways, because drawOccludedOutlines
// uses the recorded draw calls' alpha-tight rect when the TC
// art drew, and tcOccGeo's synthesized rect only when it did
// not:
//  * 'vector' — sweep 1's cover: the vector TC's bounding box
//    (32 wu wide), equal to tcOccGeo's rect for a TC at origin.
//  * 'art' — the art TC's draw box: drawSpriteW width
//    17.92 wu (tcArtCells*CS*1.12), bottom planted 4.4 wu
//    toward the front edge (dy = tcArtHalf*0.55), height =
//    width (square-image assumption; the alpha-tight rect of
//    the real PNG is narrower still, so this is generous).
// Record creation also requires the painter-depth rule
// (occlusionFor): the TC's painter key is wx+wy+tcHalf() = 8
// for a TC at the origin, so it is "in front" only of units
// with wx+wy < 8.
//
// Run: node play/occordersweep2.mjs   (diagnostic; prints tables)
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { occludedPickAt, occlusionFor, intersectionRect } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  if (a < 0) throw new Error(`marker not found: ${startMarker}`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  if (b <= a) throw new Error(`end marker not found after: ${startMarker}`);
  return src.slice(a, b);
}

// Real client code, sliced verbatim from app.js.
const w2sSrc = sliceFn(appJs, 'function w2s(wx, wy, s, ox, oy) {', '\nfunction s2w');
const s2wSrc = sliceFn(appJs, 'function s2w(sx, sy, s, ox, oy) {', '\nfunction centerCamera');
const worldToCellSrc = sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost');
const heightAtWorldSrc = sliceFn(appJs, 'function heightAtWorld(wx, wy) {', '\nfunction fogAtWorld');
const worldToScreenSrc = sliceFn(appJs, 'const worldToScreen = (wx, wy) => {', '\n};') + '\n};';
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const unitAtSrc = sliceFn(appJs, 'function unitAt(wx, wy) {', '\n  // Left-click pick for an animal');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');

// ------------------------------------------------------------------
const CS = 4, HALF = 60, GW = 30, GH = 30;
const ELEV_LIFT = 1.1;
const CAVALRY_BOX = { x0: -1.4, y0: -3.05, w: 3.5, h: 3.2 }; // app.js SCOUT_BOX/CAVALRY_BOX
const TC_POS = [0, 0];
const TC_DEPTH = TC_POS[0] + TC_POS[1] + 8; // painter key: wx+wy+tcHalf()

function makeWorld({ camS = 8, enemyPos, friendlyPos, coverMode, forceRecord }) {
  const camOx = 960, camOy = 540;
  const HEIGHTS = new Array(GW * GH).fill(0);
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, TC_POS[0], TC_POS[1], 1909); // our TC, mid-drain as live
  put(1, 2, 0, friendlyPos[0], friendlyPos[1], 45); // our Scout (the ordered unit)
  put(7, 2, 2, enemyPos[0], enemyPos[1], 45); // the hidden enemy (Scout kind)
  const st = { kinds, owners, pos, hp, count: 8, garrisoned: [] };
  const issued = [];
  const attempts = [];
  const records = [];

  const env = {
    camS, camOx, camOy, ELEV_LIFT, HEIGHTS, GW, GH, HALF, CS,
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
    isOwnUnit: (s, i) => s.owners[i] === 0 && s.hp[i * 2] >= 1,
    // Faithful sim verdicts (sim/src/garrison.rs can_garrison:
    // mounted units may NOT garrison in a TC — Castle only):
    // garrison_tc / garrison_u are refused for the Scout; every
    // other order in this scene is accepted.
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
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, occludedUnitAt, unitAt, worldToScreen, heightAtWorld, worldToCell, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));

  const eventWorld = (sx, sy) => {
    let [wx, wy] = fns.s2w(sx, sy);
    const h = fns.heightAtWorld(wx, wy);
    if (h > 0) {
      const s2 = (x, y) => fns.s2w(x, y);
      [wx, wy] = s2(sx, sy + h * ELEV_LIFT * camS);
      const h2 = fns.heightAtWorld(wx, wy);
      if (h2 !== h) [wx, wy] = s2(sx, sy + h2 * ELEV_LIFT * camS);
    }
    return [wx, wy];
  };

  // Unit rect (the sprite's dest box, as sweep 1) and TC cover.
  const [esx, esy] = fns.worldToScreen(enemyPos[0], enemyPos[1]);
  const unitRect = {
    x: esx + CAVALRY_BOX.x0 * camS, y: esy + CAVALRY_BOX.y0 * camS,
    w: CAVALRY_BOX.w * camS, h: CAVALRY_BOX.h * camS,
  };
  const [tsx, tsy] = fns.worldToScreen(TC_POS[0], TC_POS[1]);
  const coverRect = coverMode === 'vector'
    ? { x: tsx - 16 * camS, y: tsy - 10.2 * camS, w: 32 * camS, h: 18.2 * camS }
    : { x: tsx - 8.96 * camS, y: tsy + 4.4 * camS - 17.92 * camS, w: 17.92 * camS, h: 17.92 * camS };

  // Record creation through the REAL occlusionFor: fog gate,
  // painter-depth rule and the 0.15 covered-fraction threshold.
  const occRes = occlusionFor(
    { rect: unitRect, depth: enemyPos[0] + enemyPos[1], fogVisible: true },
    [{ rect: coverRect, depth: TC_DEPTH }],
  );
  const recordCreated = forceRecord != null ? forceRecord : occRes.occluded;
  if (recordCreated) {
    records.push({ id: 7, rect: unitRect, cover: [coverRect], depth: enemyPos[0] + enemyPos[1] });
  }
  return {
    ...fns, eventWorld, issued, attempts, unitRect, coverRect, records,
    enemyPos, friendlyPos, occRes, recordCreated,
  };
}

function classify(issued) {
  if (!issued.length) return 'none';
  const atk = issued.find((c) => c.t === 'attack');
  if (atk) return atk.tg === 7 ? 'attack7' : `attack${atk.tg}`;
  return issued[0].t;
}

// The LEFT-click selection outcome, replicating the canvas
// handler's precedence exactly (app.js pointer handler):
// unitAt (own units, ground radius 1.9) FIRST; the occluded
// pick is consulted only when unitAt misses; then animal/TC/
// building (all null in this scene -> selection cleared).
function leftPick(w, wx, wy) {
  const v = w.unitAt(wx, wy);
  if (v != null) return { final: v, via: 'unitAt' };
  const occ = w.occludedUnitAt(wx, wy);
  if (occ != null) return { final: occ, via: 'occluded' };
  return { final: null, via: 'none' };
}

// Grid over the enemy's sprite rect; each click classified by
// region: 'outline' (inside rect ∩ cover — where INF-12 paints)
// vs 'visible' (the rest of the sprite — drawn normally, in
// front of / beside the TC, NOT outline-painted).
function sweepSprite(w) {
  const r = w.unitRect;
  const inter = intersectionRect(r, w.coverRect);
  const regions = { outline: { n: 0, tally: {}, ent: {}, left: {} }, visible: { n: 0, tally: {}, ent: {}, left: {} } };
  const step = 2;
  for (let sy = r.y + step / 2; sy < r.y + r.h; sy += step) {
    for (let sx = r.x + step / 2; sx < r.x + r.w; sx += step) {
      const inOutline = w.recordCreated && inter && sx >= inter.x && sx < inter.x + inter.w && sy >= inter.y && sy < inter.y + inter.h;
      const reg = regions[inOutline ? 'outline' : 'visible'];
      reg.n++;
      const [wx, wy] = w.eventWorld(sx, sy);
      const ent = w.entityAt(wx, wy, true);
      const occ = w.occludedUnitAt(wx, wy);
      reg.ent[ent == null ? 'null' : ent] = (reg.ent[ent == null ? 'null' : ent] || 0) + 1;
      const lp = leftPick(w, wx, wy);
      const lk = lp.final == null ? 'clear' : `${lp.final}via-${lp.via}`;
      reg.left[lk] = (reg.left[lk] || 0) + 1;
      w.issued.length = 0; w.attempts.length = 0;
      w.orderAt(wx, wy, false, true);
      const cmd = classify([...w.issued]);
      reg.tally[cmd] = (reg.tally[cmd] || 0) + 1;
      if (cmd === 'move' && !reg.firstMove) {
        reg.firstMove = {
          screen: [+sx.toFixed(1), +sy.toFixed(1)], ground: [+wx.toFixed(2), +wy.toFixed(2)],
          ent, occ, tFromOutline: ent != null && occ === ent,
          attempts: [...w.attempts].map((c) => c.t),
          distEnemy: +Math.hypot(wx - w.enemyPos[0], wy - w.enemyPos[1]).toFixed(2),
          distFriendly: +Math.hypot(wx - w.friendlyPos[0], wy - w.friendlyPos[1]).toFixed(2),
          insideTC: Math.abs(wx) <= 8 && Math.abs(wy) <= 8,
        };
      }
    }
  }
  return regions;
}

const FAR = [-17.4, 17.4];
const SCEN = [
  { name: 'S1 control: NW fully inside, friendly FAR, vector cover', enemyPos: [-2.1, -2.1], friendlyPos: FAR, coverMode: 'vector' },
  { name: 'S2 NW fully inside, friendly COINCIDENT, vector cover', enemyPos: [-2.1, -2.1], friendlyPos: [-2.1, -2.1], coverMode: 'vector' },
  { name: 'S3 east porch (6,-6), friendly FAR, vector cover', enemyPos: [6, -6], friendlyPos: FAR, coverMode: 'vector' },
  { name: 'S4 east porch (6,-6), friendly COINCIDENT, vector cover', enemyPos: [6, -6], friendlyPos: [6, -6], coverMode: 'vector' },
  { name: 'S5 east porch (6,-6), friendly COINCIDENT, ART cover', enemyPos: [6, -6], friendlyPos: [6, -6], coverMode: 'art' },
  { name: 'S6 east porch (6,-6), friendly FAR, ART cover', enemyPos: [6, -6], friendlyPos: FAR, coverMode: 'art' },
  { name: 'S7 east edge (7.5,0), friendly COINCIDENT, vector cover', enemyPos: [7.5, 0], friendlyPos: [7.5, 0], coverMode: 'vector' },
  { name: 'S8 east porch (6,-6), friendly COINCIDENT, record ABSENT (H3-style)', enemyPos: [6, -6], friendlyPos: [6, -6], coverMode: 'vector', forceRecord: false },
  { name: 'S9 NW fully inside, friendly COINCIDENT, record ABSENT', enemyPos: [-2.1, -2.1], friendlyPos: [-2.1, -2.1], coverMode: 'vector', forceRecord: false },
  { name: 'S10 front-SE (5,5) in front of TC, friendly COINCIDENT, vector', enemyPos: [5, 5], friendlyPos: [5, 5], coverMode: 'vector' },
];

for (const s of SCEN) {
  const w = makeWorld(s);
  const regions = sweepSprite(w);
  console.log(`\n=== ${s.name}`);
  console.log(`  recordCreated=${w.recordCreated} coveredFraction=${w.occRes.fraction.toFixed(3)} unitRect=${JSON.stringify(w.unitRect)} cover=${JSON.stringify(w.coverRect)}`);
  for (const [rn, reg] of Object.entries(regions)) {
    if (!reg.n) { console.log(`  [${rn}] n=0`); continue; }
    const pct = (o) => Object.entries(o).map(([k, v]) => `${k}=${((100 * v) / reg.n).toFixed(1)}%`).join(' ');
    console.log(`  [${rn}] n=${reg.n} orders: ${pct(reg.tally)}`);
    console.log(`  [${rn}] entityAt: ${pct(reg.ent)} | leftClick: ${pct(reg.left)}`);
    if (reg.firstMove) console.log(`  [${rn}] first degraded click: ${JSON.stringify(reg.firstMove)}`);
  }
}

// Directed probes mirroring the four live attempts of run #423
// (east-porch scene, coincident friendly, both cover models):
//  A1 click the centre of the painted outline region;
//  A2 click the enemy's ground point (Scout "onto the TC tile");
//  A3 click the centre of the sprite's VISIBLE region;
//  A4 click the sprite's top (head/rider) in the visible region.
console.log('\n=== Directed probes (live attempt shapes, S5/S4 geometry)');
for (const coverMode of ['art', 'vector']) {
  const w = makeWorld({ enemyPos: [6, -6], friendlyPos: [6, -6], coverMode });
  const inter = intersectionRect(w.unitRect, w.coverRect);
  const probes = {};
  if (inter) probes.A1_outlineCentre = [inter.x + inter.w / 2, inter.y + inter.h / 2];
  probes.A2_enemyGround = w.worldToScreen(6, -6);
  // visible region: sprite points outside the cover
  const r = w.unitRect;
  probes.A3_visibleCentre = [r.x + r.w - 2, r.y + r.h / 2];
  probes.A4_visibleTop = [r.x + r.w / 2, r.y + 2];
  for (const [pn, [sx, sy]] of Object.entries(probes)) {
    const [wx, wy] = w.eventWorld(sx, sy);
    const ent = w.entityAt(wx, wy, true);
    const occ = w.occludedUnitAt(wx, wy);
    const lp = leftPick(w, wx, wy);
    w.issued.length = 0; w.attempts.length = 0;
    w.orderAt(wx, wy, false, true);
    console.log(`  [${coverMode}] ${pn} screen=[${sx.toFixed(1)},${sy.toFixed(1)}] ground=[${wx.toFixed(2)},${wy.toFixed(2)}] ent=${ent} occ=${occ} tFromOutline=${ent != null && occ === ent} left=${lp.final}via-${lp.via} attempts=${JSON.stringify(w.attempts.map((c) => c.t))} issued=${JSON.stringify(w.issued)}`);
  }
}
