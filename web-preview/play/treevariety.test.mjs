// Tree variety regression pins (bot-loop front 6, Claudio's
// order 2026-10-28: "fazer vários tipos diferentes de árvores,
// para ficar mais parecido ao DE — todas as árvores são
// iguais").
//
// Before this front every tree drew the same widened-pine
// sprite; the only per-tree differences were a flip, a ±8%
// scale and a 10% tint bucket — live, every forest still read
// as one stamped tree. Now four TYPES with distinct
// silhouettes/tones (classic, spire, broad, round) are assigned
// per tree. These pins keep: (1) the assignment deterministic
// (position hash — a tree must never change type between
// frames, damage stages or its fall); (2) every type's art on
// the closed front-1 contract (same 300x393 canvas as the
// classic, so TREE_H/TREE_WIDE draw math and the filled-tile
// read are unchanged); (3) the damage-stage sequence present
// for every type; (4) the change presentation-only (no sim
// file is part of it — treefeedback.test.mjs pins the sim
// stand geometry separately and stays untouched).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const nodesDir = join(here, '..', 'assets-original', 'nodes');

const TYPES = [
  ['tree', 'treeSpire', 'treeBroad', 'treeRound'],
  ['tree.png', 'tree-spire.png', 'tree-broad.png', 'tree-round.png'],
];

test('four tree types are loaded as node art, each with both damage stages', () => {
  for (const stem of ['tree-spire', 'tree-broad', 'tree-round']) {
    for (const suffix of ['', '-dmg1', '-dmg2']) {
      assert.match(appJs, new RegExp(`'${stem}${suffix}\\.png'`), `${stem}${suffix}.png is in NODE_ART_FILES`);
      assert.ok(existsSync(join(nodesDir, `${stem}${suffix}.png`)), `${stem}${suffix}.png exists`);
    }
  }
});

test('every variant art file is on the classic canvas contract (300x393)', () => {
  // Same canvas => same aspect => the filled-tile draw math
  // pinned by treefeedback.test.mjs holds for every type.
  for (const stem of ['tree', 'tree-spire', 'tree-broad', 'tree-round']) {
    for (const suffix of ['', '-dmg1', '-dmg2']) {
      const png = readFileSync(join(nodesDir, `${stem}${suffix}.png`));
      assert.equal(png.readUInt32BE(16), 300, `${stem}${suffix}.png width`);
      assert.equal(png.readUInt32BE(20), 393, `${stem}${suffix}.png height`);
    }
  }
});

test('the type assignment is a deterministic position hash (presentation only)', () => {
  assert.match(appJs, /function treeVariant\(x, y\) \{\s+return Math\.floor\(treeHash\(x, y, 21\) \* TREE_VARIANTS\.length\);/,
    'treeVariant derives the type from treeHash(x, y, 21)');
  assert.match(appJs, /const TREE_VARIANTS = \[/, 'the variant table exists');
  for (const key of TYPES[0]) assert.match(appJs, new RegExp(`'${key}'`), `${key} is in TREE_VARIANTS`);
  // Standing and falling draws agree on the type, or a tree
  // would change species mid-fall.
  assert.match(appJs, /treeStageImage\(frac, treeVariant\(wx, wy\)\)/, 'drawTree uses its tree variant');
  assert.match(appJs, /treeStageImage\(f\.stage === 0 \? 1 : f\.stage === 1 \? 0\.5 : 0\.2, treeVariant\(f\.x, f\.y\)\)/,
    'drawFallingTree uses the same variant for the same position');
});

test('an unloaded variant falls back to the classic set, never to nothing', () => {
  const fn = appJs.match(/function treeStageImage\(frac, variant = 0\) \{[\s\S]*?\n\}/);
  assert.ok(fn, 'treeStageImage found');
  assert.match(fn[0], /NODE_ART\.img\.tree/, 'classic fallback is in treeStageImage');
});
