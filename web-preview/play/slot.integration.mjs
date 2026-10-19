// M12 slot integration proof — runs against the LIVE relay
// (NOT part of `node --test`; run explicitly after a relay
// deploy):
//
//   node web-preview/play/slot.integration.mjs
//
// The directory's listing flag is steered by the HOST's client
// over its own room socket — so this proof is live by design:
//   A) a room created with the guest seat filled (&listed=0 on
//      the host's connection) never enters GET /lobbies' open
//      list; the host's {"type":"slot"} control frames then
//      relist it and delist it again, live.
//   B) the same control frame from a connection that is NOT the
//      host's (no host=1) is ignored — the room stays listed.
const RELAY = 'https://skirmish-relay.hoardserdure7.workers.dev';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const roomId = () =>
  `slot${Math.random().toString(36).slice(2, 10)}${Date.now().toString(36)}`;

function connect(room, params) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(
      `${RELAY.replace(/^http/, 'ws')}/room/${room}?region=wnam${params}`);
    ws.addEventListener('open', () => resolve(ws), { once: true });
    ws.addEventListener('error', (e) => reject(new Error(`ws error: ${e.message || e.type}`)), { once: true });
  });
}

async function lobbies() {
  const res = await fetch(`${RELAY}/lobbies`);
  if (!res.ok) throw new Error(`GET /lobbies -> ${res.status}`);
  return res.json();
}

async function waitFor(desc, fn, timeoutMs = 20000) {
  const t0 = Date.now();
  for (;;) {
    if (await fn()) return;
    if (Date.now() - t0 > timeoutMs) throw new Error(`timeout waiting for: ${desc}`);
    await sleep(750);
  }
}

const isOpen = async (room) => (await lobbies()).open.some((o) => o.room === room);

// ------------------------------------------------------------ A
{
  const room = roomId();
  const host = await connect(room, '&host=1&listed=0');
  await sleep(2500); // the create-time report settles
  if (await isOpen(room)) {
    throw new Error('A: a room created with a filled seat must NOT be listed');
  }
  console.log('A1 ok: created unlisted (seat filled at create time) — absent from /lobbies');

  host.send('{"type":"slot","listed":true}');
  await waitFor('host control relists the room', () => isOpen(room));
  console.log('A2 ok: host {"type":"slot","listed":true} relisted the room');

  host.send('{"type":"slot","listed":false}');
  await waitFor('host control delists the room', async () => !(await isOpen(room)));
  console.log('A3 ok: host {"type":"slot","listed":false} delisted the room');

  host.close();
}

// ------------------------------------------------------------ B
{
  const room = roomId();
  const plain = await connect(room, ''); // no host=1: NOT the host's connection
  await waitFor('plain room lists', () => isOpen(room));
  plain.send('{"type":"slot","listed":false}');
  await sleep(3000); // long enough for a (wrongly) honored control to land
  if (!(await isOpen(room))) {
    throw new Error('B: a non-host slot control must be ignored — the room was delisted');
  }
  console.log('B ok: a slot control from a non-host connection is ignored');
  plain.close();
}

console.log('SLOT INTEGRATION: PASS');
process.exit(0);
