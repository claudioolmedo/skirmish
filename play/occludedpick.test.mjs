// Occluded-outline pick regression pins (FIX #2; bot-loop
// runs #402–#413, 12+ live confirmations).
//
// The live defect: an enemy unit standing behind/inside the
// Town Center's footprint attacks the TC for minutes while
// visible ONLY as a red outline under the roof (INF-12). TC HP
// falls ~0.4–1.0 HP/s with no visible attacker. Clicking
// exactly on the outline selected ONLY the TC — the hidden
// unit could never be selected — and a Scout ordered to attack
// that point walked to the TC footprint centre and stood idle.
// The player had no counter-play.
//
// Three layers, all client-side (diagnosis on origin/main
// 0e3f666):
//  1. No left-click pick for enemy units at all: the plain
//     click tried unitAt (own units only), then animalAt
//     (Gaia/enemy animals only), then tcAt/buildingAt — a
//     non-animal enemy unit was never selectable, even in the
//     open. In DE, clicking an enemy unit selects it for
//     information.
//  2. Sprite-height displacement defeats the ground pick: the
//     visible outline is the unit's sprite clipped by the
//     occluder; the sprite rises screen-up from the ground
//     position, so a click on the outline converts to ground
//     displaced NW of the unit (the tree-canopy / Ram-body
//     family), outside entityAt's 2.2-unit radius — right-click
//     attacks missed the same way and degraded to ground moves.
//  3. Selection order gave the building the win: no unit
//     candidate was ever consulted for non-own, non-animal
//     entities, so tcAt/buildingAt took every click inside the
//     footprint.
//
// The fix (client only; sim and golden hash untouched): the
// INF-12 pass keeps its last frame's occluded-unit records
// (lastOccludedOutlines: {id, rect, cover, depth}); the pure
// pick decision (play/occlusion.js's occludedPickAt) names the
// unit when the click point falls inside the region the
// outline is actually painted on (rect ∩ cover). The left
// click consults it ahead of animalAt/tcAt/buildingAt (own
// unit → real selection; anyone else's → the animal path's
// information selection), and entityAt consults it after the
// Ram-body pick, ahead of the ground pick — so a right-click
// on the outline with military selected issues the attack
// against that entity. Fog rule: a record exists only for a
// unit the pass could outline, which already required current
// fog visibility (occlusion.js's fairness gate).
//
// These pins exercise the REAL occlusion.js decision on a
// staged TC + hidden-enemy scene, model the pristine ground
// picks to show why they missed, and pin the app.js wiring.
// A full stub-DOM staging is not used: no client flow can
// place an enemy behind the TC deterministically (sandbox has
// no enemy), which is why the house pick tests (canopypick,
// rampick) pin wiring + geometry in exactly this shape.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { occlusionFor, occludedPickAt } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');

const R = (x, y, w, h) => ({ x, y, w, h });

// Slice one nested function out of app.js between two markers.
function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

// ------------------------------------------------------------------
// The staged scene, built exactly as drawOccludedOutlines
// builds a record: an enemy unit (id 7) standing behind the
// Town Center, its sprite rect swallowed by the TC's rect, the
// TC's painter depth strictly greater, the unit's cell in
// current fog visibility.
const TC = { rect: R(60, 40, 110, 100), depth: 58 };
const ENEMY = { rect: R(100, 100, 24, 30), depth: 50, fogVisible: true };

function recordFor(unit, occluders, id) {
  const res = occlusionFor(unit, occluders);
  if (!res.occluded) return null;
  return { id, rect: unit.rect, cover: res.cover.map((o) => o.rect), depth: unit.depth };
}

const enemyRecord = recordFor(ENEMY, [TC], 7);

test('staging: the hidden enemy is occluded by the TC (INF-12 fires)', () => {
  assert.ok(enemyRecord, 'occlusionFor outlines the staged enemy');
  assert.deepEqual(enemyRecord.cover, [TC.rect], 'the TC is the cover');
});

test('click on the outline point selects the enemy', () => {
  // The centre of the painted outline region (unit ∩ TC).
  assert.equal(occludedPickAt([enemyRecord], 110, 110), 7);
  // The outline region's corners name the unit too.
  assert.equal(occludedPickAt([enemyRecord], 101, 101), 7);
  assert.equal(occludedPickAt([enemyRecord], 123, 129), 7);
});

test('clicks on the TC away from the outline still name no unit (the TC pick stands)', () => {
  assert.equal(occludedPickAt([enemyRecord], 70, 50), null, 'TC roof, far from the outline');
  assert.equal(occludedPickAt([enemyRecord], 165, 135), null, 'TC corner away from the outline');
  assert.equal(occludedPickAt([enemyRecord], 300, 300), null, 'open ground');
  assert.equal(occludedPickAt([], 110, 110), null, 'no records, no pick');
});

test('the visible (uncovered) part of a sprite is not the outline region', () => {
  // A taller unit only partly behind the TC: its lower sliver
  // sticks out below the roof line and is drawn normally.
  const tall = { rect: R(100, 90, 24, 60), depth: 50, fogVisible: true };
  const rec = recordFor(tall, [TC], 9);
  assert.ok(rec, 'partly covered past the threshold still outlines');
  assert.equal(occludedPickAt([rec], 110, 120), 9, 'the covered part names the unit');
  assert.equal(occludedPickAt([rec], 110, 145), null,
    'the visible sliver below the TC is not outline — the ordinary picks own it');
});

test('frontmost outline wins when outlines overlap', () => {
  const behind = recordFor({ rect: R(100, 100, 24, 30), depth: 50, fogVisible: true }, [TC], 7);
  const front = recordFor({ rect: R(104, 104, 24, 30), depth: 55, fogVisible: true }, [TC], 8);
  assert.equal(occludedPickAt([behind, front], 110, 110), 8);
  assert.equal(occludedPickAt([front, behind], 110, 110), 8, 'record order does not matter');
});

test('the fog gate is upstream: no visibility, no record, no pick', () => {
  const ghost = { rect: R(100, 100, 24, 30), depth: 50, fogVisible: false };
  assert.equal(recordFor(ghost, [TC], 7), null,
    'occlusionFor refuses to outline a fog-hidden unit, so no record can exist');
});

test('occludedPickAt: malformed records never pick', () => {
  assert.equal(occludedPickAt(null, 0, 0), null);
  assert.equal(occludedPickAt([null, { id: 3 }, { id: 4, rect: R(0, 0, 9, 9) }], 5, 5), null);
  assert.equal(occludedPickAt([{ id: 5, rect: R(0, 0, 9, 9), cover: [] }], 5, 5), null);
});

// ------------------------------------------------------------------
// Why the pristine picks missed this click (the ground-pick
// model, in world units): the unit stands at (100, 100); its
// sprite rises ~3 world units straight up-screen, so a click
// on the outline's centre converts to ground near (97, 97) —
// the (-d, -d) screen-up direction.
test('geometry: the pristine ground picks miss the outline click', () => {
  const click = [97, 97];
  const dUnit = Math.hypot(click[0] - 100, click[1] - 100);
  assert.ok(dUnit > 2.2, 'outside entityAt\'s 2.2 radius — the attack order missed');
  assert.ok(dUnit > 1.9, 'outside unitAt\'s 1.9 radius — the selection pick missed');
  // …and the point sits inside the TC footprint (half ~6), so
  // tcAt named the TC: the building won by default.
  assert.ok(Math.abs(click[0] - 100) <= 6 && Math.abs(click[1] - 100) <= 6,
    'the displaced point is inside the TC footprint');
});

// ------------------------------------------------------------------
// app.js wiring pins (these are what fail on pristine).

test('unit draw entries carry their entity id into the occlusion pass', () => {
  assert.match(appJs, /role: 'unit', owner, id: i,/,
    'the generic unit draw records occ.id so its outline record can name it');
  assert.match(appJs,
    /import \{ occlusionFor, intersectionRect, occludedPickAt \} from '\.\/play\/occlusion\.js';/,
    'app.js imports the pure pick decision');
});

test('drawOccludedOutlines keeps the last frame\'s records', () => {
  const body = sliceFn(appJs, 'function drawOccludedOutlines(items) {', '\nfunction draw() {');
  assert.match(body, /lastOccludedOutlines = \[\];/,
    'records reset every pass — a stale outline is never pickable');
  assert.match(body, /if \(u\.it\.occ\.id != null\) \{/,
    'only id-carrying unit records are kept (the ground relic takes no part)');
  assert.match(body, /cover: res\.cover\.map\(\(o\) => o\.rect\)/,
    'the record keeps exactly the covering occluders\' rects');
  assert.match(body, /depth: u\.depth/,
    'the record keeps the painter depth (frontmost wins)');
});

test('occludedUnitAt converts back to screen space and re-validates live state', () => {
  const body = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
  assert.match(body, /const \[sx, sy\] = worldToScreen\(wx, wy\);/,
    'the click is tested in screen space (the inverse of eventWorld)');
  assert.match(body, /occludedPickAt\(lastOccludedOutlines, sx, sy\)/,
    'the pure decision runs over the last frame\'s records');
  assert.match(body, /if \(id >= st\.count \|\| st\.hp\[id \* 2\] < 1\) return null;/,
    'a record for a dead or vanished entity names nothing');
  assert.match(body, /if \(st\.garrisoned && st\.garrisoned\[id\]\) return null;/,
    'a garrisoned unit (out of the field) names nothing');
});

test('entityAt: Ram body first, occluded outline second, ground pick last', () => {
  const body = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
  const ram = body.indexOf('if (bodyBest !== null) return bodyBest;');
  const occ = body.indexOf('const occ = screenPick ? occludedUnitAt(wx, wy) : null;');
  const ground = body.indexOf('let best = null;');
  assert.ok(ram >= 0 && occ > ram, 'the occluded candidate runs after the Ram-body pick');
  assert.ok(ground > occ, 'the occluded candidate runs ahead of the ground pick');
  assert.match(body, /if \(occ !== null\) return occ;/, 'an outline hit is the target');
});

test('the screen pick is canvas-only: orderAt threads its canvas flag', () => {
  // The canopy doctrine (SEL-16): screen-space picks belong to
  // the main canvas; the minimap draws no outlines and its
  // pick stays the plain ground pick.
  assert.match(appJs, /let t = entityAt\(wx, wy, fromCanvas\);/,
    'orderAt passes its fromCanvas flag into entityAt');
  assert.match(appJs, /smartOrderAt\(wx, wy, e\.shiftKey, true\);/,
    'the main-canvas right-click sets the flag');
  assert.match(appJs, /smartOrderAt\(w\[0\], w\[1\], ev\.shiftKey\);/,
    'the minimap right-click does NOT set the flag');
});

test('left click: the outline pick runs ahead of the animal/building picks', () => {
  const body = sliceFn(appJs, '// Plain click.', "canvas.addEventListener('dblclick'");
  const occ = body.indexOf('const occ = occludedUnitAt(wx, wy);');
  const animal = body.indexOf('const a = animalAt(wx, wy);');
  const tc = body.indexOf('const tc = tcAt(wx, wy);');
  assert.ok(occ >= 0 && animal > occ, 'the outline is consulted before animalAt');
  assert.ok(tc > occ, 'the outline is consulted before tcAt — the building no longer wins by default');
  assert.match(body, /if \(isOwnUnit\(entityState\(\), occ\)\) \{\s*selectUnits\(new Set\(\[occ\]\)\);/,
    'our own occluded unit becomes a real selection');
  assert.match(body, /selectAnimal\(occ\);/,
    'anyone else\'s occluded unit becomes an information selection (the animal path\'s machinery)');
});

test('the info panel gives an enemy unit its own line, in all three languages', () => {
  assert.match(appJs, /t\('sel\.enemyUnitInfo', \{ hp: hpTxt \}\)/,
    'the animal-context panel routes non-animal info selections to the enemy-unit line');
  assert.equal((i18nJs.match(/'sel\.enemyUnitInfo':/g) || []).length, 3,
    'sel.enemyUnitInfo exists in EN, PT and ES');
});
