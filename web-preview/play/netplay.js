// Skirmish II — lockstep netplay (M8).
//
// The deterministic sim was built for this (Bettner & Terrano,
// "1500 Archers on a 28.8"): clients exchange ONLY commands, never
// state. Both sides run the identical sim from the identical seed;
// a command bundle from each player is applied, in slot order, at
// an agreed execution turn; after every turn the two state hashes
// are compared — a mismatch is a desync and the match stops.
//
// Layout of a match (NET-02, converged 2026-11-22 to the
// documented reference model — Bettner & Terrano, confirmed for
// DE by the 2026-11-22 research, skirmish-study/research-de/
// tec11-net02.md §B):
//   * 1 turn = 20 ticks. A tick is 1/60 s of GAME time; the game
//     runs at GAME_SPEED x wall clock (DE's Normal speed), so a
//     turn spans 20 x (1000/60) / 1.7 ≈ 196 ms of real time —
//     the reference design's ≈200 ms communication turn, with
//     ≈5.1 turns per real second.
//   * Input delay: commands are stamped for execution exactly
//     2 turns after the turn in progress when they are issued
//     (N+2 ≈ 400 ms at the standard speed) — the reference
//     design's scheduled turnaround, and the mechanism that
//     hides latency: input is never executed immediately, so
//     jitter inside the 2-turn window is invisible. The offset
//     is FIXED for the whole match; it never adapts mid-match.
//     What adapts to a slow peer is the PACE, never the offset:
//     a turn's ticks begin only when BOTH bundles for it have
//     arrived (an empty bundle is a heartbeat), so the slowest
//     machine sets the pace (NET-03) and sim time never drops.
//   * Within a turn, ticks execute one by one as their own
//     wall-clock deadlines pass — the world advances smoothly
//     tick by tick exactly as in the local game; only the
//     APPLICATION of command bundles is quantized to turn
//     boundaries (a turn's bundles apply before its first
//     tick). After a stall, due ticks catch up in bounded
//     bursts per pump, never skipping sim time.
//   * Hash cadence (NET-04, bot-loop 2026-10-29): one hash per
//     TICK — each turn's hash message carries the hash after every
//     one of its ticks, plus the SIM-06 subsystem parts per tick
//     when the integrator supplies them, so a desync stops with
//     the exact tick and the first diverging subsystem named.
//     Legacy single-hash messages (older cached clients) are
//     still accepted and compared against the turn's last tick.
//
// This module is pure protocol/session logic — no DOM, no globals —
// driven through injected callbacks (send / applyBundle / tickOnce),
// so the whole state machine is testable headlessly under Node with
// a fake transport and the real WASM sim. app.js is the thin
// integrator. The relay is a dumb pipe: presence (hello messages),
// roles (how you entered the room), and handshakes all live here,
// client-side.

export const PROTOCOL_VERSION = 1;
// NET-02: the communication turn is 20 ticks — ≈196 ms of wall
// time at the standard 1.7x speed, the reference design's
// ≈200 ms turn (see the header). Bundles, hash messages and
// the rejoin accounting are all counted in these turns.
export const TURN_TICKS = 20;
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
// TURNS (NET-02: 2 turns ≈ 392 ms at this speed).
export const GAME_SPEED = 1.7;
export const TICK_MS = 1000 / 60; // one sim tick at 1.0x (game ms)
export const TURN_MS = (TURN_TICKS * TICK_MS) / GAME_SPEED; // ≈196.08 ms of wall time per turn

// SIM-11 (bot-loop, 2026-10-28): the match speed setting. DE lobbies
// offer 1.0 / 1.5 / 2.0 with 1.7 as the standard ranked speed; the
// setting multiplies how many game-seconds pass per real second and
// NOTHING else — per-game-second behaviour is identical at every
// speed, and the sim never learns the speed. A match carries ONE
// speed, chosen before it starts (the Create Lobby settings for a
// hosted match — the host's choice rides the 'start' handshake —
// or the pause menu's speed row, applied on Restart for a local
// match), and the replay meta records it so playback paces the
// match as it was played.
export const GAME_SPEEDS = [1.0, 1.5, 1.7, 2.0];
export function isGameSpeed(v) {
  return typeof v === 'number' && GAME_SPEEDS.includes(v);
}
export function turnMsFor(speed) {
  return (TURN_TICKS * TICK_MS) / (isGameSpeed(speed) ? speed : GAME_SPEED);
}

/**
 * Advance a tick accumulator by `dtMs` of wall time at `speed`
 * (default GAME_SPEED, DE Normal). The accumulator is kept in GAME
 * milliseconds: pass the previous remainder back in as `accGameMs`.
 * Returns how many whole ticks to run now and the new remainder.
 * Pure — the frame loop, the replay driver and the tests share it.
 */
export function advanceTicks(accGameMs, dtMs, speed = GAME_SPEED) {
  const s = isGameSpeed(speed) ? speed : GAME_SPEED;
  const total = accGameMs + dtMs * s;
  const ticks = Math.floor(total / TICK_MS);
  return { ticks, accMs: total - ticks * TICK_MS };
}
// NET-02: the fixed input delay, in turns. A command issued
// during turn N is stamped for turn N+2 and executes there —
// ≈400 ms of scheduled delay at the standard speed, the
// reference design's turnaround and its latency-hiding
// mechanism. It is a match constant: it never adapts mid-match
// (the pre-convergence transport proposed raises on lagging
// links; the documented model instead lets the slowest peer
// set the pace, NET-03, and keeps the offset at +2). The name
// is historical — this is no longer just the INITIAL delay.
export const INITIAL_DELAY_TURNS = 2;
// NET-02: the per-pump catch-up bound, in TICKS. After a stall
// (a late peer, a backgrounded tab) many ticks can be due at
// once; a pump executes at most this many before yielding to
// the frame loop, so catch-up compresses wall time over a few
// frames instead of freezing one frame. 240 ticks = 12 turns —
// far above the ~2 ticks a 60 fps frame owes at standard speed.
export const MAX_CATCHUP_TICKS = 240;
// NET-07's per-player command budget: a player may contribute
// at most BUNDLE_CAP commands to any one turn. The sender drops
// the excess (counted in the session's droppedOverflow) and the
// codec rejects an inbound bundle past twice the cap whole —
// command volume per player is bounded by the transport, never
// by the sim's patience. 256 is far above real play (Bettner &
// Terrano: an order every 1.5–2 s against several turns a
// second), so the cap only ever bites spam.
export const BUNDLE_CAP = 256;
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
  'push_build_town_center', 'push_build_stone_wall', 'push_build_castle',
  'push_build_university', 'push_build_siege_workshop',
  'push_build_palisade_gate', 'push_build_stone_gate', 'push_build_monastery',
];
// Appended after the literal, exactly as app.js's PUSH_BUILD
// does: tag 24 is the Fish Trap (Water W2b) — the one kind
// past the literal this table dispatches. Tags 22 (Wonder)
// and 23 (Dock) keep their pre-W2b state: local dispatch
// only, not wire-legal (see the validator), so no entries
// are added for them here.
BUILD_FNS[24] = 'push_build_fish_trap';
const BUILD_FNS_QUEUED = BUILD_FNS.map((n) => n + '_queued');
const TRAIN_AT_FNS = {
  2: 'push_train_scout', 3: 'push_train_militia', 4: 'push_train_archer',
  9: 'push_train_spearman', 10: 'push_train_skirmisher',
  12: 'push_train_knight', 14: 'push_train_cavalry_archer',
  15: 'push_train_petard', 16: 'push_train_trade_cart',
  17: 'push_train_battering_ram', 18: 'push_train_mangonel',
  19: 'push_train_scorpion',
  17: 'push_train_battering_ram', 20: 'push_train_monk',
  23: 'push_train_fishing_ship',
  24: 'push_train_galley',
  25: 'push_train_fire_galley',
  26: 'push_train_demolition_raft',
  27: 'push_train_hulk',
  32: 'push_train_transport_ship',
  // Water W5 (TRD-05): the Trade Cog.
  33: 'push_train_trade_cog',
};

const isInt = (v) => Number.isInteger(v) && v >= 0;
const isNum = (v) => typeof v === 'number' && Number.isFinite(v);

/// Structural validation for one semantic command (NET-07's first
/// line of defense: nothing malformed ever reaches the sim).
export function validateCommand(c) {
  if (!c || typeof c !== 'object' || typeof c.t !== 'string') return false;
  switch (c.t) {
    case 'move': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    // MOV-07: a formation move — move's shape plus the
    // formation tag (0 line, 1 box, 2 staggered, 3 flank).
    case 'formation_move': return isInt(c.e) && isNum(c.x) && isNum(c.y)
      && isInt(c.f) && c.f <= 3;
    case 'gather': return isInt(c.e) && isInt(c.n);
    case 'attack': return isInt(c.e) && isInt(c.tg);
    case 'attackb': return isInt(c.e) && isInt(c.b);
    case 'repair': return isInt(c.e) && isInt(c.b);
    // Package D (TRD-05): a Trade Cart tasked to a foreign
    // Market — an entity id and a building id, like repair.
    case 'trade': return isInt(c.e) && isInt(c.b);
    // Castle P5a (MNK-10): a Monk tasked to heal a unit — an
    // entity id and a target entity id, like attack's shape.
    case 'heal': return isInt(c.e) && isInt(c.tg);
    // Castle P5b (MNK-03): a Monk tasked to convert a unit —
    // an entity id and a target entity id, like heal's shape.
    case 'convert': return isInt(c.e) && isInt(c.tg);
    // Castle P5c (MNK-04): a Monk tasked to convert a building —
    // the building rides its own field (buildings have their own
    // id space, like attackb's).
    case 'convertb': return isInt(c.e) && isInt(c.b);
    // Castle P5d (MNK-09): a Monk tasked to pick up a relic —
    // an entity id and the relic's entity id, like heal's shape.
    case 'pickup': return isInt(c.e) && isInt(c.tg);
    // Castle P5d (MNK-09): a carrying Monk tasked to deposit
    // its relic in a Monastery — the building rides its own
    // field, like convertb's shape.
    case 'deposit': return isInt(c.e) && isInt(c.b);
    // The cap is the build table's own length (BUILD_FNS): every
    // kind the applier can dispatch is wire-legal, the first
    // index past the table is junk. (BLD-20 raised it to 14 with
    // the Stone Wall; 13 — the Town Center — was already
    // dispatchable but not validatable, an old inconsistency.)
    // In practice the wire vocabulary is 0..18 plus, since
    // Water W2b, the Fish Trap (24): 19/20 stay dispatchable
    // but not wire-legal (the pinned quirk — netplay.test.mjs
    // names 19 as junk), 21 is the unconstructible Monument,
    // and 22/23 (Wonder, Dock) remain local-dispatch only, as
    // they were before W2b.
    case 'build': return isInt(c.e) && isInt(c.k) && (c.k <= 18 || c.k === 24) && isNum(c.x) && isNum(c.y);
    // TRD-01..03: Market trades — a building id and a commodity
    // tag (food 1, wood 2, stone 4; gold is never a commodity).
    case 'mbuy': case 'msell':
      return isInt(c.b) && (c.r === 1 || c.r === 2 || c.r === 4);
    // BLD-20: a wall line names its wall kind (9 palisade when
    // absent — the pre-BLD-20 wire form — or 14 stone wall).
    case 'buildline': return isInt(c.e) && isNum(c.x0) && isNum(c.y0) && isNum(c.x1) && isNum(c.y1)
      && (c.k === undefined || c.k === 9 || c.k === 14);
    case 'train_at': return isInt(c.b) && (c.k === 2 || c.k === 3 || c.k === 4 || c.k === 9 || c.k === 10 || c.k === 12 || c.k === 14 || c.k === 15 || c.k === 16 || c.k === 17 || c.k === 18 || c.k === 19 || c.k === 20 || c.k === 23 || c.k === 24 || c.k === 25 || c.k === 26 || c.k === 27 || c.k === 32 || c.k === 33);
    case 'train_vil': return c.tc === undefined || isInt(c.tc);
    case 'research_tc': return (c.tech === 0 || c.tech === 1 || c.tech === 6
      || c.tech === 14 || c.tech === 15 || c.tech === 18
      || c.tech === 27 || c.tech === 28) && (c.tc === undefined || isInt(c.tc));
    case 'research_at': return isInt(c.b) && isInt(c.tech) && (c.tech <= 13 || c.tech === 16 || c.tech === 17 || c.tech === 19
      || (c.tech >= 20 && c.tech <= 26) || (c.tech >= 29 && c.tech <= 57));
    // Package K (TRD-04): a tribute names the recipient's owner
    // tag (0 or 2 — the only seats), a ResourceKind tag (food 1,
    // wood 2, gold 3, stone 4) and a positive amount. The amount
    // ceiling is a sanity bound only; the sim's own validation
    // (holdings, the fee, the 30 floor) decides acceptance.
    case 'tribute': return (c.to === 0 || c.to === 2)
      && isInt(c.r) && c.r >= 1 && c.r <= 4
      && isInt(c.a) && c.a >= 1 && c.a <= 100000;
    // Teams T3b (NET-11): a diplomacy stance change names the
    // target's owner tag (0 or 2 — the only seats) and a stance
    // tag (0 Ally / 1 Neutral / 2 Enemy, the sim's STANCE_*
    // tags). The sender is the bundle's owner (applyCommand's
    // contract), exactly like tribute's. The wire form is
    // {t:'diplomacy'} — NOT {t:'stance'}: that form is CMB-09's
    // per-unit stance (an entity id and a 0..3 tag) and the two
    // must never be confusable at the codec.
    case 'diplomacy': return (c.to === 0 || c.to === 2)
      && isInt(c.s) && c.s <= 2;
    // ECO-14 (wave-1 leftovers): the Mill reseed desk — the
    // building names the Mill; the sim derives its owner and
    // validates the rest (completed Mill, the wood, the queue).
    case 'queue_reseed': return isInt(c.b);
    case 'dequeue_reseed': return isInt(c.b);
    case 'set_auto_reseed': return isInt(c.b) && typeof c.on === 'boolean';
    // Water W2b (ECO-15): the Dock trap-reseed desk — the
    // ECO-14 trio's mirror; the building names the Dock.
    case 'queue_trap_reseed': return isInt(c.b);
    case 'dequeue_trap_reseed': return isInt(c.b);
    case 'set_auto_trap_reseed': return isInt(c.b) && typeof c.on === 'boolean';
    case 'rally_tc': return isNum(c.x) && isNum(c.y) && (c.tc === undefined || isInt(c.tc));
    case 'rally_b': return isInt(c.b) && isNum(c.x) && isNum(c.y);
    case 'force_dep': return isInt(c.e);
    // Castle P3b: garrison orders, ungarrison, Town Bell.
    case 'garrison_b': return isInt(c.e) && isInt(c.b);
    case 'garrison_tc': return isInt(c.e) && (c.tc == null || isInt(c.tc));
    case 'ungarrison_b': return isInt(c.b);
    case 'ungarrison_tc': return c.tc == null || isInt(c.tc);
    case 'town_bell': return c.tc == null || isInt(c.tc);
    // Castle P4a: the Ram garrison vocabulary — an entity id
    // and a host (Ram) entity id, like garrison_b's shape.
    case 'garrison_u': return isInt(c.e) && isInt(c.host);
    case 'ungarrison_u': return isInt(c.host);
    case 'load_t': return isInt(c.e) && isInt(c.ship);
    case 'unload_t': return isInt(c.ship);
    case 'auto_explore': return isInt(c.e) && typeof c.on === 'boolean';
    case 'attackmove': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    // Castle P4b (CMB-16): attack ground — an entity id and a
    // ground point, attackmove's shape; the queue flag rides
    // the same optional q as attackmove/patrol.
    case 'attackground': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    case 'patrol': return isInt(c.e) && isNum(c.x) && isNum(c.y);
    case 'stance': return isInt(c.e) && isInt(c.s) && c.s <= 3;
    case 'stop': return isInt(c.e);
    case 'delete': return isInt(c.e);
    case 'deleteb': return isInt(c.b);
    case 'cancel_tc': return isInt(c.tc) && isInt(c.i) && c.i <= 15;
    case 'cancel_b': return isInt(c.b) && isInt(c.i) && c.i <= 15;
    // VIC-11: a resignation names nothing — the resigning seat
    // is the bundle's owner (applyCommand's contract), exactly
    // like tribute's sender. (Distinct from the relay-level
    // 'resign' MESSAGE, which stays the transport's own signal.)
    case 'resign': return true;
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
    case 'formation_move': return cmd.q
      ? sim.push_formation_move_queued(cmd.e, cmd.x, cmd.y, cmd.f)
      : sim.push_formation_move(cmd.e, cmd.x, cmd.y, cmd.f);
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
    case 'repair_tc': return cmd.q
      ? sim.push_repair_tc_queued(cmd.e, cmd.tc)
      : sim.push_repair_tc(cmd.e, cmd.tc);
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
    case 'garrison_b': return sim.push_garrison_building(cmd.e, cmd.b);
    case 'garrison_tc': return sim.push_garrison_tc(cmd.e, cmd.tc != null ? cmd.tc : tcOf(owner));
    case 'ungarrison_b': return sim.push_ungarrison_building(cmd.b);
    case 'ungarrison_tc': return sim.push_ungarrison_tc(cmd.tc != null ? cmd.tc : tcOf(owner));
    case 'town_bell': return sim.push_town_bell(cmd.tc != null ? cmd.tc : tcOf(owner));
    case 'garrison_u': return sim.push_garrison_unit(cmd.e, cmd.host);
    case 'ungarrison_u': return sim.push_ungarrison_unit(cmd.host);
    case 'load_t': return sim.push_load_transport(cmd.e, cmd.ship);
    case 'unload_t': return sim.push_unload_transport(cmd.ship);
    case 'auto_explore': return sim.push_auto_explore(cmd.e, cmd.on);
    case 'attackmove': return cmd.q
      ? sim.push_attack_move_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_attack_move(cmd.e, cmd.x, cmd.y);
    case 'attackground': return cmd.q
      ? sim.push_attack_ground_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_attack_ground(cmd.e, cmd.x, cmd.y);
    case 'patrol': return cmd.q
      ? sim.push_patrol_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_patrol(cmd.e, cmd.x, cmd.y);
    case 'stance': return sim.push_set_stance(cmd.e, cmd.s);
    case 'stop': return sim.push_stop(cmd.e);
    case 'delete': return sim.push_delete(cmd.e);
    case 'deleteb': return sim.push_delete_building(cmd.b);
    case 'cancel_tc': return sim.push_cancel_queue_tc(cmd.tc, cmd.i);
    case 'cancel_b': return sim.push_cancel_queue_building(cmd.b, cmd.i);
    // Package D (TRD-05): task a Trade Cart to a foreign Market
    // — the sim derives validity from the cart and the depot, so
    // one form serves both seats (as repair).
    case 'trade': return sim.push_trade(cmd.e, cmd.b);
    // Castle P5a (MNK-10): a Monk's heal order — the sim
    // derives validity from the monk and the target, so one
    // form serves both seats (as trade).
    case 'heal': return sim.push_heal(cmd.e, cmd.tg);
    // Castle P5b (MNK-03): a Monk's convert order — the sim
    // derives validity from the monk and the target, so one
    // form serves both seats (as heal).
    case 'convert': return sim.push_convert(cmd.e, cmd.tg);
    // Castle P5c (MNK-04): the building form of the convert
    // order — the sim re-validates (Redemption, eligibility,
    // full faith) when the command is applied.
    case 'convertb': return sim.push_convert_building(cmd.e, cmd.b);
    // Castle P5d (MNK-09): a Monk's relic orders — the sim
    // derives validity from the monk and the relic/building, so
    // one form serves both seats (as heal).
    case 'pickup': return sim.push_pickup_relic(cmd.e, cmd.tg);
    case 'deposit': return sim.push_deposit_relic(cmd.e, cmd.b);
    // TRD-01..03: Market trades — the sim derives the owner from
    // the building, so one form serves both seats (as research_at).
    case 'mbuy': return sim.push_market_buy(cmd.b, cmd.r);
    case 'msell': return sim.push_market_sell(cmd.b, cmd.r);
    // Package K (TRD-04): a tribute — the sender is the bundle's
    // owner (applyCommand's contract), the recipient rides in
    // the command; the sim validates both seats.
    case 'tribute': return sim.push_tribute(owner, cmd.to, cmd.r, cmd.a);
    // Teams T3b (NET-11): a diplomacy stance change — the sender
    // is the bundle's owner (as tribute's), the target and the
    // stance tag ride in the command; the sim re-validates at
    // apply time (locked teams refuse whole). Guarded like the
    // teams constructor: a stale wasm without the T3a export
    // simply refuses, and a mixed pair is caught by the
    // construction-hash check before play begins.
    case 'diplomacy': return typeof sim.push_set_diplomacy === 'function'
      ? sim.push_set_diplomacy(owner, cmd.to, cmd.s)
      : false;
    // ECO-14: the reseed desk — the Mill's owner is derived in
    // the sim from the named building (as research_at), so one
    // form serves both seats.
    case 'queue_reseed': return sim.push_queue_reseed(cmd.b);
    case 'dequeue_reseed': return sim.push_dequeue_reseed(cmd.b);
    case 'set_auto_reseed': return sim.push_set_auto_reseed(cmd.b, cmd.on);
    // Water W2b: the trap-reseed desk — the Dock's owner is
    // derived in the sim from the named building, like the
    // reseed desk above. Guarded like diplomacy: a stale wasm
    // without the exports simply refuses.
    case 'queue_trap_reseed': return typeof sim.push_queue_trap_reseed === 'function'
      ? sim.push_queue_trap_reseed(cmd.b) : false;
    case 'dequeue_trap_reseed': return typeof sim.push_dequeue_trap_reseed === 'function'
      ? sim.push_dequeue_trap_reseed(cmd.b) : false;
    case 'set_auto_trap_reseed': return typeof sim.push_set_auto_trap_reseed === 'function'
      ? sim.push_set_auto_trap_reseed(cmd.b, cmd.on) : false;
    // VIC-11: resignation — the resigning seat is the bundle's
    // owner, so both clients sweep the same player at the same
    // tick and the ruins agree bit-for-bit.
    case 'resign': return sim.push_resign(owner);
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
  // P2P chat signaling (2026-10-24): WebRTC session descriptions
  // and ICE candidates for the direct player-to-player chat
  // channel (play/chat.js). The relay stays a dumb pipe for this
  // envelope like any other; the CHAT ITSELF never rides this
  // socket — text and taunt codes travel only over the encrypted
  // RTCDataChannel the signaling sets up. The codec bounds the
  // payload shapes below; the session never consumes 'rtc'
  // (receive() returns false — the app's chat link handles it),
  // and nothing here reaches the sim or the hashed state.
  'rtc',
  // Teams T3b (INF-04, NET-10): a Signal Allies flare. Like the
  // chat signaling above, a flare is a NON-command envelope: it
  // travels beside the bundle stream, never enters a bundle,
  // never reaches the sim or the hashed state, and a replay need
  // not contain it. The session never consumes it (receive()
  // returns false) — the app layer paints the marker. The codec
  // bounds the coordinates below; the sender's seat is known
  // from the transport (the only other seat in the room).
  'flare',
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
  if (msg.type === 'hash') {
    // NET-04: per-tick form {turn, hashes:[hex...], parts?:[[hex...]...]}
    // or the legacy per-turn form {turn, hash}. Parts, when present,
    // are SIM-06 subsystem snapshots: one row per tick, one hex
    // string per subsystem, in HASH_PART_NAMES order.
    const isHexList = (v) => Array.isArray(v) && v.length >= 1 && v.length <= 8
      && v.every((h) => typeof h === 'string' && h.length <= 64);
    // The hash list itself carries one entry per TICK of the
    // turn — 20 since the NET-02 convergence (the subsystem
    // rows inside `parts` stay ≤ 8 wide). The bound is a junk
    // filter with headroom over the turn size, not the turn
    // size itself.
    const isHashList = (v) => Array.isArray(v) && v.length >= 1 && v.length <= 64
      && v.every((h) => typeof h === 'string' && h.length <= 64);
    const newForm = isHashList(msg.hashes)
      && (msg.parts === undefined
        || (Array.isArray(msg.parts) && msg.parts.length === msg.hashes.length
          && msg.parts.every((row) => isHexList(row) && row.length <= 8)));
    const legacyForm = typeof msg.hash === 'string' && msg.hash.length <= 64;
    if (!isInt(msg.turn) || (!newForm && !legacyForm)) {
      return { ok: false, error: 'bad hash' };
    }
  }
  // Legacy delay proposals (pre-NET-02-convergence clients):
  // still decodable for wire compatibility, but the session
  // never consumes them — the scheduling offset is fixed now.
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
  // A flare names one world point. World coordinates are
  // bounded by the map (|x|, |y| < 100 today); the codec allows
  // generous slack for map growth and rejects anything wilder
  // as junk — a malformed flare is dropped, never painted.
  if (msg.type === 'flare') {
    if (!isNum(msg.x) || !isNum(msg.y)
      || Math.abs(msg.x) > 1000 || Math.abs(msg.y) > 1000) {
      return { ok: false, error: 'bad flare' };
    }
  }
  // WebRTC signaling for the P2P chat channel: exactly three
  // kinds, each with its one payload field, size-bounded (an SDP
  // blob is a few KB; anything larger is junk). The descriptions
  // and candidates are opaque to every layer here — validated
  // for shape only, forwarded by the relay untouched.
  if (msg.type === 'rtc') {
    if (msg.kind === 'offer' || msg.kind === 'answer') {
      if (typeof msg.sdp !== 'string' || msg.sdp.length < 1 || msg.sdp.length > 16384) {
        return { ok: false, error: 'bad rtc' };
      }
    } else if (msg.kind === 'ice') {
      if (typeof msg.candidate !== 'string' || msg.candidate.length > 1024) {
        return { ok: false, error: 'bad rtc' };
      }
      if (msg.sdpMid !== null && typeof msg.sdpMid !== 'string') {
        return { ok: false, error: 'bad rtc' };
      }
      if (msg.sdpMLineIndex !== null && !isInt(msg.sdpMLineIndex)) {
        return { ok: false, error: 'bad rtc' };
      }
    } else {
      return { ok: false, error: 'bad rtc' };
    }
  }
  return { ok: true, msg };
}

// ---------------------------------------------------------------------
// The turn session
//
// A pure state machine. app.js (or a test) drives it with pump(now)
// from its frame loop; inbound messages arrive through receive().

// SIM-06 subsystem names, in HashParts field order — the first
// part whose snapshot differs at the diverging tick names the
// subsystem a desync report blames (NET-04 diagnostics).
export const HASH_PART_NAMES = ['rng', 'world', 'economy', 'targeting'];

export function createSession({
  localSlot,
  turnTicks = TURN_TICKS,
  initialDelay = INITIAL_DELAY_TURNS,
  speed = GAME_SPEED,
  send,
  applyBundle,
  tickOnce,
  hashParts = null,
  onEvent = () => {},
  onTurn = null,
}) {
  // SIM-11: this session's pace. Both lockstep clients build their
  // session from the SAME match speed (the host's, from the 'start'
  // handshake), so their turn pacing agrees tick for tick.
  const TURN_MS = turnMsFor(speed);
  let phase = 'idle'; // idle | running | desync | ended
  let anchorTurn = 0; // execution-time anchor: turn -> wall time
  let anchorMs = 0;
  let nextExec = 0;
  let nextSendTurn = initialDelay;
  let nextSendAt = 0;
  // NET-02: the scheduling offset is a match constant (+2 turns,
  // INITIAL_DELAY_TURNS by default). startAt may restamp it from
  // the resync handshake (the remaining peer's value — the same
  // constant on any current build); nothing ever raises or
  // lowers it mid-match.
  let delay = initialDelay;
  const filling = new Map(); // turn -> cmds[] (local, unsent)
  const bundles = new Map(); // turn -> [cmds|null, cmds|null]
  const localHashes = new Map();
  const remoteHashes = new Map();
  let floorTurn = 0; // rebase floor: bundles below it are final/ignored
  let waiting = false;
  // NET-02: execution inside a turn is incremental — a turn's
  // bundles apply at its first tick, then its ticks run one by
  // one on the anchor schedule (see pump). ticksInTurn counts
  // the ticks of turn `nextExec` already run; turnHashes /
  // turnParts accumulate that turn's per-tick hash snapshots
  // until it completes and its hash message goes out.
  let ticksInTurn = 0;
  let turnHashes = null;
  let turnParts = null;
  let droppedDupes = 0;
  let droppedOverflow = 0;
  // NET-07: bundles the network delivered twice (or after their
  // turn finalized) and the session discarded — a redelivery is
  // idempotent here (the turn's slot is already filled, or below
  // the rebase floor), and this counter makes the discarding
  // explicit instead of silent.
  let droppedRedelivered = 0;

  // The wall-clock deadline of tick `k` (0-based) of `turn`:
  // ticks are spaced evenly across the turn's span, so a turn
  // completes exactly at the historical turn target (the anchor
  // plus a whole number of turns).
  const TICK_WALL_MS = TURN_MS / turnTicks;
  const tickTarget = (turn, k) =>
    anchorMs + ((turn - anchorTurn) * turnTicks + k + 1) * TICK_WALL_MS;

  function bundleFor(turn) {
    let b = bundles.get(turn);
    if (!b) {
      b = [null, null];
      bundles.set(turn, b);
    }
    return b;
  }

  // Stored hash entries are {hashes:[per-tick hex], parts:null |
  // [[per-subsystem hex] per tick]}. A legacy remote entry holds a
  // single hash, compared against the local turn's LAST tick.
  function checkHash(turn) {
    const local = localHashes.get(turn);
    const remote = remoteHashes.get(turn);
    if (local === undefined || remote === undefined) return;
    if (phase !== 'running') return;
    if (local.legacyLastTick || remote.legacyLastTick) {
      const l = local.hashes[local.hashes.length - 1];
      const r = remote.hashes[remote.hashes.length - 1];
      if (l !== r) {
        phase = 'desync';
        onEvent({
          type: 'desync', turn,
          tickInTurn: turnTicks - 1, tick: turn * turnTicks + turnTicks - 1,
          subsystem: null, localHash: l, remoteHash: r,
          localParts: null, remoteParts: null,
        });
      }
      return;
    }
    const n = Math.min(local.hashes.length, remote.hashes.length);
    for (let k = 0; k < n; k++) {
      if (local.hashes[k] === remote.hashes[k]) continue;
      let subsystem = null;
      const lp = local.parts && local.parts[k];
      const rp = remote.parts && remote.parts[k];
      if (lp && rp) {
        subsystem = 'tail';
        for (let i = 0; i < Math.min(lp.length, rp.length); i++) {
          if (lp[i] !== rp[i]) { subsystem = HASH_PART_NAMES[i] || 'tail'; break; }
        }
      }
      phase = 'desync';
      onEvent({
        type: 'desync', turn,
        tickInTurn: k, tick: turn * turnTicks + k, subsystem,
        localHash: local.hashes[k], remoteHash: remote.hashes[k],
        localParts: lp || null, remoteParts: rp || null,
      });
      return;
    }
    if (local.hashes.length !== remote.hashes.length) {
      phase = 'desync';
      onEvent({
        type: 'desync', turn,
        tickInTurn: n, tick: turn * turnTicks + n, subsystem: null,
        localHash: local.hashes[n] ?? null, remoteHash: remote.hashes[n] ?? null,
        localParts: null, remoteParts: null,
      });
    }
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

  // NET-02: a turn's execution, in three steps so pump can run
  // it incrementally. beginTurn applies both bundles (slot
  // order) before the turn's first tick — the ONLY moment
  // commands ever enter the sim; stepTick runs one tick and
  // records its hash snapshot; finishTurn ships the turn's
  // per-tick hash message and advances the frontier.
  function beginTurn(turn) {
    const b = bundles.get(turn);
    applyBundle(0, b[0]);
    applyBundle(1, b[1]);
    turnHashes = [];
    turnParts = hashParts ? [] : null;
  }

  function stepTick() {
    turnHashes.push(tickOnce());
    if (turnParts) turnParts.push(hashParts());
    ticksInTurn++;
  }

  function finishTurn(turn) {
    const b = bundles.get(turn);
    const hashes = turnHashes;
    const parts = turnParts;
    turnHashes = null;
    turnParts = null;
    ticksInTurn = 0;
    localHashes.set(turn, { hashes, parts });
    send(parts
      ? { type: 'hash', turn, hashes, parts }
      : { type: 'hash', turn, hashes });
    const hash = hashes[hashes.length - 1];
    nextExec = turn + 1;
    checkHash(turn);
    if (onTurn) onTurn(turn, b[0], b[1], hash);
  }

  // Run the in-flight turn (if any) to completion — its
  // remaining ticks, hash message and onTurn included — and
  // return the turn frontier. Callers that need the sim at a
  // turn boundary (rebase's discard accounting, app.js's
  // sendResync snapshot, whose tick count and recorded event
  // stream are only defined at a boundary) settle first. A
  // no-op when no turn is in flight.
  function settleTurn() {
    if (phase === 'running' && ticksInTurn > 0) {
      while (ticksInTurn < turnTicks) stepTick();
      finishTurn(nextExec);
    }
    return nextExec;
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
      ticksInTurn = 0;
      turnHashes = null;
      turnParts = null;
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

    /// One frame of work: send due bundles, execute due ticks.
    /// A turn's ticks begin only when BOTH of its bundles are
    /// in hand (lockstep — NET-01/03); once begun they run tick
    /// by tick on the anchor schedule, so the world advances as
    /// smoothly as the local game and only command application
    /// is quantized to turn boundaries (NET-02). After a stall
    /// the due ticks catch up in a bounded burst per pump —
    /// sim time is never dropped, only compressed.
    pump(nowMs) {
      if (phase !== 'running') return;
      while (nowMs >= nextSendAt && phase === 'running') {
        const cmds = filling.get(nextSendTurn) || [];
        filling.delete(nextSendTurn);
        sendBundle(nextSendTurn, cmds);
        nextSendTurn++;
        nextSendAt += TURN_MS;
      }
      let isWaiting = false;
      let guard = 0;
      while (phase === 'running' && guard++ < MAX_CATCHUP_TICKS) {
        const b = bundles.get(nextExec);
        if (!b || b[0] === null || b[1] === null) {
          // The next tick is due but its turn's bundles are not
          // both in hand: the slowest peer sets the pace.
          isWaiting = tickTarget(nextExec, ticksInTurn) <= nowMs;
          break;
        }
        if (tickTarget(nextExec, ticksInTurn) > nowMs) break;
        if (ticksInTurn === 0) beginTurn(nextExec);
        stepTick();
        if (ticksInTurn === turnTicks) finishTurn(nextExec);
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
        if (msg.turn < floorTurn) {
          // NET-07: a redelivery for a finalized turn — counted
          // and discarded; the turn already executed exactly once.
          droppedRedelivered++;
          return true;
        }
        const b = bundleFor(msg.turn);
        const remoteSlot = 1 - localSlot;
        if (b[remoteSlot] === null) {
          b[remoteSlot] = msg.cmds;
        } else {
          // NET-07: a redelivery of a bundle already held — the
          // slot keeps the first copy; the duplicate is counted
          // and discarded, never applied twice.
          droppedRedelivered++;
        }
        return true;
      }
      if (msg.type === 'hash') {
        if (Array.isArray(msg.hashes)) {
          remoteHashes.set(msg.turn, {
            hashes: msg.hashes,
            parts: Array.isArray(msg.parts) ? msg.parts : null,
          });
        } else {
          // Legacy per-turn hash: it is the hash after the turn's
          // last tick, so it compares against the local last tick
          // only (store a sparse entry checkHash understands).
          remoteHashes.set(msg.turn, {
            hashes: [msg.hash], legacyLastTick: true, parts: null,
          });
        }
        checkHash(msg.turn);
        return true;
      }
      // NET-02: there is no 'delay' message handling — the
      // scheduling offset never changes mid-match, so a delay
      // proposal (only a pre-convergence client sends one) is
      // not consumed here; it falls through to the app layer,
      // which ignores it.
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
      ticksInTurn = 0;
      turnHashes = null;
      turnParts = null;
      floorTurn = turn;
      nextSendTurn = turn + delay;
      nextSendAt = nowMs + TURN_MS;
      for (let t = turn; t < turn + delay; t++) bundles.set(t, [[], []]);
    },

    /// NET-02: settle any in-flight turn (see the closure) —
    /// app.js calls this before taking the resync snapshot, so
    /// the snapshot's tick count and event stream describe a
    /// turn boundary, exactly as they did when a turn executed
    /// atomically. Returns the turn frontier.
    settleTurn,

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
      // NET-02: execution inside a turn is incremental, so a
      // turn can be IN FLIGHT (bundles applied, some ticks run)
      // when the resume lands. That turn is already committed
      // in both sides' turn accounting — it cannot be discarded
      // like an unstarted one. Settle it first (its remaining
      // ticks run, its hash message and onTurn fire as usual),
      // restoring the invariant that rebase only ever discards
      // turns that never began.
      settleTurn();
      for (const t of [...bundles.keys()]) {
        if (t >= nextExec) bundles.delete(t);
      }
      filling.clear();
      for (let t = nextExec; t < atTurn; t++) bundles.set(t, [[], []]);
      floorTurn = atTurn;
      delay = Math.max(delay, initialDelay);
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
        droppedRedelivered,
      };
    },
  };
}

// ---------------------------------------------------------------------
// Teams meta (Teams T1, VIS-08)
//
// The match's team assignment — one team id per player slot,
// [host, guest] — travels exactly where the victory settings
// travel: the host's 'start' handshake (and its 'hello'
// advertisement and the resync), and the replay header. These
// helpers are the validation boundary for that field on arrival:
// a team id is one byte (0..255, 0xFF = NO_TEAM, unaffiliated),
// and anything malformed — wrong shape, out of range, not an
// array at all — falls back to the DEFAULT pair wholesale (the
// guestSlotOf discipline: junk must never ally two seats, seal a
// seat, or break a rebuild by accident). The default is distinct
// teams, [1, 2] — the configuration the game has always shipped.

export const DEFAULT_TEAMS = [1, 2];
export const NO_TEAM = 0xff;

/// The team pair a foreign value amounts to: itself (copied)
/// when it is exactly two byte-sized integers, the default pair
/// otherwise.
export function normalizeTeams(v) {
  if (Array.isArray(v) && v.length === 2
    && v.every((t) => Number.isInteger(t) && t >= 0 && t <= 0xff)) {
    return [v[0], v[1]];
  }
  return [...DEFAULT_TEAMS];
}

/// Whether a value amounts to the default (distinct-teams)
/// assignment after normalization.
export function teamsIsDefault(v) {
  const t = normalizeTeams(v);
  return t[0] === DEFAULT_TEAMS[0] && t[1] === DEFAULT_TEAMS[1];
}

/// Pack a team pair into the WASM constructor's u32 word:
/// slot 0's team in bits 0–7, slot 1's in bits 8–15. The default
/// pair packs as 513.
export function packTeams(v) {
  const t = normalizeTeams(v);
  return (t[0] | (t[1] << 8)) >>> 0;
}

/// The team pair a packed constructor word carries (the inverse
/// of packTeams; high bits past the two team bytes are ignored).
export function unpackTeams(packed) {
  if (!Number.isInteger(packed) || packed < 0) return [...DEFAULT_TEAMS];
  return [packed & 0xff, (packed >> 8) & 0xff];
}

// Teams T3b (NET-11): the Locked Teams half of the settings.
// DE's standard posture is locked (the stance table cannot
// change); an unlocked match lets the diplomacy panel's stance
// selectors issue real commands. The flag rides the SAME
// handshake/replay meta as the team pair, as its own boolean
// field, and enters the sim through bit 16 of the packed
// constructor word (T3a: set = unlocked; every pre-T3a packed
// word has the bit clear and reads locked). The validation
// boundary is strict, like the pair's: only the literal `true`
// unlocks — junk must never unlock a match by accident.
export const TEAMS_UNLOCKED_BIT = 1 << 16;

/// The unlocked flag a foreign value amounts to: true only for
/// the literal boolean true, false for everything else.
export function normalizeTeamsUnlocked(v) {
  return v === true;
}

/// The packed constructor word for a team pair plus the
/// unlocked flag (packTeams' two bytes, plus bit 16 while
/// unlocked). Locked packs exactly as packTeams always has.
export function packTeamsWithLock(v, unlocked) {
  return (packTeams(v) | (normalizeTeamsUnlocked(unlocked) ? TEAMS_UNLOCKED_BIT : 0)) >>> 0;
}

/// Whether a packed constructor word carries the unlocked bit.
export function teamsUnlockedFromPacked(packed) {
  return Number.isInteger(packed) && packed >= 0
    && (packed & TEAMS_UNLOCKED_BIT) !== 0;
}

// ---------------------------------------------------------------------
// Recorder / replay
//
// One format for online and local matches:
//   { kind:'skirmish2-replay', v:1, seed, modeTag, region?,
//     seats:[owner0, owner1?], result?, ticks, victory?, teams?,
//     events:[{tick, s0:[cmds], s1?:[cmds]}] }
// (`victory?` and `teams?` ride only while non-default — the
// recording keeps its pre-Package-C shape under default settings,
// and playback normalizes whatever it finds, junk to defaults.)
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

// Short links (glink, Claudio's request 2026-11-26): a room id is
// 7 chars and CARRIES its region in char 1, so the invite link can
// be just the site root plus the code —
// https://skirmish.online/g=wlehsls — with no region param for the
// guest client to need: it reads the region back out of the code.
// The prefix letters are distinct members of ROOM_ALPHABET, one
// per region in REGIONS, picked to hint at the region:
//   wnam 'w' (West North America), enam 'e' (East North America),
//   sam 's' (South America), weur 'u' (eUrope West), eeur 'r'
//   (euRope East), apac 'a' (Asia-Pacific), oc 'c' (oCeania).
const REGION_PREFIX = {
  wnam: 'w', enam: 'e', sam: 's', weur: 'u', eeur: 'r', apac: 'a', oc: 'c',
};
const PREFIX_REGION = Object.fromEntries(
  Object.entries(REGION_PREFIX).map(([region, ch]) => [ch, region]));

export function makeRoomId(regionId, rand = Math.random) {
  // An unknown region falls back to wnam's prefix — the same
  // default a region-less guest join has always landed on.
  let id = REGION_PREFIX[regionId] || REGION_PREFIX.wnam;
  for (let i = 0; i < 6; i++) {
    id += ROOM_ALPHABET[Math.floor(rand() * ROOM_ALPHABET.length)];
  }
  return id;
}

/// The region a room id carries in its first char, or null when
/// the id is not a short-link-era id: legacy 10-char codes (whose
/// links always named the region explicitly) and codes whose
/// prefix letter no region claims both read as null, and the
/// caller falls back to the explicit param / the wnam default.
export function regionFromRoomId(room) {
  if (typeof room !== 'string' || room.length !== 7) return null;
  return PREFIX_REGION[room[0]] || null;
}

/// The short invite link: the site root plus the code, nothing
/// else — https://skirmish.online/g=wlehsls. The root 404 page
/// bounces it to /play/?room=CODE (GitHub Pages serves the custom
/// 404 for the unknown /g= path), and the code itself carries the
/// region (see makeRoomId), so the guest needs no params at all.
/// One link form for every room, private included: a flag-less
/// guest of a private room is still asked for the password by the
/// host's 'pw-req' handshake before presence counts, so the
/// private=1 flag was only ever a prompt-timing courtesy.
export function buildShortUrl(origin, room) {
  return `${origin}/g=${room}`;
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

export function parsePlayParams(search) {
  const p = new URLSearchParams(search || '');
  // `g` is the room param's short alias, so /play/?g=CODE works
  // exactly like /play/?room=CODE (the root /g=CODE link itself
  // bounces through 404.html and lands here as ?room=).
  const room = p.get('room') || p.get('g') || null;
  // No explicit region: a short-link-era code carries its own
  // (regionFromRoomId), so the guest joins the room's real region
  // instead of the wnam default. An explicit region always wins,
  // and legacy 10-char codes derive nothing.
  const region = p.get('region') || regionFromRoomId(room);
  return {
    room,
    region,
    online: p.get('online') === '1',
    private: p.get('private') === '1',
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

/// Host-claim links (the homepage invite box): the homepage
/// mints a room code before anyone has entered the game and
/// offers it two ways — the short link for the friend (an
/// ordinary guest join, above) and /play/?dono=CODE for the
/// host ("dono" — the room's owner). Returns the code when this
/// boot should claim it: app.js then runs the ORDINARY create
/// flow with the code as an override, so the room is born with
/// exactly the code the homepage showed, on the region the code
/// carries. Returns null when there is nothing to claim: no
/// param, a malformed code, or a code this tab already claimed.
///
/// A claimable code has exactly the shape the guest side
/// accepts: 7 chars of the link alphabet with a region prefix
/// regionFromRoomId can read. Legacy 10-char codes and junk
/// claim nothing — the boot degrades to the plain Lobby Browser.
/// Consumed exactly once per tab, like a room link: the caller
/// passes the code this tab already claimed (sessionStorage in
/// app.js), so a reload never re-claims, while a DIFFERENT code
/// in the same tab is a fresh claim. Guest-first is safe by
/// construction: the friend may already be waiting inside the
/// room when the host claims it — the hello handshake pairs
/// latecomers (the first hello a client receives is answered
/// with its own), so presence completes on the host's arrival
/// and the both-ready gate fires as usual.
export function claimRoomFromParams(search, consumedCode) {
  const p = new URLSearchParams(search || '');
  const raw = p.get('dono');
  if (!raw) return null;
  const code = raw.toLowerCase();
  if (!/^[a-z2-9]{7}$/.test(code)) return null;
  if (!regionFromRoomId(code)) return null;
  if (consumedCode && code === consumedCode) return null;
  return code;
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
