// MAP-05 relief shading pins (bot-loop run #482, front 10 —
// presentation only). Live observation, run #480: with a zoomed
// view of the hills, elevation read only as flat colour steps
// separated by dark contour lines — no relief, no lit/shadow
// sides, unlike the DE where a hill has a bright face and a
// shaded face. The fix is a directional hillshade in the ground
// pass of app.js: light at screen upper-left (grid -x, with a
// -y lean), plus a two-face step (SE face darker than SW).
//
// These pins keep the three properties from drifting:
//  1. the relief term's direction and clamp, evaluated on
//     synthetic neighbourhoods (flat -> exactly 0, west flank
//     lit, east flank shaded, SE-facing darkest);
//  2. the step's SE face strictly darker than its SW face;
//  3. the projection is untouched (ELEV_LIFT stays 1.1) and the
//     per-level top lightening still applies under the relief.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('relief term: direction, clamp and flat-zero', () => {
  const m = appJs.match(/const relief = Math\.max\((-?[0-9.]+), Math\.min\((-?[0-9.]+),\s*([^;]+?)\)\);/s);
  assert.ok(m, 'relief term not found in the ground pass');
  const lo = parseFloat(m[1]);
  const hi = parseFloat(m[2]);
  assert.equal(lo, -0.30, 'relief clamp low');
  assert.equal(hi, 0.30, 'relief clamp high');
  const expr = new Function('hE', 'hW', 'hS', 'hN', `return (${m[3]});`);
  const relief = (hE, hW, hS, hN) => Math.max(lo, Math.min(hi, expr(hE, hW, hS, hN)));
  // Flat ground: exactly zero — plains render as before.
  assert.equal(relief(2, 2, 2, 2), 0);
  assert.equal(relief(0, 0, 0, 0), 0);
  // West flank of a hill (height rises toward +x grid, downhill
  // faces screen NW, toward the light): lit.
  assert.ok(relief(3, 1, 2, 2) > 0, 'west flank must be lit');
  // East flank (downhill faces screen SE, away from the light):
  // shaded, and darker than a merely SW-facing slope.
  assert.ok(relief(1, 3, 2, 2) < 0, 'east flank must be shaded');
  assert.ok(relief(1, 3, 2, 2) < relief(2, 2, 1, 3), 'SE-facing must be darkest');
  // Legibility floor (run #484, reliefprobe measurement): a
  // one-level E/W slope must clear the ground checker's own
  // delta (+5 on green) by a wide margin — at the old 0.08
  // coefficient it rendered +5, exactly the noise floor, and
  // the modal slope rendered +3, below it. 0.24 puts the
  // canonical slope at 0.16 (~+15 on green).
  assert.ok(relief(2, 1, 1, 1) > 0.10 && relief(2, 1, 1, 1) < 0.30);
  assert.equal(relief(6, 0, 6, 0), 0.30, 'clamp caps extreme slopes');
});

test('step faces: SE strictly darker than SW', () => {
  const sw = appJs.match(/poly\(\[\[cx - hw, cy\][^\]]*\][^;]*?shade\(base, (-?[0-9.]+)\)\)/s);
  const se = appJs.match(/poly\(\[\[cx, cy \+ hh\], \[cx \+ hw, cy\][^;]*?shade\(base, (-?[0-9.]+)\)\)/s);
  assert.ok(sw, 'SW step face not found');
  assert.ok(se, 'SE step face not found');
  assert.ok(parseFloat(se[1]) < parseFloat(sw[1]),
    `SE face (${se[1]}) must shade deeper than SW face (${sw[1]})`);
});

test('step faces: drawn only across a real drop, depth = drop', () => {
  // reliefprobe raster replication: faces whose front neighbour
  // is at the same level survive 0.0% — drawing them is waste,
  // and a fixed 1-level depth understates drops of 2+. The SW
  // face (neighbour hS) and SE face (neighbour hE) are each
  // conditional on a strict drop and scaled by it.
  assert.match(appJs, /if \(hS < hgt\) poly\(/,
    'SW step face must be conditional on the S neighbour being lower');
  assert.match(appJs, /if \(hE < hgt\) poly\(/,
    'SE step face must be conditional on the E neighbour being lower');
  assert.match(appJs, /\(hgt - hS\) \* liftPx/,
    'SW step face depth must scale with the drop');
  assert.match(appJs, /liftPx \* \(hgt - hE\)/,
    'SE step face depth must scale with the drop');
});

test('projection and level lightening untouched', () => {
  assert.match(appJs, /const ELEV_LIFT = 1\.1;/, 'ELEV_LIFT must stay 1.1');
  assert.match(appJs, /shade\(base, hgt \* 0\.09 \+ relief\)/,
    'raised tops keep the per-level lightening, plus relief');
  assert.match(appJs, /diamond\(cx, cy, hw, hh, shade\(base, relief\)\)/,
    'flat cells take the relief term alone (zero on plains)');
});
