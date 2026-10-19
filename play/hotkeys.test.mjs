// Headless tests for hotkeys.js (M13: the classic grid hotkeys +
// edge-scroll vector). Run: node --test play/hotkeys.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  PAGE_KINDS, PAGE_FOR_KIND, PAGE_KEYS, PAGE_BADGE, PANEL_BUTTONS,
  POSITION_CODES, GOTO_KEYS, resolveHotkey, badgeForButton, edgePan,
} from './hotkeys.js';

const build = (over = {}) => resolveHotkey({ context: 'build', page: null, ...over });

test('Q arms the economic page, W the military page (villager context)', () => {
  assert.deepEqual(build({ code: 'KeyQ' }), { type: 'arm', page: 'eco' });
  assert.deepEqual(build({ code: 'KeyW' }), { type: 'arm', page: 'mil' });
});

test('Claudio: Q then E raises a House — and grid Q does too', () => {
  assert.deepEqual(build({ page: 'eco', code: 'KeyE' }), { type: 'button', id: 'build-0' });
  assert.deepEqual(build({ page: 'eco', code: 'KeyQ' }), { type: 'button', id: 'build-0' });
});

test('economic page: the DE build positions and their aliases', () => {
  const eco = (code) => build({ page: 'eco', code });
  assert.equal(eco('KeyW').id, 'build-4', 'Mill on W');
  assert.equal(eco('KeyI').id, 'build-4', 'Mill alias I');
  assert.equal(eco('KeyR').id, 'build-3', 'Lumber Camp on R');
  assert.equal(eco('KeyZ').id, 'build-3', 'Lumber Camp alias Z');
  assert.equal(eco('KeyA').id, 'build-1', 'Farm on A');
  assert.equal(eco('KeyF').id, 'build-1', 'Farm alias F');
  assert.equal(eco('KeyG').id, 'build-5', 'Mining Camp on G (its grid E is the House)');
});

test('military page: the DE build positions and their aliases', () => {
  const mil = (code) => build({ page: 'mil', code });
  assert.equal(mil('KeyQ').id, 'build-2', 'Barracks on Q');
  assert.equal(mil('KeyB').id, 'build-2', 'Barracks alias B');
  assert.equal(mil('KeyW').id, 'build-6', 'Archery Range on W');
  assert.equal(mil('KeyA').id, 'build-6', 'Archery Range alias A');
  assert.equal(mil('KeyE').id, 'build-7', 'Stable on E');
  assert.equal(mil('KeyL').id, 'build-7', 'Stable alias L');
  assert.equal(mil('KeyS').id, 'build-8', 'Blacksmith on S');
});

test('an armed page swallows its letters; other letters fall through', () => {
  assert.equal(build({ page: 'eco', code: 'KeyD' }), null, 'D still pans');
  assert.equal(build({ page: 'mil', code: 'KeyR' }), null);
  assert.equal(build({ code: 'KeyE' }), null, 'unarmed E is not ours');
  assert.equal(build({ code: 'KeyS' }), null, 'unarmed S still pans');
});

test('A with villagers and no page armed is the DE drop-off', () => {
  assert.deepEqual(build({ code: 'KeyA' }), { type: 'drop-off' });
  // On the armed economic page the same letter is the Farm — the
  // panel context decides, as in the reference game.
  assert.equal(build({ page: 'eco', code: 'KeyA' }).type, 'button');
});

test('panel position keys run a selected building\u2019s buttons in order', () => {
  const at = (context, code) => resolveHotkey({ context, page: null, code });
  assert.equal(at('barracks', 'KeyQ').id, 'train-militia');
  assert.equal(at('barracks', 'KeyW').id, 'research-maa');
  assert.equal(at('barracks', 'KeyE'), null, 'no third button');
  assert.equal(at('range', 'KeyQ').id, 'train-archer');
  assert.equal(at('stable', 'KeyQ').id, 'train-scout');
  assert.equal(at('blacksmith', 'KeyQ').id, 'research-fletching');
  assert.equal(at('blacksmith', 'KeyW').id, 'research-scalemail');
  assert.equal(at('blacksmith', 'KeyE').id, 'research-padded');
});

test('Town Center letters are DE’s own: Villager Q, Loom A, Feudal W', () => {
  // DE's grid letter is the button's panel position, and the TC
  // panel's arrangement is not our button order (Age of Empires
  // wiki, "Hotkey"; the study video's Loom tooltip reads
  // "(Hotkey: A)"). Scoped to the TC context only: E runs
  // nothing here, and the villager context keeps its own A.
  const at = (code) => resolveHotkey({ context: 'tc', page: null, code });
  assert.equal(at('KeyQ').id, 'train', 'Create Villager — top-left of the TC panel');
  assert.equal(at('KeyA').id, 'research-loom', 'Loom — below Villager (study frame f_025)');
  assert.equal(at('KeyW').id, 'research-feudal', 'Feudal Age — top row, second slot');
  assert.equal(at('KeyE'), null, 'E is not a TC letter in DE');
  assert.equal(at('KeyS'), null);
  assert.equal(at('KeyD'), null);
  // The badges painted on the buttons carry the same letters.
  assert.equal(badgeForButton({ context: 'tc', id: 'train', index: 0 }), 'Q');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-loom', index: 1 }), 'A');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-feudal', index: 2 }), 'W');
  // Other panels keep their position badges.
  assert.equal(badgeForButton({ context: 'barracks', id: 'research-maa', index: 1 }), 'W');
});

test('no hotkeys in unit / empty / enemy contexts — WASD keeps panning', () => {
  for (const context of ['units', 'none', 'foe']) {
    assert.equal(resolveHotkey({ context, page: null, code: 'KeyQ' }), null, context);
    assert.equal(resolveHotkey({ context, page: null, code: 'KeyA' }), null, context);
  }
});

test('go-to: Ctrl+letter jumps to a building type, in any context', () => {
  const go = (code, over = {}) => resolveHotkey({ context: 'none', page: null, code, ctrl: true, ...over });
  assert.deepEqual(go('KeyZ'), { type: 'goto', kind: 3, all: false }, 'Lumber Camp');
  assert.deepEqual(go('KeyI'), { type: 'goto', kind: 4, all: false }, 'Mill');
  assert.deepEqual(go('KeyG'), { type: 'goto', kind: 5, all: false }, 'Mining Camp');
  assert.deepEqual(go('KeyS'), { type: 'goto', kind: 8, all: false }, 'Blacksmith');
  assert.deepEqual(go('KeyB'), { type: 'goto', kind: 2, all: false }, 'Barracks');
  assert.deepEqual(go('KeyA'), { type: 'goto', kind: 6, all: false }, 'Archery Range');
  assert.deepEqual(go('KeyZ', { shift: true }), { type: 'goto', kind: 3, all: true },
    'Ctrl+Shift selects all of the type');
  assert.equal(go('KeyL'), null, 'Ctrl+L (Stable) is the browser\u2019s — never bound');
  assert.equal(go('KeyQ'), null, 'Ctrl+Q is not a go-to');
  // Even mid-build: go-to wins over the panel letters.
  assert.equal(build({ page: 'eco', code: 'KeyG', ctrl: true }).type, 'goto');
});

test('every building sits on exactly one page, with a working badge', () => {
  assert.deepEqual([...PAGE_KINDS.eco, ...PAGE_KINDS.mil].sort((a, b) => a - b),
    [0, 1, 2, 3, 4, 5, 6, 7, 8], 'the two pages partition the nine buildings');
  for (const [page, kinds] of Object.entries(PAGE_KINDS)) {
    for (const kind of kinds) {
      assert.equal(PAGE_FOR_KIND[kind], page);
      const badge = PAGE_BADGE[page][kind];
      assert.ok(badge, `${page}/${kind} has a badge`);
      assert.equal(PAGE_KEYS[page][`Key${badge}`], kind,
        'the badge letter\u2019s key runs this very building');
      assert.equal(badgeForButton({ context: 'build', page, id: `build-${kind}` }), badge);
    }
  }
  // No badge where no key acts: the unarmed build panel.
  assert.equal(badgeForButton({ context: 'build', page: null, id: 'build-0' }), null);
});

test('badges in train/research contexts are the position letters', () => {
  for (const [context, ids] of Object.entries(PANEL_BUTTONS)) {
    if (context === 'tc') continue; // DE's own TC letters — the test below
    ids.forEach((id, i) => {
      assert.equal(badgeForButton({ context, id, index: i }), 'QWE'[i], `${context}/${id}`);
      assert.equal(POSITION_CODES[i], `Key${'QWE'[i]}`);
    });
  }
  // The Town Center exception: DE letters, not positions.
  assert.equal(badgeForButton({ context: 'tc', id: 'train', index: 0 }), 'Q');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-loom', index: 1 }), 'A');
  assert.equal(badgeForButton({ context: 'tc', id: 'research-feudal', index: 2 }), 'W');
  assert.equal(badgeForButton({ context: 'units', id: 'x', index: 0 }), null);
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
