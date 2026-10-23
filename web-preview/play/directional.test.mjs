// Directional sprite milestone (2026-10-19) — structural
// assertions: the directional sheets + player-color masks exist
// with the pipeline's geometry (rows = 8 directions, columns =
// frames, cell 128 — scout 160), the villager root panel carries
// Repair on E, and the fire/rubble hooks are wired in app.js.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { resolveHotkey } from './hotkeys.js';

const here = dirname(fileURLToPath(import.meta.url));
const sheetsDir = join(here, '..', 'assets-original');
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

// PNG dimensions from the IHDR chunk (bytes 16..24, big-endian).
function pngSize(path) {
  const buf = readFileSync(path);
  assert.equal(buf.toString('ascii', 1, 4), 'PNG', `${path} is a PNG`);
  return [buf.readUInt32BE(16), buf.readUInt32BE(20)];
}

// unit -> { state: frames } — the §10.1 sheet contract. The
// 2026-10-22 animation-density pass raised the most-seen cycles
// (walk 8 -> 12, work/attack/shoot 4 -> 8, gallop 8 -> 12); the
// die sheets live in DIE_SHEETS below (bigger cells).
const SHEETS = {
  villager: { idle: 4, walk: 12, gather: 8, carry: 12, build: 8 },
  archer: { idle: 4, walk: 12, shoot: 8 },
  militia: { idle: 4, walk: 12, attack: 8 },
  maa: { idle: 4, walk: 12, attack: 8 },
  spearman: { idle: 4, walk: 12, attack: 8 },
  skirmisher: { idle: 4, walk: 12, shoot: 8 },
  sheep: { idle: 4, walk: 8, graze: 4 },
  boar: { idle: 4, walk: 8, graze: 4 },
  deer: { idle: 4, walk: 8, graze: 4 },
};
const MASKED = ['villager', 'archer', 'militia', 'maa', 'scout', 'spearman', 'skirmisher'];

for (const [unit, states] of Object.entries(SHEETS)) {
  for (const [state, frames] of Object.entries(states)) {
    test(`${unit}-${state} sheet: ${frames} frames x 8 directions`, () => {
      const path = join(sheetsDir, `${unit}-${state}.png`);
      assert.ok(existsSync(path), `${unit}-${state}.png exists`);
      assert.deepEqual(pngSize(path), [frames * 128, 8 * 128]);
    });
    if (MASKED.includes(unit)) {
      test(`${unit}-${state} player-color mask exists, same geometry`, () => {
        const path = join(sheetsDir, `${unit}-${state}-pcmask.png`);
        assert.ok(existsSync(path), `${unit}-${state}-pcmask.png exists`);
        assert.deepEqual(pngSize(path), [frames * 128, 8 * 128]);
      });
    }
  }
}

test('scout sheets: 160px cells (idle 4, gallop 12) + masks', () => {
  assert.deepEqual(pngSize(join(sheetsDir, 'scout-idle.png')), [4 * 160, 8 * 160]);
  assert.deepEqual(pngSize(join(sheetsDir, 'scout-gallop.png')), [12 * 160, 8 * 160]);
  assert.ok(existsSync(join(sheetsDir, 'scout-idle-pcmask.png')));
  assert.ok(existsSync(join(sheetsDir, 'scout-gallop-pcmask.png')));
});

// Die sheets (animation pass 2026-10-22): 6 fall frames on the
// bigger death cells (foot units 232x208, scout 256x216 — the
// STATE_CANVAS values in tools/sprite-pipeline/models/*.py),
// each with its player-color mask twin.
const DIE_SHEETS = {
  villager: [232, 208], archer: [232, 208], militia: [232, 208],
  maa: [232, 208], spearman: [232, 208], skirmisher: [232, 208],
  scout: [256, 216],
};
for (const [unit, [cw, ch]] of Object.entries(DIE_SHEETS)) {
  test(`${unit}-die sheet: 6 frames x 8 directions on the death cell`, () => {
    assert.deepEqual(pngSize(join(sheetsDir, `${unit}-die.png`)), [6 * cw, 8 * ch]);
    assert.deepEqual(pngSize(join(sheetsDir, `${unit}-die-pcmask.png`)), [6 * cw, 8 * ch]);
  });
}

test('corpse system is wired in app.js (fog-gated, client-only)', () => {
  assert.match(appJs, /CORPSE_FADE_MS = 18000/);
  assert.match(appJs, /function updateCorpses\(st\)/);
  assert.match(appJs, /function drawCorpse\(site, now\)/);
  // The corpse draws are gated on the quality flag (LOW sheds
  // corpses first) inside the same ground-decal assembly as
  // the rubble sites.
  assert.match(appJs, /if \(!Q\.corpses\) continue;/);
});

test('E on the villager root panel resolves to the Repair button', () => {
  assert.deepEqual(
    resolveHotkey({ context: 'build', page: null, code: 'KeyE' }),
    { type: 'button', id: 'repair' });
  // The page letters are untouched: Q/W still arm the pages, and
  // E on an armed page still builds (eco: Farm; mil: Stable).
  assert.deepEqual(
    resolveHotkey({ context: 'build', page: null, code: 'KeyQ' }),
    { type: 'arm', page: 'eco' });
  assert.deepEqual(
    resolveHotkey({ context: 'build', page: 'eco', code: 'KeyE' }),
    { type: 'button', id: 'build-1' });
  assert.deepEqual(
    resolveHotkey({ context: 'build', page: 'mil', code: 'KeyE' }),
    { type: 'button', id: 'build-7' });
});

test('repair button is rendered in the villager root grid', () => {
  assert.match(appJs, /id: 'repair', icon: 'i-hammer'/);
  assert.match(appJs, /key: 'E', active: repairArm/);
});

test('fire overlay + rubble hooks are wired', () => {
  assert.match(appJs, /function drawBuildingFireAt/);
  assert.match(appJs, /drawBuildingFireAt\(fx, fy - /); // placed buildings
  assert.match(appJs, /drawBuildingFireAt\(sx, sy - 4\.4 \* camS, 4,/); // Town Center
  assert.match(appJs, /function updateRubble/);
  assert.match(appJs, /RUBBLE_FADE_MS = 20000/);
});

test('directional draw sits above the side-view fallback', () => {
  assert.match(appJs, /function drawDirUnit/);
  assert.match(appJs, /const orig = drawOriginalVillager\(anim, sx, sy\);\n  const ua = orig \? null : unitArtImage\('villager'/);
  assert.match(appJs, /loadDirArt\(\);\n      loadTierSprites\(Q\)/);
});
