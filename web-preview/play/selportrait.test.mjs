// Empty-selection portrait regression pins (bot-loop, 2026-11-07).
//
// Defect (live sightings #179 and #180 on the real client): after
// deselecting (portrait click or Esc), the selection panel read
// "No selection / Click a unit…" but a full-strength portrait image
// stayed visible in the portrait medallion, reading as the previous
// selection lingering — it even persisted across Restart into the
// fresh match (proof #180).
//
// Root cause: updateSelPanel's no-selection branch set the portrait
// to pngIcon('villager.png') and relied on the 'empty' class to dim
// it — but .sel-portrait.empty in ui.css only sets `color`, which
// dims SVG symbols (currentColor), not a PNG <img>. Since portraits
// became PNGs, the placeholder rendered at full strength.
//
// Fix: the no-selection branch renders NO icon — the medallion is
// simply empty, matching the "No selection" text beside it. These
// pins keep the branch icon-free and keep every real selection
// branch setting its portrait as before.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function noneBranch() {
  const marker = "selPortrait.classList.add('empty');";
  const start = appJs.indexOf(marker);
  assert.ok(start >= 0, 'the no-selection portrait branch exists');
  const end = appJs.indexOf('selInfo.textContent', start);
  assert.ok(end > start, 'the branch sets the info line after the portrait');
  return appJs.slice(start, end);
}

test('the no-selection branch renders no portrait icon', () => {
  const branch = noneBranch();
  assert.ok(
    branch.includes("selPortrait.innerHTML = '';"),
    'the empty selection must clear the portrait, not substitute an icon');
  assert.ok(
    !branch.includes('pngIcon('),
    'no PNG icon may be rendered for an empty selection ' +
    '(the .empty class cannot dim a PNG — live proofs #179/#180)');
  assert.ok(
    !branch.includes('<svg'),
    'no SVG icon may be rendered for an empty selection either');
});

test('real selection branches still set their portraits', () => {
  const clears = appJs.split("selPortrait.classList.remove('empty');").length - 1;
  assert.ok(clears >= 6,
    `every selection context removes the empty state (found ${clears})`);
  assert.ok(
    appJs.includes("selPortrait.innerHTML = pngIcon('town-center.png', 'ui-icon');"),
    'the Town Center branch still sets its portrait');
});
