//! OCCORDER9 (2026-11-19): engagement-layer probe for the
//! occluded-attacker defect, bot-loop run #432.
//!
//! The client pick/issue layer is PROVEN live (runs #431–432,
//! occorder7/occorder8): a right-click on a red outline under
//! the Town Center roof resolves the enemy and issues
//! `attack … ok:true`. But in live test 1 the ordered unit
//! never engaged: a Villager (e:3) ordered onto an enemy
//! Spearman (HP 21/45) standing inside the player's Town
//! Center footprint (east porch, draining the TC) stayed on
//! the opposite side of the TC, HP 25/25, for ~4 minutes of
//! game time; the Spearman's HP stayed exactly 21/45; the TC
//! drained 1972 → 457 and was destroyed (Defeat 46:08).
//!
//! This probe discriminates the candidate sim mechanisms:
//!   (a) approach/steering cannot reach melee range because
//!       the target stands inside a footprint;
//!   (b) the attack task's range check never certifies with
//!       no re-approach (a stale-order trap of the buildprobe
//!       shape);
//!   (c) villager attack behaviour specifically refuses or
//!       decays against military units;
//!   (d) a transient crowd at the TC strands the attacker;
//!   (e) the attacker was garrisoned inside the TC (a
//!       contained unit sits at the TC, full HP, and
//!       `attack_step` returns early for it — the one staging
//!       whose *shape* matches every live observable).
//!
//! OUTCOME: NO mechanism reproduces the live freeze.
//!   - The two `real_command_*` scenes stage the live geometry
//!     with NOTHING injected: a TwoPlayer match, the opponent's
//!     own starting Scout rides across the map by `Move`,
//!     attacks the player's Town Center by `Attack`, and the
//!     player's own Villager / Scout is Moved to the west side
//!     and ordered by `Attack`. Both engage and kill the foe
//!     (dead at ~1,931 / ~1,880 ticks of the attack window,
//!     min distance 2.95 / 2.97, attackers unharmed, the TC
//!     losing only ~19 HP meanwhile).
//!   - The spawn-staged Spearman scenes (the live kind pairing)
//!     agree: Villager, Scout and Militia attackers all close
//!     to melee reach and kill a Spearman that stays on the TC;
//!     against a Spearman that retaliates, the Villager closes,
//!     deals damage (45 → 9) and dies fighting — engagement,
//!     never refusal. (a), (b) and (c) are disproven.
//!   - The crowd scene (the player's other units standing on
//!     the TC centre/east ground) engages identically (foe
//!     dead at ~1,827 ticks). (d) is disproven in this form.
//!   - A unit cannot even REMAIN inside a BLOCKING footprint:
//!     the grid-integrity pass displaces a foe spawned at a
//!     Barracks centre out of the blocked cells on the first
//!     ticks, and an explicit Attack then engages it — (a)'s
//!     literal pocket form is not a persistent state.
//!   - (e) matches the shape but is REJECTED by measurement:
//!     a garrisoned Town Center FIRES (GAR-06). With one
//!     Villager inside, the same Spearman at the same porch
//!     dies to TC arrows in ~987 ticks while the TC loses only
//!     ~5 HP — the live foe kept exactly 21/45 for ~14,400
//!     ticks while draining 1,515 HP off the TC, which a
//!     firing TC cannot produce. The test-1 Villager was not
//!     inside the TC.
//!
//! One un-attributed observation is recorded, NOT fixed: an
//! Attack order addressed to a CONTAINED unit is accepted
//! (`can_attack` does not check containment, unlike the
//! `orderable` gate the Move family respects — GAR-02's own
//! "a garrisoned unit takes no orders"), is stored, never
//! executes and never clears while contained, and is silently
//! dropped on ejection (`place_at_exit` clears combat). It is
//! a real stuck-state of the same shape, but the TC-arrow
//! measurement above says it is not this defect's mechanism,
//! and this package does not speculate a fix for the live
//! freeze. The loop's next live vigia uses a MILITARY attacker
//! (the test-1 deviation removed) before any code change.
//!
//! Staging note: the `real_command_*` scenes inject nothing at
//! all. The remaining scenes spawn the foe (and, in one, a
//! Militia attacker) at a chosen point — scene selection in
//! the amoveprobe/retprobe discipline — while every order on
//! the path under test (the foe's Attack on the TC, the
//! attacker's Move and Attack, the GarrisonTc) goes through
//! the real `Command` path, and what is measured is what a
//! player sees: positions, HP, target state over ticks.

use crate::build::{Building, BuildingKind};
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, MatchMode, Sim, Stance};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) {
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        sim.tick();
        if done(sim) {
            return true;
        }
    }
    done(sim)
}

const FOE: u32 = 9_801;
const WINDOW: u64 = 14_400; // the live window: ~4 game-minutes

#[derive(Default, Debug)]
struct Engage {
    min_dist: f64,
    foe_hp_end: f64,
    foe_dead: bool,
    dead_tick: Option<u64>,
    atk_hp_end: f64,
    atk_dead: bool,
}

fn engage_report(tag: &str, m: &Engage) -> String {
    format!(
        "{tag} min_dist={:.2} foe_hp_end={:.0} foe_dead={} dead_tick={:?} atk_hp_end={:.0} atk_dead={}",
        m.min_dist, m.foe_hp_end, m.foe_dead, m.dead_tick, m.atk_hp_end, m.atk_dead
    )
}

/// Measure an already-issued Attack order over the live
/// window: attacker `atk`, target `foe`.
fn measure_engagement(sim: &mut Sim, atk: u32, foe: u32) -> Engage {
    let mut m = Engage {
        min_dist: f64::MAX,
        ..Default::default()
    };
    for t in 0..WINDOW {
        sim.tick();
        let a = ent(sim, atk);
        let fo = ent(sim, foe);
        let d = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        if fo.dead {
            m.dead_tick = Some(t);
            break;
        }
    }
    let a = ent(sim, atk);
    let fo = ent(sim, foe);
    m.foe_hp_end = f(fo.hp);
    m.foe_dead = fo.dead;
    m.atk_hp_end = f(a.hp);
    m.atk_dead = a.dead;
    m
}

// ---------------------------------------------------------------------
// The fully real scenes: nothing injected, TwoPlayer commands only.
// ---------------------------------------------------------------------

fn real_scene(tag: &str, atk_id: u32) -> Engage {
    let mut sim = Sim::new_match(42, MatchMode::TwoPlayer);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::ScoutCavalry)
        .unwrap()
        .id;
    // The foe rides to the east porch and attacks the TC —
    // the live foe's whole behaviour, by commands.
    let porch = (tcx + I16F16::from_num(8), tcy);
    sim.push_command(Command::Move { entity: foe, x: porch.0, y: porch.1 });
    let arrived = run_until(&mut sim, 120_000, |s| {
        let e = ent(s, foe);
        e.order.is_none()
            && steer::vec_len(e.pos_x - porch.0, e.pos_y - porch.1) < I16F16::from_num(4)
    });
    assert!(arrived, "{tag}: the foe never rode to the TC porch");
    sim.push_command(Command::Attack { entity: foe, target: 0 });
    let on_tc = run_until(&mut sim, 20_000, |s| ent(s, 0).hp < I16F16::from_num(2400));
    assert!(on_tc, "{tag}: the foe never engaged the TC");
    // The attacker takes the west side, as in the live scene.
    let west = (tcx - I16F16::from_num(8), tcy);
    sim.push_command(Command::Move { entity: atk_id, x: west.0, y: west.1 });
    let settled = run_until(&mut sim, 20_000, |s| {
        let e = ent(s, atk_id);
        e.order.is_none()
            && steer::vec_len(e.pos_x - west.0, e.pos_y - west.1) < I16F16::from_num(4)
    });
    assert!(settled, "{tag}: the attacker never settled west of the TC");
    assert!(sim.can_attack(atk_id, foe), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk_id, target: foe });
    let m = measure_engagement(&mut sim, atk_id, foe);
    eprintln!("OCCORDER9 {}", engage_report(tag, &m));
    m
}

/// Live test 1's attacker kind, staged with zero injection:
/// the player's own Villager, west of his TC, ordered onto a
/// foe that is draining the TC from inside its footprint.
/// DE expectation: he crosses, closes to melee reach and
/// fights. Measured: foe dead (~1,931 ticks), min dist 2.95,
/// Villager unharmed.
#[test]
fn real_command_villager_engages_foe_attacking_tc() {
    let m = real_scene("real-villager", 1);
    assert!(
        m.foe_dead && m.min_dist <= 3.0 && !m.atk_dead,
        "villager did not engage the TC attacker on the real command path: {}",
        engage_report("real-villager", &m)
    );
}

/// The military twin (the loop's next live vigia removes the
/// test-1 deviation — a Villager attacker — in favour of
/// this): the player's own Scout, same geometry, same result
/// (foe dead ~1,880 ticks, min dist 2.97).
#[test]
fn real_command_scout_engages_foe_attacking_tc() {
    let m = real_scene("real-scout", 4);
    assert!(
        m.foe_dead && m.min_dist <= 3.0 && !m.atk_dead,
        "scout did not engage the TC attacker on the real command path: {}",
        engage_report("real-scout", &m)
    );
}

// ---------------------------------------------------------------------
// The live kind pairing: an enemy Spearman on the TC.
// ---------------------------------------------------------------------

/// Stage a Spearman at the TC's east porch (tc + 8) ordered
/// to Attack the TC, and an attacker Moved to the west side
/// (tc − 8) and ordered to Attack it. `atk_id` is a starting
/// unit (Villager 1 / Scout 4) or `None` for a spawned
/// Militia placed on the west point.
fn spearman_scene(tag: &str, atk_id: Option<u32>, foe_attacks_tc: bool) -> (Sim, Engage, u32) {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe_pos = (tcx + I16F16::from_num(8), tcy);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    if foe_attacks_tc {
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
    } else {
        let fi = sim.entities.iter_mut().find(|e| e.id == FOE).unwrap();
        fi.stance = Stance::StandGround;
    }
    let atk = match atk_id {
        Some(id) => {
            let west = (tcx - I16F16::from_num(8), tcy);
            sim.push_command(Command::Move { entity: id, x: west.0, y: west.1 });
            let settled = run_until(&mut sim, 20_000, |s| ent(s, id).order.is_none());
            assert!(settled, "{tag}: attacker never settled west");
            id
        }
        None => {
            let id = 9_802;
            spawn(
                &mut sim,
                id,
                EntityKind::Militia,
                OWNER_PLAYER,
                tcx - I16F16::from_num(8),
                tcy,
            );
            id
        }
    };
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
    let m = measure_engagement(&mut sim, atk, FOE);
    eprintln!("OCCORDER9 {}", engage_report(tag, &m));
    (sim, m, atk)
}

/// (a)+(b)+(c) against the live pairing, Villager attacker:
/// he engages and kills the Spearman that stays on the TC.
#[test]
fn villager_engages_spearman_attacking_tc() {
    let (_, m, _) = spearman_scene("vil-vs-spearman-on-tc", Some(1), true);
    assert!(
        m.foe_dead && m.min_dist <= 3.0 && !m.atk_dead,
        "villager did not engage the spearman on the TC: {}",
        engage_report("vil-vs-spearman-on-tc", &m)
    );
}

/// The Scout and the Militia against the same Spearman:
/// both engage and kill it.
#[test]
fn scout_and_militia_engage_spearman_attacking_tc() {
    let (_, m, _) = spearman_scene("scout-vs-spearman-on-tc", Some(4), true);
    assert!(
        m.foe_dead && m.min_dist <= 3.0,
        "scout did not engage: {}",
        engage_report("scout-vs-spearman-on-tc", &m)
    );
    let (_, m, _) = spearman_scene("militia-vs-spearman-on-tc", None, true);
    assert!(
        m.foe_dead && m.min_dist <= 3.0,
        "militia did not engage: {}",
        engage_report("militia-vs-spearman-on-tc", &m)
    );
}

/// (c) in its strongest form: the Spearman is idle (Stand
/// Ground) and RETALIATES. The Villager still closes to melee
/// reach and deals damage (measured 45 → 9) before dying —
/// a villager ordered onto a military unit fights it; he is
/// never refused and never stands idle. (That he loses the
/// duel is DE combat arithmetic, not an engagement defect.)
#[test]
fn villager_engages_retaliating_spearman() {
    let (_, m, _) = spearman_scene("vil-vs-retaliating-spearman", Some(1), false);
    assert!(
        m.min_dist <= 3.0 && m.foe_hp_end < 45.0,
        "villager never engaged the retaliating spearman: {}",
        engage_report("vil-vs-retaliating-spearman", &m)
    );
}

// ---------------------------------------------------------------------
// (d) the crowd, and (a)'s literal blocking pocket.
// ---------------------------------------------------------------------

/// The player's other units stand on the TC centre/east
/// ground while the Villager attacks from the west: the
/// crowd does not strand him (foe dead ~1,827 ticks).
#[test]
fn crowd_at_tc_does_not_strand_the_attacker() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe_pos = (tcx + I16F16::from_num(8), tcy);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for (id, dx, dy) in [(2u32, 0i32, 0i32), (3, 3, 2), (4, 5, -3)] {
        sim.push_command(Command::Move {
            entity: id,
            x: tcx + I16F16::from_num(dx),
            y: tcy + I16F16::from_num(dy),
        });
    }
    let settled = run_until(&mut sim, 20_000, |s| {
        [2u32, 3, 4].iter().all(|&id| ent(s, id).order.is_none())
    });
    assert!(settled, "crowd staging: the crowd never settled");
    let west = (tcx - I16F16::from_num(8), tcy);
    sim.push_command(Command::Move { entity: 1, x: west.0, y: west.1 });
    let w = run_until(&mut sim, 20_000, |s| ent(s, 1).order.is_none());
    assert!(w, "crowd staging: the attacker never settled west");
    sim.push_command(Command::Attack { entity: 1, target: FOE });
    let m = measure_engagement(&mut sim, 1, FOE);
    eprintln!("OCCORDER9 {}", engage_report("crowd", &m));
    assert!(
        m.foe_dead && m.min_dist <= 3.0,
        "the TC crowd stranded the attacker: {}",
        engage_report("crowd", &m)
    );
}

/// A placed, complete Barracks record (amoveprobe's `placed`
/// shape) with its footprint cells dynamic-blocked.
fn placed_barracks(sim: &Sim, id: u32, owner: u8, cell: (u32, u32)) -> Building {
    let kind = BuildingKind::Barracks;
    let size = kind.footprint().w;
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    Building {
        id,
        kind,
        owner,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: cell.0,
        cell_y: cell.1,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: kind.build_thirds(),
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        repair_gold_remainder: 0,
        build_rate_remainder: 0,
        gate_along_x: true,
        queue_rate_remainder: 0,
        complete: true,
        node_id: None,
        queue: Vec::new(),
        train_progress: 0,
        rally: None,
        rally_node: None,
        weapon_target: None,
        weapon_target_building: None,
        weapon_strike_tick: None,
        weapon_next_tick: 0,
        weapon_engaged: false,
        garrison: Vec::new(),
        garrison_arrows: 0,
        relics: Vec::new(),
        relic_gold_remainder: 0,
    }
}

fn clear_land_spot(sim: &Sim) -> (u32, u32) {
    let g = sim.grid();
    for cy in 40..80 {
        for cx in 40..80 {
            let ok = (0..5).all(|dy| {
                (0..5).all(|dx| {
                    let (x, y) = (cx + dx, cy + dy);
                    !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                })
            });
            if ok {
                return (cx + 2, cy + 2);
            }
        }
    }
    panic!("seed 42: no clear land spot near the centre");
}

/// (a)'s literal form: a foe standing inside a BLOCKING
/// building footprint cannot stay there — the grid-integrity
/// pass displaces it to open ground within the first ticks —
/// and an explicit Attack from a Militia then engages it.
/// The "target inside the footprint forever" state does not
/// exist in this sim.
#[test]
fn foe_inside_blocking_footprint_is_displaced_and_engaged() {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let anchor = (cx - 1, cy - 1);
    let b = placed_barracks(&sim, 9_710, OWNER_PLAYER, anchor);
    let (bx, by) = (b.pos_x, b.pos_y);
    sim.buildings.push(b);
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            sim.grid.set_dynamic(anchor.0 + dx, anchor.1 + dy, true);
        }
    }
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, bx, by);
    spawn(
        &mut sim,
        9_802,
        EntityKind::Militia,
        OWNER_PLAYER,
        bx - I16F16::from_num(28),
        by,
    );
    // Five ticks: the integrity pass must have moved the foe
    // off the blocked cells.
    for _ in 0..5 {
        sim.tick();
    }
    let fo = ent(&sim, FOE);
    let cell = sim.grid().world_to_cell(fo.pos_x, fo.pos_y);
    eprintln!(
        "OCCORDER9 pocket-displace foe_end=({:.1},{:.1}) cell={cell:?} blocked={}",
        f(fo.pos_x),
        f(fo.pos_y),
        sim.grid().is_blocked(cell.0, cell.1)
    );
    assert!(
        !sim.grid().is_blocked(cell.0, cell.1),
        "the foe remained inside the blocking footprint at {cell:?}"
    );
    sim.push_command(Command::Attack { entity: 9_802, target: FOE });
    let m = measure_engagement(&mut sim, 9_802, FOE);
    eprintln!("OCCORDER9 {}", engage_report("pocket", &m));
    assert!(
        m.foe_dead || m.foe_hp_end < 45.0,
        "explicit attack never engaged the displaced pocket foe: {}",
        engage_report("pocket", &m)
    );
}

// ---------------------------------------------------------------------
// (e) the garrison candidate: shape match, attribution rejected.
// ---------------------------------------------------------------------

/// A Villager garrisoned in his own TC (real `GarrisonTc`)
/// is given the live Attack order onto a Spearman draining
/// that TC. Measured on the current tree:
///   - the order is ACCEPTED for the contained unit
///     (`can_attack` true) and the target is held while the
///     foe lives, yet the Villager never leaves the TC's
///     position, never strikes, and keeps 25/25 — the live
///     test-1 shape, exactly;
///   - BUT the garrisoned TC fires (GAR-06): the Spearman
///     dies to its arrows within ~987 ticks while the TC
///     loses only ~5 HP. Live, the foe kept exactly 21/45
///     for ~14,400 ticks while the TC lost 1,515 HP — a
///     firing TC cannot produce that, so the live Villager
///     was NOT garrisoned, and this stuck-state (recorded in
///     the module header) is not this defect's mechanism.
/// This test pins the discrimination measurements, not a
/// behaviour guarantee: if a future change makes a contained
/// attack order refuse, or a garrisoned TC stop firing, the
/// numbers here move and the attribution must be re-read.
#[test]
fn garrisoned_tc_fire_rejects_the_garrison_attribution() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe_pos = (tcx + I16F16::from_num(8), tcy);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    sim.push_command(Command::GarrisonTc { entity: 1, tc: 0 });
    let g = run_until(&mut sim, 20_000, |s| ent(s, 1).is_contained());
    assert!(g, "staging broken: the villager never garrisoned");
    let vil_home = (ent(&sim, 1).pos_x, ent(&sim, 1).pos_y);
    assert!(
        sim.can_attack(1, FOE),
        "measurement changed: a contained attack order is no longer accepted"
    );
    sim.push_command(Command::Attack { entity: 1, target: FOE });
    sim.tick();
    assert_eq!(
        ent(&sim, 1).attack_target,
        Some(FOE),
        "measurement changed: the contained order was not stored"
    );
    // The foe's lifetime under the firing TC (cap: a quarter
    // of the live window — live it survived all of it).
    let mut target_ticks = 0u64;
    let mut vil_moved = false;
    for _ in 0..3_600u64 {
        sim.tick();
        let a = ent(&sim, 1);
        if a.attack_target == Some(FOE) {
            target_ticks += 1;
        }
        if (a.pos_x, a.pos_y) != vil_home {
            vil_moved = true;
        }
        if ent(&sim, FOE).dead {
            break;
        }
    }
    let a = ent(&sim, 1);
    let fo = ent(&sim, FOE);
    let tc_hp = f(ent(&sim, 0).hp);
    eprintln!(
        "OCCORDER9 garrison-discrim target_ticks={target_ticks} vil_moved={vil_moved} vil_hp={:.0} contained={} foe_hp={:.0} foe_dead={} tc_hp={tc_hp:.0}",
        f(a.hp),
        a.is_contained(),
        f(fo.hp),
        fo.dead
    );
    assert!(
        !vil_moved && a.is_contained() && f(a.hp) == 25.0 && target_ticks > 0,
        "the contained attacker no longer shows the stuck shape"
    );
    assert!(
        fo.dead && tc_hp > 2_350.0,
        "discrimination changed: a garrisoned TC no longer kills the porch foe quickly (foe_hp={:.0}, tc_hp={tc_hp:.0}) — re-read the garrison attribution",
        f(fo.hp)
    );
    let _ = OWNER_PLAYER;
}
