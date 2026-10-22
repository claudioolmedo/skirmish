// Skirmish II — the classic grid hotkeys (M13). Pure tables and
// resolution logic only: no DOM access, no sim access. app.js
// feeds this module the panel context it already computes for
// renderCommands() plus the raw key event fields, and executes
// the returned action through the very same handlers a button
// click runs — a hotkey is a click by another name.
//
// The layout is the reference game's Definitive Edition grid:
// a selected object's command panel is a grid and the key IS the
// button's position on the PHYSICAL keyboard — the panel's rows
// are the keyboard's rows. Our panels are three columns wide, so
// the whole map is exactly:
//     row 1: Q W E
//     row 2: A S D
//     row 3: Z X C
// (the last slot of a 3x3 page, row 3 col 3, is C). Every letter
// below is a slot position first; badges show the slot letter.
//
// 1. Villager build pages. With villager(s) selected, Q arms the
//    ECONOMIC page and W the MILITARY page (the root panel's two
//    openers); the root's third slot, E, is Repair (BLD-17) — it
//    arms repair targeting, run through the same button handler
//    as its panel click. On an armed page the buttons sit in grid order —
//    economic: House Q, Mill W, Farm E, Lumber Camp A, Mining
//    Camp S, Back C; military: Barracks Q, Archery Range W,
//    Stable E, Blacksmith A, Back C. A building may ALSO answer
//    to a silent alias (its go-to letter, mostly) — but ONLY
//    where the alias collides with no button's slot letter in
//    the same context. Two historical aliases died by that rule
//    (pure-grid pass, 2026-10-19, Claudio's call, superseding
//    his own 2026-10-18 exception): E no longer doubles as
//    House (E is the Farm's slot) and A no longer doubles as
//    the Archery Range (A is the Blacksmith's slot).
// 2. Panel keys. A selected building's buttons take their slot
//    letters too — see PANEL_KEYS: mostly plain left-to-right
//    (Barracks: Militia Q, Man-at-Arms W; Range: Archer Q;
//    Stable: Scout Q), with DE's own two-row arrangements where
//    the reference panel has them: the Town Center (Villager Q,
//    Feudal Age W, Loom A below the Villager) and the Blacksmith
//    (Fletching Q, Scale Mail W, Padded Archer Armor A).
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
// PAGE_KINDS is the pages' GRID (render) order: slot 0 first.
export const PAGE_KINDS = {
  eco: [0, 4, 1, 3, 5],
  mil: [2, 6, 7, 8],
};

/** Which build page a building kind belongs to ('eco' | 'mil'). */
export const PAGE_FOR_KIND = {
  0: 'eco', 1: 'eco', 3: 'eco', 4: 'eco', 5: 'eco',
  2: 'mil', 6: 'mil', 7: 'mil', 8: 'mil',
};

/**
 * Key code -> building kind on an armed page: the button's slot
 * letter first (grid order: slots run Q W E / A S D / Z X C),
 * plus silent aliases that collide with no slot letter in the
 * same context. 'back' is the page's Back button (last slot, C).
 */
export const PAGE_KEYS = {
  eco: {
    KeyQ: 0, // House — slot 0
    KeyW: 4, KeyI: 4, // Mill — slot 1 (alias: its go-to I)
    KeyE: 1, KeyF: 1, // Farm — slot 2 (alias F)
    KeyA: 3, KeyR: 3, KeyZ: 3, // Lumber Camp — slot 3 (aliases R, Z)
    KeyS: 5, KeyG: 5, // Mining Camp — slot 4 (alias: its go-to G)
    KeyC: 'back', // Back — the last slot
  },
  mil: {
    KeyQ: 2, KeyB: 2, // Barracks — slot 0 (alias B)
    KeyW: 6, // Archery Range — slot 1 (old alias A dropped: A is the Blacksmith's slot)
    KeyE: 7, KeyL: 7, // Stable — slot 2 (alias L)
    KeyA: 8, // Blacksmith — slot 3
    KeyC: 'back', // Back — the last slot
  },
};

/** The one letter shown as a button's key badge on an armed page. */
export const PAGE_BADGE = {
  eco: { 0: 'Q', 4: 'W', 1: 'E', 3: 'A', 5: 'S' },
  mil: { 2: 'Q', 6: 'W', 7: 'E', 8: 'A' },
};

/** Panel contexts whose buttons answer to position keys, in order. */
export const PANEL_BUTTONS = {
  tc: ['train', 'research-loom', 'research-feudal'],
  barracks: ['train-militia', 'research-maa'],
  range: ['train-archer'],
  stable: ['train-scout'],
  blacksmith: ['research-fletching', 'research-scalemail', 'research-padded'],
};

// The slot letter of every button on every building panel
// (button id -> [KeyboardEvent.code, letter]). The plain cases
// run left to right along the grid's first row (Barracks:
// Militia Q, Man-at-Arms W; Range: Archer Q; Stable: Scout Q).
// Two panels reproduce DE's own two-row arrangements instead:
// the Town Center — Create Villager top-left (Q), the Feudal
// Age advancement top-row second (W), Loom below the Villager
// (A — the study video's in-game tooltip reads "Loom …
// (Hotkey: A)"; sources: Age of Empires wiki, "Hotkey" — the
// letter IS the button's panel position — plus the 2026-10-19
// player study, frame f_025) — and the Blacksmith, whose
// Padded Archer Armor sits under Fletching (A), not after
// Scale Mail. Villager-context letters are unaffected: 'build'
// is not in this table, so a villager's A still means force
// drop-off, resolved by context exactly as DE resolves it.
export const PANEL_KEYS = {
  tc: {
    train: ['KeyQ', 'Q'],
    'research-loom': ['KeyA', 'A'],
    'research-feudal': ['KeyW', 'W'],
  },
  barracks: {
    'train-militia': ['KeyQ', 'Q'],
    'research-maa': ['KeyW', 'W'],
  },
  range: {
    'train-archer': ['KeyQ', 'Q'],
  },
  stable: {
    'train-scout': ['KeyQ', 'Q'],
  },
  blacksmith: {
    'research-fletching': ['KeyQ', 'Q'],
    'research-scalemail': ['KeyW', 'W'],
    'research-padded': ['KeyA', 'A'],
  },
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
  // Ctrl+L (Stable on the wiki's table) stays unbound: the
  // browser reserves Ctrl+L for its address bar and the key
  // never reaches the page (hotkeys.test.mjs pins this).
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
      const hit = PAGE_KEYS[page][code];
      if (hit === 'back') return { type: 'button', id: 'page-back' };
      return hit != null ? { type: 'button', id: `build-${hit}` } : null;
    }
    if (code === 'KeyQ') return { type: 'arm', page: 'eco' };
    if (code === 'KeyW') return { type: 'arm', page: 'mil' };
    if (code === 'KeyE') return { type: 'button', id: 'repair' };
    if (code === 'KeyA') return { type: 'drop-off' };
    return null;
  }
  const buttons = PANEL_BUTTONS[context];
  if (buttons) {
    // Every context in PANEL_BUTTONS carries its slot letters in
    // PANEL_KEYS (the pure physical grid — see the table); the
    // position-code fallback below only guards a future panel
    // that has not been given its letters yet.
    const byLetter = PANEL_KEYS[context];
    if (byLetter) {
      for (const [id, [keyCode]] of Object.entries(byLetter)) {
        if (keyCode === code) return { type: 'button', id };
      }
      return null;
    }
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
  if (context === 'build' && page) {
    if (id === 'page-back') return 'C'; // the page's last slot
    if (id.startsWith('build-')) {
      return PAGE_BADGE[page][Number(id.slice(6))] || null;
    }
  }
  const byLetter = PANEL_KEYS[context];
  if (byLetter && byLetter[id]) return byLetter[id][1];
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
