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
  await import(`../app.js?roomurl2=${bootCount}`);
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
// roomurl2 (2026-11-26, the live-proof failures of roomurl):
//  A. A reloaded guest never regained presence: its fresh hello
//     earned no reply (the one hello reply was spent) and the
//     host's re-invite did not exist. onPeerGone (real notice)
//     now resets helloReplied AND sends a fresh hello, covering
//     both orders (notice-first and hello-first/lag).
//  B. A stranger's hello mid-match could draw the RESYNC when
//     the session looked stalled (the F1 seat-silence guess) —
//     it walked into a running match in the live proof. A bare
//     hello in 'playing' now ALWAYS answers 'in-progress'; the
//     resync only ever follows the relay's peer-left (proof the
//     seat is empty) plus the returner's answering hello.

async function bootHost() {
  const tab = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the claim boot created the room socket');
  return { tab, sock };
}
const hellosSent = (sock) =>
  sentFrames(sock).filter((f) => f.type === 'hello').length;
const guestSeat = (tab) => tab.el('slot-guest').innerHTML;
const say = (sock, obj) =>
  sock._emit('message', { data: JSON.stringify(obj) });
const guestHello = { type: 'hello', v: 1, role: 'guest', name: 'Guest' };

check('A1: notice first — the departure re-invites, the returner is answered, presence reconverges', async () => {
  const { tab, sock } = await bootHost();
  say(sock, guestHello);
  await sleep(60);
  assert.equal(hellosSent(sock), 2, 'connect hello + the one reply');
  assert.ok(guestSeat(tab).includes('Guest'), 'the guest seat is occupied');
  say(sock, { type: 'peer-left' });
  await sleep(60);
  assert.equal(hellosSent(sock), 3,
    'the departure notice draws a fresh re-invite hello');
  assert.ok(guestSeat(tab).includes('Waiting'), 'the seat shows empty after the departure');
  say(sock, guestHello); // the returner answers the re-invite
  await sleep(60);
  assert.equal(hellosSent(sock), 4,
    'the returner hello earns a reply again — the reply budget was reset');
  assert.ok(guestSeat(tab).includes('Guest'),
    'presence reconverges: the guest seat is occupied again');
});

check('A2: lag order — the returner knocks BEFORE the notice; the notice still re-invites and the answer settles', async () => {
  const { tab, sock } = await bootHost();
  say(sock, guestHello);
  await sleep(60);
  assert.equal(hellosSent(sock), 2, 'connect hello + the one reply');
  say(sock, guestHello); // the returner's early knock (old socket not yet noticed gone)
  await sleep(60);
  assert.equal(hellosSent(sock), 2, 'no second reply while the reply is spent');
  say(sock, { type: 'peer-left' }); // the notice lands late
  await sleep(60);
  assert.equal(hellosSent(sock), 3,
    'the late notice still sends the re-invite to the waiting returner');
  say(sock, guestHello); // the returner answers the re-invite
  await sleep(60);
  assert.ok(guestSeat(tab).includes('Guest'),
    'presence reconverges after the lag order too');
});

check('B: source contract — a bare hello in play can never draw the resync', () => {
  const src = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
  const helloCase = src.slice(src.indexOf("case 'hello':"),
    src.indexOf("case 'ready':"));
  assert.equal((helloCase.match(/sendResync\(\)/g) || []).length, 1,
    'exactly one resync answer inside the hello case: the peer-left branch');
  assert.ok(!helloCase.includes('lastTurnAt'),
    'the stall guess (lastTurnAt) is gone from the hello case');
  assert.ok(helloCase.includes("type: 'in-progress'"),
    'the third hello in play is answered in-progress');
  const gone = src.slice(src.indexOf('function onPeerGone'),
    src.indexOf('function onPeerBack'));
  assert.ok(gone.includes('net.helloReplied = false'),
    'the departure resets the hello reply (lobby rejoin)');
  assert.ok((gone.match(/sendHello\(\)/g) || []).length >= 2,
    'the departure re-invites from both the lobby and the peer-left branches');
  assert.ok(src.includes('onPeerGone(true)'),
    'only the real peer-left notice passes the re-invite flag');
});
