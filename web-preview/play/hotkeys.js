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
//    Camp S, Market D, Back C; military: Barracks Q, Archery
//    Range W, Stable E, Blacksmith A, Palisade Wall S, Back C.
//    (The Palisade Wall sits where DE puts it: the military
//    page's defensive row, after the production buildings and
//    the Blacksmith — in our three-wide compression that is the
//    next free slot, S. The Market's D is DE's own build letter
//    for it — the wiki's DE build table — and in our compressed
//    grid it is also simply the next free slot, after the
//    Mining Camp.) A building may ALSO answer
//    to a silent alias (its go-to letter, mostly) — but ONLY
//    where the alias collides with no button's slot letter in
//    the same context. Two historical aliases died by that rule
//    (pure-grid pass, 2026-10-19, Claudio's call, superseding
//    his own 2026-10-18 exception): E no longer doubles as
//    House (E is the Farm's slot) and A no longer doubles as
//    the Archery Range (A is the Blacksmith's slot).
// 2. Panel keys. A selected building's buttons take their slot
//    letters too — see PANEL_KEYS: mostly plain left-to-right
//    (Barracks: Militia Q, Spearman W, Man-at-Arms A below the
//    Militia; Range: Archer Q, Skirmisher W; Stable: Scout Q),
//    with DE's own arrangements where the reference panel has
//    them: the Town Center (first column: Villager Q, Loom A,
//    Age advancement Z; the Wheelbarrow line beside Loom, S —
//    see PANEL_KEYS) and the Blacksmith (Fletching Q, Scale Mail
//    W, Padded Archer Armor A — plus, since the 2026-10-22
//    Feudal completion, Forging E and Scale Barding Armor S
//    under Scale Mail). The Stable gained Bloodlines W; the
//    Lumber Camp, Mill and Mining Camp panels are plain top-row
//    runs (Q, then W).
//
//    PLACEMENT (2026-10-23, Claudio's report): a button's letter
//    is its CELL, and the renderer now puts every button INTO
//    its letter's cell (panelCellFor below; empty cells stay
//    empty). That was not always so: the study-fix landing gave
//    the TC DE's letters (Loom A) while the renderer kept
//    filling the grid in code order, so the panel READ Q A W /
//    S D with no button sitting in its letter's cell — and each
//    later button (Wheelbarrow S, Hand Cart D, Man-at-Arms A,
//    the Blacksmith's second row) inherited the scramble.
//    panels.test.mjs pins every panel's layout cell by cell so
//    no landing can reshuffle one silently again.
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
//       5 Mining Camp, 6 Archery Range, 7 Stable, 8 Blacksmith,
//       9 Palisade Wall, 10 Watch Tower, 11 Outpost (BLD-18),
//       12 Market (BLD-19), 14 Stone Wall (BLD-20), 15 Castle
//       (Castle P3a), 16 University (Castle P3c), 17 Siege
//       Workshop (Castle P4a; the tags are the sim's — append
//       only).
// PAGE_KINDS is the pages' GRID (render) order: slot 0 first.
// Kind 13 (Town Center, Castle Age) closes the economic page —
// DE's villager panel keeps the TC with the economic buildings,
// and kind 15 (the Castle) takes the page's last free cell
// beside it: the military page already fills all nine cells.
export const PAGE_KINDS = {
  eco: [0, 4, 1, 3, 5, 12, 13, 15],
  mil: [2, 6, 7, 8, 9, 10, 11, 14, 16, 17],
};

/** Which build page a building kind belongs to ('eco' | 'mil'). */
export const PAGE_FOR_KIND = {
  0: 'eco', 1: 'eco', 3: 'eco', 4: 'eco', 5: 'eco', 12: 'eco', 13: 'eco', 15: 'eco',
  2: 'mil', 6: 'mil', 7: 'mil', 8: 'mil', 9: 'mil', 10: 'mil', 11: 'mil', 14: 'mil', 16: 'mil', 17: 'mil',
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
    KeyA: 3, KeyR: 3, // Lumber Camp — slot 3 (alias R: DE's build letter for it)
    KeyS: 5, KeyG: 5, // Mining Camp — slot 4 (alias: its go-to G)
    KeyD: 12, KeyM: 12, // Market — slot 5 (D is DE's build letter; alias M, its go-to)
    KeyZ: 13, // Town Center — slot 6 (Castle Age; Z is DE's own TC build letter — the wiki's DE build table — and slot 6 IS the Z cell. The old T "initial" badge and the Lumber Camp's silent Z alias (its go-to letter) both died here: the cell's letter rules.)
    KeyX: 15, // Castle — slot 7 (Castle P3a; the page's last free cell — the Military page is full at 8 kinds + Back, so the Castle closes the Economic page beside the Town Center, the other Castle-Age build)
    KeyC: 'back', // Back — the last slot
  },
  mil: {
    KeyQ: 2, KeyB: 2, // Barracks — slot 0 (alias B)
    KeyW: 6, // Archery Range — slot 1 (old alias A dropped: A is the Blacksmith's slot)
    KeyE: 7, KeyL: 7, // Stable — slot 2 (alias L)
    KeyA: 8, // Blacksmith — slot 3
    KeyS: 9, // Palisade Wall — slot 4 (the defensive row, after the Blacksmith)
    KeyD: 10, // Watch Tower — slot 5 (BLD-18; next free slot — S is the wall's)
    KeyZ: 11, // Outpost — slot 6 (BLD-18)
    KeyX: 14, // Stone Wall — slot 7 (BLD-20; the defensive row's second wall, beside the Palisade as in DE)
    KeyV: 16, // University — slot 8 (Castle P3c; the page's nine kinds overflow the 3×3, so the University opens a fourth row under the Barracks — V, the free letter under the page's first column. Its DE page is this one; the Castle precedent (overflow to the eco page) is exhausted — that page is full too.)
    KeyN: 17, // Siege Workshop — slot 9 (Castle P4a; the fourth row's second cell. The bottom letter row runs Z X C V B N — B is the Barracks' silent alias on this page, so the workshop takes the next free letter, N.)
    KeyC: 'back', // Back — the last slot
  },
};

/** The one letter shown as a button's key badge on an armed page. */
export const PAGE_BADGE = {
  eco: { 0: 'Q', 4: 'W', 1: 'E', 3: 'A', 5: 'S', 12: 'D', 13: 'Z', 15: 'X' },
  mil: { 2: 'Q', 6: 'W', 7: 'E', 8: 'A', 9: 'S', 10: 'D', 11: 'Z', 14: 'X', 16: 'V', 17: 'N' },
};

/** Panel contexts whose buttons answer to position keys, in order. */
export const PANEL_BUTTONS = {
  // The military selection's panel (CMB-09/10/11), in render
  // order: Attack Move, Patrol, the four stances, Stop, Delete.
  // AutoExplore renders between the stances and Stop for scouts,
  // but is not listed: it carries no slot letter (its key is the
  // dedicated G, app.js's fall-through) — the same treatment the
  // build context's special buttons get outside these tables.
  units: ['attack-move', 'patrol', 'stance-0', 'stance-1', 'stance-2', 'stance-3',
    'stop', 'delete'],
  tc: ['train', 'research-loom', 'research-age', 'research-wheelbarrow', 'research-handcart'],
  barracks: ['train-militia', 'train-spearman', 'research-maa'],
  range: ['train-archer', 'train-skirmisher', 'train-cavalry-archer'],
  stable: ['train-scout', 'train-knight', 'research-lightcavalry', 'research-bloodlines', 'research-husbandry'],
  // Castle P3a: the Castle's whole panel — Train Petard.
  castle: ['train-petard'],
  // Castle P4a: the Siege Workshop's whole panel — Train Ram.
  siegeworkshop: ['train-ram'],
  blacksmith: ['research-fletching', 'research-scalemail', 'research-padded',
    'research-forging', 'research-barding'],
  lumbercamp: ['research-doublebit'],
  mill: ['research-horsecollar'],
  miningcamp: ['research-goldmining', 'research-stonemining'],
  // TRD-01..03: the Market's exchange, in render order — the
  // sell row over the buy row, food / wood / stone across.
  // Package D adds the bottom row: Train Trade Cart (Z) and
  // Caravan (X) — the exchange grid itself does not move.
  market: ['trade-sell-food', 'trade-sell-wood', 'trade-sell-stone',
    'trade-buy-food', 'trade-buy-wood', 'trade-buy-stone',
    'train-trade-cart', 'research-caravan'],
  // Castle P3c: the University's five researches, in render
  // order (the tech table's order: Ballistics, Murder Holes,
  // Treadmill Crane, Masonry, Fortified Wall). Heated Shot is
  // absent on purpose — deferred (anti-ship only, no water).
  university: ['research-ballistics', 'research-murderholes', 'research-treadmill',
    'research-masonry', 'research-fortifiedwall'],
};

// The slot letter of every button on every building panel
// (button id -> [KeyboardEvent.code, letter]). The plain cases
// run left to right along the grid's first row (Barracks:
// Militia Q, Man-at-Arms W; Range: Archer Q; Stable: Scout Q).
// Two panels reproduce DE's own arrangements instead: the
// Town Center — read off real DE footage (the 2026-10-19
// player study's frames of a selected TC, f_005/f_024/f_052):
// the first column stacks Create Villager (Q), Loom below it
// (A — the study video's in-game tooltip reads "Loom …
// (Hotkey: A)", frame f_025) and the Age advancement below
// Loom (Z); the Wheelbarrow line sits beside Loom (S — the
// frames show its darkened, Feudal-locked icon at row 2,
// column 2) — and the Blacksmith, whose Padded Archer Armor
// sits under Fletching (A), not after Scale Mail. (An earlier
// table claimed Age at W "top-row second" and Hand Cart at D:
// neither survives the frames — the age button is the first
// column's third cell, and Hand Cart is the Wheelbarrow cell's
// next rung, below.) Villager-context letters are unaffected:
// 'build' is not in this table, so a villager's A still means
// force drop-off, resolved by context exactly as DE resolves
// it.
export const PANEL_KEYS = {
  // The units panel's slot letters, by render order: Attack
  // Move Q, Patrol W, the stances E / A / S / D, Stop X, Delete
  // C. Two buttons ALSO carry DE's dedicated keys, badged
  // instead of their slot letter and handled in app.js's
  // fall-through like AutoExplore's G: Attack Move answers to R
  // (the wiki Hotkey page's dedicated attack-move key), and
  // AutoExplore keeps its G (it has no slot entry here at all).
  units: {
    'attack-move': ['KeyQ', 'Q'],
    patrol: ['KeyW', 'W'],
    'stance-0': ['KeyE', 'E'],
    'stance-1': ['KeyA', 'A'],
    'stance-2': ['KeyS', 'S'],
    'stance-3': ['KeyD', 'D'],
    stop: ['KeyX', 'X'],
    delete: ['KeyC', 'C'],
  },
  tc: {
    train: ['KeyQ', 'Q'],
    'research-loom': ['KeyA', 'A'],
    // The age-advance button (DE has exactly one: it offers the
    // NEXT Age — Feudal from Dark, Castle from Feudal) closes
    // the first column, under Loom: Z in both steps. (Study
    // frames f_005/f_024: the red chalice icon stacked third in
    // the TC's first column; f_052: the same cell busy while
    // Feudal researches.)
    'research-age': ['KeyZ', 'Z'],
    // The Wheelbarrow line (2026-10-22) sits right of Loom on
    // the second row: S. Hand Cart is the SAME cell's next rung
    // — DE never shows both; the panel renders whichever rung
    // is current (app.js swaps on the Wheelbarrow bit), and
    // both ids carry the cell's letter. (Castle P1's separate
    // Hand Cart at D was wrong twice over: D is where the
    // reference panel keeps Town Watch, a tech we do not have.)
    'research-wheelbarrow': ['KeyS', 'S'],
    'research-handcart': ['KeyS', 'S'],
  },
  barracks: {
    'train-militia': ['KeyQ', 'Q'],
    'train-spearman': ['KeyW', 'W'],
    // The Man-at-Arms upgrade drops to the second row (A, under
    // the Militia) now that the Spearman takes DE's second
    // Barracks slot — the same two-row arrangement the Town
    // Center (Loom A) and Blacksmith (Padded A) already use for
    // their non-unit entries.
    'research-maa': ['KeyA', 'A'],
  },
  range: {
    'train-archer': ['KeyQ', 'Q'],
    'train-skirmisher': ['KeyW', 'W'],
    // Castle P2: the Cavalry Archer continues the Range's plain
    // top row (E), like every plain panel's third unit.
    'train-cavalry-archer': ['KeyE', 'E'],
  },
  stable: {
    'train-scout': ['KeyQ', 'Q'],
    // Castle P2: the Knight takes the Stable's second top-row
    // slot (W). The researches drop to the second row, the
    // arrangement the Barracks (Man-at-Arms A under the Militia)
    // and the Town Center (Loom A) already use: the Light
    // Cavalry upgrade under its own unit (A, under the Scout),
    // Bloodlines under the Knight (S), Husbandry next (D).
    // (Bloodlines sat at W from 2026-10-22, when the Stable's
    // top row held the Scout alone.)
    'train-knight': ['KeyW', 'W'],
    'research-lightcavalry': ['KeyA', 'A'],
    'research-bloodlines': ['KeyS', 'S'],
    'research-husbandry': ['KeyD', 'D'],
  },
  blacksmith: {
    'research-fletching': ['KeyQ', 'Q'],
    'research-scalemail': ['KeyW', 'W'],
    'research-padded': ['KeyA', 'A'],
    // Forging / Scale Barding Armor (2026-10-22) complete DE's
    // own arrangement: Forging takes the top row's last slot
    // (E), Scale Barding Armor sits under Scale Mail Armor (S)
    // — the barding column mirrors the infantry armor column
    // (Scale Mail W over Padded A on the left).
    'research-forging': ['KeyE', 'E'],
    'research-barding': ['KeyS', 'S'],
  },
  // The economy camps and the Mill (2026-10-22): each panel's
  // researches run left to right on the top row like every
  // plain panel (Lumber Camp: Double-Bit Axe Q; Mill: Horse
  // Collar Q; Mining Camp: Gold Mining Q, Stone Mining W).
  lumbercamp: {
    'research-doublebit': ['KeyQ', 'Q'],
  },
  mill: {
    'research-horsecollar': ['KeyQ', 'Q'],
  },
  miningcamp: {
    'research-goldmining': ['KeyQ', 'Q'],
    'research-stonemining': ['KeyW', 'W'],
  },
  // Castle P3a: the Castle's single train button takes the
  // plain panel's first slot, like every production panel.
  castle: {
    'train-petard': ['KeyQ', 'Q'],
  },
  // Castle P4a: the Siege Workshop's single train button, same
  // first-slot rule.
  siegeworkshop: {
    'train-ram': ['KeyQ', 'Q'],
  },
  // The Market (TRD-01..03): the plain positional grid over the
  // two rendered rows — sells Q W E, buys A S D. Package D: the
  // bottom row takes Train Trade Cart at Z and Caravan at X (the
  // two cells the exchange never used).
  market: {
    'trade-sell-food': ['KeyQ', 'Q'],
    'trade-sell-wood': ['KeyW', 'W'],
    'trade-sell-stone': ['KeyE', 'E'],
    'trade-buy-food': ['KeyA', 'A'],
    'trade-buy-wood': ['KeyS', 'S'],
    'trade-buy-stone': ['KeyD', 'D'],
    'train-trade-cart': ['KeyZ', 'Z'],
    'research-caravan': ['KeyX', 'X'],
  },
  // Castle P3c: the University — the plain positional grid over
  // its five researches (top row Q W E, second row A S).
  university: {
    'research-ballistics': ['KeyQ', 'Q'],
    'research-murderholes': ['KeyW', 'W'],
    'research-treadmill': ['KeyE', 'E'],
    'research-masonry': ['KeyA', 'A'],
    'research-fortifiedwall': ['KeyS', 'S'],
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
  KeyM: 12, // Market (the wiki's DE go-to table)
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
 * Input: { context, page, code, ctrl, shift, rendered } —
 * context is the renderCommands() context string ('build' |
 * 'tc' | 'barracks' | 'range' | 'stable' | 'blacksmith' |
 * anything else), page the armed build page ('eco' | 'mil' |
 * null), code the KeyboardEvent code, ctrl/shift the modifier
 * state. `rendered`, when given (the ids currently on the
 * panel), disambiguates the one cell two ids share — the TC's
 * Wheelbarrow/Hand Cart rung: the key runs whichever rung the
 * panel actually shows.
 *
 * Actions:
 *   { type: 'arm', page }        — open a build page
 *   { type: 'button', id }       — run this command-panel button
 *   { type: 'drop-off' }         — force drop-off (DE's villager A)
 *   { type: 'goto', kind, all }  — jump to / select a building type
 */
export function resolveHotkey({ context, page, code, ctrl = false, shift = false, rendered = null }) {
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
        if (keyCode !== code) continue;
        if (rendered && !rendered.includes(id)) continue;
        return { type: 'button', id };
      }
      return null;
    }
    const idx = POSITION_CODES.indexOf(code);
    if (idx >= 0 && idx < buttons.length
      && (!rendered || rendered.includes(buttons[idx]))) {
      return { type: 'button', id: buttons[idx] };
    }
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

/** The grid cell ([row, column], 1-based) of each grid letter. */
export const GRID_CELLS = {
  Q: [1, 1], W: [1, 2], E: [1, 3],
  A: [2, 1], S: [2, 2], D: [2, 3],
  Z: [3, 1], X: [3, 2], C: [3, 3],
  // Castle P3c: the military build page's fourth row (see
  // PAGE_KEYS.mil) — the University's V opened it; Castle P4a's
  // Siege Workshop takes its second cell (N). The renderer
  // adds the fourth row only for that page.
  V: [4, 1],
  N: [4, 2],
};

/**
 * The grid letter whose CELL a rendered panel button belongs
 * in, or null when the button takes the next free cell in
 * reading order (CSS auto-flow). This is the placement half of
 * the hotkey tables: a button's letter is its position, so the
 * renderer drops every lettered button into its own cell and
 * leaves the cells of absent buttons EMPTY — the sparse DE
 * panel — instead of filling the grid in code order and
 * stamping letters on wherever a button happened to land
 * (the 2026-10-23 scramble: the TC read Q A W / S D because
 * its buttons were appended train, loom, age, wheelbarrow,
 * handcart while their letters claim Q, A, Z, S, S).
 *
 * Sources, per context: PANEL_KEYS for the building/unit
 * panels (the Attack Move's cell is its slot Q — its badge
 * stays DE's dedicated R); PAGE_BADGE for an armed build page
 * (Back rides the page's last slot, C); the build root's three
 * openers carry their arm letters (Q / W / E). The building
 * Delete rides the panel's bottom-right cell (C) UNBADGED —
 * DE keeps building deletion at the panel's bottom-right
 * corner, off the grid's letter flow, and no letter key runs
 * it (the keyboard's Delete key does, in app.js). Buttons
 * with a dedicated non-grid key and no slot (AutoExplore, G)
 * and the unit Stop/Delete riding a villager selection take
 * auto-flow, exactly where sequential fill already put them.
 */
export function panelCellFor({ context, page, id }) {
  if (id === 'delete-building' || (id === 'delete' && !page)) return 'C';
  if (context === 'build') {
    if (page) {
      if (id === 'page-back') return 'C';
      if (id.startsWith('build-')) {
        const letter = PAGE_BADGE[page][Number(id.slice(6))];
        return GRID_CELLS[letter] ? letter : null;
      }
      return null;
    }
    if (id === 'page-eco') return 'Q';
    if (id === 'page-mil') return 'W';
    if (id === 'repair') return 'E';
    return null;
  }
  const byLetter = PANEL_KEYS[context];
  if (byLetter && byLetter[id]) return byLetter[id][1];
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
