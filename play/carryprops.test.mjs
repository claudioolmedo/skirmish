// Visible-carry pins (Claudio's request, 2026-10-25: "quando
// carreguem, carreguem as madeiras, ou o ouro, ou a carne… e
// quando voltam, voltam vazio").
//
// The rule being pinned, end to end: a villager the SIM says is
// hauling a resource is drawn with that resource overhead (logs /
// gold / stone / meat — one cargo sheet per resource out of the
// sprite pipeline), and the moment the sim's carry empties (the
// deposit, at the TC or any camp) the same villager draws with
// the plain empty-handed walk. Three layers, each pinned where
// it lives:
//   1. the sim's word — a read-only WASM accessor (entity_carry)
//      reporting carry_kind per entity, plus the econ invariant
//      it relies on (carry_kind is None iff carry is zero);
//   2. the client's pick — the sheet key comes from that true
//      carry for movement states, never from the position
//      heuristic (which still drives gather presentation and the
//      drop-off button, and is pinned as still present); the
//      kind dot follows the same true state;
//   3. the art — one cargo sheet per resource in every quality
//      tier, same 12-frame cycle as 'carry', player-color masks
//      identical to the generic carry mask (a prop carries no
//      player color).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const wasmRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'wasm.rs'), 'utf8');
const econRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'econ.rs'), 'utf8');
const entityRs = readFileSync(join(here, '..', '..', 'sim', 'src', 'entity.rs'), 'utf8');
const assets = join(here, '..', 'assets-original');

const RES = ['wood', 'gold', 'stone', 'food'];

function pngSize(path) {
  const b = readFileSync(path);
  assert.equal(b.toString('ascii', 1, 4), 'PNG', `${path} is a PNG`);
  return [b.readUInt32BE(16), b.readUInt32BE(20)];
}

// ------------------------------------------------ layer 1: the sim
test('wasm exposes the true carry, read-only, in entity order', () => {
  assert.match(wasmRs, /pub fn entity_carry\(&self\) -> Vec<u8>/,
    'entity_carry accessor exists');
  assert.match(
    wasmRs,
    /Some\(kind\) => kind\.index\(\) as u8,\s*None => 255,/,
    'a loaded villager reports its ResourceKind index; empty hands are 255');
});

test('ResourceKind index order is the one the client maps', () => {
  // econ.rs: Food 0, Wood 1, Gold 2, Stone 3 (stockpile order).
  for (const [name, idx] of [['Food', 0], ['Wood', 1], ['Gold', 2], ['Stone', 3]]) {
    assert.match(
      econRs,
      new RegExp(`ResourceKind::${name} => ${idx},`),
      `${name} has index ${idx}`);
  }
  // The invariant the whole feature stands on: a kind is set
  // exactly while something is carried.
  assert.match(entityRs, /What `carry` holds\. `None` iff `carry` is zero\./,
    'carry_kind is None iff carry is zero');
});

// ------------------------------------------------ layer 2: the pick
test('client maps resource tags to cargo sheets in index order', () => {
  assert.match(appJs, /const CARRY_RES_OF_TAG = \['food', 'wood', 'gold', 'stone'\];/,
    'tag order is ResourceKind::index order (Food, Wood, Gold, Stone)');
  assert.match(appJs, /const CARRY_RES_KEYS = \['carry-wood', 'carry-gold', 'carry-stone', 'carry-food'\];/,
    'one cargo sheet key per resource');
  // The dot reuses the node-kind color slots: wood 0 (tan),
  // food 1 (berry pink), gold 2, stone 3.
  assert.match(appJs, /const CARRY_DOT_KIND = \{ food: 1, wood: 0, gold: 2, stone: 3 \};/,
    'dot color slots per resource');
});

test('true carry is read from the sim, cached per tick, 255 = empty', () => {
  assert.match(appJs, /function carryResOf\(i\) \{[\s\S]*?sim\.entity_carry\(\)/,
    'carryResOf reads the ABI accessor');
  assert.match(appJs, /if \(tags\[j\] !== 255\) trueCarryRes\.set\(j, CARRY_RES_OF_TAG\[tags\[j\]\]\);/,
    'only loaded entities enter the map');
  assert.match(appJs, /if \(sim !== carryCacheSim \|\| t !== carryCacheTick\)/,
    'the cache is keyed on the sim instance and tick (no stale loads across matches)');
  assert.match(appJs, /return \{ state, dir: a\.dir, frame, carryRes: carryResOf\(i\) \};/,
    'the villager anim carries the true resource to the draw');
});

test('sheet pick: cargo when loaded, plain walk when empty, work states untouched', () => {
  assert.match(
    appJs,
    /function vilAnimSheetKey\(anim\) \{\s*if \(anim\.state !== 'walk' && anim\.state !== 'carry'\) return anim\.state;\s*return anim\.carryRes \? `carry-\$\{anim\.carryRes\}` : 'walk';\s*\}/,
    'movement states follow the true carry; gather/build/idle/attack keep their sheets');
  // Draw fallback order: cargo sheet, then the generic bundle,
  // then the pose's own sheet — a not-yet-loaded cargo sheet can
  // never blank the villager.
  assert.match(
    appJs,
    /if \(key\.startsWith\('carry-'\)\) tries\.push\('carry'\);/,
    'generic carry is the middle fallback');
});

test('the kind dot follows the same true carry — never empty hands', () => {
  assert.match(appJs, /if \(anim\.carryRes\) \{\s*\/\/ With a carry drawing/,
    'the dot block keys on the true carry, not the heuristic');
  assert.match(appJs, /ctx\.fillStyle = CARRY_COLORS\[CARRY_DOT_KIND\[anim\.carryRes\]\];/,
    'dot color comes from the carried resource');
});

test('the position heuristic survives for its own jobs', () => {
  // Gather presentation (atNodeNow / facing) and the drop-off
  // button still run on the heuristic; this feature must not
  // have eaten them.
  assert.match(appJs, /const carryKind = new Map\(\);/, 'heuristic map still exists');
  assert.match(appJs, /carryKind\.has\(id\) && issue\(\{ t: 'force_dep', e: id \}\)/,
    'force drop-off still decides from the heuristic');
  assert.match(appJs, /for \(const state of VIL_SHEET_KEYS\) \{/,
    'the tier loader fetches base states and cargo sheets alike');
  assert.match(appJs, /const VIL_SHEET_KEYS = \[\.\.\.Object\.keys\(VIL_ORIGINAL\.frames\)/,
    'VIL_SHEET_KEYS derives from the frame table');
});

// ------------------------------------------------ layer 3: the art
test('one cargo sheet per resource, every tier, on the carry cycle', () => {
  const frames = Number(appJs.match(/frames: \{[^}]*carry: (\d+)/)[1]);
  assert.equal(frames, 12, 'the carry cycle is 12 frames (client table)');
  for (const res of RES) {
    const [w, h] = pngSize(join(assets, `villager-carry-${res}.png`));
    assert.deepEqual([w, h], [frames * 128, 8 * 128], `${res}: base sheet is 12 frames x 8 dirs`);
    const [mw, mh] = pngSize(join(assets, `villager-carry-${res}-pcmask.png`));
    assert.deepEqual([mw, mh], [w, h], `${res}: mask twin matches the sheet`);
    const [w16, h16] = pngSize(join(assets, 'tier16', `villager-carry-${res}.png`));
    assert.deepEqual([w16, h16], [frames * 128, 16 * 128], `${res}: tier16 sheet is 12 frames x 16 dirs`);
    const [w32, h32] = pngSize(join(assets, 'tier32', `villager-carry-${res}.png`));
    assert.deepEqual([w32, h32], [frames * 128, 32 * 128], `${res}: tier32 sheet is 12 frames x 32 dirs`);
  }
});

test('the four cargoes are four different pictures', () => {
  const bytes = RES.map((res) => readFileSync(join(assets, `villager-carry-${res}.png`)));
  const generic = readFileSync(join(assets, 'villager-carry.png'));
  for (let a = 0; a < RES.length; a++) {
    assert.ok(!bytes[a].equals(generic), `${RES[a]} differs from the generic bundle`);
    for (let b = a + 1; b < RES.length; b++) {
      assert.ok(!bytes[a].equals(bytes[b]), `${RES[a]} differs from ${RES[b]}`);
    }
  }
});

test('a prop carries no player color', () => {
  // The mask system marks exactly the materials in the model's
  // PC_MATS; the villager's is the tunic alone, and no load —
  // generic or cargo — is built from it, so a prop can never be
  // repainted in a player's color (the foe's haulers wear the
  // same logs/gold/stone/meat as ours, on a red tunic).
  const renderPy = readFileSync(join(here, '..', '..', 'tools', 'sprite-pipeline', 'render.py'), 'utf8');
  const villagerPy = readFileSync(join(here, '..', '..', 'tools', 'sprite-pipeline', 'models', 'villager.py'), 'utf8');
  assert.match(villagerPy, /PC_MATS = \{'tunic'\}/, 'only the tunic carries player color');
  const loads = renderPy.slice(
    renderPy.search(/^LOAD_TRIS =/m), renderPy.search(/^LOAD_SETS =/m));
  assert.ok(!loads.includes("mat='tunic'"), 'no load is built from the tunic material');
  assert.match(renderPy, /'wood': _load_wood\(\), 'gold': _load_gold\(\),\s*'stone': _load_stone\(\), 'food': _load_food\(\)/,
    'LOAD_SETS wires one load builder per resource');
});
