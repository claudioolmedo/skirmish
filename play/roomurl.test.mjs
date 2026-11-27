// roomurl (Claudio's direction, 2026-11-26): the game's link
// belongs IN the browser address bar while you play, and a
// late opener of that link gets a landing instead of a dead
// wait. The behaviours pinned here, all in the real ../app.js
// (the readylock harness pattern — one real client per case,
// the peer played by injected frames):
//
//   1. Entering a room (claim/create or link join) puts the
//      room's play URL in the bar and writes the bar-room mark;
//      creating also writes the hosted mark.
//   2. A RELOAD of that bar re-enters: as guest with only the
//      bar-room mark, as HOST (re-claim) with the hosted mark
//      too — while a fresh link click keeps the old
//      consume-once rule (mark alone, no bar-room mark: the
//      boot lands on the front door).
//   3. A private room's reload lands on the create view with
//      the code still armed (its password lived only in the
//      old page's memory); Create reopens it under the same
//      link.
//   4. A stranger's hello against a match that is starting
//      (third hello, peer present) is answered 'in-progress'
//      and nothing else; the opener's client shows the closed
//      landing ("already under way"), whose Browse button
//      leaves to the front door and clears the bar.
//   5. 'room-full' shows the same landing with the full copy.
//   6. Fifteen seconds of total silence in a joined room
//      shows the soft waiting notice; a hello afterwards
//      dissolves it (waiting stays the default — nobody is
//      ever ejected).
//   7. A migration moves the bar (and the mark) to the new
//      room code.
//
// Run from web-preview/play/: node --test roomurl.test.mjs

import { test, after } from 'node:test';

// The booted app.js keeps lobby/heartbeat intervals alive, so
// the test process would never drain its event loop on its own
// (the reason the *.smoke.mjs harnesses end in process.exit).
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
    WebSocket: FakeWS,
    fetch: tab.fetch,
    Image: FakeImage,
    navigator: {
      hardwareConcurrency: 4, deviceMemory: 8, maxTouchPoints: 0,
      userAgent: 'roomurl-test',
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
  installGlobals(tab);
  await import(`../app.js?roomurl=${bootCount}`);
  await sleep(400);
}

const roomSocket = () =>
  FakeWS.instances.find((w) => w.url.includes('/room/') && !w.url.includes('probe-'));

const sentFrames = (sock) => sock.sent
  .map((d) => { try { return JSON.parse(d); } catch { return null; } })
  .filter(Boolean);

async function waitForStart(sock, ms = 5000) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    if (sentFrames(sock).some((f) => f.type === 'start')) return true;
    await sleep(100);
  }
  return false;
}

// The room code the homepage minted in Claudio's report (w =
// wnam in char 1, the rest from the link alphabet).
const CODE = 'wbdtwx2';


// ------------------------------------------------------------------ cases

check('entering a room puts its link in the address bar (claim + join)', async () => {
  const host = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(host);
  assert.ok(roomSocket(), 'the claim boot created the room');
  assert.ok(host.location.search.includes(`room=${CODE}`),
    `the host bar names the room, got: ${host.location.search}`);
  assert.equal(host.session.getItem('skirmish2.barRoom'), CODE,
    'the bar-room mark is written');
  assert.equal(host.session.getItem('skirmish2.hostedRoom'), CODE,
    'creating the room writes the hosted mark');

  const guest = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(guest);
  assert.ok(roomSocket(), 'the link boot joined the room');
  assert.ok(guest.location.search.includes(`room=${CODE}`),
    `the guest bar names the room, got: ${guest.location.search}`);
  assert.equal(guest.session.getItem('skirmish2.barRoom'), CODE);
  assert.equal(guest.session.getItem('skirmish2.hostedRoom'), null,
    'a guest never writes the hosted mark');
});

check('a fresh link click keeps the consume-once rule (no bar-room mark, no rejoin)', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  tab.session.setItem('skirmish2.roomLinkConsumed', CODE);
  await boot(tab);
  assert.equal(roomSocket(), undefined,
    'a consumed link with no bar-room mark lands on the front door');
});

check('a reload of the bar rejoins as guest (bar-room mark only)', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  tab.session.setItem('skirmish2.roomLinkConsumed', CODE);
  tab.session.setItem('skirmish2.barRoom', CODE);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the reload re-entered the room despite the consumed mark');
  const hello = sentFrames(sock).find((f) => f.type === 'hello');
  assert.equal(hello.role, 'guest', 'it rejoins as guest');
});

check('a reload of the bar re-claims the host seat (bar-room + hosted marks)', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  tab.session.setItem('skirmish2.barRoom', CODE);
  tab.session.setItem('skirmish2.hostedRoom', CODE);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the reload re-created the room');
  const hello = sentFrames(sock).find((f) => f.type === 'hello');
  assert.equal(hello.role, 'host', 'it re-claims as host');
  assert.ok(tab.location.search.includes(`room=${CODE}`),
    'the bar names the room again');
});

check('a private room reload lands on the create view with the code armed', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}&private=1`);
  tab.session.setItem('skirmish2.barRoom', CODE);
  tab.session.setItem('skirmish2.hostedRoom', CODE);
  await boot(tab);
  assert.equal(roomSocket(), undefined,
    'no room is created before the password is set again');
  assert.equal(tab.el('create-section').hidden, false,
    'the create view is shown');
  assert.ok(tab.el('lobby-status').textContent.includes('Welcome back'),
    `the status explains the reopening, got: ${tab.el('lobby-status').textContent}`);
  tab.el('btn-create-game').click();
  await sleep(120);
  const sock = roomSocket();
  assert.ok(sock && sock.url.includes(`/room/${CODE}`),
    'Create reopens the room under the very same code');
});

check("a stranger's hello against a starting match is answered in-progress, nothing else", async () => {
  const tab = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the claim boot created the room');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Guest' });
  await sleep(60);
  peerSay({ type: 'ready', ready: true });
  await sleep(60);
  tab.el('btn-ready').click();
  assert.ok(await waitForStart(sock), 'the match is starting');
  const before = sentFrames(sock).length;
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Guest' }); // the peer's reply hello (2nd)
  await sleep(60);
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Stranger' }); // a third client (3rd)
  await sleep(60);
  const after = sentFrames(sock).slice(before);
  assert.ok(after.some((f) => f.type === 'in-progress'),
    'the stranger is answered in-progress');
  assert.ok(!after.some((f) => f.type === 'start' || f.type === 'resync'),
    'nothing about the match itself is re-sent to anyone');
});

check('the opener of an under-way match gets the closed landing; Browse leaves and clears the bar', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the link boot joined the room channel');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'in-progress' });
  await sleep(60);
  assert.equal(tab.el('room-closed-panel').hidden, false,
    'the closed landing shows');
  assert.ok(tab.el('room-closed-text').textContent.includes('already under way'),
    `with the under-way words, got: ${tab.el('room-closed-text').textContent}`);
  tab.el('btn-closed-browse').click();
  await sleep(60);
  assert.equal(tab.location.search, '',
    `Browse leaves to the front door with a clean bar, got: ${tab.location.search}`);
  assert.equal(tab.session.getItem('skirmish2.barRoom'), null,
    'the bar-room mark is gone');
});

check('room-full shows the closed landing with the full copy', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the link boot joined the room channel');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'room-full' });
  await sleep(60);
  assert.equal(tab.el('room-closed-panel').hidden, false,
    'the closed landing shows');
  assert.ok(tab.el('room-closed-text').textContent.includes('room is full'),
    `with the full words, got: ${tab.el('room-closed-text').textContent}`);
});

check('fifteen seconds of silence shows the soft waiting notice; a hello dissolves it', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the link boot joined the room');
  await sleep(1000);
  assert.equal(tab.el('room-closed-panel').hidden, true,
    'no notice while the wait is young');
  const t0 = Date.now();
  let shown = false;
  while (Date.now() - t0 < 22000) {
    if (tab.el('room-closed-panel').hidden === false) { shown = true; break; }
    await sleep(500);
  }
  assert.ok(shown, 'the waiting notice appears after ~15 s of silence');
  assert.ok(tab.el('room-closed-text').textContent.includes('Still waiting'),
    `with the soft words, got: ${tab.el('room-closed-text').textContent}`);
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'host', name: 'Host' });
  await sleep(80);
  assert.equal(tab.el('room-closed-panel').hidden, true,
    'the host arriving dissolves the notice — the wait was never ended');
});

check('a migration moves the bar and the mark to the new room code', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the link boot joined the room');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'host', name: 'Host' });
  await sleep(60);
  peerSay({ type: 'migrate', region: 'weur', code: 'ubdtwx2', nonce: 'nonce-1234' });
  await sleep(120);
  assert.ok(tab.location.search.includes('room=ubdtwx2'),
    `the bar followed the room to the new code, got: ${tab.location.search}`);
  assert.equal(tab.session.getItem('skirmish2.barRoom'), 'ubdtwx2',
    'the bar-room mark followed too');
});
