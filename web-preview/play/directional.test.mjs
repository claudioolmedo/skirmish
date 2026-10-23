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

// unit -> { state: frames } — the §10.1 sheet contract.
const SHEETS = {
  villager: { idle: 4, walk: 8, gather: 4, carry: 8, build: 4 },
  archer: { idle: 4, walk: 8, shoot: 4 },
  militia: { idle: 4, walk: 8, attack: 4 },
  maa: { idle: 4, walk: 8, attack: 4 },
  sheep: { idle: 4, walk: 8, graze: 4 },
  boar: { idle: 4, walk: 8, graze: 4 },
  deer: { idle: 4, walk: 8, graze: 4 },
  wolf: { idle: 4, walk: 8, attack: 4 },
};
const MASKED = ['villager', 'archer', 'militia', 'maa', 'scout'];

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

test('scout sheets: 160px cells (idle 4, gallop 8) + masks', () => {
  assert.deepEqual(pngSize(join(sheetsDir, 'scout-idle.png')), [4 * 160, 8 * 160]);
  assert.deepEqual(pngSize(join(sheetsDir, 'scout-gallop.png')), [8 * 160, 8 * 160]);
  assert.ok(existsSync(join(sheetsDir, 'scout-idle-pcmask.png')));
  assert.ok(existsSync(join(sheetsDir, 'scout-gallop-pcmask.png')));
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
