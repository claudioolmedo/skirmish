// Panel layout pins — EVERY command panel's arrangement as a
// (cell letter → command id) snapshot. Claudio's report,
// 2026-10-23: the Town Center's badges read Q A W / S D because
// letters were assigned per command while the renderer filled
// the grid in code order — the two systems disagreed and the
// panel scrambled (the same defect sat in the Barracks and the
// Blacksmith). The fix (2026-10-23) places each button INTO
// the cell of its letter via panelCellFor(); this test fails
// loudly if any panel's arrangement moves again — in ANY
// context — so a future landing cannot reshuffle a panel
// silently. DE's grid rule: the hotkey IS the panel position
// (QWE / ASD / ZXC), per the project's own study frames of the
// reference game (2026-10-19) and the public hotkey references.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { panelCellFor, badgeForButton, PAGE_KINDS } from './hotkeys.js';

// Each layout: the ids the renderer puts on the panel, the
// cell each must occupy, and the ids that intentionally stay
// unplaced (CSS auto-flow — letters outside the 3×3 grid,
// like the Scout's G for Auto-Explore).
const LAYOUTS = {
  'town center (Dark Age — Wheelbarrow on the panel)': {
    context: 'tc', page: null,
    ids: ['train', 'research-loom', 'research-age', 'research-wheelbarrow', 'delete-building'],
    cells: { Q: 'train', A: 'research-loom', Z: 'research-age', S: 'research-wheelbarrow', C: 'delete-building' },
    unplaced: [],
  },
  'town center (after Wheelbarrow — Hand Cart takes the S cell, DE chain rule)': {
    context: 'tc', page: null,
    ids: ['train', 'research-loom', 'research-age', 'research-handcart', 'delete-building'],
    cells: { Q: 'train', A: 'research-loom', Z: 'research-age', S: 'research-handcart', C: 'delete-building' },
    unplaced: [],
  },
  barracks: {
    context: 'barracks', page: null,
    ids: ['train-militia', 'train-spearman', 'research-maa', 'delete-building'],
    cells: { Q: 'train-militia', W: 'train-spearman', A: 'research-maa', C: 'delete-building' },
    unplaced: [],
  },
  'archery range': {
    context: 'range', page: null,
    ids: ['train-archer', 'train-skirmisher', 'train-cavalry-archer', 'delete-building'],
    cells: { Q: 'train-archer', W: 'train-skirmisher', E: 'train-cavalry-archer', C: 'delete-building' },
    unplaced: [],
  },
  stable: {
    context: 'stable', page: null,
    ids: ['train-scout', 'train-knight', 'research-lightcavalry', 'research-bloodlines', 'research-husbandry', 'delete-building'],
    cells: { Q: 'train-scout', W: 'train-knight', A: 'research-lightcavalry', S: 'research-bloodlines', D: 'research-husbandry', C: 'delete-building' },
    unplaced: [],
  },
  blacksmith: {
    context: 'blacksmith', page: null,
    ids: ['research-fletching', 'research-padded', 'research-scalemail', 'research-forging', 'research-barding', 'delete-building'],
    cells: {
      Q: 'research-fletching', W: 'research-scalemail', E: 'research-forging',
      A: 'research-padded', S: 'research-barding', C: 'delete-building',
    },
    unplaced: [],
  },
  'lumber camp': {
    context: 'lumbercamp', page: null,
    ids: ['research-doublebit', 'delete-building'],
    cells: { Q: 'research-doublebit', C: 'delete-building' },
    unplaced: [],
  },
  mill: {
    context: 'mill', page: null,
    ids: ['research-horsecollar', 'delete-building'],
    cells: { Q: 'research-horsecollar', C: 'delete-building' },
    unplaced: [],
  },
  'mining camp': {
    context: 'miningcamp', page: null,
    ids: ['research-goldmining', 'research-stonemining', 'delete-building'],
    cells: { Q: 'research-goldmining', W: 'research-stonemining', C: 'delete-building' },
    unplaced: [],
  },
  market: {
    context: 'market', page: null,
    ids: ['trade-sell-food', 'trade-sell-wood', 'trade-sell-stone', 'trade-buy-food', 'trade-buy-wood', 'trade-buy-stone', 'delete-building'],
    cells: {
      Q: 'trade-sell-food', W: 'trade-sell-wood', E: 'trade-sell-stone',
      A: 'trade-buy-food', S: 'trade-buy-wood', D: 'trade-buy-stone', C: 'delete-building',
    },
    unplaced: [],
  },
  'military units (attack-move sits in the Q cell, badged with DE’s dedicated R)': {
    context: 'units', page: null,
    ids: ['attack-move', 'patrol', 'stance-0', 'stance-1', 'stance-2', 'stance-3', 'stop', 'auto-explore', 'delete'],
    cells: {
      Q: 'attack-move', W: 'patrol', E: 'stance-0',
      A: 'stance-1', S: 'stance-2', D: 'stance-3', X: 'stop', C: 'delete',
    },
    unplaced: ['auto-explore'],
  },
  'villager build root': {
    context: 'build', page: null,
    badges: false, // the openers' Q / W / E badges are pinned by hotkeys.smoke
    ids: ['page-eco', 'page-mil', 'repair', 'stop', 'delete'],
    cells: { Q: 'page-eco', W: 'page-mil', E: 'repair', C: 'delete' },
    unplaced: ['stop'],
  },
  'villager economic build page': {
    context: 'build', page: 'eco',
    ids: [...PAGE_KINDS.eco.map((k) => `build-${k}`), 'page-back', 'stop', 'delete'],
    cells: {
      Q: 'build-0', W: 'build-4', E: 'build-1',
      A: 'build-3', S: 'build-5', D: 'build-12', Z: 'build-13', C: 'page-back',
    },
    unplaced: ['stop', 'delete'],
  },
  'villager military build page': {
    context: 'build', page: 'mil',
    ids: [...PAGE_KINDS.mil.map((k) => `build-${k}`), 'page-back', 'stop', 'delete'],
    cells: {
      Q: 'build-2', W: 'build-6', E: 'build-7',
      A: 'build-8', S: 'build-9', D: 'build-10', Z: 'build-11', X: 'build-14', C: 'page-back',
    },
    unplaced: ['stop', 'delete'],
  },
};

// The one exception to "badge == cell letter": a building's
// own Delete button carries no badge at all. (Attack Move's
// slot letter here is Q, its cell; the R painted on the button
// is app.js's dedicated-key override, pinned by orders.smoke.)
const BADGE_EXCEPTIONS = { 'delete-building': null };

for (const [name, layout] of Object.entries(LAYOUTS)) {
  test(`panel layout pinned: ${name}`, () => {
    const actual = {};
    for (const id of layout.ids) {
      const cell = panelCellFor({ context: layout.context, page: layout.page, id });
      if (layout.unplaced.includes(id)) {
        assert.equal(cell, null, `${id} is intentionally unplaced (auto-flow)`);
        continue;
      }
      assert.ok(cell, `${id} must have a cell`);
      assert.equal(actual[cell], undefined, `cell ${cell} claimed twice (${actual[cell]} and ${id})`);
      actual[cell] = id;
    }
    assert.deepEqual(actual, layout.cells);
    if (layout.badges === false) return;
    // The badge on a placed button is its cell's letter.
    for (const [cell, id] of Object.entries(layout.cells)) {
      const badge = badgeForButton({ context: layout.context, page: layout.page, id });
      const expected = id in BADGE_EXCEPTIONS ? BADGE_EXCEPTIONS[id] : cell;
      assert.equal(badge, expected, `${id} sits in cell ${cell} and must be badged ${expected ?? '(nothing)'}`);
    }
  });
}

// Badge COST pins (bot-loop regression, 2026-10-24): the price
// printed on a train button's face must be the price the sim
// actually charges (sim/src/units.rs table; current DE values).
// The Militia badge read 60 food — the pre-update-141935 price —
// while the sim charged 50 and the button's own title said 50:
// a player with 50–59 food saw a price they could not trust.
// Every train button is pinned here so a cost change in the sim
// table (or a stray edit in app.js) fails loudly instead of
// drifting on the panel face.
const appJsCosts = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
function badgeCost(id) {
  const at = appJsCosts.indexOf(`id: '${id}'`);
  assert.ok(at >= 0, `${id} button exists in app.js`);
  const block = appJsCosts.slice(at, at + 500);
  const first = /costIcon: 'i-(\w+)', cost: (\d+)/.exec(block);
  assert.ok(first, `${id} carries a badge cost`);
  const second = /cost2Icon: 'i-(\w+)', cost2: (\d+)/.exec(block);
  const out = { [first[1]]: Number(first[2]) };
  if (second) out[second[1]] = Number(second[2]);
  return out;
}
test('train badge costs match the sim table', () => {
  assert.deepEqual(badgeCost('train'), { food: 50 });
  assert.deepEqual(badgeCost('train-militia'), { food: 50, gold: 20 });
  assert.deepEqual(badgeCost('train-spearman'), { food: 35, wood: 25 });
  assert.deepEqual(badgeCost('train-archer'), { wood: 25, gold: 45 });
  assert.deepEqual(badgeCost('train-skirmisher'), { food: 25, wood: 35 });
  assert.deepEqual(badgeCost('train-scout'), { food: 80 });
});
