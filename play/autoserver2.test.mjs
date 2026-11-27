// Regression tests for the autoserver decision never firing live
// (Claudio's feature, live proof 2026-11-26 ~20:09 PT): two real
// tabs in a room for 15 s — neither screen ever showed "Server: …
// — automatic (best for both)"; both kept the plain server line.
// autoserver.test.mjs stayed green because it is netplay unit
// tests + app.js source pins: the app-level integration was never
// executed, and its FakeWS opens in a microtask.
//
// Root cause (autoserver2 diagnosis, verified link by link — the
// codec, the relay and the message flow are all sound): the room
// sweep's probe budget. probeAllRegions gave each probeRegionPing
// a 1500 ms timeout, and a probe whose WebSocket has not OPENED
// by then resolves null (the handshake fallback needs 'open' to
// have fired). Every probe targets a fresh, cold Durable Object;
// from a slower vantage point all seven opens can miss 1.5 s at
// once. An all-null table on either side makes bestRegionForPair
// return null ("no basis"), and the decision closed silently —
// no 'server-ok', no retry, plain line on both screens. The
// lobby's own probes use the same probeRegionPing with its
// default 7000 ms timeout and demonstrably work live.
//
// The fix:
//   - probeAllRegions' budget 1500 → 4000 ms (still bounded,
//     parallel, never blocking the room UI);
//   - an all-null OWN table no longer closes the exchange: the
//     sweep withholds it and retries ONCE per pairing after a
//     short delay; if the retry also measures nothing, today's
//     silent fallback proceeds exactly as before.
//
// These tests drive the REAL ../app.js in Node against a stub
// DOM and a stub WebSocket whose PROBE sockets open LATE, the
// way the real relay behaves from a distant browser (the
// readylock.test.mjs harness pattern). Run from web-preview/play/:
// node --test autoserver2.test.mjs

import { test, after } from 'node:test';

// The booted app.js keeps lobby/heartbeat intervals alive, so
// the test process would never drain its event loop on its own.
// Count failures ourselves and exit with the honest code once
// the reporter has had a moment to flush.
let failures = 0;
const check = (name, fn) => test(name, async () => {
  try { await fn(); } catch (e) { failures += 1; throw e; }
});
after(() => { setTimeout(() => process.exit(failures ? 1 : 0), 300); });
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const WASM_BYTES = readFileSync(new URL('../pkg/skirmish_sim_bg.wasm', import.meta.url));

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

// A WebSocket whose PROBE sockets open late, per LateWS.plan:
//   { mode: 'late', lateMs }        — every probe socket opens
//                                     lateMs after construction;
//   { mode: 'firstSlow', slowMs, slowUntil }
//                                   — probe sockets created before
//                                     the absolute time slowUntil
//                                     open slowMs after construction
//                                     (past even a raised budget);
//                                     later ones open at once.
// Room sockets always open in a microtask, as in readylock.
class LateWS {
  static instances = [];
  static plan = { mode: 'late', lateMs: 2000 };
  constructor(url) {
    this.url = url;
    this.readyState = 0;
    this.listeners = {};
    this.sent = [];
    LateWS.instances.push(this);
    const open = () => {
      if (this.readyState !== 0) return;
      this.readyState = 1;
      this._emit('open', {});
    };
    if (url.includes('probe-')) {
      const plan = LateWS.plan;
      if (plan.mode === 'firstSlow' && Date.now() >= plan.slowUntil) {
        queueMicrotask(open);
      } else {
        const ms = plan.mode === 'firstSlow' ? plan.slowMs : plan.lateMs;
        setTimeout(open, ms);
      }
    } else {
      queueMicrotask(open);
    }
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
  tab.body = tab.el('body');
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
    createElementNS: (_ns, tag) => new El(tag),
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener() {},
    removeEventListener() {},
    body: tab.body,
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
      return new Response(new Uint8Array(WASM_BYTES), {
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

class FakeImage {
  static plan = { mode: 'load', delayMs: 0 };
  constructor() {
    this.onload = null; this.onerror = null;
    this._listeners = {};
    this.complete = false;
    this.naturalWidth = 0;
    this.naturalHeight = 0;
  }
  addEventListener(ev, fn) { (this._listeners[ev] ||= []).push(fn); }
  removeEventListener(ev, fn) {
    const l = this._listeners[ev] || [];
    const i = l.indexOf(fn);
    if (i >= 0) l.splice(i, 1);
  }
  set src(v) {
    this._src = v;
    const plan = FakeImage.plan;
    if (plan.mode === 'hang') return;
    setTimeout(() => {
      this.complete = true;
      this.naturalWidth = 64;
      this.naturalHeight = 64;
      for (const fn of this._listeners.load || []) fn({});
      if (this.onload) this.onload({});
    }, plan.delayMs || 0);
  }
  get src() { return this._src; }
}

function installGlobals(tab) {
  const defs = {
    document: tab.document,
    location: tab.location,
    history: tab.history,
    sessionStorage: tab.session,
    localStorage: tab.local,
    WebSocket: LateWS,
    fetch: tab.fetch,
    Image: FakeImage,
    navigator: {
      hardwareConcurrency: 4, deviceMemory: 8, maxTouchPoints: 0,
      userAgent: 'autoserver2-test',
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
  LateWS.instances = [];
  tab.fetchCalls.length = 0;
  installGlobals(tab);
  await import(`../app.js?autoserver2=${bootCount}`);
  await sleep(400);
}

const roomSocket = () =>
  LateWS.instances.find((w) => w.url.includes('/room/') && !w.url.includes('probe-'));

const sentFrames = (sock) => sock.sent
  .map((d) => { try { return JSON.parse(d); } catch { return null; } })
  .filter(Boolean);

const tableMeasured = (table) =>
  !!table && Object.values(table).some((v) => typeof v === 'number');

async function waitForPings(sock, ms, { measured = true } = {}) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    const f = sentFrames(sock).find(
      (fr) => fr.type === 'pings' && (!measured || tableMeasured(fr.table)));
    if (f) return f;
    await sleep(100);
  }
  return null;
}

async function waitForFrame(sock, type, ms) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    const f = sentFrames(sock).find((fr) => fr.type === type);
    if (f) return f;
    await sleep(100);
  }
  return null;
}

// The room code from Claudio's live tests (w = wnam in char 1).
const CODE = 'wbdtwx2';
// A peer table that favours the room's own region (wnam): the
// pair's best is the current region, so a working decision is a
// stay — 'server-ok' plus the automatic line, no migration.
const PEER_TABLE = {
  wnam: 40, enam: 120, sam: 200, weur: 180, eeur: 210, apac: 250, oc: 260,
};

// ------------------------------------------------------------------ cases

check('host: probes opening at ~2 s still produce a decision (server-ok + automatic line)', async () => {
  LateWS.plan = { mode: 'late', lateMs: 2000 };
  const tab = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the claim boot created the room and connected');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Guest' });
  const pings = await waitForPings(sock, 9000);
  assert.ok(pings,
    'with probe sockets opening at ~2 s, the host must still share a MEASURED ping table (on the broken code the sweep times out at 1.5 s and the shared table is all null)');
  peerSay({ type: 'pings', table: PEER_TABLE });
  const ok = await waitForFrame(sock, 'server-ok', 4000);
  assert.ok(ok, 'with both tables in and wnam best for the pair, the host confirms the stay with server-ok');
  const line = tab.el('room-server-line').textContent;
  assert.ok(line.includes('automatic (best for both)'),
    `the host's server line must read automatic, got: ${line}`);
});

check('host: a first sweep slower than the whole budget is retried once and still decides', async () => {
  // Probe sockets created in the first seconds open at 4.5 s —
  // past even the raised 4 s budget — so the first sweep measures
  // nothing. The one retry (its sockets open at once) must still
  // produce a measured table and the decision.
  LateWS.plan = { mode: 'firstSlow', slowMs: 4500, slowUntil: Date.now() + 5500 };
  const tab = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the claim boot created the room and connected');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Guest' });
  const pings = await waitForPings(sock, 16000);
  assert.ok(pings,
    'an all-null first sweep must be retried once per pairing, and the retry must share a measured table (on the broken code the all-null table is shared at once and the exchange closes forever)');
  peerSay({ type: 'pings', table: PEER_TABLE });
  const ok = await waitForFrame(sock, 'server-ok', 4000);
  assert.ok(ok, 'after the retry, the pair decision still lands (server-ok)');
});

check('guest: late probes still share a measured table, and server-ok turns its line automatic', async () => {
  LateWS.plan = { mode: 'late', lateMs: 2000 };
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the link boot joined the room and connected');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'host', name: 'Host' });
  const pings = await waitForPings(sock, 9000);
  assert.ok(pings,
    'with probe sockets opening at ~2 s, the guest must still share a MEASURED ping table');
  peerSay({ type: 'server-ok', region: 'wnam' });
  await sleep(120);
  const line = tab.el('room-server-line').textContent;
  assert.ok(line.includes('automatic (best for both)'),
    `the guest's server line must read automatic after server-ok, got: ${line}`);
});
