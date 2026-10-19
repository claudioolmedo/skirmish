// M12 AI-room smoke (stub DOM; NOT part of `node --test` — the
// file is named *.smoke.mjs on purpose). Proves the full solo
// online flow Claudio asked for: Lobby Browser → create a room
// with the guest seat set to an AI → the room screen shows the
// configuration → Start → the loading screen shows BOTH seats →
// the match begins against the AI, with the real WASM sim —
// and that along the way no lockstep handshake is ever sent
// (an AI match is a local match; there is no second client).
//
// It also proves the directory steering from the client side:
// a seat filled at create time rides the connection as listed=0,
// and changing the room-screen selector sends the host's slot
// control (listed false/true) over the room socket.
//
// Run:  node web-preview/play/airoom.smoke.mjs
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM = readFileSync(join(HERE, '..', 'pkg', 'skirmish_sim_bg.wasm'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const RELAY = 'https://skirmish-relay.hoardserdure7.workers.dev';

// ------------------------------------------------------------- DOM stub
let ctxProxy;
function makeCtx() {
  if (!ctxProxy) {
    ctxProxy = new Proxy({}, {
      get: (t, k) => {
        if (k === 'canvas') return { width: 0, height: 0 };
        if (k === 'measureText') return () => ({ width: 10 });
        if (k === 'createLinearGradient' || k === 'createRadialGradient') {
          return () => ({ addColorStop() {} });
        }
        if (typeof k === 'string') return () => {};
        return undefined;
      },
      set: () => true,
    });
  }
  return ctxProxy;
}

class ClassList {
  constructor() { this.s = new Set(); }
  add(...c) { c.forEach((x) => this.s.add(x)); }
  remove(...c) { c.forEach((x) => this.s.delete(x)); }
  toggle(c, force) {
    if (force === undefined) { this.s.has(c) ? this.s.delete(c) : this.s.add(c); }
    else if (force) this.s.add(c); else this.s.delete(c);
  }
  contains(c) { return this.s.has(c); }
}

class El {
  constructor(tag = 'div', id = '') {
    this.tagName = String(tag).toUpperCase();
    this.id = id;
    this.children = [];
    this.style = {};
    this.dataset = {};
    this.classList = new ClassList();
    this.className = '';
    this.textContent = '';
    this.innerHTML = '';
    this.value = '';
    this.checked = false;
    this.disabled = false;
    this.hidden = false;
    this.title = '';
    this.listeners = {};
    this.width = 300;
    this.height = 150;
  }
  addEventListener(t, fn) { (this.listeners[t] ||= []).push(fn); }
  removeEventListener() {}
  dispatch(t, ev = {}) {
    const e = { preventDefault() {}, stopPropagation() {}, target: this, ...ev };
    for (const fn of this.listeners[t] || []) fn(e);
    return e;
  }
  click() { this.dispatch('click'); }
  append(...kids) { this.children.push(...kids); }
  appendChild(c) { this.children.push(c); c.parentElement = this; return c; }
  prepend(c) { this.children.unshift(c); return c; }
  removeChild(c) { this.children = this.children.filter((x) => x !== c); return c; }
  remove() {}
  querySelector() { return null; }
  querySelectorAll() { return []; }
  closest() { return null; }
  getBoundingClientRect() { return { left: 0, top: 0, width: 300, height: 150 }; }
  getContext() { return makeCtx(); }
  focus() {}
  select() {}
  setPointerCapture() {}
  releasePointerCapture() {}
}

function makeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    clear: () => m.clear(),
  };
}

// ------------------------------------------------------------- fake WS
class FakeWS {
  static instances = [];
  constructor(url) {
    this.url = String(url);
    this.readyState = 0;
    this.sent = [];
    this.handlers = {};
    FakeWS.instances.push(this);
    queueMicrotask(() => {
      this.readyState = 1;
      this._emit('open', {});
      if (this.url.includes('probe-')) {
        for (let i = 0; i < 2; i += 1) {
          setTimeout(() => { try { this._emit('message', { data: 'probe' }); } catch { /* closed */ } }, 5 + i * 5);
        }
      }
    });
  }
  addEventListener(t, fn) { (this.handlers[t] ||= []).push(fn); }
  removeEventListener() {}
  _emit(t, ev) { for (const fn of this.handlers[t] || []) fn(ev); }
  send(data) { this.sent.push(String(data)); }
  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this._emit('close', {});
  }
}

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
    if (s.endsWith('.wasm')) {
      // The REAL bytes: the load genuinely instantiates.
      return new Response(new Uint8Array(WASM), {
        headers: { 'Content-Type': 'application/wasm' },
      });
    }
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
  return tab;
}

const IMG_LOADS = [];
class FakeImage {
  constructor() { this.onload = null; this.onerror = null; }
  addEventListener() {}
  removeEventListener() {}
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
      userAgent: 'airoom-smoke',
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
  await import(`../app.js?airoom=${bootCount}`);
  await sleep(400);
}

const roomSockets = () =>
  FakeWS.instances.filter((w) => w.url.includes('/room/') && !w.url.includes('probe-'));

// ------------------------------------------------------------------ (1)
// Create with the seat filled at create time: the connection
// declares listed=0, the room screen configures itself, and the
// full flow — Start → loading (both seats) → match — runs alone.
{
  const tab = makeTab('https://skirmish.online/play/');
  await boot(tab);
  assert.equal(tab.el('online-overlay').style.display, 'flex', 'lobby up at boot');
  assert.equal(tab.document.body.classList.contains('in-match'), false, 'no game visible in the lobby');

  tab.el('btn-create-open').click();
  assert.equal(tab.el('create-section').hidden, false, 'create panel opens');
  // The match settings are stated facts plus the one live choice.
  tab.el('create-guest-slot').value = 'ai-normal';
  tab.el('btn-create-game').click();
  await sleep(150);

  const rooms = roomSockets();
  assert.equal(rooms.length, 1, 'creating connects to the room');
  assert.ok(rooms[0].url.includes('host=1'), 'host connection presents host=1');
  assert.ok(rooms[0].url.includes('listed=0'), 'a seat filled at create time is declared unlisted');
  assert.equal(tab.el('room-section').hidden, false, 'room screen shows');
  assert.equal(tab.el('guest-slot-select').value, 'ai-normal', 'room selector mirrors the create choice');
  assert.ok(tab.el('slot-guest').innerHTML.includes('AI · Normal'), 'guest seat names the AI');
  assert.ok(tab.el('room-config-line').textContent.includes('1v1 · Arabia · Conquest · Guest: AI · Normal'),
    'configuration summary line reads like the reference lobby');
  assert.equal(tab.el('room-listed-line').hidden, true, 'a sealed seat shows no listed line');
  assert.equal(tab.el('btn-ready').disabled, false, 'the host may ready alone against an AI seat');
  assert.ok(tab.el('lobby-status').textContent.includes('AI · Normal'), 'status explains the AI seat');

  // Start: the loading screen shows BOTH seats before the match.
  tab.el('btn-ready').click();
  assert.equal(tab.el('loading-overlay').style.display, 'flex', 'loading screen opens on Start');
  assert.ok(tab.el('load-slot-host').innerHTML.includes('Blue — Host'), 'loading shows the host seat');
  assert.ok(tab.el('load-slot-guest').innerHTML.includes('AI · Normal'), 'loading shows the AI seat');
  assert.equal(tab.el('load-slot-guest').style.display, '', 'the AI seat row is not hidden');

  // The real load runs; the match begins against the AI.
  let began = false;
  for (let i = 0; i < 40 && !began; i += 1) {
    await sleep(250);
    began = tab.document.body.classList.contains('in-match');
  }
  assert.ok(began, 'the match begins (body.in-match) after the real load');
  assert.equal(tab.el('loading-overlay').style.display, 'none', 'loading screen closes');
  assert.equal(tab.el('online-overlay').style.display, 'none', 'lobby overlay closes');
  // No lockstep handshake ever happened: the room socket saw the
  // lobby traffic only (hello, ready) — never a 'start'.
  const types = rooms[0].sent.map((s) => { try { return JSON.parse(s).type; } catch { return '?'; } });
  assert.ok(types.includes('hello'), 'the room hello went out');
  assert.ok(!types.includes('start'), 'no lockstep start — the AI match is local');
  assert.equal(rooms[0].readyState, 3, 'the room socket closed as the match began (the room dissolved)');
  console.log('PASS (1) create with AI · Normal → room → loading (both seats) → local match vs the AI');
}

// ------------------------------------------------------------------ (2)
// Create Open, then reconfigure the seat in the room: the
// directory steering rides the socket as slot control frames,
// and a Closed seat takes Start away entirely.
{
  const tab = makeTab('https://skirmish.online/play/');
  await boot(tab);
  tab.el('btn-create-open').click();
  tab.el('btn-create-game').click();
  await sleep(150);
  const rooms = roomSockets();
  assert.equal(rooms.length, 1, 'creating connects to the room');
  assert.ok(rooms[0].url.includes('host=1'), 'host connection presents host=1');
  assert.ok(!rooms[0].url.includes('listed=0'), 'an open seat is created listed');
  assert.equal(tab.el('btn-ready').disabled, true, 'alone with an open seat, Start waits for a guest');

  const sel = tab.el('guest-slot-select');
  sel.value = 'ai-easy';
  sel.dispatch('change');
  assert.ok(rooms[0].sent.includes('{"type":"slot","listed":false}'),
    'filling the seat sends the delist control');
  assert.equal(tab.el('btn-ready').disabled, false, 'an AI seat readies the host alone');
  assert.ok(tab.el('slot-guest').innerHTML.includes('AI · Easy'), 'guest seat names AI · Easy');

  sel.value = 'closed';
  sel.dispatch('change');
  assert.equal(tab.el('btn-ready').disabled, true, 'a closed seat cannot start');
  assert.ok(tab.el('slot-guest').innerHTML.includes('Closed'), 'guest seat reads Closed');

  sel.value = 'open';
  sel.dispatch('change');
  assert.ok(rooms[0].sent.includes('{"type":"slot","listed":true}'),
    'reopening the seat sends the relist control');
  assert.equal(tab.el('btn-ready').disabled, true, 'open again: Start waits for a human');
  console.log('PASS (2) the room-screen selector steers the directory over the socket');
}

console.log('AIROOM SMOKE: all cases passed');
process.exit(0);
