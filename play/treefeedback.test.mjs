// Tree hit-feedback regression pins (Claudio's live report,
// 2026-10-24: "uma vez eu vi funcionando uma árvore caindo ou
// tremendo quando está cortando, mas já não tem mais").
//
// What died: the per-strike SHAKE. The forest pass (2026-10-20)
// fires the wobble/chips/chop-sound from updateVilAnim's gather
// state, which is gated on the client-side at-node heuristic in
// updateCarry. That gate (AT_NODE_R2 = 2.6²) was calibrated
// (2026-10-16) for villagers who walked to the node POINT and
// stopped within 2.5 units. M7 (2026-10-18) moved the tree
// stand to the ring cell's CENTER — measured stops 4.06–5.66
// units from the trunk — outside the gate forever after: the
// cutter never registered as gathering at his tree, so no
// wobble, no chips, no chop sound ever fired on the common
// open-ground approach. The 2026-10-24 fix moved the sim's
// stand point to the trunk (2.5 orthogonal / ≈3.33 diagonal),
// gave the tree order a tight steering arrival radius
// (steer::TREE_ARRIVE = 0.5, so the gather flip certifies at
// ≤3.0 / ≤3.83), and gave trees their own client gate
// (AT_TREE_R2 = 3.9²) with a sticky hold (AT_TREE_STICK_R2 =
// 4.8²) for crewmate shove drift.
//
// These pins keep the two layers' geometry from drifting
// apart again: the client gate is checked against the SIM's
// stand distances, parsed from the sim sources themselves.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const econRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'econ.rs'), 'utf8');
const libRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'lib.rs'), 'utf8');
const steerRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'steer.rs'), 'utf8');

function constValue(src, name) {
  const m = src.match(new RegExp(`const ${name} = ([0-9.]+) \\* ([0-9.]+);`));
  assert.ok(m, `${name} found in app.js`);
  return Number(m[1]);
}

function bitsValue(src, name) {
  const m = src.match(new RegExp(`${name}[^;]*from_bits\\((\\d[\\d_]*)\\)`));
  assert.ok(m, `${name} found in the sim sources`);
  return Number(m[1].replaceAll('_', '')) / 65536;
}

// The sim's woodcutter stand distances from the trunk: the cell
// boundary (2.0 orthogonal, 2√2 diagonal) plus TREE_STAND_INSET.
const inset = bitsValue(econRs, 'TREE_STAND_INSET');
const diagBoundary = (() => {
  const m = libRs.match(/I16F16::from_bits\((185_364)\)/);
  assert.ok(m, 'the diagonal boundary constant (2√2) is in tree_stand_point');
  return Number(m[1].replaceAll('_', '')) / 65536;
})();
// The tree order's steering arrival radius and the gather
// flip's per-cell stand limits (stand + arrival radius).
const treeArrive = bitsValue(steerRs, 'TREE_ARRIVE');
const flipLimitDiag = 251000 / 65536; // pinned literally in lib.rs
const flipLimitOrth = 196608 / 65536;
const standOrth = 2.0 + inset;
const standDiag = diagBoundary + inset;

test('sim stand geometry is the trunk geometry the fix promises', () => {
  assert.ok(Math.abs(standOrth - 2.5) < 0.01, `orthogonal stand ${standOrth}`);
  assert.ok(standDiag > 3.3 && standDiag < 3.36, `diagonal stand ${standDiag}`);
  // The tree order carries the tight arrival radius, and steer
  // applies a per-order final reach (the generic 1.0 radius
  // completed the order ~4.3 units out — the original defect).
  assert.ok(Math.abs(treeArrive - 0.5) < 0.01, `tree arrival radius ${treeArrive}`);
  assert.match(steerRs, /final_reach: I16F16/, 'steer takes a per-order final reach');
  assert.match(libRs, /arrive: steer::TREE_ARRIVE,/, 'the tree order carries TREE_ARRIVE');
  // The arrival rule: ring-cell ENTRY alone must not certify —
  // only range, the per-cell stand limit beside the tree, or a
  // completed order beside the tree (bounded, crowded case).
  assert.match(
    libRs,
    /let at_tree_stand = beside_tree\s+&& \(d <= stand_limit/,
    'work_tick arrival requires the stand limit beside the tree',
  );
  assert.match(
    libRs,
    /I16F16::from_bits\(251_000\)/,
    'the diagonal flip limit is pinned in lib.rs',
  );
  assert.match(
    libRs,
    /I16F16::from_bits\(196_608\)/,
    'the orthogonal flip limit is pinned in lib.rs',
  );
  // The flip limits are exactly stand + arrival radius: the
  // flip can fire the moment steering settles him at the stand,
  // never a generic radius earlier.
  assert.ok(Math.abs(flipLimitDiag - (standDiag + treeArrive)) < 0.02, `diagonal flip limit ${flipLimitDiag}`);
  assert.ok(Math.abs(flipLimitOrth - (standOrth + treeArrive)) < 0.02, `orthogonal flip limit ${flipLimitOrth}`);
});

test('client at-node gate covers the trunk stands (and only just)', () => {
  const gate = constValue(appJs, 'AT_TREE_R2');
  const plain = constValue(appJs, 'AT_NODE_R2');
  const stick = constValue(appJs, 'AT_TREE_STICK_R2');
  // The tree gate must contain BOTH flip limits — the
  // historical failure was a gate (2.6) below the diagonal
  // stand (≈3.33); a cutter certified at 3.83 must register.
  assert.ok(gate > flipLimitDiag, `tree gate ${gate} must exceed the diagonal flip limit ${flipLimitDiag}`);
  assert.ok(plain < standDiag, 'the plain gate stays below the diagonal stand (trees need their own)');
  // ...but stay under the old ring-cell-center distance (4.0):
  // the gate must not swallow a villager standing a full tile out.
  assert.ok(gate < 4.0, `tree gate ${gate} must stay under 4.0`);
  // Hysteresis: the sticky hold is wider than the entry gate,
  // and narrower than two full tiles.
  assert.ok(stick > gate && stick <= 5.0, `sticky gate ${stick}`);
});

test('the old failure, numerically: the pre-fix stop was outside every gate', () => {
  // Measured pre-fix stops (scratch probe, seeds 1/3/7/42):
  // 4.06 .. 5.66 units. Even the closest of them is outside the
  // tree gate — which is why the fix had to move the SIM's stand
  // point, not only widen the client gate.
  const gate = constValue(appJs, 'AT_TREE_R2');
  assert.ok(4.06 > gate, 'closest pre-fix stop still outside the gate');
});

test('hit feedback is wired: gather impact frame -> noteTreeHit on trees only', () => {
  assert.match(appJs, /if \(state === 'gather' && frame === GATHER_IMPACT_FRAME\s+&& gTarget && gTarget\.kind === 0\) \{\s+noteTreeHit\(gTarget, wx, wy, now\);/);
  // noteTreeHit records the wobble + spawns chips.
  assert.match(appJs, /function noteTreeHit\(node, vx, vy, now\) \{[\s\S]*?treeWobble\.set\(key, now\);[\s\S]*?woodChips\.push/);
  // drawTree turns a recent hit into the decaying shake.
  assert.match(appJs, /const hitAt = treeWobble\.get\(key\);[\s\S]*?if \(age < 240\)/);
  // updateCarry applies the per-kind gate (trees wider).
  assert.match(appJs, /const r2 = n\.kind === 0 \? AT_TREE_R2 : AT_NODE_R2;/);
  assert.match(appJs, /AT_TREE_STICK_R2/, 'the sticky trunk hold is in place');
});

test('the shake is VISIBLE at default zoom (bot-loop, 2026-10-25)', () => {
  // The wiring pins above cannot catch a shake that exists but is
  // too small to see — exactly what happened: 0.09 units / 0.02 rad
  // is 0.45 px at INITIAL_S (5 px/unit), and three live sessions
  // reported a perfectly still tree. Pin the peak on-screen
  // displacement of the wobble at the default zoom instead.
  const sM = appJs.match(/const INITIAL_S = ([0-9.]+);/);
  assert.ok(sM, 'INITIAL_S found in app.js');
  const S = Number(sM[1]);
  const hM = appJs.match(/const TREE_H = ([0-9.]+);/);
  assert.ok(hM, 'TREE_H found in app.js');
  const H = Number(hM[1]);
  const oxM = appJs.match(/ox = Math\.sin\(age \/ 22\) \* ([0-9.]+) \* u \* d;/);
  assert.ok(oxM, 'wobble shift amplitude found in drawTree');
  const wobM = appJs.match(/wob = Math\.sin\(age \/ 22\) \* ([0-9.]+) \* d;/);
  assert.ok(wobM, 'wobble sway amplitude found in drawTree');
  const shiftPx = Number(oxM[1]) * S;
  const topPx = Math.sin(Number(wobM[1])) * H * S;
  assert.ok(shiftPx >= 1.2, `trunk shift ${shiftPx.toFixed(2)} px at default zoom must read on screen`);
  assert.ok(topPx >= 1.5, `treetop swing ${topPx.toFixed(2)} px at default zoom must read on screen`);
});

test('the fall is wired: a tree leaving the live set tips over, with its sound', () => {
  assert.match(appJs, /function updateTreeTracking\(nodes, now\) \{[\s\S]*?fallingTrees\.push\(\{/);
  assert.match(appJs, /playSfx\('work\.treefall', gain\)/);
  // And it runs every frame over the node list.
  assert.match(appJs, /updateTreeTracking\(nodes, performance\.now\(\)\);/);
  // The fall leaves a stump.
  assert.match(appJs, /stumpSites\.push\(\{ x: f\.x, y: f\.y, t0: now \}\);/);
});

test('the felled tree LIES on the ground before becoming a stump (bot-loop, 2026-10-25)', () => {
  // DE's sequence (Claudio's 2026-10-25 decision): standing ->
  // fallen on the ground -> stump. The old code faded the tree
  // out DURING its 500 ms tip-over and removed it 120 ms later,
  // so the "fallen" state effectively never reached the screen
  // (every bot-loop live session saw trees simply vanish).
  const fallenM = appJs.match(/const FALLEN_MS = ([0-9_]+);/);
  assert.ok(fallenM, 'FALLEN_MS found in app.js');
  const fallenMs = Number(fallenM[1].replaceAll('_', ''));
  assert.ok(fallenMs >= 2000, `the felled tree must lie long enough to be seen (${fallenMs} ms)`);
  // Removal and stump spawn both wait for the full fall + dwell.
  assert.match(appJs, /now - f\.t0 > FALL_MS \+ FALLEN_MS && !f\.stumpDone/);
  assert.match(appJs, /now - f\.t0 > FALL_MS \+ FALLEN_MS\) fallingTrees\.splice/);
  assert.doesNotMatch(appJs, /FALL_MS \+ 120/, 'the old instant removal is gone');
  // The falling draw must not fade the tree away mid-fall.
  const fallFn = appJs.match(/function drawFallingTree\(f, now\) \{[\s\S]*?\n\}/);
  assert.ok(fallFn, 'drawFallingTree found');
  assert.doesNotMatch(fallFn[0], /alpha/, 'drawFallingTree draws at full opacity');
});
