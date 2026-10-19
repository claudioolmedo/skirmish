// Live room-model proof for M10 (directory flags + heartbeat).
// NOT part of `node --test` (it needs the live relay, and part B
// takes ~3 minutes by design). Run from web-preview/play/:
//   node room.integration.mjs
//
// A) A host socket that declares ?private=1 lists its room WITH
//    locked: true (private rooms are listed like the reference
//    game's — flagged, never hidden); a plain host socket lists
//    with locked: false. The password is never involved: the
//    relay only ever sees the flag.
// B) A lone public host socket that simply WAITS is still listed
//    after 150+ s — the M10 alarm heartbeat (~45 s) keeps the
//    entry inside the directory's 120 s prune window. (Before
//    M10 this room vanished at ~120 s: the live-found bug.)

const RELAY = 'https://skirmish-relay.hoardserdure7.workers.dev';
const REGION = 'wnam';
const suffix = Math.random().toString(36).slice(2, 8);
const privRoom = 'privproof' + suffix;
const pubRoom = 'pubproof' + suffix;

const fail = (msg) => {
  console.error('ROOM PROOF FAIL:', msg);
  process.exit(1);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function getLobbies() {
  const res = await fetch(`${RELAY}/lobbies`);
  if (!res.ok) fail(`GET /lobbies -> ${res.status}`);
  return res.json();
}

async function waitFor(what, pred, timeoutMs = 20000) {
  const t0 = Date.now();
  for (;;) {
    const data = await getLobbies();
    if (pred(data)) return data;
    if (Date.now() - t0 > timeoutMs) fail(`timeout waiting for: ${what}`);
    await sleep(750);
  }
}

function connect(room, extra = '') {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(
      `${RELAY.replace(/^http/, 'ws')}/room/${room}?region=${REGION}${extra}`);
    ws.addEventListener('open', () => resolve(ws), { once: true });
    ws.addEventListener('error', () => reject(new Error('ws error')), { once: true });
  });
}

// --- A: locked flag in the directory -------------------------------
const privHost = await connect(privRoom, '&private=1');
await waitFor('private room listed', (d) =>
  d.open.some((e) => e.room === privRoom));
let data = await getLobbies();
const privEntry = data.open.find((e) => e.room === privRoom);
if (privEntry.locked !== true) {
  fail(`private room listed without locked:true — ${JSON.stringify(privEntry)}`);
}
console.log(`A ok: private room ${privRoom} listed WITH locked:true`);

const pubHost = await connect(pubRoom);
await waitFor('public room listed', (d) =>
  d.open.some((e) => e.room === pubRoom));
data = await getLobbies();
const pubEntry = data.open.find((e) => e.room === pubRoom);
if (pubEntry.locked !== false) {
  fail(`public room listed without locked:false — ${JSON.stringify(pubEntry)}`);
}
console.log(`A ok: public room ${pubRoom} listed with locked:false`);
privHost.close();

// --- B: the waiting host survives the prune window ------------------
const t0 = Date.now();
for (const waitS of [60, 120, 155]) {
  const target = t0 + waitS * 1000;
  while (Date.now() < target) await sleep(2000);
  data = await getLobbies();
  const still = data.open.some((e) => e.room === pubRoom);
  console.log(`B: t+${waitS}s — public room ${still ? 'STILL LISTED' : 'GONE'}`);
  if (!still) fail(`public room aged out of the directory at ~${waitS}s (heartbeat missing)`);
}
console.log('B ok: lone waiting host still listed after 155 s (heartbeat works)');
pubHost.close();
console.log('ROOM PROOF PASS');
process.exit(0);
