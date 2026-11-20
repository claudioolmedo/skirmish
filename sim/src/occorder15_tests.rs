//! OCCORDER15 (2026-11-20): engagement at the Town Center
//! centre, round 15 — bot-loop run #442, on build dfa0864
//! (FIX #12 live). The pick layer is proven on that build:
//! a LEFT-click on the red outline of an enemy Militia
//! (HP 13/40) hidden under the player's TC roof at the
//! CENTRE of the footprint SELECTS the enemy, and the
//! right-click resolves it too. The failure is downstream:
//! the player's Scout (Aggressive) ordered to attack, both
//! directly on the central outline and LOW at the foe's
//! feet, MOVES INTO the TC footprint and STANDS there —
//! HP unchanged on both sides, no attack, no "Garrisoned."
//! toast, idle-military 1 -> 0 (the unit holds an order but
//! never engages) for ~30 s observed.
//!
//! DISCRIMINATION (measured in this harness, real Commands
//! only; foe Militia staged at the TC centre / in the
//! TC-attacker disc, draining the TC by a real Attack
//! order, HP set to the live 13 as scene selection):
//!   - With NO bodies around the TC, every geometry
//!     engages: foe at the exact centre (0,0) and at the
//!     settled disc spot (0.7,-2.9); attacker Scout from
//!     W / E / N / S / SE / inside the footprint, and the
//!     player's own starting Scout as the attacker. A*
//!     paths to the foe's exact position from every start
//!     (the TC footprint cells are not grid-blocked), and
//!     MELEE_REACH is measured to the foe's centre — the
//!     task's candidate faults (order resolution, an
//!     unreachable approach point inside the footprint,
//!     combat range vs the footprint edge) are DISPROVEN
//!     for the sparse scene, exactly as engageprobe found
//!     for the Scout foe.
//!   - The Town Center's own standing bodies change it.
//!     With only the game's starting units in place
//!     (villagers at (5,0) / (-5,0) / (0,5), the starting
//!     Scout at (5,5)) a Scout attacking from the south
//!     takes 960 ticks to land its first blow: it parks in
//!     the pocket between the (0,5) villager and the (5,5)
//!     Scout at dist 6.24 from the foe, vel ~0, for most of
//!     that time.
//!   - With the dropoff ring the live TC carries — eight
//!     villagers standing at r = 5 around the centre — the
//!     south approach NEVER engages (3,600 ticks, zero
//!     damage, attack target and move order held the whole
//!     window): the attacker cycles between pockets at
//!     dist 6.07 and 7.17 from the foe, its waypoint
//!     alternating between the bump pass's two detour cells
//!     (4,4) and (4,8), in contact toward the foe on 1,266
//!     of 3,600 ticks. The west approach to the same ring
//!     engages (the south lane is the double-blocked one:
//!     the ring's (0,5) body coincides with the starting
//!     villager's stand and the (5,5) Scout closes the SE
//!     corridor). A test-side held press (suppress flag set
//!     after every tick) kills the foe at tick 684 —
//!     contact is the proximate blocker, as in engagefix.
//!
//! MECHANISM: engagefix's MOV-04 press arms only inside
//! MELEE_REACH + SEPARATION_RADIUS (5.0) of the victim —
//! calibrated to a body ring AT the victim (r <= 3), where
//! the chaser contact-stops one body-depth outside the
//! ring, at ~4.1-5.0. A ring standing further out stops
//! the chaser one body-depth outside ITSELF: at r = 5 the
//! stop lands at 6.05-7.90, outside the band, so the press
//! never arms and the MOV-03 bump pass orbits the chaser
//! between detour pockets forever. The band's premise —
//! the chaser's remaining obstacle is bodies, not terrain
//! — is true at every ring radius, and it is directly
//! testable: the chaser is in contact with a body toward
//! its victim, or it is not.
//!
//! THE FIX (combat.rs, attack_step's chase arm only): the
//! press now also arms when the melee chaser of a
//! STATIONARY victim is in contact with a mobile body
//! lying toward the victim (melee_chaser_contact_blocked:
//! steering's own pair-separation test, with the
//! contact_project skips — sheep never block a mover,
//! same-formation pairs never collide). The stationary-
//! victim gate is unchanged, so the measured pursuit
//! behaviour against a MOVING victim (repairprobe's
//! fleeing repairer, engagefix's guard) is untouched, and
//! the distance band is kept as the second arm. No rule,
//! balance or design change: a melee attacker ordered
//! onto a stationary foe is no longer body-blocked out of
//! its own attack, at any ring radius — the MOV-04
//! principle, completed.
//!
//! Post-fix measurements (same scenes): ring r = 5 south
//! first damage at tick 204, foe dead; starters-south
//! 960 -> 204; the rotated ring 1,078 -> 204; every scene
//! in this suite lands its first blow within ~250 ticks.
//! Pre-fix the suite is 3/6 (the ring5, starters and
//! rotated-ring pins fail; the r = 6, disc and control
//! scenes engage slowly and pass, as regression variants).

use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, Sim, Stance};
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

const FOE: u32 = 9_901;
const ATK: u32 = 9_902;

/// Stage: foe Militia at `foe_off` (HP 13, the live value)
/// draining the TC by a real Attack order, settled 120
/// ticks; extra idle villagers at `bodies` (absolute TC
/// offsets); attacker Scout spawned at `atk_off`, set
/// Aggressive by command, ordered Attack by command.
/// Returns (first damage tick, foe dead, end report).
fn scene(
    tag: &str,
    foe_off: (f64, f64),
    bodies: &[(f64, f64)],
    atk_off: (i32, i32),
    window: u64,
) -> (Option<u64>, bool, String) {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::Militia,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(foe_off.0),
        tcy + I16F16::from_num(foe_off.1),
    );
    {
        let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
        sim.entities[fi].hp = I16F16::from_num(13);
    }
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for (i, (dx, dy)) in bodies.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    for _ in 0..120 {
        sim.tick();
    }
    assert!(
        ent(&sim, 0).hp < I16F16::from_num(2400) || foe_off == (0.0, 0.0),
        "{tag}: staging — foe is not draining the TC"
    );
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx + I16F16::from_num(atk_off.0),
        tcy + I16F16::from_num(atk_off.1),
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let mut first_damage: Option<u64> = None;
    let mut min_dist = f64::MAX;
    for t in 0..window {
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
        let d = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        if d < min_dist {
            min_dist = d;
        }
        if first_damage.is_none() && f(fo.hp) < foe_hp0 {
            first_damage = Some(t);
        }
        if fo.dead {
            break;
        }
    }
    let a = ent(&sim, ATK);
    let fo = ent(&sim, FOE);
    let report = format!(
        "{tag} first_dmg={first_damage:?} min_dist={min_dist:.2} foe_hp {foe_hp0:.0}->{:.0} dead={} atk_end=({:.2},{:.2}) atk_target={:?} order={}",
        f(fo.hp),
        fo.dead,
        f(a.pos_x - tcx),
        f(a.pos_y - tcy),
        a.attack_target,
        a.order.is_some()
    );
    eprintln!("OCC15 {report}");
    (first_damage, fo.dead, report)
}

fn ring(r: f64, rot_deg: f64, n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let a = (i as f64 * 360.0 / n as f64 + rot_deg).to_radians();
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

/// Pin A — the run #442 freeze in its indefinite form:
/// centre foe, the TC dropoff ring at r = 5, attacker from
/// the south. On the pre-fix build this scene deals ZERO
/// damage in the full window (min_dist 6.05, order held,
/// pocket orbit inside the footprint — the live signature).
#[test]
fn ring5_south_attacker_engages_centre_foe() {
    let bodies = ring(5.0, 0.0, 8);
    let (dmg, _dead, report) = scene("ring5-south", (0.0, 0.0), &bodies, (0, 16), 3_600);
    assert!(
        dmg.is_some(),
        "the r=5 dropoff ring stranded the Scout for the full window: {report}"
    );
}

/// Pin A variants: the same ring rotated (no body dead-on
/// south) and a wider ring at r = 6 — the press must arm
/// by contact at any ring radius, not by a wider band.
/// Pre-fix the rotated ring engages only at tick 1,078
/// (the pocket orbit leaks through a wider corridor
/// eventually) and the r = 6 ring at 303; post-fix both
/// land the first blow at 204. The rotated scene is
/// therefore pinned on promptness, the r = 6 scene on
/// engagement.
#[test]
fn ring_rotated_and_ring6_south_attacker_engages_centre_foe() {
    let (dmg, _dead, report) = scene("ring5-rot22-south", (0.0, 0.0), &ring(5.0, 22.5, 8), (0, 16), 3_600);
    assert!(
        matches!(dmg, Some(t) if t <= 600),
        "the rotated r=5 ring delayed the engagement past 600 ticks: {report}"
    );
    let (dmg, _dead, report) = scene("ring6-south", (0.0, 0.0), &ring(6.0, 0.0, 8), (0, 16), 3_600);
    assert!(
        dmg.is_some(),
        "the r=6 ring stranded the Scout: {report}"
    );
}

/// Pin B — the delay form with NO staged bodies at all:
/// only the game's own starting units stand around the TC
/// (villagers (5,0)/(-5,0)/(0,5), Scout (5,5)). Pre-fix
/// the south attacker parks in their pocket at dist 6.24
/// and lands its first blow at tick 960; the live watch
/// (~30 s, no damage) sits inside that shape. Post-fix the
/// first blow must land promptly.
#[test]
fn starters_south_first_damage_is_prompt() {
    let (dmg, _dead, report) = scene("starters-south", (0.0, 0.0), &[], (0, 16), 3_600);
    assert!(
        matches!(dmg, Some(t) if t <= 600),
        "the starting bodies alone delayed the engagement past 600 ticks: {report}"
    );
}

/// The foe at its real settled disc spot (0.7,-2.9) — the
/// position a TC-attacker verifiably stands in — with the
/// dropoff ring around the TC CENTRE (where the bodies
/// actually stand), attacker from the south-east.
#[test]
fn disc_foe_with_tc_centre_ring_engages() {
    let bodies = ring(5.0, 0.0, 8);
    let (dmg, _dead, report) = scene("disc-ring5-se", (0.7, -2.9), &bodies, (12, 12), 3_600);
    assert!(
        dmg.is_some(),
        "the settled disc foe behind the dropoff ring was never engaged: {report}"
    );
}

/// Control: the sparse centre scene (no bodies) engages —
/// it engaged pre-fix too; the fix must not disturb it.
#[test]
fn centre_foe_no_bodies_control_engages() {
    let (dmg, dead, report) = scene("centre-control", (0.0, 0.0), &[], (0, 16), 3_600);
    assert!(
        dmg.is_some() && dead,
        "CONTROL FAILED — sparse centre scene did not engage: {report}"
    );
}

/// Diagnostic (always passes): the pre-fix freeze's time
/// series, re-measured on the current build — contact and
/// band occupancy over the window, with the attacker's
/// pocket trace. On the pre-fix build this prints contact
/// toward the foe on ~1,266/3,600 ticks with the attacker
/// orbiting at dist 6.07-7.17 and zero damage; post-fix it
/// prints the attacker standing in range from ~tick 300.
#[test]
fn probe_ring5_time_series() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, tcx, tcy);
    {
        let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
        sim.entities[fi].hp = I16F16::from_num(13);
    }
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for (i, (dx, dy)) in ring(5.0, 0.0, 8).iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    for _ in 0..120 {
        sim.tick();
    }
    spawn(&mut sim, ATK, EntityKind::ScoutCavalry, OWNER_PLAYER, tcx, tcy + I16F16::from_num(16));
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let mut contact_toward_ticks = 0u64;
    for t in 0..3_600u64 {
        sim.tick();
        let a = ent(&sim, ATK);
        let (ax, ay) = (a.pos_x, a.pos_y);
        for e in sim.entities() {
            if e.id == ATK || e.id == FOE || e.dead || !e.kind.is_mobile() {
                continue;
            }
            let d = f(steer::vec_len(e.pos_x - ax, e.pos_y - ay));
            let rsum = f(crate::units::spec(EntityKind::ScoutCavalry).radius
                + crate::units::spec(e.kind).radius);
            if d < rsum
                && f((e.pos_x - ax) * (tcx - ax) + (e.pos_y - ay) * (tcy - ay)) > 0.0
            {
                contact_toward_ticks += 1;
                break;
            }
        }
        if t % 600 == 0 {
            eprintln!(
                "OCC15 ts t={t} atk=({:.2},{:.2}) dist_foe={:.2} foe_hp={:.0} suppress={}",
                f(ax - tcx),
                f(ay - tcy),
                f(steer::vec_len(tcx - ax, tcy - ay)),
                f(ent(&sim, FOE).hp),
                a.suppress_separation
            );
        }
        if ent(&sim, FOE).dead {
            eprintln!("OCC15 ts foe dead at t={t}");
            break;
        }
    }
    eprintln!("OCC15 ts contact_toward_ticks={contact_toward_ticks}");
}
