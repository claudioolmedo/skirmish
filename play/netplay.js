// Skirmish II — lockstep netplay (M8).
//
// The deterministic sim was built for this (Bettner & Terrano,
// "1500 Archers on a 28.8"): clients exchange ONLY commands, never
// state. Both sides run the identical sim from the identical seed;
// a command bundle from each player is applied, in slot order, at
// an agreed execution turn; after every turn the two state hashes
// are compared — a mismatch is a desync and the match stops.
//
// Layout of a match:
//   * 1 turn = 3 ticks. A tick is 1/60 s of GAME time; the game
//     runs at GAME_SPEED x wall clock (DE's Normal speed), so a
//     turn spans 3 x (1000/60) / 1.7 ≈ 29.4 ms of real time and
//     there are 34 turns per real second.
//   * Input delay: commands are stamped for the turn that is
//     `delay` turns in the future (starts at 4 turns, the
//     reference design's turnaround). A turn executes only when BOTH
//     bundles for it have arrived; an empty bundle is a heartbeat.
//     If a side's bundle is late, execution waits — the slowest
//     machine sets the pace (NET-03), the game never drops sim time.
//   * The delay adapts up (host proposes, both apply at an agreed
//     future turn) when too many turns execute late; it never drops
//     below the initial value mid-match.
//   * Hash cadence: one hash per turn (after the turn's 3rd tick).
//
// This module is pure protocol/session logic — no DOM, no globals —
// driven through injected callbacks (send / applyBundle / tickOnce),
// so the whole state machine is testable headlessly under Node with
// a fake transport and the real WASM sim. app.js is the thin
// integrator. The relay is a dumb pipe: presence (hello messages),
// roles (how you entered the room), and handshakes all live here,
// client-side.

export const PROTOCOL_VERSION = 1;
export const TURN_TICKS = 3;
// Game speed (2026-10-19 study fix, gap 2): DE's Normal speed
// runs game time at 1.7x the wall clock — until now the client
// ran 1.0x, so every build, train and research took 1.7x longer
// in real time than a DE player expects. This ONE constant is
// where real time becomes ticks, everywhere: the local frame
// loop's accumulator (via advanceTicks), the netplay turn
// pacing below, and replay playback. The sim itself is
// untouched — its ticks, rules and hashes do not know the
// wall clock exists, and both lockstep clients run this same
// build, so they pace identically. Input delay is counted in
// TURNS and is unchanged (4 turns ≈ 118 ms at this speed).
export const GAME_SPEED = 1.7;
export const TICK_MS = 1000 / 60; // one sim tick at 1.0x (game ms)
export const TURN_MS = (TURN_TICKS * TICK_MS) / GAME_SPEED; // ≈29.41 ms of wall time per turn

/**
 * Advance a tick accumulator by `dtMs` of wall time at
 * GAME_SPEED. The accumulator is kept in GAME milliseconds:
 * pass the previous remainder back in as `accGameMs`. Returns
 * how many whole ticks to run now and the new remainder. Pure —
 * the frame loop, the replay driver and the tests share it.
 */
export function advanceTicks(accGameMs, dtMs) {
  const total = accGameMs + dtMs * GAME_SPEED;
  const ticks = Math.floor(total / TICK_MS);
  return { ticks, accMs: total - ticks * TICK_MS };
}
export const INITIAL_DELAY_TURNS = 4;
export const MAX_DELAY_TURNS = 10;
export const BUNDLE_CAP = 256; // NET-07: per-turn command volume bound
export const REJOIN_GRACE_MS = 90_000;
// Peer-loss detection fallback (2026-10-22, rejoin fix F1): the
// relay (v6+) originates a 'peer-left' frame the moment a room
// socket closes, which is the primary signal. A peer whose
// connection dies WITHOUT a close ever reaching the relay (a
// pulled cable, a phone losing signal — the socket lingers as a
// zombie) produces no frame, so the client also treats a
// continuous execution stall of this length mid-match as the
// peer being gone. 20 s is far beyond any live-match stall a
// connected peer produces (bundles flow every turn), and if the
// peer turns out to be alive after all (a long-backgrounded
// tab), its resumed bundle traffic revives the match — see
// onPeerBack in app.js.
export const PEER_STALL_TIMEOUT_MS = 20_000;
// M10 loading phase: how long a client waits on the loading
// screen for the peer's loaded signal (the guest's start-ack /
// the host's go) before the match is cancelled and everyone
// returns to the Lobby Browser. 90 s is generous on purpose —
// the heavy load (WASM fetch + init, artwork, world build)
// happens in this window, and a slow phone on a weak network
// deserves the same patience the rejoin window gives a drop.
export const LOAD_TIMEOUT_MS = 90_000;

// The fixed region list (Claudio's decision, 2026-10-18): the same
// locations for everyone; the player picks. The relay's /regions is
// the source of truth at runtime; this mirror is the offline/test
// fallback and carries the display names.
export const REGIONS = [
  { id: 'wnam', name: 'North America West' },
  { id: 'enam', name: 'North America East' },
  { id: 'sam', name: 'South America' },
  { id: 'weur', name: 'Europe West' },
  { id: 'eeur', name: 'Europe East' },
  { id: 'apac', name: 'Asia-Pacific' },
  { id: 'oc', name: 'Oceania' },
];

// ---------------------------------------------------------------------
// Semantic commands
//
// The wire/app command vocabulary. Every order a player can give is
// a small JSON object. Coordinates are f64 world units: JSON's
// shortest-round-trip number formatting reproduces the exact same
// double on the far side, and the sim's fixed-point conversion is
// deterministic, so both sims consume bit-identical input.
// Town-Center orders may carry an explicit Town Center id
// (cmd.tc): additional TCs are buildable from the Castle Age and
// each carries its own queue, so the issuing client names the
// one its panel acted on. When cmd.tc is absent the applier
// resolves the issuing player's first Town Center from its sim
// (identical on both clients by construction).

const BUILD_FNS = [
  'push_build_house', 'push_build_farm', 'push_build_barracks',
  'push_build_lumber_camp', 'push_build_mill', 'push_build_mining_camp',
  'push_build_archery_range', 'push_build_stable', 'push_build_blacksmith',
  'push_build_palisade_wall',
  'push_build_watch_tower', 'push_build_outpost', 'push_build_market',
  'push_build_town_center', 'push_build_stone_wall',
];
const BUILD_FNS_QUEUED = BUILD_FNS.map((n) => n + '_queued');
const TRAIN_AT_FNS = {
  2: 'push_train_scout', 3: 'push_train_militia', 4: 'push_train_archer',
  9: 'push_train_spearman', 10: 'push_train_skirmisher',
};

const isInt = (v) => Number.isInteger(v) && v >= 0;
const isNum = (v) => typeof v === 'number' && Number.isFinite(v);

/// Structural validation for one semantic command (NET-07's first
/// line of defense: nothing malformed ever reaches the sim).
export function validateCommand(c) {
  if (!c || typeof c !== 'object' || typeof c.t !== 'string') return false;
  switch (c.t) {
    case 'move': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    case 'gather': return isInt(c.e) && isInt(c.n);
    case 'attack': return isInt(c.e) && isInt(c.tg);
    case 'attackb': return isInt(c.e) && isInt(c.b);
    case 'repair': return isInt(c.e) && isInt(c.b);
    // The cap is the build table's own length (BUILD_FNS): every
    // kind the applier can dispatch is wire-legal, the first
    // index past the table is junk. (BLD-20 raised it to 14 with
    // the Stone Wall; 13 — the Town Center — was already
    // dispatchable but not validatable, an old inconsistency.)
    case 'build': return isInt(c.e) && isInt(c.k) && c.k <= 14 && isNum(c.x) && isNum(c.y);
    // TRD-01..03: Market trades — a building id and a commodity
    // tag (food 1, wood 2, stone 4; gold is never a commodity).
    case 'mbuy': case 'msell':
      return isInt(c.b) && (c.r === 1 || c.r === 2 || c.r === 4);
    // BLD-20: a wall line names its wall kind (9 palisade when
    // absent — the pre-BLD-20 wire form — or 14 stone wall).
    case 'buildline': return isInt(c.e) && isNum(c.x0) && isNum(c.y0) && isNum(c.x1) && isNum(c.y1)
      && (c.k === undefined || c.k === 9 || c.k === 14);
    case 'train_at': return isInt(c.b) && (c.k === 2 || c.k === 3 || c.k === 4 || c.k === 9 || c.k === 10);
    case 'train_vil': return c.tc === undefined || isInt(c.tc);
    case 'research_tc': return (c.tech === 0 || c.tech === 1 || c.tech === 6
      || c.tech === 14 || c.tech === 15) && (c.tc === undefined || isInt(c.tc));
    case 'research_at': return isInt(c.b) && isInt(c.tech) && c.tech <= 13;
    case 'rally_tc': return isNum(c.x) && isNum(c.y) && (c.tc === undefined || isInt(c.tc));
    case 'rally_b': return isInt(c.b) && isNum(c.x) && isNum(c.y);
    case 'force_dep': return isInt(c.e);
    case 'auto_explore': return isInt(c.e) && typeof c.on === 'boolean';
    case 'attackmove': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    case 'patrol': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    case 'stance': return isInt(c.e) && isInt(c.s) && c.s <= 3;
    case 'stop': return isInt(c.e);
    case 'delete': return isInt(c.e);
    case 'deleteb': return isInt(c.b);
    case 'cancel_tc': return isInt(c.tc) && isInt(c.i) && c.i <= 15;
    case 'cancel_b': return isInt(c.b) && isInt(c.i) && c.i <= 15;
    default: return false;
  }
}

/// Apply one semantic command to a sim on behalf of `owner`.
/// `tcOf(owner)` resolves that player's Town Center entity id.
/// Mirrors app.js's local push calls exactly — the same sim entry
/// points, the per-seat forms where the seat matters. Returns the
/// sim's acceptance (informational only: in lockstep the appliers on
/// both clients run the same validation against the same state, so
/// acceptance agrees by construction).
export function applyCommand(sim, cmd, owner, tcOf) {
  switch (cmd.t) {
    case 'move': return cmd.q
      ? sim.push_move_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_move(cmd.e, cmd.x, cmd.y);
    case 'gather': return cmd.q
      ? sim.push_gather_queued(cmd.e, cmd.n)
      : sim.push_gather(cmd.e, cmd.n);
    case 'attack': return cmd.q
      ? sim.push_attack_queued(cmd.e, cmd.tg)
      : sim.push_attack(cmd.e, cmd.tg);
    case 'attackb': return sim.push_attack_building(cmd.e, cmd.b);
    case 'repair': return cmd.q
      ? sim.push_repair_queued(cmd.e, cmd.b)
      : sim.push_repair(cmd.e, cmd.b);
    case 'build': return cmd.q
      ? sim[BUILD_FNS_QUEUED[cmd.k]](cmd.e, cmd.x, cmd.y)
      : sim[BUILD_FNS[cmd.k]](cmd.e, cmd.x, cmd.y);
    case 'buildline': return cmd.k === 14
      ? sim.push_build_stone_wall_line(cmd.e, cmd.x0, cmd.y0, cmd.x1, cmd.y1)
      : sim.push_build_wall_line(cmd.e, cmd.x0, cmd.y0, cmd.x1, cmd.y1);
    case 'train_at': return sim[TRAIN_AT_FNS[cmd.k]](cmd.b);
    case 'train_vil': return sim.push_train_villager_at(cmd.tc != null ? cmd.tc : tcOf(owner));
    case 'research_tc': return sim.push_research_tc_at(cmd.tc != null ? cmd.tc : tcOf(owner), cmd.tech);
    case 'research_at': return sim.push_research_at(cmd.b, cmd.tech);
    case 'rally_tc': return sim.push_set_rally_tc_of(cmd.tc != null ? cmd.tc : tcOf(owner), cmd.x, cmd.y);
    case 'rally_b': return sim.push_set_rally_building(cmd.b, cmd.x, cmd.y);
    case 'force_dep': return sim.push_force_deposit(cmd.e);
    case 'auto_explore': return sim.push_auto_explore(cmd.e, cmd.on);
    case 'attackmove': return cmd.q
      ? sim.push_attack_move_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_attack_move(cmd.e, cmd.x, cmd.y);
    case 'patrol': return cmd.q
      ? sim.push_patrol_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_patrol(cmd.e, cmd.x, cmd.y);
    case 'stance': return sim.push_set_stance(cmd.e, cmd.s);
    case 'stop': return sim.push_stop(cmd.e);
    case 'delete': return sim.push_delete(cmd.e);
    case 'deleteb': return sim.push_delete_building(cmd.b);
    case 'cancel_tc': return sim.push_cancel_queue_tc(cmd.tc, cmd.i);
    case 'cancel_b': return sim.push_cancel_queue_building(cmd.b, cmd.i);
    // TRD-01..03: Market trades — the sim derives the owner from
    // the building, so one form serves both seats (as research_at).
    case 'mbuy': return sim.push_market_buy(cmd.b, cmd.r);
    case 'msell': return sim.push_market_sell(cmd.b, cmd.r);
    default: return false;
  }
}

/// The entity id of `owner`'s Town Center in this sim (kind tag 1).
export function tcIdOf(sim, owner) {
  const kinds = sim.entity_kinds();
  const owners = sim.entity_owners();
  for (let i = 0; i < kinds.length; i++) {
    if (kinds[i] === 1 && owners[i] === owner) return i;
  }
  return -1;
}

// ---------------------------------------------------------------------
// Message codec
//
// Everything on the room socket is one JSON object with a `type`.
// decodeMessage validates the envelope and, for bundles, every
// command inside; anything malformed is rejected whole (never
// partially applied).

const MESSAGE_TYPES = new Set([
  'hello', 'start', 'start-ack', 'start-abort', 'go',
  'bundle', 'hash', 'delay', 'resign', 'resync', 'resync-done',
  'resume', 'claim', 'room-full',
  // Relay v6: originated by the room object itself (never by a
  // client) when a member's socket closes — the surviving
  // client's signal that its peer is gone and the rejoin grace
  // window opens. Carries no fields.
  'peer-left',
  // M10 room model: ready flags and the private-room password
  // handshake. All lobby traffic — none of it ever reaches the sim
  // or the hashed state (NET-10 discipline).
  'ready', 'pw', 'pw-req', 'pw-ok', 'pw-no',
  // M10 loading phase: sent by a client that cancels a match
  // during loading (it is leaving, or its own load failed), so the
  // peer returns to the lobby too instead of waiting out the
  // timeout. A disconnect needs no message — it cancels by itself.
  'cancel',
]);

export function encodeMessage(msg) {
  return JSON.stringify(msg);
}

export function decodeMessage(text) {
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    return { ok: false, error: 'not json' };
  }
  if (!msg || typeof msg !== 'object' || !MESSAGE_TYPES.has(msg.type)) {
    return { ok: false, error: 'unknown type' };
  }
  if (msg.type === 'bundle') {
    if (!isInt(msg.turn) || !Array.isArray(msg.cmds) || msg.cmds.length > 512) {
      return { ok: false, error: 'bad bundle' };
    }
    for (const c of msg.cmds) {
      if (!validateCommand(c)) return { ok: false, error: 'bad command' };
    }
  }
  if (msg.type === 'hash' && (!isInt(msg.turn) || typeof msg.hash !== 'string')) {
    return { ok: false, error: 'bad hash' };
  }
  if (msg.type === 'delay' && (!isInt(msg.turns) || !isInt(msg.atTurn))) {
    return { ok: false, error: 'bad delay' };
  }
  if (msg.type === 'ready' && typeof msg.ready !== 'boolean') {
    return { ok: false, error: 'bad ready' };
  }
  // A password is a short human string; anything else is junk.
  if (msg.type === 'pw' && (typeof msg.pw !== 'string' || msg.pw.length > 128)) {
    return { ok: false, error: 'bad pw' };
  }
  return { ok: true, msg };
}

// ---------------------------------------------------------------------
// The turn session
//
// A pure state machine. app.js (or a test) drives it with pump(now)
// from its frame loop; inbound messages arrive through receive().

export function createSession({
  localSlot,
  turnTicks = TURN_TICKS,
  initialDelay = INITIAL_DELAY_TURNS,
  send,
  applyBundle,
  tickOnce,
  onEvent = () => {},
  onTurn = null,
}) {
  let phase = 'idle'; // idle | running | desync | ended
  let anchorTurn = 0; // execution-time anchor: turn -> wall time
  let anchorMs = 0;
  let nextExec = 0;
  let nextSendTurn = initialDelay;
  let nextSendAt = 0;
  let delay = initialDelay;
  let pendingDelay = null; // {atTurn, turns}
  const filling = new Map(); // turn -> cmds[] (local, unsent)
  const bundles = new Map(); // turn -> [cmds|null, cmds|null]
  const localHashes = new Map();
  const remoteHashes = new Map();
  let floorTurn = 0; // rebase floor: bundles below it are final/ignored
  let waiting = false;
  let lateWindow = []; // booleans: did each recent turn execute late?
  let droppedDupes = 0;
  let droppedOverflow = 0;

  const execTarget = (turn) => anchorMs + (turn - anchorTurn + 1) * TURN_MS;

  function bundleFor(turn) {
    let b = bundles.get(turn);
    if (!b) {
      b = [null, null];
      bundles.set(turn, b);
    }
    return b;
  }

  function checkHash(turn) {
    const local = localHashes.get(turn);
    const remote = remoteHashes.get(turn);
    if (local === undefined || remote === undefined) return;
    if (local !== remote && phase === 'running') {
      phase = 'desync';
      onEvent({ type: 'desync', turn, localHash: local, remoteHash: remote });
    }
  }

  function syncDelay() {
    if (!pendingDelay) return;
    if (nextSendTurn < pendingDelay.atTurn) return;
    // The open turn jumps forward by the delay delta: the skipped
    // turns are sent empty at once (their input window is gone),
    // and anything recorded for the old open turn moves to the new
    // one — a player's orders are delayed, never lost.
    const newOpen = nextSendTurn + (pendingDelay.turns - delay);
    const carried = filling.get(nextSendTurn) || [];
    for (let t = nextSendTurn; t < newOpen; t++) {
      sendBundle(t, t === nextSendTurn ? carried.slice(0, 0) : []);
    }
    if (carried.length) filling.set(newOpen, carried);
    filling.delete(nextSendTurn);
    nextSendTurn = newOpen;
    delay = pendingDelay.turns;
    pendingDelay = null;
    onEvent({ type: 'delay', turns: delay });
  }

  function sendBundle(turn, cmds) {
    const clean = [];
    for (const c of cmds) {
      // NET-07: exact duplicates within one bundle collapse —
      // the classic accidental double-issue changes nothing.
      if (clean.some((o) => JSON.stringify(o) === JSON.stringify(c))) {
        droppedDupes++;
        continue;
      }
      clean.push(c);
      if (clean.length >= BUNDLE_CAP) {
        droppedOverflow += cmds.length - clean.length;
        break;
      }
    }
    bundleFor(turn)[localSlot] = clean;
    send({ type: 'bundle', turn, cmds: clean });
  }

  function maybeProposeDelay(now) {
    if (localSlot !== 0 || pendingDelay || delay >= MAX_DELAY_TURNS) return;
    if (lateWindow.length < 200) return;
    const late = lateWindow.filter(Boolean).length;
    if (late / lateWindow.length <= 0.35) return;
    const turns = delay + 1;
    const atTurn = nextExec + 4;
    pendingDelay = { atTurn, turns };
    send({ type: 'delay', turns, atTurn });
    lateWindow = [];
  }

  function executeTurn(turn, now) {
    const b = bundles.get(turn);
    applyBundle(0, b[0]);
    applyBundle(1, b[1]);
    let hash;
    for (let k = 0; k < turnTicks; k++) hash = tickOnce();
    localHashes.set(turn, hash);
    send({ type: 'hash', turn, hash });
    const late = now > execTarget(turn) + TURN_MS;
    lateWindow.push(late);
    if (lateWindow.length > 240) lateWindow.shift();
    nextExec = turn + 1;
    checkHash(turn);
    if (onTurn) onTurn(turn, b[0], b[1], hash);
    maybeProposeDelay(now);
  }

  return {
    /// Anchor the pacing clock and open the match. Turns
    /// [0, delay) are empty by construction on both sides — that
    /// buffer IS the input delay.
    start(nowMs) {
      phase = 'running';
      anchorTurn = 0;
      anchorMs = nowMs;
      nextExec = 0;
      nextSendTurn = delay;
      nextSendAt = nowMs + TURN_MS;
      floorTurn = 0;
      for (let t = 0; t < delay; t++) bundles.set(t, [[], []]);
    },

    /// Record a local command into the currently-open input turn.
    /// Returns the turn it was stamped for.
    localCommand(cmd) {
      if (phase !== 'running') return -1;
      if (!validateCommand(cmd)) return -1;
      const list = filling.get(nextSendTurn) || [];
      list.push(cmd);
      filling.set(nextSendTurn, list);
      return nextSendTurn;
    },

    /// One frame of work: send due bundles, execute ready turns.
    pump(nowMs) {
      if (phase !== 'running') return;
      syncDelay();
      while (nowMs >= nextSendAt && phase === 'running') {
        const cmds = filling.get(nextSendTurn) || [];
        filling.delete(nextSendTurn);
        sendBundle(nextSendTurn, cmds);
        nextSendTurn++;
        nextSendAt += TURN_MS;
      }
      let isWaiting = false;
      let guard = 0;
      while (phase === 'running' && guard++ < 40) {
        const b = bundles.get(nextExec);
        if (!b || b[0] === null || b[1] === null) {
          isWaiting = execTarget(nextExec) <= nowMs;
          break;
        }
        if (execTarget(nextExec) > nowMs) break;
        executeTurn(nextExec, nowMs);
      }
      if (isWaiting !== waiting) {
        waiting = isWaiting;
        onEvent({ type: 'waiting', waiting });
      }
    },

    /// Feed an inbound (already decoded) message. Returns true when
    /// the session consumed it; lobby/handshake messages return
    /// false for the app layer to handle.
    receive(msg) {
      if (!msg || typeof msg !== 'object') return false;
      if (msg.type === 'bundle') {
        if (msg.turn < floorTurn) return true;
        const b = bundleFor(msg.turn);
        const remoteSlot = 1 - localSlot;
        if (b[remoteSlot] === null) b[remoteSlot] = msg.cmds;
        return true;
      }
      if (msg.type === 'hash') {
        remoteHashes.set(msg.turn, msg.hash);
        checkHash(msg.turn);
        return true;
      }
      if (msg.type === 'delay') {
        // Only the host proposes; the guest applies.
        if (localSlot === 1 && msg.turns > delay && msg.turns <= MAX_DELAY_TURNS) {
          pendingDelay = { atTurn: msg.atTurn, turns: msg.turns };
        }
        return true;
      }
      return false;
    },

    /// Rejoin, rejoiner side: the sim has been rebuilt and
    /// fast-forwarded from the recorded command stream (see
    /// createRecorder / the resync message) to the resync frontier
    /// — the last turn the remaining peer had executed when it
    /// captured the transfer. The session then opens at `turn`
    /// (= frontier + delay + 2, chosen by the remaining peer):
    /// turns [turn, turn+delay) are empty by construction (the
    /// same input-delay buffer as start()), and pacing anchors at
    /// the resume moment. The remaining peer runs rebase() to the
    /// same `turn`.
    ///
    /// THE TURN-ACCOUNTING INVARIANT (rejoin fix F2, 2026-10-22):
    /// when turn `turn` opens, EACH peer's sim has executed
    /// exactly turn * turnTicks ticks. The remaining peer honours
    /// it by EXECUTING the gap turns [frontier, turn) — rebase()
    /// pre-fills them empty and pump() ticks them. The rejoiner
    /// honoured it by fast-forwarding only the transferred ticks
    /// (= frontier * turnTicks) and then SKIPPING the gap — its
    /// sim stayed delay + 2 turns behind the host's forever, and
    /// the first resumed hash exchange desynced both sides (the
    /// 2026-10-22 live exercise, finding F2). So: pass
    /// `rebuiltTicks` (the rebuilt sim's actual tick count) and
    /// startAt first ticks the command-free gap turns itself.
    /// The gap turns carry no commands on either side by
    /// construction (the transfer's events end at the frontier;
    /// rebase pre-fills empties), so bare ticks here and executed
    /// empty turns there land on the identical state. The ticks
    /// produce no hash messages — the host's gap-turn hashes
    /// arrive and simply find no local counterpart.
    startAt(turn, nowMs, delayTurns = delay, rebuiltTicks = null) {
      if (typeof rebuiltTicks === 'number') {
        const target = turn * turnTicks;
        for (let t = rebuiltTicks; t < target; t++) tickOnce();
      }
      phase = 'running';
      delay = delayTurns;
      anchorTurn = turn;
      anchorMs = nowMs;
      nextExec = turn;
      floorTurn = turn;
      nextSendTurn = turn + delay;
      nextSendAt = nowMs + TURN_MS;
      for (let t = turn; t < turn + delay; t++) bundles.set(t, [[], []]);
    },

    /// Rejoin support, remaining-peer side: discard every
    /// unexecuted bundle (the two sides may disagree about them —
    /// the disconnect ate messages), pre-fill the small gap
    /// [nextExec, atTurn) as empty, and re-anchor the pacing clock
    /// at the resume moment. The pre-filled gap EXECUTES through
    /// pump() like any other turns — that execution is this side's
    /// half of the turn-accounting invariant (see startAt): by the
    /// time atTurn opens, this sim has ticked atTurn * turnTicks
    /// ticks, exactly like the rejoined peer's. Executed turns are
    /// final and identical, so the match continues from solid
    /// ground. Orders issued into the discarded window are lost —
    /// the honest cost of a dropped connection.
    rebase(atTurn, nowMs) {
      for (const t of [...bundles.keys()]) {
        if (t >= nextExec) bundles.delete(t);
      }
      filling.clear();
      for (let t = nextExec; t < atTurn; t++) bundles.set(t, [[], []]);
      floorTurn = atTurn;
      delay = Math.max(delay, initialDelay);
      pendingDelay = null;
      anchorTurn = atTurn;
      anchorMs = nowMs;
      nextSendTurn = atTurn + delay;
      nextSendAt = nowMs + TURN_MS;
      // The pre-filled gap's own sends: turns [atTurn, atTurn+delay)
      // still need bundles from both sides — they are produced by
      // the normal deadline flow starting at nextSendTurn, EXCEPT
      // the gap turns themselves, which both sides mark sent-empty
      // locally by pre-filling their own slot too:
      for (let t = atTurn; t < atTurn + delay; t++) bundles.set(t, [[], []]);
    },

    stop() {
      if (phase === 'running') phase = 'ended';
    },

    state() {
      return {
        phase,
        turn: nextExec,
        tick: nextExec * turnTicks,
        delay,
        waiting,
        droppedDupes,
        droppedOverflow,
      };
    },
  };
}

// ---------------------------------------------------------------------
// Recorder / replay
//
// One format for online and local matches:
//   { kind:'skirmish2-replay', v:1, seed, modeTag, region?, 
//     seats:[owner0, owner1?], result?, ticks, 
//     events:[{tick, s0:[cmds], s1?:[cmds]}] }
// Playback re-executes: construct the sim from (seed, modeTag),
// then at each event tick apply s0 then s1 (slot order — the same
// order the session used), ticking in between. Fog in the viewer is
// the recorded seat's (seats[0] by default), derived by the sim,
// never stored (NET-08).

export function createRecorder(meta) {
  const events = [];
  return {
    /// Pre-load existing events (rejoin: the rejoiner adopts the
    /// recording the remaining peer transferred, then keeps
    /// appending as the match resumes).
    seed(prior) {
      events.length = 0;
      for (const e of prior || []) events.push(e);
    },
    addTurn(turn, cmds0, cmds1) {
      if (!cmds0.length && !cmds1.length) return;
      const e = { tick: turn * TURN_TICKS, s0: cmds0 };
      if (cmds1 && cmds1.length) e.s1 = cmds1;
      events.push(e);
    },
    addLocal(tick, cmd) {
      const last = events[events.length - 1];
      if (last && last.tick === tick && !last.s1) last.s0.push(cmd);
      else events.push({ tick, s0: [cmd] });
    },
    finish(extra = {}) {
      return { kind: 'skirmish2-replay', v: 1, ...meta, ...extra, events };
    },
  };
}

export function isReplay(x) {
  return !!x && typeof x === 'object' && x.kind === 'skirmish2-replay'
    && x.v === 1 && Number.isInteger(x.seed) && Number.isInteger(x.modeTag)
    && Array.isArray(x.events);
}

/// The events of a recording, validated and in tick order.
export function replayEvents(rec) {
  if (!isReplay(rec)) return null;
  const out = [];
  for (const e of rec.events) {
    if (!e || !Number.isInteger(e.tick) || !Array.isArray(e.s0)) return null;
    for (const c of e.s0) if (!validateCommand(c)) return null;
    if (e.s1 !== undefined) {
      if (!Array.isArray(e.s1)) return null;
      for (const c of e.s1) if (!validateCommand(c)) return null;
    }
    out.push(e);
  }
  out.sort((a, b) => a.tick - b.tick);
  return out;
}

// ---------------------------------------------------------------------
// Relay transport + region ping
//
// The relay is a dumb broadcast pipe; this client is the whole
// transport. socketFactory is injected so tests can substitute
// fakes; in the browser and in Node 22+ it defaults to the global
// WebSocket.

export function wsBase(httpBase) {
  return httpBase.replace(/^http/, 'ws');
}

export function createRelayClient({ socketFactory, onMessage, onOpen, onClose, onError }) {
  const factory = socketFactory || ((url) => new WebSocket(url));
  let ws = null;
  return {
    connect(url) {
      ws = factory(url);
      const add = (ev, fn) => {
        if (typeof ws.addEventListener === 'function') ws.addEventListener(ev, fn);
        else ws['on' + ev] = fn;
      };
      add('open', () => onOpen && onOpen());
      add('message', (ev) => {
        const text = typeof ev === 'string' ? ev : ev.data;
        const dec = decodeMessage(text);
        if (dec.ok) onMessage && onMessage(dec.msg);
        // Undecodable traffic is dropped, never fatal.
      });
      add('close', () => onClose && onClose());
      add('error', (e) => onError && onError(e));
      return ws;
    },
    send(obj) {
      if (ws && ws.readyState === 1) {
        ws.send(encodeMessage(obj));
        return true;
      }
      return false;
    },
    sendRaw(text) {
      if (ws && ws.readyState === 1) {
        ws.send(text);
        return true;
      }
      return false;
    },
    close() {
      if (ws) {
        try { ws.close(); } catch { /* already gone */ }
        ws = null;
      }
    },
    get connected() {
      return !!ws && ws.readyState === 1;
    },
  };
}

/// Honest region ping: the round trip to a Durable Object PLACED in
/// the region, not to the nearest edge. The client opens a WebSocket
/// to a fresh probe room created under that region's location hint —
/// the object is instantiated in the region — and times an echo
/// round trip. The first echo pays the cold-start cost (object
/// creation + routing), so a second probe is sent and THAT round
/// trip is reported: it is the latency match traffic will see. If
/// echoes never come back, the WebSocket handshake time to the
/// probe room is reported instead, flagged via:'handshake'.
export function probeRegionPing({
  relayHttp, region, socketFactory, timeoutMs = 7000, rand,
}) {
  return new Promise((resolve) => {
    const factory = socketFactory || ((url) => new WebSocket(url));
    const suffix = (rand || Math.random().toString(36).slice(2, 10));
    const url = `${wsBase(relayHttp)}/room/probe-${region}-${suffix}?region=${encodeURIComponent(region)}`;
    const now = () => (typeof performance !== 'undefined' ? performance.now() : Date.now());
    const t0 = now();
    let ws;
    try {
      ws = factory(url);
    } catch {
      resolve(null);
      return;
    }
    let done = false;
    let openedAt = 0;
    let sentAt = 0;
    let echoes = 0;
    const finish = (v) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      try { ws.close(); } catch { /* noop */ }
      resolve(v);
    };
    const timer = setTimeout(() => {
      finish(openedAt ? { ms: Math.round(openedAt - t0), via: 'handshake' } : null);
    }, timeoutMs);
    const add = (ev, fn) => {
      if (typeof ws.addEventListener === 'function') ws.addEventListener(ev, fn);
      else ws['on' + ev] = fn;
    };
    add('open', () => {
      openedAt = now();
      sentAt = openedAt;
      ws.send('probe');
    });
    add('message', () => {
      echoes++;
      if (echoes === 1) {
        sentAt = now();
        ws.send('probe');
      } else {
        finish({ ms: Math.round(now() - sentAt), via: 'echo' });
      }
    });
    add('error', () => finish(null));
  });
}

// ---------------------------------------------------------------------
// Lobby helpers

const ROOM_ALPHABET = 'abcdefghjkmnpqrstuvwxyz23456789'; // no lookalikes

export function makeRoomId(rand = Math.random) {
  let id = '';
  for (let i = 0; i < 10; i++) {
    id += ROOM_ALPHABET[Math.floor(rand() * ROOM_ALPHABET.length)];
  }
  return id;
}

export function buildPlayUrl(base, room, region, opts = {}) {
  const sep = base.includes('?') ? '&' : '?';
  let url = `${base}${sep}room=${encodeURIComponent(room)}&region=${encodeURIComponent(region)}`;
  // M10: a private room's link says so, so the guest client can ask
  // for the password up front. The flag is not a secret — the room
  // is listed as locked in the directory anyway.
  if (opts && opts.private) url += '&private=1';
  return url;
}

/// Growth package (2026-10-24): a CHALLENGE link — the invite
/// link plus the challenge flag and the challenger's display
/// name, so the friend who opens it lands straight in the match
/// AND the chain knows who challenged them. The name is a
/// display string the challenger chose to share (it rides their
/// own link); it is NOT an identity claim — the relay cleans and
/// caps it again when the pairing is reported.
export function buildChallengeUrl(base, room, region, fromName) {
  const name = typeof fromName === 'string' ? fromName : '';
  return buildPlayUrl(base, room, region)
    + `&challenge=1&from=${encodeURIComponent(name)}`;
}

export function parsePlayParams(search) {
  const p = new URLSearchParams(search || '');
  return {
    room: p.get('room') || null,
    region: p.get('region') || null,
    online: p.get('online') === '1',
    private: p.get('private') === '1',
    // Growth package: a challenge link's flag + challenger name.
    // `from` is returned RAW (capped) — app.js runs it through
    // the name cleaner before it is shown or reported anywhere.
    challenge: p.get('challenge') === '1',
    from: (p.get('from') || '').slice(0, 64) || null,
  };
}

/// Reload rule (Claudio's direction): a room link is consumed
/// exactly ONCE per tab session. Returns the parsed params when
/// this boot should join the link's room, or null when it should
/// open the plain Lobby Browser. `consumedRoom` is the room id
/// this tab already took from a link — app.js keeps it in
/// sessionStorage, which survives a reload but not a new tab, so:
/// a fresh tab opening the link joins; reloading the same tab
/// (mark present, room matching) lands on the front door instead
/// of diving back into the room. The game loads only when a match
/// is actually entered.
export function linkJoinRoom(search, consumedRoom) {
  const params = parsePlayParams(search);
  if (!params.room) return null;
  if (consumedRoom && params.room === consumedRoom) return null;
  return params;
}

/// The region with the lowest measured ping (ties: list order).
/// Null when nothing measured — the lobby then defaults to the
/// first region rather than guessing.
export function pickFastestRegion(pings) {
  let best = null;
  let bestMs = Infinity;
  for (const r of REGIONS) {
    const ms = pings ? pings[r.id] : null;
    if (typeof ms === 'number' && ms < bestMs) {
      bestMs = ms;
      best = r.id;
    }
  }
  return best;
}
