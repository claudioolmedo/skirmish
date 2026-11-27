import { cleanDisplayName, GUEST_NAME } from './growth.js';

// room.js — the M10 room model's decision logic, kept pure so it
// is unit-testable without a DOM or a socket (same pattern as
// lobby.js / groups.js).
//
// The model (Claudio's direction, after the reference game's
// Steam lobbies):
//   - A room is PUBLIC (listed, anyone may join) or PRIVATE
//     (listed with a lock; joining needs the password).
//   - A match starts when ALL joined players have pressed Start
//     (ready) — the host's client runs the existing start
//     handshake the moment both flags are up.
//   - In a private room a guest does not count as present until
//     the host's client has accepted their password ('pw-ok').
//     The password lives only in the host client's memory; the
//     relay never sees it stored anywhere (it forwards bytes).

/// The host-side password check. The expected password must be a
/// non-empty string for anything to match — an unset password can
/// never be "entered correctly" (a public room has no check at
/// all; this function is only consulted for private rooms).
export function checkPassword(expected, given) {
  return (
    typeof expected === 'string' &&
    expected.length > 0 &&
    typeof given === 'string' &&
    given === expected
  );
}

/// The host-side migration-nonce check (auto-server): after the
/// pair's best server moves a PRIVATE room to a new region, the
/// guest who already passed the password check presents the
/// one-time nonce the host issued in its 'migrate' message
/// instead of being asked again. Same discipline as the password
/// check — an unset nonce can never be "presented correctly" —
/// and the nonce admits exactly like 'pw-ok' did: it is bound to
/// this room and this move, lives only in the two clients'
/// memory, and dies with the room.
export function checkNonce(expected, given) {
  return (
    typeof expected === 'string' &&
    expected.length > 0 &&
    typeof given === 'string' &&
    given === expected
  );
}

/// HOST view: is the peer admitted to the room? In a public room,
/// presence is admission. In a private room, only a guest who
/// passed the password check counts.
export function peerAdmitted({ privacy, peerPwOk }) {
  return privacy !== 'private' || !!peerPwOk;
}

/// GUEST view: is THIS client admitted? A guest of a public room
/// is admitted by connecting; a guest of a private room only once
/// its password state reached 'ok' (see pwState in app.js:
/// 'na' | 'needed' | 'sent' | 'wrong' | 'ok').
export function guestAdmitted({ isPrivate, pwState }) {
  return !isPrivate || pwState === 'ok';
}

/// May this player flip their ready flag right now? Only in the
/// lobby phase, with an admitted, present peer on both sides of
/// the gate (a lone player readying up starts nothing and means
/// nothing) — EXCEPT the M12 guest-slot cases below.
export function canToggleReady({ phase, peerPresent, admitted, guestSlot = 'open' }) {
  if (guestSlot === 'closed') return false; // no opponent, ever
  if (slotIsAi(guestSlot)) return phase === 'lobby'; // the host readies alone
  return phase === 'lobby' && !!peerPresent && !!admitted;
}

/// The start rule: the HOST's client — and only the host's —
/// initiates the start handshake, automatically, the moment both
/// joined players are ready (and the peer is admitted + present,
/// in the lobby phase). With an AI guest slot there is no second
/// player to wait for: the host's own ready IS the whole gate
/// (the AI seat always counts as ready). A closed slot can never
/// start.
export function startGate({
  role,
  phase,
  peerPresent,
  admitted,
  selfReady,
  peerReady,
  guestSlot = 'open',
}) {
  if (role !== 'host' || !selfReady) return false;
  if (slotIsAi(guestSlot)) return phase === 'lobby';
  return (
    canToggleReady({ phase, peerPresent, admitted, guestSlot }) &&
    !!peerReady
  );
}

// --- M12: the guest slot -------------------------------------------
//
// The host configures the room's second seat, the way the
// reference game's lobby does: Open (a human may take it), an AI
// at a difficulty (the in-sim scripted opponent fills it), or
// Closed (nobody). With an AI slot the host can run the whole
// online flow alone — room, ready, loading — and then play a
// LOCAL match against the scripted AI: no second client exists,
// so there is no lockstep session (ARCHITECTURE.md says so
// plainly). A seat filled by AI or Closed is not an open game:
// the room leaves the public directory while the slot is sealed.

export const GUEST_SLOTS = ['open', 'ai-easy', 'ai-normal', 'closed'];

export function isGuestSlot(v) {
  return GUEST_SLOTS.includes(v);
}

/// The slot value, or 'open' for anything unrecognized — a junk
/// value must never seal a seat by accident.
export function guestSlotOf(v) {
  return isGuestSlot(v) ? v : 'open';
}

export function slotIsAi(slot) {
  return slot === 'ai-easy' || slot === 'ai-normal';
}

/// The seat is filled without a human: no human may join, and the
/// room does not belong in the open-games directory.
export function slotSealed(slot) {
  return slotIsAi(slot) || slot === 'closed';
}

/// The local match mode an AI slot plays ('easy' | 'normal',
/// the sim's MatchMode names), or null for a non-AI slot.
export function slotAiMode(slot) {
  if (slot === 'ai-easy') return 'easy';
  if (slot === 'ai-normal') return 'normal';
  return null;
}

export function slotLabel(slot, s = null) {
  switch (slot) {
    case 'ai-easy': return (s && s.aiEasy) || 'AI · Easy';
    case 'ai-normal': return (s && s.aiNormal) || 'AI · Normal';
    case 'closed': return (s && s.closed) || 'Closed';
    default: return (s && s.open) || 'Open';
  }
}

// --- Teams T1: the guest seat's team pick ---------------------------
//
// Beside the guest slot, the host picks the guest seat's TEAM
// (Teams T1, VIS-08): 'opponent' (the default — distinct teams,
// the game as it has always shipped) or 'ally' (the guest seat
// shares the host's team, so the two clients render one shared
// line of sight). The pick is a room-level choice like the slot
// itself: the host sets it, it rides the 'start' handshake as the
// sim's team settings, and it exists only for a human guest seat
// — an AI seat is an opponent by construction (an allied AI seat
// cannot exist, and an enemy AI on your team is a contradiction
// the UI must not offer), and solo play has no second seat at all.

export const TEAM_PICKS = ['opponent', 'ally'];

export function isTeamPick(v) {
  return TEAM_PICKS.includes(v);
}

/// The pick value, or 'opponent' for anything unrecognized — a
/// junk value must never ally two seats by accident (the
/// guestSlotOf discipline).
export function teamPickOf(v) {
  return isTeamPick(v) ? v : 'opponent';
}

/// The sim's per-slot team ids for a pick, in slot order (slot 0
/// = host, slot 1 = guest): distinct teams [1, 2] for an
/// opponent, one shared team [1, 1] for an ally. A fresh array
/// every call — callers store and spread these.
export function teamsForPick(pick) {
  return teamPickOf(pick) === 'ally' ? [1, 1] : [1, 2];
}

/// The pick a pair of per-slot team ids amounts to: 'ally' only
/// for a genuine shared team (equal ids, and not NO_TEAM 0xFF —
/// two unaffiliated players are enemies, not allies). Anything
/// else — distinct ids, malformed input — reads 'opponent'.
export function teamPickForTeams(teams) {
  if (!Array.isArray(teams) || teams.length !== 2) return 'opponent';
  const [a, b] = teams;
  if (!Number.isInteger(a) || !Number.isInteger(b)) return 'opponent';
  return a === b && a !== 0xff ? 'ally' : 'opponent';
}

export function teamLabel(pick, s = null) {
  return teamPickOf(pick) === 'ally'
    ? ((s && s.ally) || 'Ally')
    : ((s && s.opponent) || 'Opponent');
}

/// The room screen's configuration summary line, e.g.
/// "1v1 · Arabia · Conquest · Guest: AI · Normal". The settings
/// shown are the game's facts today (map, players) plus the
/// host's choices — the guest slot, the match speed (SIM-11)
/// and the victory conditions (Package C: VIC-05/06/07 — a
/// ready label composed by the caller, kind plus any clock /
/// score suffixes). From the guest's own seat the guest part
/// simply reads "You". Teams T1: an allied guest seat adds its
/// segment (`s.teamAlly`, composed by the caller like the other
/// localized pieces); the default opponent pick adds nothing —
/// default rooms read exactly as before. Teams T3b: an unlocked
/// room adds its segment the same way (`s.teamsUnlocked`).
// --- Peer display name on the seat lines ---------------------------
//
// The peer's chosen display name rides its 'hello' (growth.js:
// myName() in app.js) and app.js stores the cleaned copy in
// net.peerName — chat has shown it all along. The room slot lines
// and the loading rows, however, built their seat labels from
// role + colour only ('Guest — Red' / 'Red — Guest' and the host
// twins), so the other player NEVER saw the chosen name there
// (bot-loop #422, live two-client proof: a guest named
// "GuestPlayer" was only ever "Guest — Red" to the host).
//
// The label rule: a CUSTOM peer name replaces the role word and
// keeps the colour (the caller's localized `named` template,
// e.g. '{name} — Red' / 'Red — {name}'); anything else falls back
// to the caller's classic label. "Anything else" includes the
// GUEST_NAME sentinel itself — displayName() sends 'Guest' when
// nothing was typed, so an unset name must read as the localized
// classic label, never as the English word "Guest" in a pt/es UI.

/// The peer's CUSTOM display name, cleaned, or null when the
/// peer set none (unset / blank / the 'Guest' sentinel / junk).
export function customPeerName(peerName) {
  const name = cleanDisplayName(peerName);
  return name && name !== GUEST_NAME ? name : null;
}

/// Escape a string for interpolation into the seat lines'
/// innerHTML. A peer name is foreign input (chat already treats
/// it as text-only for the same reason); the label templates and
/// fallbacks are the client's own trusted strings, so only the
/// name itself goes through here.
export function escapeSeatName(name) {
  return String(name)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/// The seat label for the PEER's seat: the peer's custom name in
/// the caller's `named` template (a '{name}' string, as the i18n
/// tables carry it) when one is set, else `fallback` (the classic
/// role + colour label). Callers pass a null peerName for their
/// OWN seat, which always yields the fallback.
export function peerSeatLabel(peerName, { fallback = '', named = null } = {}) {
  const name = customPeerName(peerName);
  if (!name) return fallback;
  const safe = escapeSeatName(name);
  if (typeof named === 'string' && named.includes('{name}')) {
    return named.split('{name}').join(safe);
  }
  return safe;
}

export function roomConfigText({ guestSlot = 'open', selfIsGuest = false, speed = 1.7, victory = 'Conquest', teamPick = 'opponent', teamsUnlocked = false } = {}, s = null) {
  const guest = selfIsGuest
    ? ((s && s.you) || 'You')
    : slotLabel(guestSlot, s && s.slot);
  // SIM-11: the match speed is part of the configuration summary.
  const speedTxt = `${speed}×`;
  let base = s && s.format ? s.format(guest, speedTxt, victory) : `1v1 · Arabia · ${victory} · Guest: ${guest} · Speed: ${speedTxt}`;
  if (teamPickOf(teamPick) === 'ally') {
    base = `${base} · ${(s && s.teamAlly) || 'Team: Ally'}`;
  }
  // Teams T3b (NET-11): an unlocked-teams room names the posture
  // in the summary, like the allied pick does; the default
  // locked posture adds nothing — default rooms read exactly as
  // before. Only the literal true counts (junk never unlocks).
  if (teamsUnlocked === true) {
    base = `${base} · ${(s && s.teamsUnlocked) || 'Unlocked teams'}`;
  }
  return base;
}
