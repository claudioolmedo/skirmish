// Live directory proof for the M9 open-games lobby.
// NOT part of `node --test` (it needs the live relay). Run from
// web-preview/play/:
//   node lobby.integration.mjs
//
// Opens a real room socket on the LIVE relay as a host, polls
// GET /lobbies until the room is listed (region correct, 1/2),
// joins with a second socket until the room leaves the open list
// (inProgress >= 1), then closes both and polls until the entry is
// gone. Exit code 0 = pass; the printed transcript is the evidence.

const RELAY = 'https://skirmish-relay.hoardserdure7.workers.dev';
const REGION = 'wnam';
const room = 'lobbyproof' + Math.random().toString(36).slice(2, 8);
const wsUrl = `${RELAY.replace(/^http/, 'ws')}/room/${room}?region=${REGION}`;

const fail = (msg) => {
  console.error('LOBBY PROOF FAIL:', msg);
  process.exit(1);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function getLobbies() {
  const res = await fetch(`${RELAY}/lobbies`);
  if (!res.ok) fail(`GET /lobbies -> ${res.status}`);
  return res.json();
}

async function waitFor(what, pred, timeoutMs = 15000) {
  const t0 = Date.now();
  for (;;) {
    const dir = await getLobbies();
    if (pred(dir)) {
      console.log(`ok (${Date.now() - t0} ms): ${what} —`, JSON.stringify(dir));
      return dir;
    }
    if (Date.now() - t0 > timeoutMs) {
      fail(`timed out waiting for: ${what}; last directory: ${JSON.stringify(dir)}`);
    }
    await sleep(300);
  }
}

function connect() {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    ws.addEventListener('open', () => resolve(ws), { once: true });
    ws.addEventListener('error', (e) => reject(new Error('socket error ' + e.message)), { once: true });
  });
}

console.log('room:', room, 'region:', REGION);

// -- host opens the room -> it must appear in the open list --------
const host = await connect();
console.log('host socket open');
await waitFor('room listed as open 1/2 in wnam', (dir) => {
  const e = dir.open.find((g) => g.room === room);
  return e && e.region === REGION && e.players === 1
    && Number.isFinite(e.createdAt);
});

// -- a guest joins -> the room leaves the open list ----------------
const guest = await connect();
console.log('guest socket open');
await waitFor('room no longer open; inProgress >= 1', (dir) =>
  !dir.open.some((g) => g.room === room) && dir.inProgress >= 1);

// -- both leave -> the entry disappears ----------------------------
host.close();
guest.close();
await waitFor('directory entry gone after close', (dir) =>
  !dir.open.some((g) => g.room === room));

console.log('LOBBY PROOF PASS');
process.exit(0);
