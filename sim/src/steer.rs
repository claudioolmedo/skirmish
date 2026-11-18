//! Fixed-point steering: separation + arrival (M1).
//!
//! All math here is integer-based and therefore deterministic on every
//! platform: no floats, no trigonometry, no hash maps — the same contract
//! as the rest of the sim. Distances are computed from raw fixed-point bits
//! with a hand-rolled integer square root.

use crate::entity::EntityKind;
use fixed::types::I16F16;

/// The M1/M2 uniform speed in world units per tick. Steering takes
/// each entity's own speed as a parameter, and every production call
/// passes the entity's table speed (`Sim::current_speed`) — no
/// movement path clamps to this constant, and since the pace
/// correction the table rows are all far below it (fastest ≈ 0.103).
/// Kept as the snapshot default and for the steering unit tests.
/// Constants via raw bits (value × 2^16): from_num is not const in fixed 1.x.
pub const MAX_SPEED: I16F16 = I16F16::from_bits(65_536); // 1.0
/// Inside this radius of the final waypoint, speed scales down linearly
/// with distance (arrival behavior — no overshoot oscillation).
pub const SLOW_RADIUS: I16F16 = I16F16::from_bits(262_144); // 4.0
/// A waypoint closer than this counts as reached.
pub const WAYPOINT_REACH: I16F16 = I16F16::from_bits(65_536); // 1.0
/// The tighter final-waypoint reach for a woodcutter's tree order
/// (2026-10-24): with the generic 1.0 reach, steering completed the
/// order while the cutter was still ~4.3 units from the trunk —
/// the arrival circle around his stand point is entered long
/// before he reaches it — and he chopped from a full tile away
/// (Claudio's live report). Half a unit makes his stopping point
/// land at the stand itself (≤ stand + 0.5 from the trunk).
pub const TREE_ARRIVE: I16F16 = I16F16::from_bits(32_768); // 0.5
/// Separation distance between two standard units (M1/M2 semantics):
/// since M3 the actual distance is the sum of the two units' table
/// radii (MOV-05); two 1.0-radius units still separate at 2.0.
pub const SEPARATION_RADIUS: I16F16 = I16F16::from_bits(131_072); // 2.0

/// MOV-03 bump window: an ordered unit that makes near-zero
/// progress for this many consecutive ticks while a mobile
/// neighbour stands inside the pair's separation distance is
/// "bumped" — the movement phase repaths its order around the
/// blockers' current cells (the reference's bump-stop-repath;
/// Klingers, Meeting C++ 2025: "Units bump into others, stop
/// and repath around"). DE publishes NO number for this window
/// — the DE team's own pathfinding thread concedes the choice
/// is an open tuning question — so 30 ticks (0.5 s at 60 tps)
/// is a feel value, recorded as such, never as a DE fact.
pub const BUMP_WINDOW_TICKS: u32 = 30;
/// MOV-03 repath retry interval while waiting: when a bump
/// repath finds no route around the blockers (a 1-wide gap),
/// the unit waits and the repath is retried every this many
/// ticks. Same status as `BUMP_WINDOW_TICKS`: no DE number
/// exists; 60 ticks (1 s) by feel.
pub const BUMP_REPATH_INTERVAL: u32 = 60;

/// One steering-snapshot entry: position, collision radius, kind
/// and the entity's formation stamp (0 = not moving as a
/// formation). The sim builds the snapshot from the id-ordered
/// entity list each tick. MOV-03 reads the last two: units of one
/// formation do not collide with each other at all (sk-04 §2),
/// and the herdable-press carve-out keys on kinds.
pub type SnapshotEntry = (I16F16, I16F16, I16F16, EntityKind, u32);
/// Separation displacement factor: 0.5 = 32768/65536 raw bits.
///
/// Separation is *displacement*-based: a unit moves half the overlap per
/// tick, so a pair at distance d separates to exactly the radius in one
/// tick and stops — critically damped, no overshoot. (M1 used a 0.75
/// force scale; M2 testing showed the stiff spring, stateless velocity,
/// and 1-unit steps form a period-2 oscillation when units converge on
/// one point, and force equilibrium deadlocks against standing units.)
const SEPARATION_DISPLACEMENT: I16F16 = I16F16::from_bits(32768);

/// Integer square root (Newton's method). Pure integer ops — deterministic.
pub fn isqrt(x: u64) -> u64 {
    if x < 2 {
        return x;
    }
    let mut r = x;
    let mut y = r.div_ceil(2);
    while y < r {
        r = y;
        y = (r + x / r) / 2;
    }
    r
}

/// Length of a fixed-point vector, as fixed-point.
///
/// Computed from raw bits: `sqrt((dx·2^16)² + (dy·2^16)²) = dist·2^16`,
/// so the integer square root lands exactly back in raw fixed-point units.
/// Max input magnitude (~200 world units) keeps every intermediate safely
/// inside i64/u64 range.
pub fn vec_len(dx: I16F16, dy: I16F16) -> I16F16 {
    let dxb = dx.to_bits() as i64;
    let dyb = dy.to_bits() as i64;
    I16F16::from_bits(isqrt((dxb * dxb + dyb * dyb) as u64) as i32)
}

/// Separation push for one entity: the summed, *unclamped* displacement
/// of every neighbor inside the pair's separation distance — half the
/// overlap, pointing away from the neighbor. (This is the term `steer`
/// adds to the desired velocity, factored out unchanged.) Since M3 the
/// separation distance of a pair is the sum of their table radii
/// (MOV-05); for two standard 1.0-radius units it is the historical
/// `SEPARATION_RADIUS` (2.0).
///
/// MOV-03 (2026-11-17): this displacement term is NO LONGER the
/// steering separation — DE has no push-through between player
/// units (sk-04: standing units are solid; a collision's outcome
/// is a repath, never a displacement), so steering now uses
/// `contact_project` and orderless units hold their ground. The
/// term survives for exactly one documented carve-out: herdables
/// DO displace villagers (official Update 153015 — sheep traffic
/// pushes villagers; the update only attenuated it). The sim's
/// herdable press pass applies this same half-overlap
/// displacement to a villager, summed over herdable neighbours
/// only; the magnitude is the pre-MOV-03 system's — DE publishes
/// no number for it.
pub fn separation(positions: &[SnapshotEntry], me: usize) -> (I16F16, I16F16) {
    let (px, py, my_r, _, _) = positions[me];
    let mut vx = I16F16::ZERO;
    let mut vy = I16F16::ZERO;
    // Iteration is over the id-ordered snapshot, so the sum is
    // deterministic.
    for (j, &(ox, oy, other_r, _, _)) in positions.iter().enumerate() {
        if j == me {
            continue;
        }
        let sep_r = my_r + other_r;
        let sep_r_bits = sep_r.to_bits() as i64;
        let sep_r2 = sep_r_bits * sep_r_bits;
        let sx = px - ox;
        let sy = py - oy;
        let sxb = sx.to_bits() as i64;
        let syb = sy.to_bits() as i64;
        let d2 = sxb * sxb + syb * syb;
        if d2 == 0 || d2 >= sep_r2 {
            continue;
        }
        let d = I16F16::from_bits(isqrt(d2 as u64) as i32);
        let push = sep_r - d; // overlap, > 0 inside the radius
        vx += sx / d * push * SEPARATION_DISPLACEMENT;
        vy += sy / d * push * SEPARATION_DISPLACEMENT;
    }
    (vx, vy)
}

/// MOV-03 contact projection: the steering-side half of "no
/// pushing". For each neighbour already inside the pair's
/// separation distance (MOV-05: the sum of the two table radii),
/// remove the component of MY velocity that points INTO the
/// neighbour — I stop at contact and keep whatever tangential
/// component remains (sliding around a single blocker), and
/// nobody is ever displaced: the neighbour's velocity is not
/// mine to change. Applied per neighbour in snapshot order;
/// each projection is an orthogonal decomposition, so the
/// result never exceeds the incoming speed and never reverses
/// a component the order asked for.
///
/// Skips, both documented (sk-04): a pair sharing one formation
/// stamp does not collide at all (units in the same formation
/// do not collide with each other in DE); and a herdable
/// pressing a villager is not projected — the villager yields
/// through the herdable press pass instead (Update 153015).
/// A coincident pair (d == 0) has no contact normal and is
/// skipped: separating coincident spawns is its own package,
/// not MOV-03.
pub fn contact_project(
    positions: &[SnapshotEntry],
    me: usize,
    my_formation: u32,
    mut vx: I16F16,
    mut vy: I16F16,
) -> (I16F16, I16F16) {
    if vx == I16F16::ZERO && vy == I16F16::ZERO {
        return (vx, vy);
    }
    let (px, py, my_r, my_kind, _) = positions[me];
    for (j, &(ox, oy, other_r, other_kind, other_formation)) in positions.iter().enumerate() {
        if j == me {
            continue;
        }
        if my_formation != 0 && other_formation == my_formation {
            continue; // same formation: no collision at all
        }
        if my_kind == EntityKind::Sheep && other_kind == EntityKind::Villager {
            continue; // the herdable press: the villager yields
        }
        if other_kind == EntityKind::Sheep && my_kind != EntityKind::Sheep {
            // The herdable yield: a unit under way does not stop
            // for a sheep — it herds it. The press pass moves the
            // sheep out of the unit's ground (the other half of
            // the §A.5.1 herdable collision system: sheep traffic
            // displaces villagers, and units herd sheep — a
            // sheep that never yielded would make the documented
            // collision outcome, a correct path (Update 81058),
            // impossible wherever a sheep stands in a chokepoint).
            continue;
        }
        let sep_r = my_r + other_r;
        let sep_r_bits = sep_r.to_bits() as i64;
        let sep_r2 = sep_r_bits * sep_r_bits;
        let sx = px - ox;
        let sy = py - oy;
        let sxb = sx.to_bits() as i64;
        let syb = sy.to_bits() as i64;
        let d2 = sxb * sxb + syb * syb;
        if d2 == 0 || d2 >= sep_r2 {
            continue;
        }
        let d = I16F16::from_bits(isqrt(d2 as u64) as i32);
        // Unit normal from the neighbour toward me.
        let (nx, ny) = (sx / d, sy / d);
        // The velocity component along that normal: negative
        // means I am moving INTO the neighbour.
        let vn = vx * nx + vy * ny;
        if vn < I16F16::ZERO {
            vx -= vn * nx;
            vy -= vn * ny;
        }
    }
    (vx, vy)
}

/// Steering decision for one entity for one tick.
///
/// `positions` is the id-ordered snapshot of every mobile entity;
/// `me` is this entity's index into it. `max_speed` is the entity's
/// own table speed (M3: speed is a per-unit stat — MOV-01).
/// `apply_separation` is false for a melee unit actively attacking:
/// contact must not push it out of its target's reach (MOV-04).
/// `my_formation` is the entity's formation stamp (0 = none):
/// same-formation pairs never collide (MOV-03).
/// `final_reach` is the arrival radius for the LAST waypoint only
/// (intermediate waypoints always use `WAYPOINT_REACH`): most
/// orders pass `WAYPOINT_REACH` itself; a woodcutter's tree order
/// passes `TREE_ARRIVE` so he settles at his stand point beside
/// the trunk instead of stopping a full arrival radius short.
/// Returns `(vel_x, vel_y, new_wp_index, arrived)`.
pub fn steer(
    positions: &[SnapshotEntry],
    me: usize,
    waypoints: &[(I16F16, I16F16)],
    wp_index: usize,
    max_speed: I16F16,
    apply_separation: bool,
    my_formation: u32,
    final_reach: I16F16,
) -> (I16F16, I16F16, usize, bool) {
    let (px, py, _, _, _) = positions[me];

    // Advance past already-reached waypoints. Reaching the last one means
    // the order is complete.
    let mut idx = wp_index;
    let (wx, wy, dist) = loop {
        if idx >= waypoints.len() {
            return (I16F16::ZERO, I16F16::ZERO, idx, true);
        }
        let (wx, wy) = waypoints[idx];
        let d = vec_len(wx - px, wy - py);
        let reach = if idx + 1 == waypoints.len() {
            final_reach
        } else {
            WAYPOINT_REACH
        };
        if d >= reach {
            break (wx, wy, d);
        }
        idx += 1;
    };

    // Desired velocity toward the waypoint, slowing inside SLOW_RADIUS on
    // the final leg so the entity settles instead of orbiting the target.
    let mut speed = max_speed;
    if idx + 1 == waypoints.len() && dist < SLOW_RADIUS && dist > I16F16::ZERO {
        speed = max_speed * dist / SLOW_RADIUS;
    }
    let (mut vx, mut vy) = if dist > I16F16::ZERO {
        // |delta| <= dist always, so delta/dist can never overflow.
        ((wx - px) / dist * speed, (wy - py) / dist * speed)
    } else {
        (I16F16::ZERO, I16F16::ZERO)
    };

    // MOV-03 contact: a neighbour inside the pair's separation
    // distance is a wall, not a spring — the component of the
    // desired velocity that points into it is removed, the
    // tangential remainder slides around it, and the neighbour
    // is never displaced (see `contact_project`). Except while
    // a melee attack is engaged (MOV-04).
    //
    // History: until MOV-03 this spot ADDED the `separation`
    // displacement, capped at half the unit's speed (the 2026-10
    // pace correction — uncapped, two neighbours flanking the
    // desired direction could cancel an order exactly, the
    // seed-9 forest-pocket freeze). Displacement itself was the
    // divergence: DE units bump, stop and repath (sk-04); the
    // pocket case is now the bump pass's business, not a push.
    if apply_separation {
        let (seek_x, seek_y) = (vx, vy);
        (vx, vy) = contact_project(positions, me, my_formation, vx, vy);
        // Sequential projection against two flanking neighbours
        // can leave a small component pointing back OUT of the
        // pocket (the feasible cone between two contacts ahead
        // opens backward). A unit never moves against its own
        // order because of contact: if the projected velocity
        // lost the ordered direction entirely, it is a stop —
        // the bump pass's repath is what unsticks him.
        if vx * seek_x + vy * seek_y < I16F16::ZERO {
            vx = I16F16::ZERO;
            vy = I16F16::ZERO;
        }
        // Dead-on deflection: when contact has (nearly) stopped
        // the unit outright — the head-on meeting, where the
        // desired velocity is almost purely radial into the
        // blocker — the tangential remainder is ~zero and the
        // unit would sit out a whole bump window for what is
        // only a passing manoeuvre. DE units slide past each
        // other; the slide needs a tangential component to
        // start from. Deflect the seek to the unit's own left
        // (deterministic; two units meeting head-on deflect to
        // opposite world sides and pass), project the deflected
        // velocity against the same neighbours, and keep
        // whichever velocity is faster. Nobody is displaced
        // either way: this only chooses the unit's own heading.
        // Measured motive: two villagers meeting dead-on on a
        // woodline path each burned a full bump window per
        // meeting — an 18% tax on a 3-villager gather cycle
        // against the pre-MOV-03 flow, all of it stopped time.
        let seek_len = vec_len(seek_x, seek_y);
        if seek_len > I16F16::ZERO && vec_len(vx, vy) * 4 < seek_len {
            let inv = I16F16::ONE / seek_len;
            let (dvx0, dvy0) = (-seek_y * inv * seek_len, seek_x * inv * seek_len);
            let (mut dvx, mut dvy) = contact_project(positions, me, my_formation, dvx0, dvy0);
            if dvx * seek_x + dvy * seek_y < I16F16::ZERO {
                dvx = I16F16::ZERO;
                dvy = I16F16::ZERO;
            }
            if vec_len(dvx, dvy) > vec_len(vx, vy) {
                vx = dvx;
                vy = dvy;
            }
        }
    }

    // Clamp the combined velocity to the entity's own speed.
    let vlen = vec_len(vx, vy);
    if vlen > max_speed {
        vx = vx / vlen * max_speed;
        vy = vy / vlen * max_speed;
    }
    (vx, vy, idx, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isqrt_matches_known_values() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(2), 1);
        assert_eq!(isqrt(16), 4);
        assert_eq!(isqrt(1_000_000), 1000);
        // Largest magnitude the sim can produce: (200·2^16)²·2.
        let big: u64 = 2 * (200u64 * 65536) * (200u64 * 65536);
        assert_eq!(isqrt(big) as i64, 18_536_380); // ⌊200·2^16·√2⌋, cross-checked
    }

    #[test]
    fn vec_len_pythagorean() {
        // 3-4-5 triangle in fixed point.
        let d = vec_len(I16F16::from_num(3), I16F16::from_num(4));
        assert_eq!(d, I16F16::from_num(5));
        assert_eq!(vec_len(I16F16::ZERO, I16F16::ZERO), I16F16::ZERO);
    }

    fn entry(x: i32, y: i32) -> SnapshotEntry {
        (
            I16F16::from_num(x),
            I16F16::from_num(y),
            I16F16::from_num(1), // standard 1.0 collision radius
            EntityKind::Villager,
            0, // no formation stamp
        )
    }

    fn entry_kind(x: i32, y: i32, kind: EntityKind, formation: u32) -> SnapshotEntry {
        (
            I16F16::from_num(x),
            I16F16::from_num(y),
            I16F16::from_num(1),
            kind,
            formation,
        )
    }

    #[test]
    fn steer_heads_for_waypoint() {
        let positions = [entry(0, 0)];
        let wps = [(I16F16::from_num(10), I16F16::ZERO)];
        let (vx, vy, idx, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        assert!(!arrived);
        assert_eq!(idx, 0);
        assert!(vx > I16F16::ZERO, "must move toward +x");
        assert_eq!(vy, I16F16::ZERO);
        assert!(vec_len(vx, vy) <= MAX_SPEED);
    }

    #[test]
    fn steer_slows_near_final_waypoint() {
        let positions = [entry(0, 0)];
        // 2 units away, inside SLOW_RADIUS (4): speed must be scaled to ~half.
        let wps = [(I16F16::from_num(2), I16F16::ZERO)];
        let (vx, _, _, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        assert!(!arrived);
        assert!(vx < MAX_SPEED, "arrival slowdown missing: {vx}");
        assert!(vx > I16F16::ZERO);
    }

    #[test]
    fn steer_final_reach_governs_only_the_last_waypoint() {
        // 0.75 units from the target: inside the generic 1.0 reach
        // (order completes) but outside a tree order's 0.5 reach
        // (he keeps walking to his stand point).
        let positions = [entry(0, 0)];
        let wps = [(I16F16::from_num(0.75), I16F16::ZERO)];
        let (_, _, _, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        assert!(arrived, "generic reach must complete the order");
        let (vx, _, _, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, 0, TREE_ARRIVE);
        assert!(!arrived, "tree reach must keep him walking");
        assert!(vx > I16F16::ZERO);
        // An intermediate waypoint still uses the generic reach even
        // on a tree order: 0.75 from waypoint 0 of 2, he advances.
        let wps2 = [
            (I16F16::from_num(0.75), I16F16::ZERO),
            (I16F16::from_num(10), I16F16::ZERO),
        ];
        let (_, _, idx, arrived) = steer(&positions, 0, &wps2, 0, MAX_SPEED, true, 0, TREE_ARRIVE);
        assert!(!arrived);
        assert_eq!(idx, 1, "intermediate waypoint must use WAYPOINT_REACH");
    }

    #[test]
    fn steer_respects_the_units_own_speed() {
        // M3 (MOV-01): the clamp is the entity's table speed, not the
        // old global constant — a sheep (0.875) steers slower than a
        // villager even on the same order.
        let positions = [entry(0, 0)];
        let wps = [(I16F16::from_num(100), I16F16::ZERO)];
        let slow = I16F16::from_bits(57_344); // 0.875
        let (vx, _, _, _) = steer(&positions, 0, &wps, 0, slow, true, 0, WAYPOINT_REACH);
        assert_eq!(vx, slow);
    }

    #[test]
    fn separation_term_still_displaces_for_the_herdable_carve_out() {
        // MOV-03: `separation` itself is unchanged math — it is
        // the herdable press pass's displacement term now. Two
        // entities 1 unit apart: the term pushes each away from
        // the other by half the overlap (0.5).
        let p = [entry(0, 0), entry(1, 0)];
        assert_eq!(
            separation(&p, 0),
            (I16F16::from_num(-0.5), I16F16::ZERO)
        );
        assert_eq!(
            separation(&p, 1),
            (I16F16::from_num(0.5), I16F16::ZERO)
        );
        let far = [entry(0, 0), entry(3, 0)];
        assert_eq!(separation(&far, 0), (I16F16::ZERO, I16F16::ZERO));
    }

    #[test]
    fn contact_stops_the_approach_and_keeps_the_slide() {
        // MOV-03: two entities 1 unit apart on x (inside the 2.0
        // separation distance), both ordered far +x. Entity 0's
        // desired velocity points INTO entity 1: it is removed
        // wholesale — entity 0 stops at contact, and entity 1 is
        // not displaced by anything (steering never writes a
        // neighbour's velocity). Entity 1, moving away, keeps
        // full speed.
        let p = [entry(0, 0), entry(1, 0)];
        let wps = [(I16F16::from_num(100), I16F16::ZERO)];
        let (vx0, vy0, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        let (vx1, _, _, _) = steer(&p, 1, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        // The into-component is gone (vx == 0: the approach is
        // stopped) — but the dead-on deflection turns the stop
        // into a slide to the unit's own left, the passing
        // manoeuvre, instead of a full stop until the bump pass
        // repaths (re-pinned with the deflection: a same-
        // direction leader is passed, not queued behind, when
        // there is room to pass).
        assert_eq!(vx0, I16F16::ZERO, "approach not stopped");
        assert!(vy0 > I16F16::ZERO, "no dead-on deflection: ({vx0}, {vy0})");
        assert!(vec_len(vx0, vy0) <= MAX_SPEED);
        assert_eq!(vx1, MAX_SPEED);
        // Offset blocker: the tangential component survives —
        // the traveller slides around instead of stopping dead.
        let p2 = [entry(0, 0), entry(1, 1)];
        let (vx, vy, _, _) = steer(&p2, 0, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        assert!(vx > I16F16::ZERO && vy < I16F16::ZERO, "no slide: ({vx}, {vy})");
        assert!(vec_len(vx, vy) <= MAX_SPEED);
    }

    #[test]
    fn contact_skips_same_formation_pairs_and_the_herdable_press() {
        // MOV-03: same formation stamp — no collision at all:
        // the traveller keeps full speed straight at its mate.
        let wps = [(I16F16::from_num(100), I16F16::ZERO)];
        let p = [
            entry_kind(0, 0, EntityKind::Villager, 7),
            entry_kind(1, 0, EntityKind::Villager, 7),
        ];
        let (vx, vy, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, true, 7, WAYPOINT_REACH);
        assert_eq!((vx, vy), (MAX_SPEED, I16F16::ZERO));
        // A different stamp (or none) collides as usual: the
        // into-component is removed (the dead-on deflection may
        // turn the stop into a sideways slide, never a pass
        // through).
        let p = [
            entry_kind(0, 0, EntityKind::Villager, 7),
            entry_kind(1, 0, EntityKind::Villager, 9),
        ];
        let (vx, _, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, true, 7, WAYPOINT_REACH);
        assert_eq!(vx, I16F16::ZERO);
        // The herdable press: a sheep steering into a villager
        // is not projected (the villager yields via the press
        // pass); a villager steering into a sheep stops.
        let p = [
            entry_kind(0, 0, EntityKind::Sheep, 0),
            entry_kind(1, 0, EntityKind::Villager, 0),
        ];
        let (vx, _, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        assert_eq!(vx, MAX_SPEED, "sheep was projected off the villager");
        let (vx, _, _, _) = steer(&p, 1, &wps, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        assert_eq!(vx, MAX_SPEED, "villager moving away from the sheep");
        let wps_back = [(I16F16::from_num(-100), I16F16::ZERO)];
        let (vx, _, _, _) = steer(&p, 1, &wps_back, 0, MAX_SPEED, true, 0, WAYPOINT_REACH);
        // The herdable yield (re-pinned): a villager under way
        // does not stop for a sheep — he herds it (the press
        // pass moves the sheep out of his ground), so he keeps
        // his full speed toward it.
        assert_eq!(vx, -MAX_SPEED, "villager stopped for the sheep instead of herding it");
    }

    #[test]
    fn separation_can_be_suppressed_for_an_engaged_attacker() {
        // MOV-04: with separation suppressed, an overlapped entity
        // steers purely by its order — no sideways push at all.
        let p = [entry(0, 0), entry(1, 0)];
        let wps = [(I16F16::from_num(100), I16F16::ZERO)];
        let (vx, vy, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, false, 0, WAYPOINT_REACH);
        assert_eq!((vx, vy), (MAX_SPEED, I16F16::ZERO));
    }

    #[test]
    fn contact_in_the_seed9_pocket_stops_without_reversing() {
        // The seed-9 forest-pocket geometry that once froze three
        // villagers bit-identically for 300k+ ticks: two neighbours
        // flanking the desired direction at d≈1.95. Under the old
        // displacement the pair's pushes cancelled the order
        // exactly (a stable fixed point the pace correction then
        // capped). Under MOV-03 the traveller simply STOPS at
        // contact — symmetric projections leave no sideways drift
        // and, the invariant that matters, projection never
        // reverses the ordered direction or pushes the unit
        // backward: vy stays ≥ 0, |vx| stays a rounding hair.
        // Unsticking him is the bump pass's repath, not a push.
        let r = I16F16::from_num(1);
        let positions = [
            (I16F16::ZERO, I16F16::ZERO, r, EntityKind::Villager, 0),
            (I16F16::from_num(-0.972), I16F16::from_num(1.686), r, EntityKind::Villager, 0),
            (I16F16::from_num(0.966), I16F16::from_num(1.688), r, EntityKind::Villager, 0),
        ];
        let wps = [(I16F16::ZERO, I16F16::from_num(100))];
        let villager_speed = I16F16::from_bits(3_495); // 0.8 tiles/s
        let (vx, vy, _, arrived) = steer(&positions, 0, &wps, 0, villager_speed, true, 0, WAYPOINT_REACH);
        assert!(!arrived);
        assert!(vy >= I16F16::ZERO, "projection reversed the order: vy = {vy}");
        assert!(
            vy < villager_speed / 4,
            "the pocket did not stop the traveller: vy = {vy}"
        );
        assert!(
            vx.abs() < I16F16::from_bits(256),
            "asymmetric sideways drift in a symmetric pocket: vx = {vx}"
        );
    }
}
