// Skirmish II control groups + hotkey helpers — standalone module
// for the play page (SEL-07, SEL-08/09).
//
// Pure logic only: no DOM access, no sim access. app.js owns the
// group state through this module and feeds it plain id lists; the
// sim remains the truth about which units live (recall filters
// through a caller-supplied predicate). Unit-tested headlessly —
// see groups.test.mjs.
//
// The model is DE's, as the wiki's Hotkey page documents it
// (SEL-07): Ctrl+<Number> assigns the selection to groups 1–10
// (the 0 key is group 10) and Ctrl+Alt+<Number> to groups
// 11–20; <Number> (or Alt+<Number> for the upper layer) recalls,
// pressing it twice in quick succession centers the camera,
// Shift+<Number> APPENDS the selection to the group, and
// Ctrl+Shift+<Number> selects the group and snaps the view.

/** How many control groups exist (1–10 plain layer, 11–20 Alt). */
export const GROUP_COUNT = 20;

/**
 * Create the group store. Membership is by entity id, kept in
 * assignment order, duplicates removed. The store never validates
 * ids — whether a member is alive/selectable is the caller's
 * business at recall time (the sim is the truth).
 */
export function createControlGroups() {
  const groups = new Map(); // n -> number[]
  const check = (n) => {
    if (!Number.isInteger(n) || n < 1 || n > GROUP_COUNT) {
      throw new Error(`control group must be 1..${GROUP_COUNT}, got ${n}`);
    }
  };
  return {
    /** Replace group n's contents with `ids` (Ctrl+number). */
    assign(n, ids) {
      check(n);
      groups.set(n, [...new Set(ids)]);
    },
    /** Add `ids` to group n, keeping existing members. */
    addToGroup(n, ids) {
      check(n);
      const cur = groups.get(n) || [];
      groups.set(n, [...new Set([...cur, ...ids])]);
    },
    /** Group n's member ids, assignment order (a copy). */
    members(n) {
      check(n);
      return [...(groups.get(n) || [])];
    },
    /**
     * Group n's members filtered by `keep(id)` — the caller passes
     * its liveness/selectability predicate, so dead units drop out
     * at recall time without the store tracking deaths.
     */
    recall(n, keep) {
      check(n);
      return (groups.get(n) || []).filter(keep);
    },
    /** Forget every group (a new match starts with none). */
    clear() {
      groups.clear();
    },
  };
}

/**
 * Double-tap detection for group recall: `tap(n, nowMs)` returns
 * 'center' when the same group is tapped again within `windowMs`
 * of the previous tap, 'select' otherwise. A third rapid tap
 * centers again (each tap re-arms the window).
 */
export function createTapTracker(windowMs = 350) {
  let lastN = 0;
  let lastT = -Infinity;
  return {
    tap(n, nowMs) {
      const again = n === lastN && nowMs - lastT <= windowMs;
      lastN = n;
      lastT = nowMs;
      return again ? 'center' : 'select';
    },
    reset() {
      lastN = 0;
      lastT = -Infinity;
    },
  };
}

/**
 * The next id in a cycling list (the idle keys, SEL-08/09): the
 * first id strictly after `lastId`, wrapping to the head. `ids` is
 * in the sim's id order. Returns null for an empty list; a
 * `lastId` that is no longer in the list (it got busy, or died)
 * simply resumes at the next greater id, or wraps.
 */
export function nextInCycle(ids, lastId) {
  if (!ids.length) return null;
  for (const id of ids) {
    if (id > lastId) return id;
  }
  return ids[0];
}
