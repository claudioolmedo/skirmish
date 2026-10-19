// Skirmish II — the classic grid hotkeys (M13). Pure tables and
// resolution logic only: no DOM access, no sim access. app.js
// feeds this module the panel context it already computes for
// renderCommands() plus the raw key event fields, and executes
// the returned action through the very same handlers a button
// click runs — a hotkey is a click by another name.
//
// The layout is the reference game's Definitive Edition grid:
// a selected object's command panel is a grid and the key IS the
// button's position — top row Q W E R T, middle A S D F G,
// bottom Z X C V B. Two layers sit on top of that principle:
//
// 1. Villager build pages. With villager(s) selected, Q arms the
//    ECONOMIC page and W the MILITARY page; on an armed page the
//    letters follow the wiki's DE build positions (economic:
//    House Q, Mill W, Mining Camp E, Lumber Camp R, Farm A,
//    Town Center Z; military: Barracks Q, Archery Range W,
//    Stable E, Blacksmith S), each building ALSO answering to
//    its documented go-to letter as an alias. One deliberate
//    divergence, Claudio's call (2026-10-18): the sequence
//    Q, E must raise a House, so on the economic page E = House
//    too — and the Mining Camp, whose grid letter E is thereby
//    taken, answers to G (its go-to letter). Recorded in
//    ARCHITECTURE.md (M13).
// 2. Panel position keys. With a building selected, Q / W / E
//    run the panel's buttons left to right — the grid principle
//    applied to our own panel (Town Center: Villager, Loom,
//    Feudal Age; Barracks: Militia, Man-at-Arms; Blacksmith:
//    Fletching, Scale Mail, Padded Archer Armor; Range/Stable:
//    their single train button on Q).
//
// Go-to (jump to a building type) is Ctrl+letter with the
// reference's letters: Lumber Camp Z, Mill I, Mining Camp G,
// Blacksmith S, Barracks B, Archery Range A. Ctrl+Shift+letter
// selects every building of the type instead of cycling.
// Ctrl+L (Stable) is NOT bound: browsers reserve it for the
// address bar and the page never receives it — recorded in
// ARCHITECTURE.md (M13). H alone (go to Town Center) predates
// this module and stays in app.js with the M4 keys.
//
// Building kinds below mirror the sim's BuildKind order, the
// same numbering app.js's BUILD_* tables use.

// kind: 0 House, 1 Farm, 2 Barracks, 3 Lumber Camp, 4 Mill,
//       5 Mining Camp, 6 Archery Range, 7 Stable, 8 Blacksmith.
export const PAGE_KINDS = {
  eco: [0, 1, 3, 4, 5],
  mil: [2, 6, 7, 8],
};

/** Which build page a building kind belongs to ('eco' | 'mil'). */
export const PAGE_FOR_KIND = {
  0: 'eco', 1: 'eco', 3: 'eco', 4: 'eco', 5: 'eco',
  2: 'mil', 6: 'mil', 7: 'mil', 8: 'mil',
};

/**
 * Key code -> building kind on an armed page: the DE grid
 * position first, the building's go-to letter as an alias.
 * Economic E is House (Claudio) as well as Q; the Mining Camp
 * sits on G because its grid E is the House's.
 */
export const PAGE_KEYS = {
  eco: {
    KeyQ: 0, KeyE: 0, // House (grid Q; E by Claudio's call)
    KeyW: 4, KeyI: 4, // Mill
    KeyR: 3, KeyZ: 3, // Lumber Camp
    KeyA: 1, KeyF: 1, // Farm
    KeyG: 5, // Mining Camp (grid E taken by the House — see header)
  },
  mil: {
    KeyQ: 2, KeyB: 2, // Barracks
    KeyW: 6, KeyA: 6, // Archery Range
    KeyE: 7, KeyL: 7, // Stable
    KeyS: 8, // Blacksmith
  },
};

/** The one letter shown as a button's key badge on an armed page. */
export const PAGE_BADGE = {
  eco: { 0: 'E', 1: 'A', 3: 'R', 4: 'W', 5: 'G' },
  mil: { 2: 'Q', 6: 'W', 7: 'E', 8: 'S' },
};

/** Panel contexts whose buttons answer to position keys, in order. */
export const PANEL_BUTTONS = {
  tc: ['train', 'research-loom', 'research-feudal'],
  barracks: ['train-militia', 'research-maa'],
  range: ['train-archer'],
  stable: ['train-scout'],
  blacksmith: ['research-fletching', 'research-scalemail', 'research-padded'],
};
export const POSITION_CODES = ['KeyQ', 'KeyW', 'KeyE'];
export const POSITION_LETTERS = ['Q', 'W', 'E'];

/** Go-to: Ctrl + key code -> building kind (Stable unbound: Ctrl+L). */
export const GOTO_KEYS = {
  KeyZ: 3, // Lumber Camp
  KeyI: 4, // Mill
  KeyG: 5, // Mining Camp
  KeyS: 8, // Blacksmith
  KeyB: 2, // Barracks
  KeyA: 6, // Archery Range
};

/**
 * Resolve one key press to an action, or null when the key is
 * not ours in this context (the caller then lets it fall through
 * to the older handlers — WASD panning included: WASD only stops
 * panning in contexts where the grid claims the letter, exactly
 * like the reference game).
 *
 * Input: { context, page, code, ctrl, shift } — context is the
 * renderCommands() context string ('build' | 'tc' | 'barracks' |
 * 'range' | 'stable' | 'blacksmith' | anything else), page the
 * armed build page ('eco' | 'mil' | null), code the KeyboardEvent
 * code, ctrl/shift the modifier state.
 *
 * Actions:
 *   { type: 'arm', page }        — open a build page
 *   { type: 'button', id }       — run this command-panel button
 *   { type: 'drop-off' }         — force drop-off (DE's villager A)
 *   { type: 'goto', kind, all }  — jump to / select a building type
 */
export function resolveHotkey({ context, page, code, ctrl = false, shift = false }) {
  if (ctrl) {
    const kind = GOTO_KEYS[code];
    return kind != null ? { type: 'goto', kind, all: !!shift } : null;
  }
  if (context === 'build') {
    if (page) {
      const kind = PAGE_KEYS[page][code];
      return kind != null ? { type: 'button', id: `build-${kind}` } : null;
    }
    if (code === 'KeyQ') return { type: 'arm', page: 'eco' };
    if (code === 'KeyW') return { type: 'arm', page: 'mil' };
    if (code === 'KeyA') return { type: 'drop-off' };
    return null;
  }
  const buttons = PANEL_BUTTONS[context];
  if (buttons) {
    const idx = POSITION_CODES.indexOf(code);
    if (idx >= 0 && idx < buttons.length) return { type: 'button', id: buttons[idx] };
  }
  return null;
}

/**
 * The key letter to badge on a command-panel button, or null.
 * On an armed build page: the page letter for that building.
 * In a train/research context: the position letter by the
 * button's index in the panel. Elsewhere (the unarmed build
 * panel included): no badge — no key runs the button directly.
 */
export function badgeForButton({ context, page, id, index }) {
  if (context === 'build' && page && id.startsWith('build-')) {
    return PAGE_BADGE[page][Number(id.slice(6))] || null;
  }
  if (PANEL_BUTTONS[context] && index >= 0 && index < POSITION_LETTERS.length) {
    return POSITION_LETTERS[index];
  }
  return null;
}

/**
 * Edge scrolling (M13 addendum): which way the camera pans for
 * a pointer at (x, y) inside a (w, h) viewport, in camOx/camOy
 * sign space (the space applyKeys pans in: +x moves the view
 * left, i.e. the camera travels left). The pointer within
 * `margin` px of an edge pans toward that edge; corners pan
 * diagonally (both axes). Returns { x, y } with entries in
 * { -1, 0, 1 }.
 */
export function edgePan(x, y, w, h, margin) {
  return {
    x: x <= margin ? 1 : x >= w - 1 - margin ? -1 : 0,
    y: y <= margin ? 1 : y >= h - 1 - margin ? -1 : 0,
  };
}
