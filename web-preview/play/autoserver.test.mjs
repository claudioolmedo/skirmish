// Auto-server tests (Claudio's direction, 2026-11-26): once both
// players are in a room, the game measures BOTH players' pings to
// every region and automatically puts BOTH on the server that is
// best for the pair — no manual picking.
//
// The pure decision logic lives in play/netplay.js
// (sanitizePingTable / bestRegionForPair / roomCodeForRegion /
// makeMigrateNonce / probeAllRegions) and play/room.js
// (checkNonce); the wire shapes ride the netplay codec ('pings',
// 'migrate', 'server-ok'). The app.js wiring — probe kick on
// presence, the host's decision, the one-time migration over the
// ordinary connectToRoom, the nonce admission in the hello case,
// the room screen's server line — is pinned here from source,
// the glink.test.mjs pattern.
//
// Client only: nothing here touches the sim or the golden hash,
// and the exchange never blocks ready/start (a peer whose table
// never arrives leaves the room on its original region).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  REGIONS, decodeMessage, encodeMessage,
  sanitizePingTable, bestRegionForPair, roomCodeForRegion,
  makeMigrateNonce, probeAllRegions, regionFromRoomId,
} from './netplay.js';
import { checkNonce, checkPassword } from './room.js';
import { STRINGS } from './i18n.js';

const appJs = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const playHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8');

function functionBody(src, name) {
  const start = src.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `${name} must exist in app.js`);
  const open = src.indexOf('{', start);
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}') { depth--; if (depth === 0) return src.slice(open, i + 1); }
  }
  throw new Error(`unbalanced ${name}`);
}

function caseBody(src, label) {
  const at = src.indexOf(`case '${label}':`);
  assert.ok(at >= 0, `case '${label}' must exist in app.js`);
  const next = src.indexOf("\n    case '", at + 1);
  return src.slice(at, next > at ? next : at + 2000);
}

// ------------------------------------------------------------ the pair rule

test('bestRegionForPair minimizes the worse player\'s ping (max rule)', () => {
  // Host is glued to wnam, guest to weur; enam is nobody's best
  // but decent for both — the pair plays at its slowest member,
  // so the middle server wins over either extreme.
  const host = { wnam: 20, enam: 60, sam: 90, weur: 200, eeur: 230, apac: 300, oc: 320 };
  const guest = { wnam: 210, enam: 70, sam: 120, weur: 25, eeur: 60, apac: 180, oc: 200 };
  assert.equal(bestRegionForPair(host, guest), 'enam');
});

test('bestRegionForPair: a tie on max is broken by the sum', () => {
  const host = { wnam: 50, enam: 40 };
  const guest = { wnam: 80, enam: 90 };
  // max ties at 80/90? no: wnam max 80, enam max 90 — wnam wins outright.
  assert.equal(bestRegionForPair(host, guest), 'wnam');
  const host2 = { wnam: 50, enam: 30 };
  const guest2 = { wnam: 90, enam: 90 };
  // max ties at 90; sums 140 vs 120 — enam wins.
  assert.equal(bestRegionForPair(host2, guest2), 'enam');
});

test('bestRegionForPair: a tie on max and sum is broken by the host ping', () => {
  const host = { wnam: 30, enam: 90 };
  const guest = { wnam: 90, enam: 30 };
  // max 90 both, sum 120 both; host ping 30 < 90 — wnam wins.
  assert.equal(bestRegionForPair(host, guest), 'wnam');
  const hostB = { wnam: 90, enam: 30 };
  const guestB = { wnam: 30, enam: 90 };
  // The mirror image: max and sum still tied, host ping now
  // favours enam — the tie-break follows the host's ping.
  assert.equal(bestRegionForPair(hostB, guestB), 'enam');
});

test('bestRegionForPair: a full tie falls back to REGIONS list order', () => {
  const host = { enam: 50, wnam: 50 };
  const guest = { enam: 50, wnam: 50 };
  assert.equal(bestRegionForPair(host, guest), 'wnam'); // wnam precedes enam in REGIONS
});

test('bestRegionForPair: unmeasured is infinitely far for that player', () => {
  // The guest cannot reach wnam at all; enam is reachable by
  // both and wins despite the host's tiny wnam ping.
  const host = { wnam: 5, enam: 80 };
  const guest = { wnam: null, enam: 90 };
  assert.equal(bestRegionForPair(host, guest), 'enam');
  // Each side reaches exactly one, different, region: every max
  // is infinite, every sum infinite — the host-ping tie-break
  // still lands deterministically on the host's reachable one.
  const host2 = { wnam: 40 };
  const guest2 = { weur: 40 };
  assert.equal(bestRegionForPair(host2, guest2), 'wnam');
});

test('bestRegionForPair: no basis, no decision (either side measured nothing)', () => {
  assert.equal(bestRegionForPair({}, { wnam: 10 }), null);
  assert.equal(bestRegionForPair({ wnam: 10 }, {}), null);
  assert.equal(bestRegionForPair(null, null), null);
  assert.equal(bestRegionForPair({ wnam: null, enam: null }, { wnam: 10 }), null);
  // Junk is not measurement.
  assert.equal(bestRegionForPair({ wnam: 'fast', enam: -3, sam: NaN }, { wnam: 10 }), null);
});

test('sanitizePingTable fills every region and drops junk', () => {
  const t = sanitizePingTable({ wnam: 42, weur: 'x', mars: 1, apac: -1, oc: 0 });
  assert.deepEqual(Object.keys(t).sort(), REGIONS.map((r) => r.id).sort());
  assert.equal(t.wnam, 42);
  assert.equal(t.weur, null);
  assert.equal(t.apac, null);
  assert.equal(t.oc, 0);
  assert.equal(t.mars, undefined);
  assert.deepEqual(sanitizePingTable(null), sanitizePingTable({}));
});

// ------------------------------------------------------------ codes & nonce

test('roomCodeForRegion swaps only the prefix — the suffix (room identity) survives', () => {
  assert.equal(roomCodeForRegion('wlehsls', 'weur'), 'ulehsls');
  assert.equal(roomCodeForRegion('ulehsls', 'wnam'), 'wlehsls');
  assert.equal(regionFromRoomId(roomCodeForRegion('wlehsls', 'apac')), 'apac');
  assert.equal(roomCodeForRegion('wlehsls', 'nowhere'), null);
  assert.equal(roomCodeForRegion('short', 'weur'), null);
  assert.equal(roomCodeForRegion(null, 'weur'), null);
});

test('makeMigrateNonce mints 16 alphabet chars from the given randomness', () => {
  let i = 0;
  const seq = () => { i = (i + 0.07) % 1; return i; };
  const nonce = makeMigrateNonce(seq);
  assert.equal(nonce.length, 16);
  assert.ok(/^[abcdefghjkmnpqrstuvwxyz23456789]{16}$/.test(nonce));
  assert.notEqual(makeMigrateNonce(() => 0), makeMigrateNonce(() => 0.999));
});

test('checkNonce admits only an exact match against a set nonce', () => {
  assert.equal(checkNonce('abc123nonce45678', 'abc123nonce45678'), true);
  assert.equal(checkNonce('abc123nonce45678', 'abc123nonce45679'), false);
  assert.equal(checkNonce('', ''), false); // an unset nonce admits nothing
  assert.equal(checkNonce(null, 'abc123nonce45678'), false);
  assert.equal(checkNonce('abc123nonce45678', null), false);
  assert.equal(checkNonce('abc123nonce45678', 42), false);
  // Same discipline as the password check it stands beside.
  assert.equal(checkPassword('', ''), false);
});

// ------------------------------------------------------------ the wire shapes

test('codec: pings / migrate / server-ok round-trip and junk is rejected', () => {
  const table = { wnam: 23, weur: null, apac: 141 };
  const pings = decodeMessage(encodeMessage({ type: 'pings', table }));
  assert.ok(pings.ok && pings.msg.table.wnam === 23 && pings.msg.table.weur === null);

  const mig = decodeMessage(encodeMessage({
    type: 'migrate', region: 'weur', code: 'ulehsls', nonce: 'abc123nonce45678',
  }));
  assert.ok(mig.ok, 'a well-formed migrate decodes');

  assert.ok(decodeMessage(encodeMessage({ type: 'server-ok', region: 'wnam' })).ok);

  const junk = [
    { type: 'pings', table: { mars: 10 } }, // unknown region
    { type: 'pings', table: { wnam: -1 } }, // negative ping
    { type: 'pings', table: { wnam: 'fast' } },
    { type: 'pings', table: [1, 2] },
    { type: 'pings' },
    { type: 'migrate', region: 'weur', code: 'wlehsls', nonce: 'abc123nonce45678' }, // code is wnam's
    { type: 'migrate', region: 'nowhere', code: 'ulehsls', nonce: 'abc123nonce45678' },
    { type: 'migrate', region: 'weur', code: 'ulehsls', nonce: 'short' },
    { type: 'migrate', region: 'weur', code: 'ulehsls' },
    { type: 'server-ok', region: 'nowhere' },
    { type: 'server-ok' },
  ];
  for (const m of junk) {
    assert.equal(decodeMessage(encodeMessage(m)).ok, false, JSON.stringify(m));
  }
});

// ------------------------------------------------------------ the probe sweep

test('probeAllRegions measures every region in parallel; silence reads null', async () => {
  // Fake socket: answers the probe echoes like the relay's probe
  // rooms — except 'oc', which never opens (unreachable).
  const factory = (url) => {
    const listeners = {};
    const ws = {
      readyState: 0,
      addEventListener(ev, fn) { (listeners[ev] = listeners[ev] || []).push(fn); },
      send() {
        queueMicrotask(() => {
          for (const fn of listeners.message || []) fn({ data: 'probe' });
        });
      },
      close() { ws.readyState = 3; },
    };
    const region = /probe-([a-z]+)-/.exec(url)[1];
    queueMicrotask(() => {
      if (region === 'oc') {
        for (const fn of listeners.error || []) fn(new Error('no route'));
        return;
      }
      ws.readyState = 1;
      for (const fn of listeners.open || []) fn();
    });
    return ws;
  };
  const table = await probeAllRegions({
    relayHttp: 'https://relay.invalid', socketFactory: factory, budgetMs: 500,
  });
  assert.deepEqual(Object.keys(table).sort(), REGIONS.map((r) => r.id).sort());
  assert.equal(typeof table.wnam, 'number');
  assert.equal(table.oc, null);
});

// ------------------------------------------------------------ app.js wiring

test('presence kicks the exchange; the kick is guarded and sends the table once', () => {
  const kick = functionBody(appJs, 'kickAutoServer');
  assert.ok(kick.includes("net.phase !== 'lobby'"), 'lobby only');
  assert.ok(kick.includes('net.peerPresent'), 'needs both players present');
  assert.ok(kick.includes('myAdmitted()'), 'a private-room guest probes only once admitted');
  assert.ok(kick.includes('net.autoMigrated'), 'a spent migration closes the exchange');
  assert.ok(kick.includes('net.autoTableSent'), 'one share per peer');
  assert.ok(kick.includes('probeAllRegions({ relayHttp: RELAY_HTTP })'), 'the bounded parallel sweep');
  assert.ok(kick.includes("{ type: 'pings', table }"), 'the measured table is shared');
  assert.ok(!kick.includes('maybeStartMatch') && !kick.includes('hostStartMatch'),
    'the exchange never touches the start gate');
  // Every lobby message lands on the kick (onNetMessage's tail).
  const tail = appJs.slice(appJs.indexOf("case 'flare':"));
  assert.ok(tail.includes('kickAutoServer();'), 'onNetMessage ends by kicking the exchange');
});

test('the host decides for the pair: stay confirms, a better region migrates once', () => {
  const decide = functionBody(appJs, 'maybeDecideAutoServer');
  assert.ok(decide.includes("net.role !== 'host'"), 'host only');
  assert.ok(decide.includes("net.phase !== 'lobby'"), 'never once loading has begun');
  assert.ok(decide.includes('bestRegionForPair(net.autoPings, net.peerPings)'),
    'the pair rule over the two tables');
  assert.ok(decide.includes('if (!best) return;'),
    'no basis (a table measured nothing): closed silently, no verdict claimed');
  assert.ok(decide.includes('net.autoServerDone = true'), 'the decision closes the pairing');
  assert.ok(decide.includes("{ type: 'server-ok', region: net.region }"),
    'a stay is confirmed to the guest');
  assert.ok(decide.includes('roomCodeForRegion(net.room, best)'),
    'the new code keeps the room suffix');
  assert.ok(decide.includes('makeMigrateNonce()'), 'the migration mints its nonce');
  assert.ok(decide.includes("type: 'migrate', region: best, code, nonce: net.migrateNonce"),
    'the migrate order carries region, code and nonce');
  assert.ok(decide.includes('applyMigration(best, code)'), 'the host leads its own move');
});

test('migration: one-time, lobby-only, ready flags reset, ordinary reconnect', () => {
  const mig = functionBody(appJs, 'applyMigration');
  assert.ok(mig.includes("net.phase !== 'lobby' || net.autoMigrated"),
    'the once-only guard and the lobby gate are the first line');
  assert.ok(mig.includes('net.autoMigrated = true'), 'the move is spent as it starts');
  assert.ok(mig.includes('net.selfReady = false') && mig.includes('net.peerReady = false'),
    'ready flags reset — the pair re-readies on the new server');
  assert.ok(mig.includes('net.peerPresent = false'), 'presence clears until the hellos cross again');
  assert.ok(!mig.includes('net.pwState ='), 'the guest keeps its admission — pwState is never reassigned');
  assert.ok(mig.includes('net.peerPwOk = false'),
    'the host\'s old pw-ok does not carry over — the nonce is the admission on the new region');
  assert.ok(mig.includes('net.region = region') && mig.includes('net.room = code'),
    'region and code move together');
  assert.ok(mig.includes('buildShortUrl(location.origin, code)'),
    'the invite link follows the room');
  assert.ok(mig.includes('connectToRoom()'), 'the reconnect is the ordinary one — no parallel path');
  assert.ok(mig.includes("t('status.serverMoved'"), 'the status line explains the move');
  assert.ok(!mig.includes('maybeStartMatch') && !mig.includes('hostStartMatch'),
    'migration never fires the start gate');
});

test('the guest follows a migrate once and keeps the nonce for its hello', () => {
  const body = caseBody(appJs, 'migrate');
  assert.ok(body.includes("net.role === 'guest'"), 'guests follow; the host leads');
  assert.ok(body.includes("net.phase === 'lobby'"), 'never after loading began');
  assert.ok(body.includes('!net.autoMigrated'), 'once only');
  assert.ok(body.includes('net.migrateNonce ='), 'the nonce is kept');
  assert.ok(body.includes('applyMigration(msg.region, msg.code)'), 'and spent on the move');
});

test('server-ok: the guest marks the stayed server automatic, for this room only', () => {
  const body = caseBody(appJs, 'server-ok');
  assert.ok(body.includes("net.role === 'guest'"));
  assert.ok(body.includes('msg.region === net.region'),
    'a confirmation for another region is not this room\'s verdict');
  assert.ok(body.includes('net.autoServerDone = true'));
});

test('the peer table is sanitized on arrival and only feeds the host decision', () => {
  const body = caseBody(appJs, 'pings');
  assert.ok(body.includes('sanitizePingTable(msg.table)'), 'foreign input is never trusted raw');
  assert.ok(body.includes('maybeDecideAutoServer()'), 'and wakes the host decision');
});

test('private rooms: the nonce admits in the hello case; both hellos can carry it', () => {
  const helloAt = appJs.indexOf("case 'hello': {");
  const hello = appJs.slice(helloAt, helloAt + 3400);
  assert.ok(hello.includes('checkNonce(net.migrateNonce, msg.nonce)'),
    'the host admits a nonce-carrying hello');
  assert.ok(hello.includes('net.peerPwOk = true'),
    'admission is the pw-ok flag itself — the gate downstream is unchanged');
  const connect = functionBody(appJs, 'connectToRoom');
  assert.ok(connect.includes("net.migrateNonce ? { nonce: net.migrateNonce } : {}"),
    'the connect hello presents the nonce');
  assert.ok(hello.includes("net.migrateNonce ? { nonce: net.migrateNonce } : {}"),
    'the reply hello presents it too (guest-first orderings included)');
  // The password door itself is untouched for everyone else.
  assert.ok(hello.includes("net.client.send({ type: 'pw-req' })"),
    'a guest without the nonce is still asked for the password');
});

test('a new peer reopens the exchange; a migrated room\'s decision stays final', () => {
  const helloAt = appJs.indexOf("case 'hello': {");
  const hello = appJs.slice(helloAt, helloAt + 4200);
  assert.ok(hello.includes('net.peerPings = null'), 'the old peer\'s table is dropped');
  assert.ok(hello.includes('net.autoTableSent = false'), 'the new peer is owed my table');
  assert.ok(hello.includes('if (!net.autoMigrated) {'), 'the pairing reopens — unless the room already moved');
  assert.ok(hello.includes('net.autoBest = null'), 'and the old verdict is cleared with it');
});

test('stale sockets cannot touch the new connection (migration replaces the socket)', () => {
  const connect = functionBody(appJs, 'connectToRoom');
  assert.ok(connect.includes('net.client === client'),
    'handlers answer only for the current client');
  assert.ok(connect.includes('net.client = client'), 'the new client is installed before connect');
});

test('the room screen carries the server line, plain then automatic', () => {
  const section = playHtml.slice(playHtml.indexOf('id="room-section"'), playHtml.indexOf('id="lobby-status"'));
  assert.ok(section.includes('id="room-server-line"'), 'the line exists on the room screen');
  const render = functionBody(appJs, 'renderRoom');
  assert.ok(render.includes('netUi.roomServerLine'), 'renderRoom fills it');
  assert.ok(render.includes("t('room.serverAuto', { region })"), 'decided: automatic, best for both');
  assert.ok(render.includes("t('room.server', { region })"), 'undecided: the plain region');
  assert.ok(render.includes('net.autoBest'), 'a real decision (autoBest) chooses the wording');
  assert.equal(STRINGS.en['room.serverAuto'], 'Server: {region} — automatic (best for both)');
  assert.equal(STRINGS.en['room.server'], 'Server: {region}');
  for (const loc of ['pt-BR', 'es']) {
    assert.ok(STRINGS[loc]['room.serverAuto'].includes('{region}'), `${loc} serverAuto interpolates`);
    assert.ok(STRINGS[loc]['status.serverMoved'].includes('{region}'), `${loc} serverMoved interpolates`);
  }
});
