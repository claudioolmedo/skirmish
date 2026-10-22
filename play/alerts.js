// Skirmish II — attack-alert location logic (INF-01, 2026-10-22
// night run). Pure functions only: no DOM, no sim access.
//
// INF-01's letter: when your units or buildings come under
// attack you get (a) an audible warning — the audio layer's
// underAttack cue, hard-throttled by its gate — and (b) a
// FLASHING MARKER AT THE LOCATION: the minimap flashes there,
// and the go-to key (SEL-10) jumps the camera to it. Alerts are
// rate-limited so a raid cannot spam the channel.
//
// app.js diffs own-seat HP per frame (the same diff that drives
// the underAttack cue); this module turns that diff into a
// located alert: which own thing lost HP, and where it stands.
// The marker's flash lifetime and blink phase are computed here
// so the minimap renderer stays a dumb painter and the behavior
// is unit-testable headlessly.

/// How long the minimap keeps flashing at an alert location.
export const ALERT_FLASH_MS = 6000;
/// Blink cadence of the flash: on for the first ON_MS of each
/// PERIOD_MS window.
export const ALERT_BLINK_PERIOD_MS = 500;
export const ALERT_BLINK_ON_MS = 300;
/// One located alert per this interval at most — the same clock
/// as the underAttack cue's gate in play/audio.js
/// (CUE_INTERVALS_MS.underAttack), so the horn and the marker
/// can never disagree about whether an attack was announced.
export const ALERT_MIN_INTERVAL_MS = 9000;

const EPS = 1e-9;

/**
 * The first entry (in `next`'s iteration order) that lost HP
 * between the two snapshots and has a known position.
 * `prev`/`next`: Map key -> hp. `meta`: Map key -> {x, y}.
 * Keys born between snapshots (not in `prev`) are not drops.
 * Returns { key, x, y } or null.
 */
export function firstHpDrop(prev, next, meta) {
  if (!prev || !next || !meta) return null;
  for (const [key, value] of next) {
    const before = prev.get(key);
    if (before == null || !(value < before - EPS)) continue;
    const m = meta.get(key);
    if (m) return { key, x: m.x, y: m.y };
  }
  return null;
}

/**
 * The HP drop nearest to a world point (the camera center, for
 * distance-scaled combat noise — INF-10). Same snapshot shapes
 * as firstHpDrop. Returns { key, x, y, dist } or null.
 */
export function nearestHpDrop(prev, next, meta, cx, cy) {
  if (!prev || !next || !meta) return null;
  let best = null;
  for (const [key, value] of next) {
    const before = prev.get(key);
    if (before == null || !(value < before - EPS)) continue;
    const m = meta.get(key);
    if (!m) continue;
    const dist = Math.hypot(m.x - cx, m.y - cy);
    if (!best || dist < best.dist) best = { key, x: m.x, y: m.y, dist };
  }
  return best;
}

/** An alert is { x, y, at } (at = ms timestamp of the attack). */
export function alertFresh(alert, now) {
  return !!alert && now - alert.at >= 0 && now - alert.at < ALERT_FLASH_MS;
}

/** The blink phase: true while the marker is in its lit window. */
export function alertFlashOn(alert, now) {
  if (!alertFresh(alert, now)) return false;
  return (now - alert.at) % ALERT_BLINK_PERIOD_MS < ALERT_BLINK_ON_MS;
}

/** Whether a new located alert may be raised at `now`. */
export function alertDue(alert, now) {
  return !alert || now - alert.at >= ALERT_MIN_INTERVAL_MS;
}
