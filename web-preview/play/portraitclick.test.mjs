// Group-portrait click regression pins (bot-loop, 2026-10-30).
//
// Defect: in a group selection the bottom panel rendered one
// portrait cell per unit as an inert <span> — clicking a portrait
// did nothing and the group stayed selected. Two independent live
// sessions on the 726e8ed build (959c8288 and 9160971d) reported it
// unprompted. In DE, clicking one portrait in the group grid
// narrows the selection to just that unit.
//
// These pins keep the cells clickable: each group cell is a
// <button> whose click selects exactly its own unit through the
// shared selectUnits() funnel (so the SEL-05 cap and the
// selection invariants still apply).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function groupCellBlock() {
  const start = appJs.indexOf('const shown = selUnits.slice(0, 12);');
  assert.ok(start >= 0, 'the group-cell render block exists in app.js');
  const end = appJs.indexOf('selUnits.length > shown.length', start);
  assert.ok(end > start, 'the group-cell block is delimited');
  return appJs.slice(start, end);
}

test('group portrait cells are buttons, not inert spans', () => {
  const block = groupCellBlock();
  assert.ok(
    block.includes("document.createElement('button')"),
    'each group cell is created as a button');
  assert.ok(
    !block.includes("document.createElement('span')"),
    'no inert span cell remains in the group grid');
});

test('clicking a group portrait narrows the selection to that unit', () => {
  const block = groupCellBlock();
  assert.ok(
    block.includes('const cellId = shown[i];'),
    'each cell captures its own unit id');
  assert.ok(
    block.includes("cell.addEventListener('click'"),
    'each cell carries a click handler');
  assert.ok(
    block.includes('selectUnits(new Set([cellId]))'),
    'the click selects exactly that one unit via selectUnits');
});
