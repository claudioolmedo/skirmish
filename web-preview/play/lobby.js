// Skirmish II — open-games lobby browser helpers (M9).
//
// Pure and Node-tested, in the netplay.js / quality.js pattern:
// app.js owns the DOM, the fetch and the timers; everything that can
// be decided without a DOM lives here. The directory itself lives on
// the relay (GET /lobbies): rooms report their own socket counts to a
// Lobby Durable Object; this module only shapes what comes back.
//
// The table follows the traditional lobby browser (Claudio's
// direction, zero-learning): an auto-generated game title, the
// server/location, players as filled/max, and the measured ping.
//
// M11 shaped the screen on the reference Lobby Browser: a named
// screen, a Game Name · Players · Map · Location · Server table, a
// Search / Refresh / Password / Server controls row, a Connection
// Quality panel (every region with its live ping), and one bottom
// action bar (Play solo / Join Lobby / Create Lobby). The filter,
// search and selection decisions live here, pure.

export const LOBBIES_REFRESH_MS = 5000;
export const MAX_LOBBY_ROWS = 50;

/// The one map every room plays today. The table's Map column is
/// this honest constant — when more maps exist the room will carry
/// its own and this becomes a fallback.
export const LOBBY_MAP_NAME = 'Arabia';

/// Ping quality bands, for the colored square beside a ping (in
/// the table's Server column and in the Connection Quality panel):
/// green under 120 ms (a comfortable lockstep link), yellow under
/// 220 ms (playable, watch it), red at 220 ms and above.
export const PING_GOOD_MS = 120;
export const PING_OK_MS = 220;

/// 'good' | 'ok' | 'bad' for a measured ping, null when there is
/// no measurement yet (the square renders hollow).
export function pingQuality(ms) {
  if (typeof ms !== 'number' || !Number.isFinite(ms)) return null;
  if (ms < PING_GOOD_MS) return 'good';
  if (ms < PING_OK_MS) return 'ok';
  return 'bad';
}

const ROOM_RE = /^[A-Za-z0-9_-]{1,64}$/;

/// Validate + normalize one GET /lobbies payload. Returns
/// { open: [{ room, region, players: 1, createdAt, locked }], inProgress }
/// or null when the payload is not directory-shaped at all.
/// Individual junk entries are dropped, never trusted: probe rooms
/// are not games, and "open" in a 1v1 world means exactly one seat
/// is taken.
export function parseLobbies(json) {
  if (!json || typeof json !== 'object' || !Array.isArray(json.open)) {
    return null;
  }
  const open = [];
  for (const e of json.open) {
    if (!e || typeof e !== 'object') continue;
    if (typeof e.room !== 'string' || !ROOM_RE.test(e.room)) continue;
    if (e.room.startsWith('probe-')) continue;
    if (e.players !== 1) continue;
    open.push({
      room: e.room,
      region: typeof e.region === 'string' ? e.region : '',
      players: 1,
      createdAt: Number.isFinite(e.createdAt) ? e.createdAt : 0,
      // M10: private rooms are listed like the reference game's —
      // flagged locked, never hidden. Only a literal true counts;
      // the password itself is never in the directory.
      locked: e.locked === true,
    });
  }
  open.sort((a, b) => a.createdAt - b.createdAt);
  const inProgress = Number.isFinite(json.inProgress)
    ? Math.max(0, Math.floor(json.inProgress))
    : 0;
  return { open: open.slice(0, MAX_LOBBY_ROWS), inProgress };
}

/// The auto-generated game title. Nobody types a name in v1: a game
/// is "1v1 Arabia" on its region, the way a DE lobby row reads.
export function lobbyTitle(regionName) {
  return `1v1 Arabia — ${regionName}`;
}

/// One table row, ready to render. Carries the raw sort keys
/// (createdAt, pingMs) alongside the display strings. M11: the row
/// speaks in the reference browser's columns — Game Name, Players,
/// Map, Location (the region's name) and Server (the ping to that
/// region, with its quality band).
export function lobbyRowModel(entry, { regionName, pingMs } = {}) {
  const name = regionName || entry.region || 'Unknown server';
  const locked = entry.locked === true;
  const ms = typeof pingMs === 'number' && Number.isFinite(pingMs) ? pingMs : null;
  return {
    room: entry.room,
    region: entry.region,
    title: lobbyTitle(name),
    server: name,
    playersText: `${entry.players}/2`,
    mapText: LOBBY_MAP_NAME,
    pingMs: ms,
    pingText: ms !== null ? `${Math.round(ms)} ms` : '…',
    quality: pingQuality(ms),
    createdAt: entry.createdAt || 0,
    // M10: the lock marker a locked row carries (rendered before
    // the title, the way the reference browser marks passworded
    // games).
    locked,
    lockText: locked ? '🔒' : '',
  };
}

/// The controls row, as a pure decision over row models (M11):
/// - `search` filters by game-title text (case-insensitive
///   substring; the title carries the region name, so a place
///   name finds its games too);
/// - `showPassword` unchecked hides locked (🔒) rows — the
///   reference browser's Password toggle;
/// - `region` (a region id, '' for Any) keeps only that server.
/// Input order is preserved; the input array is not mutated.
export function filterLobbyRows(rows, { search = '', showPassword = true, region = '' } = {}) {
  const q = String(search == null ? '' : search).trim().toLowerCase();
  const regionId = typeof region === 'string' ? region : '';
  return rows.filter((r) => {
    if (!showPassword && r.locked) return false;
    if (regionId && r.region !== regionId) return false;
    if (q && !String(r.title).toLowerCase().includes(q)) return false;
    return true;
  });
}

/// Does the current selection survive into a row set? Used twice:
/// against the directory itself on refresh (a game that filled or
/// vanished drops the selection) and against the VISIBLE rows for
/// the Join Lobby button (a filtered-out selection cannot join).
export function selectionSurvives(selectedRoom, rows) {
  if (typeof selectedRoom !== 'string' || !selectedRoom) return false;
  return rows.some((r) => r && r.room === selectedRoom);
}

/// Sort row models for the table. 'created' is the natural lobby
/// order (oldest game first); 'location' is the guide's recommended
/// sort (group games by server); 'ping' puts the fastest first with
/// unmeasured rows last in both directions. Ties keep lobby order.
export function sortLobbyRows(rows, key = 'created', dir = 1) {
  const out = rows.slice();
  const d = dir === -1 ? -1 : 1;
  const base = (a, b) => {
    if (key === 'location') {
      return a.server.localeCompare(b.server) || (a.createdAt - b.createdAt);
    }
    if (key === 'ping') {
      const pa = a.pingMs == null ? Infinity : a.pingMs;
      const pb = b.pingMs == null ? Infinity : b.pingMs;
      if (pa !== pb) return pa < pb ? -1 : 1; // never inverted: unmeasured stays last
      return a.createdAt - b.createdAt;
    }
    return a.createdAt - b.createdAt;
  };
  out.sort((a, b) => (key === 'ping' ? base(a, b) : d * base(a, b)));
  return out;
}

/// The join parameters for a listed game ({ room, region, private }
/// in the shape openLobby takes), or null when the entry is not
/// joinable. A locked row joins as a private room: the guest
/// client asks for the password before presence counts.
export function joinTarget(entry) {
  if (!entry || typeof entry.room !== 'string' || !ROOM_RE.test(entry.room)) {
    return null;
  }
  return {
    room: entry.room,
    region: typeof entry.region === 'string' && entry.region ? entry.region : 'wnam',
    private: entry.locked === true,
  };
}
