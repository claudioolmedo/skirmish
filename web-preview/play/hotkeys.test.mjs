// Headless tests for hotkeys.js (M13: the classic grid hotkeys +
// edge-scroll vector). Run: node --test play/hotkeys.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  PAGE_KINDS, PAGE_FOR_KIND, PAGE_KEYS, PAGE_BADGE, PANEL_BUTTONS,
  PANEL_KEYS, GOTO_KEYS, resolveHotkey, badgeForButton, edgePan,
} from './hotkeys.js';

const build = (over = {}) => resolveHotkey({ context: 'build', page: null, ...over });

test('Q arms the economic page, W the military page (villager context)', () => {
  assert.deepEqual(build({ code: 'KeyQ' }), { type: 'arm', page: 'eco' });
  assert.deepEqual(build({ code: 'KeyW' }), { type: 'arm', page: 'mil' });
});

test('pure grid: Q then Q raises a House — the letter is the slot', () => {
  assert.deepEqual(build({ page: 'eco', code: 'KeyQ' }), { type: 'button', id: 'build-0' });
  // E is the Farm's slot — it no longer doubles as House (the
  // 2026-10-18 exception was superseded by the pure grid).
  assert.deepEqual(build({ page: 'eco', code: 'KeyE' }), { type: 'button', id: 'build-1' });
});

test('economic page: slot letters Q W E / A S D, Back on C, silent aliases', () => {
  const eco = (code) => build({ page: 'eco', code });
  assert.equal(eco('KeyQ').id, 'build-0', 'House on Q (slot 0)');
  assert.equal(eco('KeyW').id, 'build-4', 'Mill on W (slot 1)');
  assert.equal(eco('KeyI').id, 'build-4', 'Mill alias I');
  assert.equal(eco('KeyE').id, 'build-1', 'Farm on E (slot 2)');
  assert.equal(eco('KeyF').id, 'build-1', 'Farm alias F');
  assert.equal(eco('KeyA').id, 'build-3', 'Lumber Camp on A (slot 3)');
  assert.equal(eco('KeyR').id, 'build-3', 'Lumber Camp alias R');
  assert.equal(eco('KeyZ').id, 'build-13', 'Town Center on Z — DE\u2019s own build letter (the Lumber Camp\u2019s silent Z alias died: the cell\u2019s letter rules)');
  assert.equal(eco('KeyS').id, 'build-5', 'Mining Camp on S (slot 4)');
  assert.equal(eco('KeyG').id, 'build-5', 'Mining Camp alias G (its go-to)');
  assert.equal(eco('KeyD').id, 'build-12', 'Market on D (slot 5 — DE\u2019s own build letter for it)');
  assert.equal(eco('KeyM').id, 'build-12', 'Market alias M (its go-to)');
  assert.deepEqual(eco('KeyC'), { type: 'button', id: 'page-back' }, 'Back on C (last slot)');
});

test('military page: slot letters Q W E / A, Back on C, silent aliases', () => {
  const mil = (code) => build({ page: 'mil', code });
  assert.equal(mil('KeyQ').id, 'build-2', 'Barracks on Q');
  assert.equal(mil('KeyB').id, 'build-2', 'Barracks alias B');
  assert.equal(mil('KeyW').id, 'build-6', 'Archery Range on W');
  assert.equal(mil('KeyE').id, 'build-7', 'Stable on E');
  assert.equal(mil('KeyL').id, 'build-7', 'Stable alias L');
  assert.equal(mil('KeyA').id, 'build-8', 'Blacksmith on A — the Range\u2019s old A alias is dropped');
  assert.equal(mil('KeyS').id, 'build-9', 'Palisade Wall on S (slot 4, the defensive row)');
  assert.deepEqual(mil('KeyC'), { type: 'button', id: 'page-back' }, 'Back on C (last slot)');
});

test('an armed page swallows its letters; other letters fall through', () => {
  assert.equal(build({ page: 'eco', code: 'KeyD' }).id, 'build-12', 'D places the Market');
  assert.equal(build({ page: 'eco', code: 'KeyT' }), null, 'T places nothing — the Town Center sits on Z, its DE build letter');
  assert.equal(build({ page: 'eco', code: 'KeyU' }), null, 'U still pans');
  assert.equal(build({ page: 'mil', code: 'KeyR' }), null);
  assert.deepEqual(build({ code: 'KeyE' }), { type: 'button', id: 'repair' }, 'unarmed E is Repair (BLD-17, the root panel’s third slot)');
  assert.equal(build({ code: 'KeyS' }), null, 'unarmed S still pans');
});

test('A with villagers and no page armed is the DE drop-off', () => {
  assert.deepEqual(build({ code: 'KeyA' }), { type: 'drop-off' });
  // On the armed economic page the same letter is the Lumber
  // Camp (its slot) — the panel context decides, as in the
  // reference game.
  assert.equal(build({ page: 'eco', code: 'KeyA' }).id, 'build-3');
});

test('panel slot letters run a selected building\u2019s buttons', () => {
  const at = (context, code) => resolveHotkey({ context, page: null, code });
  assert.equal(at('barracks', 'KeyQ').id, 'train-militia');
  // The Spearman takes DE's second Barracks slot (W); the
  // Man-at-Arms upgrade drops to the second row (A, under the
  // Militia) — the TC/Blacksmith two-row convention.
  assert.equal(at('barracks', 'KeyW').id, 'train-spearman');
  assert.equal(at('barracks', 'KeyA').id, 'research-maa');
  assert.equal(at('barracks', 'KeyE'), null, 'no third first-row button');
  assert.equal(at('range', 'KeyQ').id, 'train-archer');
  assert.equal(at('range', 'KeyW').id, 'train-skirmisher');
  assert.equal(at('stable', 'KeyQ').id, 'train-scout');
  // Castle P2: the Knight takes the Stable's second top-row
  // slot (W); the Cavalry Archer continues the Range's top
  // row (E).
  assert.equal(at('stable', 'KeyW').id, 'train-knight');
  assert.equal(at('range', 'KeyE').id, 'train-cavalry-archer');
  assert.equal(at('blacksmith', 'KeyQ').id, 'research-fletching');
  assert.equal(at('blacksmith', 'KeyW').id, 'research-scalemail');
  // Padded Archer Armor sits under Fletching in DE's panel:
  // second row, first slot — A, not E.
  assert.equal(at('blacksmith', 'KeyA').id, 'research-padded');
  // The Feudal completion set (2026-10-22): Forging completes
  // the Blacksmith's top row (E); Scale Barding Armor sits
  // under Scale Mail (S); the camps and the Mill are plain
  // top-row panels.
  assert.equal(at('blacksmith', 'KeyE').id, 'research-forging');
  assert.equal(at('blacksmith', 'KeyS').id, 'research-barding');
  // Castle P2 moved the Stable's researches to the second
  // row (the Barracks/TC convention): the Light Cavalry
  // upgrade under its own unit (A), Bloodlines under the
  // Knight (S), Husbandry next (D). Bloodlines sat at W
  // until the Knight took that slot.
  assert.equal(at('stable', 'KeyA').id, 'research-lightcavalry');
  assert.equal(at('stable', 'KeyS').id, 'research-bloodlines');
  assert.equal(at('stable', 'KeyD').id, 'research-husbandry');
  assert.equal(at('lumbercamp', 'KeyQ').id, 'research-doublebit');
  assert.equal(at('mill', 'KeyQ').id, 'research-horsecollar');
  assert.equal(at('miningcamp', 'KeyQ').id, 'research-goldmining');
  assert.equal(at('miningcamp', 'KeyW').id, 'research-stonemining');
});

test('Town Center letters are DE’s own: Villager Q, Loom A, Age Z, the Wheelbarrow line S', () => {
  // DE's grid letter is the button's panel position, and the TC
  // panel's arrangement is not our button order — it is the
  // reference panel's own, read off the 2026-10-19 player
  // study's frames of a selected TC (f_005/f_024, Dark Age;
  // f_052, Feudal researching): the first column stacks
  // Villager, Loom (its tooltip reads "(Hotkey: A)", frame
  // f_025) and the Age advancement; the Wheelbarrow icon sits
  // darkened at row 2, column 2 until Feudal. Hand Cart is that
  // same cell's next rung — the panel never shows both — so S
  // resolves to whichever rung is rendered. Scoped to the TC
  // context only: W/E/D run nothing here (D is Town Watch's
  // cell in DE — a tech we do not have), and the villager
  // context keeps its own A.
  const at = (code, rendered) => resolveHotkey({ context: 'tc', page: null, code, rendered });
  assert.equal(at('KeyQ').id, 'train', 'Create Villager — top-left of the TC panel');
  assert.equal(at('KeyA').id, 'research-loom', 'Loom — below Villager (study frame f_025)');
  assert.equal(at('KeyZ').id, 'research-age', 'Age advancement — below Loom, first column\u2019s foot');
  assert.equal(at('KeyW'), null, 'W is not a TC letter in DE');
  assert.equal(at('KeyE'), null, 'E is not a TC letter in DE');
  assert.equal(at('KeyD'), null, 'D is not a TC letter (Town Watch\u2019s cell in DE — a tech we do not have)');
  assert.equal(at('KeyS').id, 'research-wheelbarrow', 'Wheelbarrow — right of Loom');
  assert.equal(at('KeyS', ['train', 'research-loom', 'research-age', 'research-handcart']).id,
    'research-handcart', 'Hand Cart — the Wheelbarrow cell\u2019s next rung, once Wheelbarrow is in');
  assert.equal(at('KeyS', ['train', 'research-loom', 'research-age', 'research-wheelbarrow']).id,
    'research-wheelbarrow', '— and Wheelbarrow while it is the rung on the panel');
  // The badges painted on the buttons carry the same letters.
  assert.equal(badgeForButton({ context: 'tc', id: 'train', index: 0 }), 'Q');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-loom', index: 1 }), 'A');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-age', index: 2 }), 'Z');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-wheelbarrow', index: 3 }), 'S');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-handcart', index: 4 }), 'S');
  // Other panels keep their position badges.
  assert.equal(badgeForButton({ context: 'barracks', id: 'research-maa', index: 2 }), 'A');
  assert.equal(badgeForButton({ context: 'barracks', id: 'train-spearman', index: 1 }), 'W');
  assert.equal(badgeForButton({ context: 'range', id: 'train-skirmisher', index: 1 }), 'W');
});

test('market panel: sells on Q W E, buys on A S D (TRD-01..03)', () => {
  const at = (code) => resolveHotkey({ context: 'market', page: null, code });
  assert.deepEqual(at('KeyQ'), { type: 'button', id: 'trade-sell-food' });
  assert.deepEqual(at('KeyW'), { type: 'button', id: 'trade-sell-wood' });
  assert.deepEqual(at('KeyE'), { type: 'button', id: 'trade-sell-stone' });
  assert.deepEqual(at('KeyA'), { type: 'button', id: 'trade-buy-food' });
  assert.deepEqual(at('KeyS'), { type: 'button', id: 'trade-buy-wood' });
  assert.deepEqual(at('KeyD'), { type: 'button', id: 'trade-buy-stone' });
  assert.equal(at('KeyZ'), null, 'no third row on the market panel');
  assert.equal(badgeForButton({ context: 'market', id: 'trade-sell-food' }), 'Q');
  assert.equal(badgeForButton({ context: 'market', id: 'trade-buy-stone' }), 'D');
});

test('no hotkeys in empty / enemy contexts — WASD keeps panning', () => {
  for (const context of ['none', 'foe']) {
    assert.equal(resolveHotkey({ context, page: null, code: 'KeyQ' }), null, context);
    assert.equal(resolveHotkey({ context, page: null, code: 'KeyA' }), null, context);
  }
});

test('units context: the military panel answers on its slot letters (CMB-09/10/11)', () => {
  const at = (code) => resolveHotkey({ context: 'units', page: null, code });
  assert.deepEqual(at('KeyQ'), { type: 'button', id: 'attack-move' });
  assert.deepEqual(at('KeyW'), { type: 'button', id: 'patrol' });
  assert.deepEqual(at('KeyE'), { type: 'button', id: 'stance-0' });
  assert.deepEqual(at('KeyA'), { type: 'button', id: 'stance-1' });
  assert.deepEqual(at('KeyS'), { type: 'button', id: 'stance-2' });
  assert.deepEqual(at('KeyD'), { type: 'button', id: 'stance-3' });
  assert.deepEqual(at('KeyX'), { type: 'button', id: 'stop' });
  assert.deepEqual(at('KeyC'), { type: 'button', id: 'delete' });
  // R (DE's dedicated attack-move key) and G (AutoExplore) are
  // NOT grid slots: they fall through to app.js's handlers.
  assert.equal(at('KeyR'), null);
  assert.equal(at('KeyG'), null);
  assert.equal(at('KeyZ'), null);
  // Badges are the slot letters (app.js badges Attack Move with
  // its dedicated R and AutoExplore with G instead).
  assert.equal(badgeForButton({ context: 'units', id: 'patrol' }), 'W');
  assert.equal(badgeForButton({ context: 'units', id: 'stance-2' }), 'S');
  assert.equal(badgeForButton({ context: 'units', id: 'stop' }), 'X');
  assert.equal(badgeForButton({ context: 'units', id: 'delete' }), 'C');
});

test('go-to: Ctrl+letter jumps to a building type, in any context', () => {
  const go = (code, over = {}) => resolveHotkey({ context: 'none', page: null, code, ctrl: true, ...over });
  assert.deepEqual(go('KeyZ'), { type: 'goto', kind: 3, all: false }, 'Lumber Camp');
  assert.deepEqual(go('KeyI'), { type: 'goto', kind: 4, all: false }, 'Mill');
  assert.deepEqual(go('KeyG'), { type: 'goto', kind: 5, all: false }, 'Mining Camp');
  assert.deepEqual(go('KeyS'), { type: 'goto', kind: 8, all: false }, 'Blacksmith');
  assert.deepEqual(go('KeyB'), { type: 'goto', kind: 2, all: false }, 'Barracks');
  assert.deepEqual(go('KeyA'), { type: 'goto', kind: 6, all: false }, 'Archery Range');
  assert.deepEqual(go('KeyM'), { type: 'goto', kind: 12, all: false }, 'Market');
  assert.deepEqual(go('KeyZ', { shift: true }), { type: 'goto', kind: 3, all: true },
    'Ctrl+Shift selects all of the type');
  assert.equal(go('KeyL'), null, 'Ctrl+L (Stable) is the browser\u2019s — never bound');
  assert.equal(go('KeyQ'), null, 'Ctrl+Q is not a go-to');
  // Even mid-build: go-to wins over the panel letters.
  assert.equal(build({ page: 'eco', code: 'KeyG', ctrl: true }).type, 'goto');
});

test('every building sits on exactly one page, in grid order, with a working badge', () => {
  assert.deepEqual([...PAGE_KINDS.eco, ...PAGE_KINDS.mil].sort((a, b) => a - b),
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14], 'the two pages partition the fifteen buildings');
  // Grid (render) order: the slots run Q W E / A S / ... in order.
  assert.deepEqual(PAGE_KINDS.eco, [0, 4, 1, 3, 5, 12, 13], 'economic: House, Mill, Farm, Lumber, Mining, Market, Town Center');
  assert.deepEqual(PAGE_KINDS.mil, [2, 6, 7, 8, 9, 10, 11, 14],
    'military: Barracks, Range, Stable, Blacksmith, Palisade Wall, Watch Tower, Outpost, Stone Wall');
  for (const [page, kinds] of Object.entries(PAGE_KINDS)) {
    for (const kind of kinds) {
      assert.equal(PAGE_FOR_KIND[kind], page);
      const badge = PAGE_BADGE[page][kind];
      assert.ok(badge, `${page}/${kind} has a badge`);
      assert.equal(PAGE_KEYS[page][`Key${badge}`], kind,
        'the badge letter\u2019s key runs this very building');
      assert.equal(badgeForButton({ context: 'build', page, id: `build-${kind}` }), badge);
    }
    // The page's Back button: last slot, C, badged and keyed.
    assert.equal(badgeForButton({ context: 'build', page, id: 'page-back' }), 'C');
  }
  // No badge where no key acts: the unarmed build panel.
  assert.equal(badgeForButton({ context: 'build', page: null, id: 'build-0' }), null);
});

test('badges in train/research contexts are the slot letters', () => {
  // Every panel's buttons carry their PANEL_KEYS letter.
  for (const [context, ids] of Object.entries(PANEL_BUTTONS)) {
    for (const id of ids) {
      assert.equal(badgeForButton({ context, id, index: 0 }),
        PANEL_KEYS[context][id][1], `${context}/${id}`);
      assert.equal(PANEL_KEYS[context][id][0], `Key${PANEL_KEYS[context][id][1]}`);
    }
  }
  // The grid shapes, spelled out: first-row panels on Q/W, the
  // Blacksmith's third research on A (second row), the TC's
  // DE arrangement Q / A / W.
  assert.equal(badgeForButton({ context: 'barracks', id: 'train-militia', index: 0 }), 'Q');
  assert.equal(badgeForButton({ context: 'barracks', id: 'train-spearman', index: 1 }), 'W');
  assert.equal(badgeForButton({ context: 'barracks', id: 'research-maa', index: 2 }), 'A');
  assert.equal(badgeForButton({ context: 'blacksmith', id: 'research-padded', index: 2 }), 'A');
  assert.equal(badgeForButton({ context: 'tc', id: 'train', index: 0 }), 'Q');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-loom', index: 1 }), 'A');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-age', index: 2 }), 'Z');
  // A context with no panel at all badges nothing. (This line
  // used 'units' as its example until CMB-09/10/11 gave the
  // military selection a real panel; 'none' carries the intent.)
  assert.equal(badgeForButton({ context: 'none', id: 'x', index: 0 }), null);
});

test('edgePan: edges pan, corners go diagonal, the middle rests', () => {
  const M = 6, W = 1280, H = 800;
  assert.deepEqual(edgePan(0, 400, W, H, M), { x: 1, y: 0 }, 'left edge: view left');
  assert.deepEqual(edgePan(W - 1, 400, W, H, M), { x: -1, y: 0 }, 'right edge: view right');
  assert.deepEqual(edgePan(640, 0, W, H, M), { x: 0, y: 1 }, 'top edge: view up');
  assert.deepEqual(edgePan(640, H - 1, W, H, M), { x: 0, y: -1 }, 'bottom edge: view down');
  assert.deepEqual(edgePan(0, 0, W, H, M), { x: 1, y: 1 }, 'corner: diagonal');
  assert.deepEqual(edgePan(W - 1, H - 1, W, H, M), { x: -1, y: -1 }, 'corner: diagonal');
  assert.deepEqual(edgePan(640, 400, W, H, M), { x: 0, y: 0 }, 'center: rest');
  assert.deepEqual(edgePan(M, 400, W, H, M), { x: 1, y: 0 }, 'the margin is inclusive');
  assert.deepEqual(edgePan(M + 1, 400, W, H, M), { x: 0, y: 0 }, 'one px past it: rest');
});
