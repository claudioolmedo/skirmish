//! Fixed-point steering: separation + arrival (M1).
//!
//! All math here is integer-based and therefore deterministic on every
//! platform: no floats, no trigonometry, no hash maps — the same contract
//! as the rest of the sim. Distances are computed from raw fixed-point bits
//! with a hand-rolled integer square root.

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

/// One steering-snapshot entry: position + collision radius. The sim
/// builds the snapshot from the id-ordered entity list each tick.
pub type SnapshotEntry = (I16F16, I16F16, I16F16);
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
/// The sim also applies it to entities with *no* move order (M2, see
/// `shove`): a stationary villager must be shovable. If only ordered
/// entities reacted, an idle unit standing on another's path could cancel
/// its desired velocity exactly — a perfect head-on equilibrium, i.e. a
/// deadlock. With mutual shove, the blocker slides aside (half the
/// overlap per tick) and the traveler gets through.
pub fn separation(positions: &[SnapshotEntry], me: usize) -> (I16F16, I16F16) {
    let (px, py, my_r) = positions[me];
    let mut vx = I16F16::ZERO;
    let mut vy = I16F16::ZERO;
    // Iteration is over the id-ordered snapshot, so the sum is
    // deterministic.
    for (j, &(ox, oy, other_r)) in positions.iter().enumerate() {
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

/// The velocity an entity with *no* move order gets this tick: just the
/// separation shove, clamped to the entity's own speed. Standing units
/// are nudged aside by neighbors instead of acting as immovable walls.
pub fn shove(positions: &[SnapshotEntry], me: usize, max_speed: I16F16) -> (I16F16, I16F16) {
    let (mut vx, mut vy) = separation(positions, me);
    let vlen = vec_len(vx, vy);
    if vlen > max_speed {
        vx = vx / vlen * max_speed;
        vy = vy / vlen * max_speed;
    }
    (vx, vy)
}

/// Steering decision for one entity for one tick.
///
/// `positions` is the id-ordered snapshot of every mobile entity;
/// `me` is this entity's index into it. `max_speed` is the entity's
/// own table speed (M3: speed is a per-unit stat — MOV-01).
/// `apply_separation` is false for a melee unit actively attacking:
/// separation must not push it out of its target's reach (MOV-04).
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
    final_reach: I16F16,
) -> (I16F16, I16F16, usize, bool) {
    let (px, py, _) = positions[me];

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

    // Separation from neighbors, on top of the desired velocity —
    // except while a melee attack is engaged (MOV-04).
    if apply_separation {
        let (mut sx, mut sy) = separation(positions, me);
        // Pace correction (2026-10): separation is a per-tick
        // DISPLACEMENT calibrated at the M1/M2 prototype pace, when a
        // unit's speed was 1.0 world units/tick and a pair's push
        // (≤ ~0.05 for a slight overlap) was a few percent of it. At
        // the corrected tile scale a villager's whole speed is 0.053
        // units/tick, so two neighbors flanking the desired direction
        // can sum to a push that cancels the order exactly — a stable
        // fixed point (three villagers wedged in a forest pocket on
        // seed 9 froze bit-identically for 300k+ ticks: each pair at
        // d≈1.95, the y-pushes summing to −3068 bits against a +3072
        // desired, the residual axis-aligned step vetoed by the tree
        // slide). Cap the separation term at half the unit's own
        // speed: an order always keeps at least half its authority,
        // exact cancellation becomes impossible, and crowds still
        // spread (the term is unchanged below the cap). The orderless
        // `shove` path is NOT capped this way — a standing unit must
        // remain displaceable at up to its full speed, or it becomes
        // the immovable wall M2 designed the shove against.
        let slen = vec_len(sx, sy);
        let cap = max_speed / 2;
        if slen > cap {
            sx = sx / slen * cap;
            sy = sy / slen * cap;
        }
        vx += sx;
        vy += sy;
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
        )
    }

    #[test]
    fn steer_heads_for_waypoint() {
        let positions = [entry(0, 0)];
        let wps = [(I16F16::from_num(10), I16F16::ZERO)];
        let (vx, vy, idx, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, WAYPOINT_REACH);
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
        let (vx, _, _, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, WAYPOINT_REACH);
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
        let (_, _, _, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, WAYPOINT_REACH);
        assert!(arrived, "generic reach must complete the order");
        let (vx, _, _, arrived) = steer(&positions, 0, &wps, 0, MAX_SPEED, true, TREE_ARRIVE);
        assert!(!arrived, "tree reach must keep him walking");
        assert!(vx > I16F16::ZERO);
        // An intermediate waypoint still uses the generic reach even
        // on a tree order: 0.75 from waypoint 0 of 2, he advances.
        let wps2 = [
            (I16F16::from_num(0.75), I16F16::ZERO),
            (I16F16::from_num(10), I16F16::ZERO),
        ];
        let (_, _, idx, arrived) = steer(&positions, 0, &wps2, 0, MAX_SPEED, true, TREE_ARRIVE);
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
        let (vx, _, _, _) = steer(&positions, 0, &wps, 0, slow, true, WAYPOINT_REACH);
        assert_eq!(vx, slow);
    }

    #[test]
    fn separation_pushes_apart() {
        // Two entities 1 unit apart on x, both ordered far away: the left
        // one must feel a push toward -x on top of its desired velocity.
        let p = [entry(0, 0), entry(1, 0)];
        let wps = [(I16F16::from_num(100), I16F16::ZERO)];
        let (vx0, _, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, true, WAYPOINT_REACH);
        let (vx1, _, _, _) = steer(&p, 1, &wps, 0, MAX_SPEED, true, WAYPOINT_REACH);
        // Entity 0 is pushed left (negative x contribution)…
        assert!(vx0 < MAX_SPEED, "entity 0 felt no separation push");
        // …entity 1 is pushed right (it keeps full speed toward +x).
        assert_eq!(vx1, MAX_SPEED);
    }

    #[test]
    fn separation_can_be_suppressed_for_an_engaged_attacker() {
        // MOV-04: with separation suppressed, an overlapped entity
        // steers purely by its order — no sideways push at all.
        let p = [entry(0, 0), entry(1, 0)];
        let wps = [(I16F16::from_num(100), I16F16::ZERO)];
        let (vx, vy, _, _) = steer(&p, 0, &wps, 0, MAX_SPEED, false, WAYPOINT_REACH);
        assert_eq!((vx, vy), (MAX_SPEED, I16F16::ZERO));
    }

    #[test]
    fn separation_cannot_cancel_an_order() {
        // Pace-correction regression: two neighbors flanking the
        // desired direction at d≈1.95 (the seed-9 forest-pocket
        // geometry) sum to a −y push of ≈3068 bits — enough to cancel
        // a villager's +3072 desired y EXACTLY at the corrected speed
        // (3495 bits), freezing the unit forever. With the separation
        // term capped at half the unit's speed, the order keeps at
        // least half its authority: vy must stay ≥ ~half speed.
        let r = I16F16::from_num(1);
        let positions = [
            (I16F16::ZERO, I16F16::ZERO, r),
            (I16F16::from_num(-0.972), I16F16::from_num(1.686), r),
            (I16F16::from_num(0.966), I16F16::from_num(1.688), r),
        ];
        let wps = [(I16F16::ZERO, I16F16::from_num(100))];
        let villager_speed = I16F16::from_bits(3_495); // 0.8 tiles/s
        let (_, vy, _, arrived) = steer(&positions, 0, &wps, 0, villager_speed, true, WAYPOINT_REACH);
        assert!(!arrived);
        assert!(
            vy >= villager_speed / 2 - I16F16::from_bits(64),
            "separation cancelled the order: vy bits = {}",
            vy.to_bits()
        );
    }

    #[test]
    fn shove_moves_half_the_overlap() {
        // Two orderless entities 1 unit apart: each is shoved away from
        // the other by half the overlap (0.5), so one tick of mutual
        // shove separates the pair to exactly the separation radius.
        let p = [entry(0, 0), entry(1, 0)];
        let (vx0, vy0) = shove(&p, 0, MAX_SPEED);
        let (vx1, vy1) = shove(&p, 1, MAX_SPEED);
        assert_eq!((vx0, vy0), (I16F16::from_num(-0.5), I16F16::ZERO));
        assert_eq!((vx1, vy1), (I16F16::from_num(0.5), I16F16::ZERO));
        // Beyond the radius there is no shove at all.
        let far = [entry(0, 0), entry(3, 0)];
        assert_eq!(shove(&far, 0, MAX_SPEED), (I16F16::ZERO, I16F16::ZERO));
    }
}
