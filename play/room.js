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

export function slotLabel(slot) {
  switch (slot) {
    case 'ai-easy': return 'AI · Easy';
    case 'ai-normal': return 'AI · Normal';
    case 'closed': return 'Closed';
    default: return 'Open';
  }
}

/// The room screen's configuration summary line, e.g.
/// "1v1 · Arabia · Conquest · Guest: AI · Normal". The settings
/// shown are the game's fixed facts today (map, victory,
/// players) plus the guest slot — from the guest's own seat the
/// last part simply reads "You".
export function roomConfigText({ guestSlot = 'open', selfIsGuest = false } = {}) {
  const guest = selfIsGuest ? 'You' : slotLabel(guestSlot);
  return `1v1 · Arabia · Conquest · Guest: ${guest}`;
}
