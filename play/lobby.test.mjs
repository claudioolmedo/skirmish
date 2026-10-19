// Unit tests for lobby.js (M9) — run from web-preview/play/:
//   node --test lobby.test.mjs
// The directory payload shaping, row models and sort orders behind
// the play page's open-games lobby browser.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  LOBBIES_REFRESH_MS, MAX_LOBBY_ROWS,
  parseLobbies, lobbyTitle, lobbyRowModel, sortLobbyRows, joinTarget,
} from './lobby.js';

test('parseLobbies accepts a well-formed directory payload', () => {
  const parsed = parseLobbies({
    open: [
      { room: 'abc123def4', region: 'wnam', players: 1, createdAt: 2000 },
      { room: 'zzz999yyyy', region: 'weur', players: 1, createdAt: 1000 },
    ],
    inProgress: 3,
  });
  assert.equal(parsed.inProgress, 3);
  assert.equal(parsed.open.length, 2);
  // Lobby order is oldest first, regardless of payload order.
  assert.equal(parsed.open[0].room, 'zzz999yyyy');
  assert.equal(parsed.open[1].room, 'abc123def4');
  assert.deepEqual(parsed.open[0], {
    room: 'zzz999yyyy', region: 'weur', players: 1, createdAt: 1000, locked: false,
  });
});

test('parseLobbies rejects payloads that are not directory-shaped', () => {
  assert.equal(parseLobbies(null), null);
  assert.equal(parseLobbies(42), null);
  assert.equal(parseLobbies('open'), null);
  assert.equal(parseLobbies({}), null);
  assert.equal(parseLobbies({ open: 'nope' }), null);
  assert.deepEqual(parseLobbies({ open: [] }), { open: [], inProgress: 0 });
});

test('parseLobbies drops junk entries and clamps inProgress', () => {
  const parsed = parseLobbies({
    open: [
      { room: 'probe-wnam-x1', region: 'wnam', players: 1, createdAt: 1 },
      { room: 'full000000', region: 'wnam', players: 2, createdAt: 2 },
      { room: 'bad room!', region: 'wnam', players: 1, createdAt: 3 },
      { room: 42, region: 'wnam', players: 1, createdAt: 4 },
      null,
      'garbage',
      { room: 'good000001', region: 'sam', players: 1, createdAt: 5 },
      { room: 'good000002', players: 1 },
    ],
    inProgress: -4,
  });
  assert.equal(parsed.inProgress, 0);
  // createdAt 0 (missing in the payload) sorts before createdAt 5.
  assert.deepEqual(parsed.open.map((g) => g.room), ['good000002', 'good000001']);
  const noTs = parsed.open.find((g) => g.room === 'good000002');
  assert.equal(noTs.createdAt, 0);
  assert.equal(noTs.region, '');
});

test('parseLobbies caps the list length', () => {
  const open = [];
  for (let i = 0; i < MAX_LOBBY_ROWS + 10; i++) {
    open.push({ room: `room${String(i).padStart(6, '0')}`, region: 'wnam', players: 1, createdAt: i });
  }
  assert.equal(parseLobbies({ open, inProgress: 0 }).open.length, MAX_LOBBY_ROWS);
});

test('lobbyTitle and lobbyRowModel read like a DE lobby row', () => {
  assert.equal(lobbyTitle('North America West'), '1v1 Arabia — North America West');
  const row = lobbyRowModel(
    { room: 'abc123def4', region: 'wnam', players: 1, createdAt: 7 },
    { regionName: 'North America West', pingMs: 46.4 });
  assert.equal(row.title, '1v1 Arabia — North America West');
  assert.equal(row.server, 'North America West');
  assert.equal(row.playersText, '1/2');
  assert.equal(row.pingText, '46 ms');
  assert.equal(row.pingMs, 46.4);
  const unmeasured = lobbyRowModel(
    { room: 'abc123def4', region: 'oc', players: 1, createdAt: 7 },
    { regionName: 'Oceania', pingMs: null });
  assert.equal(unmeasured.pingText, '…');
  assert.equal(unmeasured.pingMs, null);
  // No name known: fall back to the region id rather than blank.
  const raw = lobbyRowModel(
    { room: 'abc123def4', region: 'eeur', players: 1, createdAt: 7 }, {});
  assert.equal(raw.server, 'eeur');
});

test('sortLobbyRows sorts by location with lobby order as the tiebreak', () => {
  const mk = (server, createdAt) => ({
    room: `${server}${createdAt}`, server, createdAt, pingMs: null,
  });
  const rows = [mk('Weur', 3), mk('Enam', 2), mk('Enam', 1), mk('Apac', 9)];
  assert.deepEqual(
    sortLobbyRows(rows, 'location', 1).map((r) => r.server + r.createdAt),
    ['Apac9', 'Enam1', 'Enam2', 'Weur3']);
  assert.deepEqual(
    sortLobbyRows(rows, 'location', -1).map((r) => r.server + r.createdAt),
    ['Weur3', 'Enam2', 'Enam1', 'Apac9']);
  // Input is not mutated.
  assert.equal(rows[0].server, 'Weur');
});

test('sortLobbyRows by ping keeps unmeasured rows last in both directions', () => {
  const mk = (pingMs, createdAt) => ({
    room: `r${createdAt}`, server: 'Wnam', createdAt, pingMs,
  });
  const rows = [mk(null, 1), mk(90, 2), mk(30, 3), mk(null, 4)];
  assert.deepEqual(sortLobbyRows(rows, 'ping', 1).map((r) => r.createdAt), [3, 2, 1, 4]);
  assert.deepEqual(sortLobbyRows(rows, 'ping', -1).map((r) => r.createdAt), [3, 2, 1, 4]);
  assert.deepEqual(sortLobbyRows(rows, 'created', -1).map((r) => r.createdAt), [4, 3, 2, 1]);
  assert.deepEqual(sortLobbyRows(rows).map((r) => r.createdAt), [1, 2, 3, 4]);
});

test('joinTarget validates the room and defaults the region', () => {
  assert.deepEqual(
    joinTarget({ room: 'abc123def4', region: 'weur' }),
    { room: 'abc123def4', region: 'weur', private: false });
  assert.deepEqual(
    joinTarget({ room: 'abc123def4' }),
    { room: 'abc123def4', region: 'wnam', private: false });
  assert.equal(joinTarget(null), null);
  assert.equal(joinTarget({ room: 'bad room' }), null);
  assert.equal(joinTarget({}), null);
});

test('parseLobbies keeps the M10 locked flag — strictly boolean', () => {
  const parsed = parseLobbies({
    open: [
      { room: 'lockroom01', region: 'wnam', players: 1, createdAt: 5, locked: true },
      { room: 'openroom01', region: 'weur', players: 1, createdAt: 6 },
      { room: 'junkroom01', region: 'sam', players: 1, createdAt: 7, locked: 'yes' },
    ],
    inProgress: 0,
  });
  assert.equal(parsed.open[0].locked, true);
  assert.equal(parsed.open[1].locked, false);
  // Anything but a literal true is not a lock (and never a password).
  assert.equal(parsed.open[2].locked, false);
});

test('lobbyRowModel carries the lock marker for locked rooms', () => {
  const locked = lobbyRowModel(
    { room: 'lockroom01', region: 'wnam', players: 1, createdAt: 5, locked: true },
    { regionName: 'North America West', pingMs: 42 });
  assert.equal(locked.locked, true);
  assert.equal(locked.lockText, '🔒');
  const open = lobbyRowModel(
    { room: 'openroom01', region: 'wnam', players: 1, createdAt: 6 },
    { regionName: 'North America West', pingMs: 42 });
  assert.equal(open.locked, false);
  assert.equal(open.lockText, '');
});

test('joinTarget joins a locked row as a private room', () => {
  assert.deepEqual(
    joinTarget({ room: 'lockroom01', region: 'eeur', locked: true }),
    { room: 'lockroom01', region: 'eeur', private: true });
});

test('the refresh cadence is the briefed ~5 s', () => {
  assert.equal(LOBBIES_REFRESH_MS, 5000);
});
