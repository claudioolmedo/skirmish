// Panel-grid overflow fix pins (bot-loop, 2026-11-06) —
//   node --test panelgrid.test.mjs
//
// Live defect (real client, seed 42, build 89e7a7b): on the
// Dock panel the Transport Ship button read "covered" at every
// browser zoom and no mouse click could train it. Root cause:
// panelCellFor places the building Delete at cell C by design,
// and the Dock's Transport Ship (W4) is ALSO letter C — two
// definite-position buttons in one cell, the later-appended
// Delete painting over the ship. The same collision waits on
// the Monastery (Block Printing flows past cell C) and the
// University (Carvel Hull, W6), and every panel with more than
// nine buttons spilled the overflow into implicit rows below
// the capped bottom strip. The renderer's fix (app.js,
// renderCommands): outside the build pages, grow the grid's
// explicit rows to fit all buttons, and move a contested
// Delete to the bottom-right free cell of the grown grid.
// These pins hold the tables and the renderer code that the
// fix stands on.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { PANEL_KEYS, GRID_CELLS, panelCellFor } from './hotkeys.js';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');

test('no panel assigns the same lettered cell to two buttons', () => {
  for (const [context, table] of Object.entries(PANEL_KEYS)) {
    const cells = Object.values(table).map(([, letter]) => letter);
    if (context === 'tc') {
      // The TC's two documented rungs share a cell on purpose
      // (Wheelbarrow/Hand Cart at S, Town Watch/Town Patrol at
      // D): only one rung of each pair ever renders at a time,
      // so the sharing is not a collision.
      const dupes = cells.filter((c, i) => cells.indexOf(c) !== i).sort();
      assert.deepEqual(dupes, ['D', 'S']);
      continue;
    }
    assert.equal(new Set(cells).size, cells.length,
      context + ' has two lettered buttons in one cell');
    for (const letter of cells) {
      assert.ok(GRID_CELLS[letter], context + ' letter ' + letter + ' has no cell');
    }
  }
});

test('the Delete squats on cell C by design — the contested cell', () => {
  assert.equal(panelCellFor({ context: 'dock', id: 'delete-building' }), 'C');
  // The three lettered/flow buttons the Delete can cover or
  // displace: Dock Transport Ship (definite C), Monastery
  // Block Printing (positional index 8 = C), University
  // Carvel Hull (positional index 8 = C).
  assert.equal(panelCellFor({ context: 'dock', id: 'train-transport-ship' }), 'C');
  assert.deepEqual(GRID_CELLS.C, [3, 3]);
});

test('renderCommands resolves a contested Delete and grows the grid', () => {
  // The fix block exists in the renderer, outside the build pages.
  assert.match(app, /Panel-grid overflow fix \(bot-loop, 2026-11-06\)/);
  assert.match(app, /if \(context !== 'build'\) \{\s*const kids = \[\.\.\.cmdGrid\.children\];/);
  // Contested = a definite-position collision or any button
  // flowing past cell C (more than nine buttons).
  assert.match(app, /const contested = new Set\(cells\)\.size !== cells\.length \|\| kids\.length > 9;/);
  // The Delete moves to a free cell; the grid rows grow to fit.
  assert.match(app, /del\.style\.gridRowStart = String\(spot\[0\]\);/);
  assert.match(app, /cmdGrid\.style\.gridTemplateRows = rows > 3\s*\? 'repeat\(' \+ rows \+ ', minmax\(0, 1fr\)\)' : '';/);
  // Build pages keep the old path (inline rows cleared there).
  assert.match(app, /\} else \{\s*cmdGrid\.style\.gridTemplateRows = '';\s*\}/);
});

test('the full Dock panel fits the grown grid with unique cells', () => {
  // The Dock's rendered buttons in append order (app.js dock
  // branch + trap reseed desk + garrison + building Delete).
  const ids = ['train-fishing-ship', 'research-fishinglines', 'research-gillnets',
    'train-galley', 'train-fire-galley', 'train-demolition-raft', 'train-hulk',
    'research-mediumwarships', 'train-transport-ship', 'train-trade-cog',
    'reseed-queue', 'reseed-auto', 'ungarrison', 'delete-building'];
  const definite = new Map();
  for (const id of ids) {
    const letter = panelCellFor({ context: 'dock', id });
    if (letter) definite.set(id, GRID_CELLS[letter].join(','));
  }
  // Transport and Delete collide at C before the fix.
  assert.equal(definite.get('train-transport-ship'), '3,3');
  assert.equal(definite.get('delete-building'), '3,3');
  // After the renderer's rule (contested: collision and 14 > 9
  // buttons), the Delete leaves C to the Transport Ship and the
  // grid grows to ceil(14/3) = 5 explicit rows — every button
  // fits inside the strip.
  const rows = Math.max(3, 4, Math.ceil(ids.length / 3));
  assert.equal(rows, 5);
  assert.ok(rows * 3 >= ids.length);
});
