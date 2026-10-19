// Stub-DOM smoke for the M11 Lobby Browser: the reference shape —
// controls row (Search · Refresh · Password · Server), the table
// (Game Name · Players · Map · Location · Server), selection +
// Join Lobby, and the Connection Quality panel.
//
// Drives the REAL ../app.js in Node against a stub DOM, a stub
// WebSocket and a stub fetch (no network, no live relay). Run from
// web-preview/play/:
//   node lobby.smoke.mjs
// A smoke, not a unit test: not picked up by `node --test`.

import assert from 'node:assert/strict';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ------------------------------------------------------------------ stubs

class El {
  constructor(tag = 'div', id = '') {
    this.tagName = tag.toUpperCase();
    this.id = id;
    this.children = [];
    this.style = {};
    this.dataset = {};
    this.classList = {
      _set: new Set(),
      add(...cs) { cs.forEach((c) => this._set.add(c)); },
      remove(...cs) { cs.forEach((c) => this._set.delete(c)); },
      toggle(c, on) { if (on === undefined) on = !this._set.has(c); on ? this._set.add(c) : this._set.delete(c); },
      contains(c) { return this._set.has(c); },
    };
    this._listeners = {};
    this.hidden = false;
    this.disabled = false;
    this.checked = false;
    this.value = '';
    this.textContent = '';
    this._innerHTML = '';
    this.width = 300;
    this.height = 150;
  }
  get innerHTML() { return this._innerHTML; }
  set innerHTML(v) { this._innerHTML = String(v); this.children = []; }
  addEventListener(ev, fn) { (this._listeners[ev] ||= []).push(fn); }
  removeEventListener() {}
  dispatch(ev, extra = {}) {
    const e = {
      preventDefault() {}, stopPropagation() {},
      target: this, currentTarget: this, ...extra,
    };
    for (const fn of this._listeners[ev] || []) fn(e);
  }
  click() { this.dispatch('click'); }
  append(...kids) { this.children.push(...kids); }
  appendChild(k) { this.children.push(k); return k; }
  querySelectorAll() { return []; }
  querySelector() { return null; }
  closest() { return null; }
  focus() {}
  getContext() { return CTX; }
}

const CTX = new Proxy({}, {
  get(t, p) {
    if (p === 'measureText') return () => ({ width: 10 });
    if (p === 'createLinearGradient' || p === 'createRadialGradient') {
      return () => ({ addColorStop() {} });
    }
    if (p === 'getImageData') return () => ({ data: new Uint8ClampedArray(4) });
    if (typeof p === 'string' && !(p in t)) t[p] = () => {};
    return t[p];
  },
  set(t, p, v) { t[p] = v; return true; },
});

class FakeWS {
  static instances = [];
  constructor(url) {
    this.url = url;
    this.readyState = 0;
    this.listeners = {};
    this.sent = [];
    FakeWS.instances.push(this);
    queueMicrotask(() => {
      if (this.readyState !== 0) return;
      this.readyState = 1;
      this._emit('open', {});
    });
  }
  addEventListener(ev, fn) { (this.listeners[ev] ||= []).push(fn); }
  _emit(ev, data) { for (const fn of this.listeners[ev] || []) fn(data); }
  send(data) {
    this.sent.push(data);
    if (data === 'probe') {
      queueMicrotask(() => {
        this._emit('message', { data: 'probe' });
        queueMicrotask(() => this._emit('message', { data: 'probe' }));
      });
    }
  }
  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this._emit('close', {});
  }
}

function makeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => { m.set(k, String(v)); },
    removeItem: (k) => { m.delete(k); },
    clear: () => m.clear(),
    _map: m,
  };
}

const DIRECTORY = {
  open: [
    { room: 'westopen01', region: 'wnam', players: 1, createdAt: 1000 },
    { room: 'westlock01', region: 'wnam', players: 1, createdAt: 2000, locked: true },
    { room: 'euroopen01', region: 'weur', players: 1, createdAt: 3000 },
  ],
  inProgress: 1,
};

function makeTab(href) {
  const elements = new Map();
  const url = new URL(href);
  const tab = {
    session: makeStorage(),
    local: makeStorage(),
    fetchCalls: [],
    el(id) {
      if (!elements.has(id)) elements.set(id, new El(id === 'view' || id === 'minimap' ? 'canvas' : 'div', id));
      return elements.get(id);
    },
  };
  const location = {
    get href() { return url.href; },
    get origin() { return url.origin; },
    get pathname() { return url.pathname; },
    get search() { return url.search; },
    get hash() { return url.hash; },
  };
  const document = {
    getElementById: (id) => tab.el(id),
    createElement: (tag) => new El(tag),
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener() {},
    removeEventListener() {},
    body: new El('body'),
    documentElement: new El('html'),
    readyState: 'complete',
    hidden: false,
  };
  const history = {
    replaceState: (_a, _b, u) => {
      if (u != null) {
        const next = new URL(u, url.href);
        url.pathname = next.pathname;
        url.search = next.search;
        url.hash = next.hash;
      }
    },
    pushState() {},
  };
  const fetch = async (u) => {
    const s = String(u);
    tab.fetchCalls.push(s);
    if (s.endsWith('/regions')) {
      return { ok: true, json: async () => ({ regions: [{ id: 'wnam', label: 'North America West' }, { id: 'weur', label: 'Europe West' }] }) };
    }
    if (s.endsWith('/lobbies')) {
      return { ok: true, json: async () => DIRECTORY };
    }
    return { ok: true, json: async () => ({}) };
  };
  tab.location = location;
  tab.document = document;
  tab.history = history;
  tab.fetch = fetch;
  return tab;
}

const IMG_LOADS = [];
class FakeImage {
  constructor() { this.onload = null; this.onerror = null; }
  set src(v) { this._src = v; IMG_LOADS.push(v); }
  get src() { return this._src; }
}

function installGlobals(tab) {
  const defs = {
    document: tab.document,
    location: tab.location,
    history: tab.history,
    sessionStorage: tab.session,
    localStorage: tab.local,
    WebSocket: FakeWS,
    fetch: tab.fetch,
    Image: FakeImage,
    navigator: {
      hardwareConcurrency: 4, deviceMemory: 8, maxTouchPoints: 0,
      userAgent: 'lobby-smoke',
      clipboard: { writeText: () => Promise.resolve() },
    },
    window: {
      innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1,
      addEventListener() {}, removeEventListener() {},
    },
    requestAnimationFrame: () => 0,
    cancelAnimationFrame: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
  };
  for (const [k, v] of Object.entries(defs)) {
    Object.defineProperty(globalThis, k, { value: v, configurable: true, writable: true });
  }
}

let bootCount = 0;
async function boot(tab) {
  bootCount += 1;
  FakeWS.instances = [];
  tab.fetchCalls.length = 0;
  IMG_LOADS.length = 0;
  installGlobals(tab);
  await import(`../app.js?lobbysmoke=${bootCount}`);
  await sleep(400); // fetches, probes and renders settle
}

const roomSockets = () =>
  FakeWS.instances.filter((w) => w.url.includes('/room/') && !w.url.includes('probe-'));
const gameRows = (tab) =>
  tab.el('games-list').children.filter((c) => c.dataset && c.dataset.room);
const rowOf = (tab, room) => gameRows(tab).find((c) => c.dataset.room === room);

// ------------------------------------------------------------------ cases

const tab = makeTab('https://skirmish.online/play/');
tab.el('games-show-locked').checked = true; // the markup default
await boot(tab);

// (1) The table renders the directory in the reference columns,
//     and Connection Quality lists every region with a ping.
{
  const rows = gameRows(tab);
  assert.deepEqual(rows.map((r) => r.dataset.room),
    ['westopen01', 'westlock01', 'euroopen01'],
    'all three directory games render, oldest first');
  const head = tab.el('games-list').children[0];
  for (const label of ['Game Name', 'Players', 'Map', 'Location', 'Server']) {
    assert.ok(head.innerHTML.includes(label), `header carries ${label}`);
  }
  const west = rowOf(tab, 'westopen01');
  assert.ok(west.innerHTML.includes('1/2'), 'players render as filled/max');
  assert.ok(west.innerHTML.includes('Arabia'), 'map column is Arabia');
  assert.ok(west.innerHTML.includes('q-square'), 'server cell carries the quality square');
  const locked = rowOf(tab, 'westlock01');
  assert.ok(locked.innerHTML.includes('🔒'), 'locked row carries the lock');

  const cq = tab.el('conn-quality-list').children;
  assert.equal(cq.length, 2, 'Connection Quality lists both regions');
  assert.ok(cq[0].innerHTML.includes('North America West'), 'CQ names the region');
  assert.ok(/\d/.test(cq[0].innerHTML), 'CQ shows a measured ping number');
  assert.ok(cq[0].innerHTML.includes('q-square'), 'CQ carries quality squares');
  console.log('PASS (1) table + Connection Quality render the directory and probes');
}

// (2) Join Lobby is disabled until a row is selected; selecting
//     enables it.
{
  assert.equal(tab.el('btn-join-lobby').disabled, true, 'Join starts disabled');
  rowOf(tab, 'westopen01').dispatch('click');
  assert.equal(tab.el('btn-join-lobby').disabled, false, 'selecting a row enables Join');
  // The app marks selection via className; the stub's classList is
  // a separate set, so read the class string back.
  assert.ok(rowOf(tab, 'westopen01').className.includes('selected'), 'row highlights');
  console.log('PASS (2) selection enables Join Lobby');
}

// (3) Search filters live by title text — and a selection the
//     search hides cannot be joined.
{
  const search = tab.el('games-search');
  search.value = 'europe';
  search.dispatch('input');
  assert.deepEqual(gameRows(tab).map((r) => r.dataset.room), ['euroopen01'],
    'search leaves only the matching row');
  assert.equal(tab.el('btn-join-lobby').disabled, true,
    'the hidden selection disables Join');
  search.value = '';
  search.dispatch('input');
  assert.equal(gameRows(tab).length, 3, 'clearing the search restores the rows');
  assert.equal(tab.el('btn-join-lobby').disabled, false,
    'the selection survived and re-enables Join');
  console.log('PASS (3) search filters; hidden selection cannot join');
}

// (4) The Password toggle: off hides locked rows, on restores them.
{
  const toggle = tab.el('games-show-locked');
  toggle.checked = false;
  toggle.dispatch('change');
  assert.deepEqual(gameRows(tab).map((r) => r.dataset.room),
    ['westopen01', 'euroopen01'], 'locked row hidden with Password off');
  toggle.checked = true;
  toggle.dispatch('change');
  assert.equal(gameRows(tab).length, 3, 'locked row returns with Password on');
  console.log('PASS (4) Password toggle hides locked rows');
}

// (5) The Server filter keeps one region.
{
  const sel = tab.el('games-region-filter');
  sel.value = 'weur';
  sel.dispatch('change');
  assert.deepEqual(gameRows(tab).map((r) => r.dataset.room), ['euroopen01'],
    'Server filter keeps Europe West only');
  sel.value = '';
  sel.dispatch('change');
  assert.equal(gameRows(tab).length, 3, 'Any restores every region');
  console.log('PASS (5) Server filter keeps one region');
}

// (6) Create Lobby opens the create panel; back returns.
{
  tab.el('btn-create-open').click();
  assert.equal(tab.el('create-section').hidden, false, 'create panel opens');
  assert.equal(tab.el('games-section').hidden, true, 'browser steps aside');
  tab.el('btn-create-back').click();
  assert.equal(tab.el('create-section').hidden, true, 'create panel closes');
  assert.equal(tab.el('games-section').hidden, false, 'browser returns');
  console.log('PASS (6) Create Lobby opens the panel and back returns');
}

// (7) Selecting the locked row and pressing Join Lobby routes
//     into the room (the M10 password prompt takes it from there).
{
  rowOf(tab, 'westlock01').dispatch('click');
  assert.equal(tab.el('btn-join-lobby').disabled, false, 'locked row is selectable');
  tab.el('btn-join-lobby').click();
  await sleep(120);
  const rooms = roomSockets();
  assert.equal(rooms.length, 1, 'joining connects to the room');
  assert.ok(rooms[0].url.includes('/room/westlock01'), 'connects to the selected room');
  assert.equal(tab.el('games-section').hidden, true, 'room screen takes over');
  console.log('PASS (7) Join Lobby joins the selected (locked) room');
}

// (8) M12.1 mobile pass, verified BY CONSTRUCTION: this VM has
//     no layout engine, so no smoke can measure rendered pixels.
//     What can honestly be checked is that the shipped play.css
//     carries the phone rules the narrow layout is built from —
//     and that the desktop rules they override are still intact
//     above them. (Claudio's phone remains the visual check.)
{
  const { readFileSync } = await import('node:fs');
  const css = readFileSync(new URL('./play.css', import.meta.url), 'utf8');
  const marker = '@media (max-width: 700px)';
  const at = css.lastIndexOf(marker);
  assert.ok(at > 0, 'play.css carries the ≤700px phone media query');
  const phoneCss = css.slice(at);
  const desktopCss = css.slice(0, at);

  assert.ok(phoneCss.includes('grid-template-areas')
    && phoneCss.includes('"title title players"')
    && phoneCss.includes('"map loc server"'),
    'phone rows condense to two lines (title+players / map·region·server)');
  assert.ok(phoneCss.includes('.game-head') && phoneCss.includes('display: none'),
    'the five-column header gives way on phones');
  assert.ok(phoneCss.includes('.games-search') && phoneCss.includes('flex: 1 1 100%'),
    'Search takes a full-width line of its own');
  assert.ok(phoneCss.includes('.region-row') && phoneCss.includes('min-height: 44px'),
    'region rows are 44px thumb targets');
  assert.ok(phoneCss.includes('.slot-select') && phoneCss.includes('min-height: 44px'),
    'the guest-slot selector is a 44px thumb target');
  assert.ok(phoneCss.includes('.games-actions') && phoneCss.includes('position: sticky'),
    'the action bar sticks within the scrolling lobby card');
  assert.ok(phoneCss.includes('.ad-slot-box') && phoneCss.includes('aspect-ratio'),
    'the ad scales down inside its slot instead of overflowing');
  assert.ok(desktopCss.includes('grid-template-columns: minmax(0, 1fr) 52px 58px 128px 88px'),
    'the desktop five-column table rule is untouched');
  console.log('PASS (8) M12.1 phone rules ship in play.css (by construction; no layout engine here)');
}

// (9) The compact bottom strip (Claudio, 2026-10-19: the
//     content-sized bar covered ~half of a short desktop
//     viewport). Same honesty rule as (8): no layout engine
//     here, so assert the shipped ui.css carries the cap and
//     the fitting rules the shallow strip is built from.
{
  const { readFileSync } = await import('node:fs');
  const css = readFileSync(new URL('./ui.css', import.meta.url), 'utf8');
  const barAt = css.indexOf('.ui-bottombar');
  const barRule = css.slice(barAt, css.indexOf('}', barAt));
  assert.ok(barRule.includes('height: clamp(150px, 20vh, 232px)'),
    'the bottom strip height is capped against the viewport');
  assert.ok(css.includes('grid-template-rows: repeat(3, minmax(0, 1fr)) minmax(0, auto)'),
    'the three command rows share the strip height; the hint takes only its line(s)');
  assert.ok(!css.includes('min-height: 68px'),
    'the old 68px button floor that drove the giant bar is gone');
  const phoneAt = css.lastIndexOf('@media (max-width: 700px)');
  assert.ok(phoneAt > 0 && css.slice(phoneAt).includes('.ui-bottombar { height: clamp(134px, 27vh, 30vh)'),
    'on phones the strip takes a larger share but stays under ~30vh');
  // Legibility pass (Claudio, 2026-10-19: the horizontal cells
  // truncated names to "Lumber..." and greyed the text): the
  // command panel is DE-stacked — full name in bright parchment,
  // gold cost, and a disabled state that never dims the text.
  assert.ok(css.includes('flex: 0 0 348px'),
    'the command panel is widened to ~348px so full names fit their cells');
  const labelAt = css.indexOf('.cmd-btn .cmd-label');
  const labelRule = css.slice(labelAt, css.indexOf('}', labelAt));
  assert.ok(!labelRule.includes('ellipsis') && labelRule.includes('white-space: normal'),
    'button names never truncate — they wrap in full');
  assert.ok(labelRule.includes('-webkit-line-clamp: 2') && labelRule.includes('color: #f0e5c8'),
    'names wrap to at most two lines in bright parchment #f0e5c8');
  assert.ok(css.includes('height: clamp(18px, 3.6vh, 32px)'),
    'cell icons scale with the viewport height (DE-large on tall screens)');
  const costAt = css.indexOf('.cmd-btn .cmd-cost {');
  assert.ok(css.slice(costAt, css.indexOf('}', costAt)).includes('color: #e8c766'),
    'costs render in gold #e8c766, not the old dim grey');
  assert.ok(!css.includes('.cmd-btn:disabled { opacity')
    && css.includes('.cmd-btn:disabled .cmd-label'),
    'a disabled button darkens its cell and icon only — name and cost stay legible');
  console.log('PASS (9) compact bottom strip + legible DE-stacked command cells ship in ui.css (by construction; no layout engine here)');
}

// (10) The PNG command-icon set (2026-10-19): all twenty original
//      icons ship under ui-art/icons/ and app.js maps every one
//      of them into the command panel / selection portraits.
{
  const { existsSync, statSync, readFileSync } = await import('node:fs');
  const ICONS = ['town-center', 'house', 'farm', 'barracks', 'lumber-camp',
    'mill', 'mining-camp', 'archery-range', 'stable', 'blacksmith',
    'villager', 'militia', 'man-at-arms', 'archer', 'scout',
    'loom', 'feudal', 'fletching', 'scale-mail', 'padded-armor'];
  for (const name of ICONS) {
    const p = new URL(`./ui-art/icons/${name}.png`, import.meta.url);
    assert.ok(existsSync(p) && statSync(p).size > 1000,
      `icon ${name}.png ships in ui-art/icons/`);
  }
  const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
  for (const name of ICONS) {
    assert.ok(app.includes(`${name}.png`),
      `app.js maps the ${name} icon into a button or portrait`);
  }
  console.log('PASS (10) all 20 PNG command icons ship and are mapped in app.js');
}

console.log('LOBBY SMOKE: all cases passed');
process.exit(0);
