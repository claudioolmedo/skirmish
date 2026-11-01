// Order-arm banner pin (bot-loop, 2026-11-01) — regression for the
// live-observed banner bug (session 3cba1a49): arming Attack Ground
// flashed "Patrol: click a point — the selected units patrol…"
// because the arm handler in app.js flashed TWICE — first the
// correct three-way text (attackMove / attackGround / patrol), then
// a leftover two-way flash (attackMove / patrol) that overwrote it
// and had no attack-ground case at all. The duplicate flash is
// removed; this pin keeps the arm block at exactly one flash whose
// ternary covers all three order kinds.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('order-arm handler flashes exactly once, with the attack-ground case', () => {
  const start = appJs.indexOf("id === 'attack-move' || id === 'patrol' || id === 'attack-ground'");
  assert.ok(start >= 0, 'order-arm handler found');
  const block = appJs.slice(start, appJs.indexOf("playAudioCue('click')", start));
  const flashes = block.match(/flash\(t\(/g) || [];
  assert.equal(flashes.length, 1,
    'exactly one arm banner flash (a second one overwrites the first)');
  assert.match(block, /kind === 'attackground' \? 'flash\.attackGroundArm'/,
    'the single flash covers the attack-ground kind');
});
