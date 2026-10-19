// Stub-DOM smoke for the reload rule (Claudio's direction):
// reloading /play must land on the Lobby Browser — a room link
// is consumed exactly once per tab session, the address bar is
// cleaned of room params, and the game (.wasm) loads only when a
// match is actually entered.
//
// This drives the REAL ../app.js in Node against a stub DOM, a
// stub WebSocket and a stub fetch (no network, no live relay).
// Run from web-preview/play/:
//   node reload.smoke.mjs
// It is a smoke, not a unit test: it is not picked up by
// `node --test *.test.mjs`.

import assert from 'node:assert/strict';

const MARK_KEY = 'skirmish2.roomLinkConsumed';
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
    // Region probes expect echoes: answer 'probe' twice (cold+warm).
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

// One "tab": a full set of globals. A reload = re-importing app.js
// with the same tab (same sessionStorage); a fresh tab = new tab.
function makeTab(href, session = makeStorage()) {
  const elements = new Map();
  const url = new URL(href);
  const tab = {
    session,
    local: makeStorage(),
    fetchCalls: [],
    replaceCalls: [],
    clipWrites: [],
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
      tab.replaceCalls.push(u);
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
      return { ok: true, json: async () => ({ open: [], inProgress: 0 }) };
    }
    return { ok: true, json: async () => ({}) };
  };
  tab.location = location;
  tab.document = document;
  tab.history = history;
  tab.fetch = fetch;
  tab.navigate = (next) => {
    const n = new URL(next, url.href);
    url.pathname = n.pathname;
    url.search = n.search;
    url.hash = n.hash;
  };
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
      userAgent: 'reload-smoke',
      clipboard: { writeText: (t) => { tab.clipWrites.push(t); return Promise.resolve(); } },
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
  await import(`../app.js?smoke=${bootCount}`);
  await sleep(350); // let open handlers, probes and fetches settle
}

const roomSockets = () =>
  FakeWS.instances.filter((w) => w.url.includes('/room/') && !w.url.includes('probe-'));
const noWasmFetched = (tab) =>
  !tab.fetchCalls.some((u) => u.endsWith('.wasm')) && IMG_LOADS.length === 0;

// ------------------------------------------------------------------ cases

const LINK_ROOM = 'abc123def4';
const LINK = `https://skirmish.online/play/?room=${LINK_ROOM}&region=wnam`;

// (a) Fresh tab opens the link: joins the room; the mark is set;
//     the address bar is cleaned; no game load happens.
{
  const tab = makeTab(LINK);
  await boot(tab);
  const rooms = roomSockets();
  assert.equal(rooms.length, 1, 'fresh link open connects to the room');
  assert.ok(rooms[0].url.includes(`/room/${LINK_ROOM}`), 'connects to the link room');
  assert.equal(tab.session.getItem(MARK_KEY), LINK_ROOM, 'consume mark written');
  assert.equal(tab.location.search, '', 'address bar cleaned of room params');
  assert.equal(tab.el('games-section').hidden, true, 'browser sections hidden in the room screen');
  assert.equal(tab.el('loading-overlay').style.display, 'none', 'loading overlay stays hidden');
  assert.ok(noWasmFetched(tab), 'no .wasm fetched while joining');
  console.log('PASS (a) fresh link open joins, marks, cleans the URL');

  // (b) Reload in the SAME tab (same sessionStorage, URL back on
  //     the link — e.g. the copied link or a restored tab): the
  //     Lobby Browser opens; no join attempt; no game load.
  tab.navigate(LINK);
  tab.replaceCalls.length = 0;
  await boot(tab);
  assert.equal(roomSockets().length, 0, 'reload does NOT re-join the room');
  assert.equal(tab.el('games-section').hidden, false, 'Lobby Browser is shown');
  assert.equal(tab.el('online-overlay').style.display, 'flex', 'lobby overlay is the first screen');
  assert.equal(tab.el('loading-overlay').style.display, 'none', 'loading overlay stays hidden');
  assert.equal(tab.location.search, '', 'stale room params are cleaned on reload too');
  assert.ok(noWasmFetched(tab), 'no .wasm fetched on reload');
  console.log('PASS (b) reload lands on the Lobby Browser');

  // (c) Same tab, a DIFFERENT room's link: a fresh join (the
  //     consume mark is per-room).
  const OTHER = 'xyz987abcd';
  tab.navigate(`https://skirmish.online/play/?room=${OTHER}&region=weur`);
  await boot(tab);
  const rooms2 = roomSockets();
  assert.equal(rooms2.length, 1, 'a different link still joins');
  assert.ok(rooms2[0].url.includes(`/room/${OTHER}`));
  assert.equal(tab.session.getItem(MARK_KEY), OTHER, 'mark moves to the new room');
  assert.equal(tab.location.search, '', 'address bar cleaned again');
  console.log('PASS (c) a different room link joins in the same tab');
}

// (d) Create flow: boot plain, click Create — a room is created,
//     location.search stays empty, and Copy builds the invite
//     link from in-memory state on demand.
{
  const tab = makeTab('https://skirmish.online/play/');
  await boot(tab);
  assert.equal(roomSockets().length, 0, 'plain boot opens no room');
  tab.el('btn-create-game').click();
  await sleep(100);
  const rooms = roomSockets();
  assert.equal(rooms.length, 1, 'create connects the new room');
  const m = rooms[0].url.match(/\/room\/([a-z0-9]+)/);
  assert.ok(m, 'room id in the socket URL');
  assert.equal(tab.location.search, '', 'creating a room leaves location.search empty');
  assert.ok(!tab.location.href.includes('room='), 'no room params in the address bar');
  assert.equal(tab.el('loading-overlay').style.display, 'none', 'loading overlay stays hidden');
  tab.el('btn-copy-link').click();
  await sleep(20);
  assert.equal(tab.clipWrites.length, 1, 'copy writes the invite link');
  assert.ok(tab.clipWrites[0].includes(`room=${m[1]}`), 'copied link carries the in-memory room');
  assert.ok(tab.clipWrites[0].includes('region='), 'copied link carries the region');
  assert.equal(tab.location.search, '', 'address bar still clean after copying');
  assert.ok(noWasmFetched(tab), 'no .wasm fetched while creating');
  console.log('PASS (d) create keeps the URL clean; copy builds the link on demand');
}

console.log('RELOAD SMOKE: all cases passed');
process.exit(0);
