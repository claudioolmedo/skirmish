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
/// nothing).
export function canToggleReady({ phase, peerPresent, admitted }) {
  return phase === 'lobby' && !!peerPresent && !!admitted;
}

/// The start rule: the HOST's client — and only the host's —
/// initiates the start handshake, automatically, the moment both
/// joined players are ready (and the peer is admitted + present,
/// in the lobby phase).
export function startGate({
  role,
  phase,
  peerPresent,
  admitted,
  selfReady,
  peerReady,
}) {
  return (
    role === 'host' &&
    canToggleReady({ phase, peerPresent, admitted }) &&
    !!selfReady &&
    !!peerReady
  );
}
