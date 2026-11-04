//! MOV-06 (public API): groups travel at their slowest member's
//! speed — proven through the ordinary command queue on the
//! standard opening package, the way the loop's external tests
//! pin every other rule: the starting Scout Cavalry (1.20
//! tiles/s) ordered together with a Villager (0.8 tiles/s) in
//! one batch travels at the Villager's speed and arrives with
//! it, instead of stringing out ahead at its own pace.
//!
//! Staging: a scanned open corridor (uniform height, nothing
//! blocked) hosts the travel legs, so path lengths are exact.
//! Both units are walked to the corridor's start by SEPARATE
//! solo orders in separate ticks first (no group — which the
//! solo assertions below also pin), then group-ordered along it.

use fixed::types::I16F16;
use skirmish_sim::{Command, Entity, EntityKind, Sim};

const VILLAGER_SPEED_BITS: i32 = 3_495; // 0.8 tiles/s (the scout's own is 5243 = 1.20)

fn fx(v: f64) -> I16F16 {
    I16F16::from_num(v)
}

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (f(e.pos_x), f(e.pos_y))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn unit_of(sim: &Sim, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == kind && e.owner == 0 && !e.dead)
        .map(|e| e.id)
        .unwrap_or_else(|| panic!("no living {kind:?}"))
}

/// The open corridor nearest to `near`: a run of `len`
/// unblocked uniform-height cells in one row. Returns the
/// centers of its first and last cells.
fn corridor(sim: &Sim, near: (f64, f64), len: u32) -> ((f64, f64), (f64, f64)) {
    let g = sim.grid();
    let mut best: Option<(f64, (u32, u32))> = None;
    for y in 4..116 {
        let mut run = 0;
        let mut h0 = 0;
        for x in 4..116 {
            if g.is_blocked(x, y) {
                run = 0;
                continue;
            }
            if run == 0 {
                h0 = g.height_at(x, y);
            }
            if g.height_at(x, y) != h0 {
                run = 0;
                continue;
            }
            run += 1;
            if run >= len {
                let start = (x + 1 - len, y);
                let (cx, cy) = g.cell_center(start.0, start.1);
                let d = dist((f(cx), f(cy)), near);
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, start));
                }
            }
        }
    }
    let (_, start) = best.expect("no open corridor on this map");
    let a = g.cell_center(start.0, start.1);
    let b = g.cell_center(start.0 + len - 1, start.1);
    ((f(a.0), f(a.1)), (f(b.0), f(b.1)))
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

/// Stage the scout and one villager at the corridor start by
/// solo orders in SEPARATE ticks (so no group can form), and
/// return (scout, villager, corridor_start, corridor_end).
fn staged_pair(sim: &mut Sim) -> (u32, u32, (f64, f64), (f64, f64)) {
    let scout = unit_of(sim, EntityKind::ScoutCavalry);
    let vil = unit_of(sim, EntityKind::Villager);
    let (start, end) = corridor(sim, pos(sim, scout), 16);
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(start.0),
        y: fx(start.1),
    });
    sim.tick(); // separate ticks: separate issue batches
    sim.push_command(Command::Move {
        entity: vil,
        x: fx(start.0),
        y: fx(start.1),
    });
    let ok = run_until(sim, 30_000, |s| {
        ent(s, scout).order.is_none() && ent(s, vil).order.is_none()
    });
    assert!(ok, "the staging walks never completed");
    (scout, vil, start, end)
}

#[test]
fn group_move_clamps_the_scout_to_the_villager() {
    let mut sim = Sim::new(42);
    let (scout, vil, _start, end) = staged_pair(&mut sim);
    // The staging orders were solo: neither may carry a stamp.
    // (Asserted again on fresh solo orders in the next test.)
    // One issue batch: both units, one destination.
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(end.0),
        y: fx(end.1),
    });
    sim.push_command(Command::Move {
        entity: vil,
        x: fx(end.0),
        y: fx(end.1),
    });
    // Road each member still has to cover, for the arrival
    // arithmetic below (the staging stops land within reach of
    // the corridor start, not exactly on it).
    let road_scout = dist(pos(&sim, scout), end);
    let road_vil = dist(pos(&sim, vil), end);
    sim.tick();
    let so = ent(&sim, scout).order.clone().expect("scout has no order");
    assert_ne!(so.group_id, 0, "the fast member was not group-stamped");
    assert_eq!(
        so.group_cap.to_bits(),
        VILLAGER_SPEED_BITS,
        "the cap must be the slowest member's speed"
    );
    assert_eq!(
        ent(&sim, vil).order.as_ref().unwrap().group_id,
        0,
        "the slowest member must not be stamped"
    );
    // Travel: the scout's speed never exceeds the cap, arrivals
    // land together, and the pair never strings out.
    let mut arrived: [Option<u64>; 2] = [None, None];
    let mut max_gap = 0.0f64;
    for t in 0..20_000 {
        sim.tick();
        let sv = ent(&sim, scout);
        let sp = (sv.vel_x.to_bits() as i64).pow(2) + (sv.vel_y.to_bits() as i64).pow(2);
        assert!(
            sp <= (VILLAGER_SPEED_BITS as i64 + 64).pow(2),
            "the scout exceeded the group cap: |v|^2 bits {sp}"
        );
        if arrived.iter().any(|a| a.is_none()) {
            max_gap = max_gap.max(dist(pos(&sim, scout), pos(&sim, vil)));
        }
        for (i, id) in [scout, vil].iter().enumerate() {
            if arrived[i].is_none() && ent(&sim, *id).order.is_none() {
                arrived[i] = Some(t);
            }
        }
        if arrived.iter().all(|a| a.is_some()) {
            break;
        }
    }
    let [sa, va] = arrived;
    let sa = sa.expect("the scout never arrived");
    let va = va.expect("the villager never arrived");
    // Arrival arithmetic, stated: both members travel at the
    // cap (3495 bits = 0.0533 wu/tick ⇒ 18.75 ticks per world
    // unit), so the arrival gap must match the gap in the road
    // each had to cover — plus the formation effect: en route
    // the pair settles at the separation equilibrium (2.0 wu
    // apart, leader first), so the leader enters the arrival
    // radius up to ~2 wu earlier, at slow-radius speeds (half
    // pace ⇒ ~75 ticks), plus jitter: tolerance 120 ticks.
    // The failure this rule kills: unclamped, the scout covers
    // its road at 1.20 tiles/s and lands ~350 ticks before the
    // villager — far outside this window.
    let expected = (road_scout - road_vil) * 18.75;
    let observed = sa as f64 - va as f64;
    assert!(
        (observed - expected).abs() <= 120.0,
        "arrivals do not match capped travel: scout {sa}, villager {va}, \
         roads {road_scout:.1}/{road_vil:.1} wu (expected gap {expected:.0} ticks)"
    );
    assert!(max_gap <= 10.0, "the pair strung out: max gap {max_gap:.1} wu");
    assert!(dist(pos(&sim, scout), end) < 1.5);
    assert!(dist(pos(&sim, vil), end) < 1.5);
}

#[test]
fn solo_scout_order_is_unstamped_and_faster_than_grouped() {
    // The grouped scout's road, measured solo in a twin sim:
    // same seed, same staging, but the leg is ordered for the
    // scout ALONE — it must cover measurably more ground in the
    // same window (its own 1.20 against the cap's 0.8).
    let leg = |grouped: bool| -> f64 {
        let mut sim = Sim::new(42);
        let (scout, vil, start, end) = staged_pair(&mut sim);
        let from = pos(&sim, scout);
        if grouped {
            sim.push_command(Command::Move {
                entity: vil,
                x: fx(end.0),
                y: fx(end.1),
            });
        }
        sim.push_command(Command::Move {
            entity: scout,
            x: fx(end.0),
            y: fx(end.1),
        });
        sim.tick();
        let o = ent(&sim, scout).order.clone().expect("no order");
        if grouped {
            assert_ne!(o.group_id, 0);
        } else {
            assert_eq!(o.group_id, 0, "a solo order was group-stamped");
            let _ = start; // (staging endpoint; arrival stops within reach of it)
        }
        for _ in 0..600 {
            sim.tick();
        }
        dist(from, pos(&sim, scout))
    };
    let solo = leg(false);
    let grouped = leg(true);
    assert!(
        solo > grouped + 8.0,
        "solo scout covered {solo:.1} wu vs grouped {grouped:.1} in 600 ticks — no clamp difference"
    );
}

#[test]
fn cap_releases_after_group_arrival() {
    let mut sim = Sim::new(42);
    let (scout, vil, start, end) = staged_pair(&mut sim);
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(end.0),
        y: fx(end.1),
    });
    sim.push_command(Command::Move {
        entity: vil,
        x: fx(end.0),
        y: fx(end.1),
    });
    sim.tick(); // install the orders before watching for their end
    let done = run_until(&mut sim, 20_000, |s| {
        ent(s, scout).order.is_none() && ent(s, vil).order.is_none()
    });
    assert!(done, "the group never arrived");
    // Clear the return road first: the villager walks off
    // perpendicular (a solo order in its own tick) and waits
    // there — otherwise the scout's measured leg is a
    // separation negotiation with a standing villager, not a
    // speed measurement.
    let ep = pos(&sim, vil);
    let mut parked = false;
    for p in [(ep.0, ep.1 + 40.0), (ep.0, ep.1 - 40.0), (ep.0 + 40.0, ep.1), (ep.0 - 40.0, ep.1)] {
        sim.push_command(Command::Move {
            entity: vil,
            x: fx(p.0),
            y: fx(p.1),
        });
        sim.tick();
        if ent(&sim, vil).order.is_none() {
            continue; // unreachable candidate: the order died at once
        }
        let done = run_until(&mut sim, 20_000, |s| ent(s, vil).order.is_none());
        if done && dist(pos(&sim, scout), pos(&sim, vil)) > 8.0 {
            parked = true;
            break;
        }
    }
    assert!(parked, "the villager could not be parked off the scout's road");
    // Back along the corridor, the scout ALONE: unstamped, and
    // covering ground at its own speed (in 400 ticks the cap
    // would allow ~21 wu; the scout's own speed covers ~32).
    let from = pos(&sim, scout);
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(start.0),
        y: fx(start.1),
    });
    sim.tick();
    assert_eq!(
        ent(&sim, scout).order.as_ref().unwrap().group_id,
        0,
        "the cap survived the group order's end"
    );
    for _ in 0..400 {
        sim.tick();
    }
    let covered = dist(from, pos(&sim, scout));
    assert!(
        covered > 26.0,
        "the scout stayed capped after arrival: covered {covered:.1} wu in 400 ticks"
    );
}

#[test]
fn group_move_save_round_trip_and_twins() {
    let script = |sim: &mut Sim| {
        let (scout, vil, _s, end) = staged_pair(sim);
        sim.push_command(Command::Move {
            entity: scout,
            x: fx(end.0),
            y: fx(end.1),
        });
        sim.push_command(Command::Move {
            entity: vil,
            x: fx(end.0),
            y: fx(end.1),
        });
        (scout, vil)
    };
    // Twins agree bit-for-bit over the whole group travel.
    let run = || -> Vec<u64> {
        let mut sim = Sim::new(42);
        script(&mut sim);
        (0..3_000).map(|_| sim.tick()).collect()
    };
    assert_eq!(run(), run(), "group travel broke replay determinism");
    // A save taken mid-travel restores the stamp and the hash.
    let mut sim = Sim::new(42);
    let (scout, _vil) = script(&mut sim);
    for _ in 0..120 {
        sim.tick();
    }
    assert_ne!(ent(&sim, scout).order.as_ref().unwrap().group_id, 0);
    let restored = Sim::load_game(&sim.save_game()).expect("group save did not restore");
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(
        ent(&restored, scout).order.as_ref().unwrap().group_cap.to_bits(),
        VILLAGER_SPEED_BITS
    );
}
