//! ENGAGEGROUP (2026-11-21): candidate (2) from the
//! engageexec3 handoff — LIVE-ONLY ORDER CONTEXT. engageexec2
//! excluded the TC edge + blocked cell; engageexec3 excluded
//! the TC centre/interior for a LONE foe + LONE Militia ordered
//! by a lone real `Command::Attack`. The live #460 attacker,
//! though, was a human with a selection: the hypothesis staged
//! here is that the attack order reached the sim as part of a
//! MULTI-UNIT (group / formation-stamped) order, and that the
//! group context — not the geometry — drops the engagement.
//!
//! WHAT A GROUP ATTACK ACTUALLY IS IN THIS CODEBASE (found in
//! the code first, per the package):
//! - The sim has NO group attack command. `Command::Attack`
//!   is per-entity, and `apply_attack` records NO group
//!   membership: MOV-06 grouping (`record_group_member`,
//!   `GroupOrderKind`) covers only `Move` and `AttackMove`
//!   destinations — a targeted Attack is explicitly not a
//!   "move order" in MOV-06's sense, so a multi-unit attack
//!   earns no group id, no speed cap and no formation stamp.
//! - The client (`web-preview/app.js`, `orderAt`) issues a
//!   multi-selection right-click attack as ONE
//!   `issue({t:'attack', e:id, tg})` PER SELECTED UNIT inside
//!   one synchronous click handler — i.e. N individual
//!   `Command::Attack`s landing in ONE tick's command drain,
//!   in selection order. That batch is the group attack's
//!   only representation, and it is what these scenes push:
//!   the three Attack commands are queued back-to-back with
//!   no tick between them.
//! - Formation stamps (`MoveOrder::formation_id`, MOV-07)
//!   exist only on move orders installed by `FormationMove`;
//!   `apply_attack` clears the entity's move order outright
//!   (`e.order = None`), so no formation stamp can survive
//!   onto an attack itself, and the combat chase re-issues
//!   its move orders through `set_move_order`, which builds
//!   them fresh (formation_id 0). The formation scene below
//!   pins exactly that: the group marches in formation
//!   (stamp verified live on the orders), then the attack
//!   batch lands — if the stamp mattered, it could only act
//!   through state the attack leaves behind, and the capture
//!   reads that state per member.
//!
//! Staging (engagepersist idiom): the foe Scout is SPAWNED
//! at the TC centre (scene selection only) and ordered by a
//! real `Command::Attack` on the TC entity (id 0); it stands
//! there draining (asserted). The 3 Militia are spawned
//! outside, set Aggressive by real commands (except the
//! default-stance scene), `can_attack` asserted per member,
//! then ONE batch of 3 real `Command::Attack`s on the foe —
//! approached from E and from S, plus a separated-member
//! variant and a formation-march variant.
//!
//! Capture at the idle moment, PER MEMBER (first tick that
//! member holds no move order, foe alive, no damage yet):
//! attack_target retained?, order target, chase_retry_tick,
//! suppress_separation, dist to foe, foe cell + blocked flag
//! — then whether damage lands within 600 ticks of the idle
//! moment and the foe's death tick. A member that goes idle
//! OUT of melee reach while the foe lives is counted
//! (idle-out-of-range stall ticks): that, a dropped target,
//! or an in-range no-strike would CONFIRM a branch. The
//! healthy outcome is pinned by assertion, so a future tree
//! that breaks group engagement fails loudly here.

use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, FormationKind, Sim, Stance};
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

const FOE: u32 = 9_801;
const ATK: [u32; 3] = [9_811, 9_812, 9_813];
const MELEE_REACH: f64 = 3.0;

#[derive(Debug, Clone)]
struct MemberCapture {
    member: u32,
    tick: u64,
    attack_target: Option<u32>,
    order_target: Option<(f64, f64)>,
    chase_retry_tick: u64,
    suppress: bool,
    dist: f64,
    foe_cell: (u32, u32),
    foe_cell_blocked: bool,
}

#[derive(Debug, Default)]
struct GroupTrace {
    first_dmg: Option<u64>,
    dead_tick: Option<u64>,
    /// Per member: idle-moment capture (as above).
    idle: Vec<Option<MemberCapture>>,
    /// Per member: first tick its attack_target stopped being
    /// the foe while the foe lived.
    drop_tick: Vec<Option<u64>>,
    /// Per member: closest approach to the foe.
    min_dist: Vec<f64>,
    /// Per member: ticks spent holding no move order, out of
    /// melee reach, foe alive, after its idle capture.
    stall_ticks: Vec<u64>,
    /// Per member: damage landed within 600 ticks after its
    /// idle moment.
    dmg600: Vec<Option<bool>>,
}

fn member_capture(sim: &Sim, member: u32) -> MemberCapture {
    let a = ent(sim, member);
    let fo = ent(sim, FOE);
    let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    MemberCapture {
        member,
        tick: sim.tick,
        attack_target: a.attack_target,
        order_target: a.order.as_ref().map(|o| (f(o.target_x), f(o.target_y))),
        chase_retry_tick: a.chase_retry_tick,
        suppress: a.suppress_separation,
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        foe_cell: cell,
        foe_cell_blocked: sim.grid.is_blocked(cell.0, cell.1),
    }
}

fn print_capture(tag: &str, c: &MemberCapture) {
    eprintln!(
        "ENGAGEGROUP CAPTURE {tag} member={} tick={} attack_target={:?} order_target={:?} chase_retry_tick={} suppress={} dist={:.2} foe_cell={:?} foe_cell_blocked={}",
        c.member, c.tick, c.attack_target, c.order_target, c.chase_retry_tick,
        c.suppress, c.dist, c.foe_cell, c.foe_cell_blocked
    );
}

/// Foe Scout at the TC centre draining; 3 Militia spawned at
/// `atk_offs` (TC-relative); ONE batch of real Attack commands
/// (the client's group-attack representation). `aggressive`
/// mirrors engageexec3's staging; the default-stance scene
/// turns it off.
fn group_scene(tag: &str, atk_offs: [(f64, f64); 3], aggressive: bool, window: u64) -> GroupTrace {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..300 {
        sim.tick();
    }
    assert!(
        f(ent(&sim, 0).hp) < 2400.0,
        "{tag}: foe at TC centre is not draining the TC (hp {:.0})",
        f(ent(&sim, 0).hp)
    );
    for (i, id) in ATK.iter().enumerate() {
        spawn(
            &mut sim,
            *id,
            EntityKind::Militia,
            OWNER_PLAYER,
            tcx + I16F16::from_num(atk_offs[i].0),
            tcy + I16F16::from_num(atk_offs[i].1),
        );
    }
    if aggressive {
        for id in ATK {
            sim.push_command(Command::SetStance { entity: id, stance: Stance::Aggressive });
        }
        sim.tick();
    }
    for id in ATK {
        assert!(sim.can_attack(id, FOE), "{tag}: can_attack refused member {id}");
    }
    // THE GROUP ORDER: one batch, no tick between the three
    // Attack commands — the client's one-click representation.
    for id in ATK {
        sim.push_command(Command::Attack { entity: id, target: FOE });
    }
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let mut tr = GroupTrace {
        idle: vec![None, None, None],
        drop_tick: vec![None, None, None],
        min_dist: vec![f64::MAX; 3],
        stall_ticks: vec![0; 3],
        dmg600: vec![None; 3],
        ..Default::default()
    };
    for t in 0..window {
        sim.tick();
        let fo = ent(&sim, FOE);
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            tr.first_dmg = Some(t);
        }
        for (i, id) in ATK.iter().enumerate() {
            let a = ent(&sim, *id);
            let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
            if dist < tr.min_dist[i] {
                tr.min_dist[i] = dist;
            }
            if tr.drop_tick[i].is_none() && !fo.dead && a.attack_target != Some(FOE) {
                tr.drop_tick[i] = Some(t);
            }
            if tr.idle[i].is_none()
                && t >= 30
                && a.order.is_none()
                && !fo.dead
                && tr.first_dmg.is_none()
            {
                let c = member_capture(&sim, *id);
                print_capture(tag, &c);
                tr.idle[i] = Some(c);
            } else if tr.idle[i].is_some()
                && a.order.is_none()
                && !fo.dead
                && dist > MELEE_REACH
            {
                tr.stall_ticks[i] += 1;
            }
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
    }
    for i in 0..3 {
        tr.dmg600[i] = Some(match (&tr.idle[i], tr.first_dmg) {
            (Some(idle), Some(fd)) => fd <= idle.tick + 600,
            _ => tr.first_dmg.is_some(),
        });
    }
    eprintln!(
        "ENGAGEGROUP {tag} first_dmg={:?} dead={:?} min_dist={:?} drop={:?} stall={:?} dmg600={:?}",
        tr.first_dmg, tr.dead_tick, tr.min_dist, tr.drop_tick, tr.stall_ticks, tr.dmg600
    );
    tr
}

/// The healthy group outcome, asserted as the pin: the foe
/// dies, no member drops the target while it lives, no member
/// stalls idle out of reach, and every member's idle capture
/// (when one occurs before first damage) shows the target
/// retained with no chase hold.
fn assert_group_engages(tag: &str, tr: &GroupTrace) {
    assert!(
        tr.first_dmg.is_some() && tr.dead_tick.is_some(),
        "{tag}: group did not kill the foe: {tr:?}"
    );
    for i in 0..3 {
        assert!(
            tr.drop_tick[i].is_none(),
            "{tag}: member {} dropped the target at {:?}",
            ATK[i], tr.drop_tick[i]
        );
        assert!(
            tr.stall_ticks[i] <= 600,
            "{tag}: member {} stalled idle out of reach {} ticks",
            ATK[i], tr.stall_ticks[i]
        );
        if let Some(c) = &tr.idle[i] {
            assert_eq!(
                c.attack_target,
                Some(FOE),
                "{tag}: member {} idle capture lost the target: {c:?}",
                ATK[i]
            );
        }
    }
}

/// Group of 3 from the EAST (run #460's approach side) and
/// from the SOUTH, one Attack batch each.
#[test]
fn group_of_three_attacks_centre_scout_east_and_south() {
    let tr = group_scene(
        "group-E",
        [(24.0, -3.0), (24.0, 0.0), (24.0, 3.0)],
        true,
        7_200,
    );
    assert_group_engages("group-E", &tr);
    let tr = group_scene(
        "group-S",
        [(-3.0, 24.0), (0.0, 24.0), (3.0, 24.0)],
        true,
        7_200,
    );
    assert_group_engages("group-S", &tr);
}

/// The group order issued while one member is far/separated:
/// two Militia at the east approach, the third far south —
/// all three still receive the SAME one-batch Attack.
#[test]
fn group_attack_with_separated_member() {
    let tr = group_scene(
        "group-separated",
        [(24.0, -2.0), (24.0, 2.0), (0.0, 56.0)],
        true,
        7_200,
    );
    assert_group_engages("group-separated", &tr);
}

/// The live player's plain click: no stance commands at all,
/// just the one-batch group Attack, default stance.
#[test]
fn group_attack_default_stance() {
    let tr = group_scene(
        "group-default-stance",
        [(24.0, -3.0), (24.0, 0.0), (24.0, 3.0)],
        false,
        7_200,
    );
    assert_group_engages("group-default-stance", &tr);
}

/// Formation-stamped context: the group first marches under
/// a real `FormationMove` (Line) — its orders carry a nonzero
/// formation stamp, verified live — and only once formed up
/// west of the TC does the one-batch Attack land. Pins that
/// `apply_attack` leaves no formation residue the chase could
/// trip on (the attack clears the stamped order; the capture
/// reads the post-attack state per member).
#[test]
fn group_attacks_after_formation_march() {
    let tag = "group-formation";
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..300 {
        sim.tick();
    }
    assert!(f(ent(&sim, 0).hp) < 2400.0, "{tag}: foe not draining");
    for (i, id) in ATK.iter().enumerate() {
        let off = [(-30.0, -3.0), (-30.0, 0.0), (-30.0, 3.0)][i];
        spawn(
            &mut sim,
            *id,
            EntityKind::Militia,
            OWNER_PLAYER,
            tcx + I16F16::from_num(off.0),
            tcy + I16F16::from_num(off.1),
        );
        sim.push_command(Command::SetStance { entity: *id, stance: Stance::Aggressive });
    }
    sim.tick();
    // One formation order batch: same destination, Line.
    let (dx, dy) = (tcx + I16F16::from_num(-16), tcy);
    for id in ATK {
        sim.push_command(Command::FormationMove {
            entity: id,
            x: dx,
            y: dy,
            formation: FormationKind::Line,
        });
    }
    sim.tick();
    let stamped = ATK
        .iter()
        .filter(|id| ent(&sim, **id).order.as_ref().is_some_and(|o| o.formation_id != 0))
        .count();
    eprintln!("ENGAGEGROUP {tag} formation-stamped members after march order: {stamped}/3");
    assert!(stamped >= 2, "{tag}: formation march did not stamp the group ({stamped}/3)");
    // March until the group has formed up (orders done) or the
    // window ends.
    for _ in 0..1_800 {
        sim.tick();
        if ATK.iter().all(|id| ent(&sim, *id).order.is_none()) {
            break;
        }
    }
    for id in ATK {
        assert!(sim.can_attack(id, FOE), "{tag}: can_attack refused member {id}");
    }
    for id in ATK {
        sim.push_command(Command::Attack { entity: id, target: FOE });
    }
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let mut first_dmg = None;
    let mut dead = None;
    let mut idle: Vec<Option<MemberCapture>> = vec![None, None, None];
    for t in 0..7_200u64 {
        sim.tick();
        let fo = ent(&sim, FOE);
        if first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            first_dmg = Some(t);
        }
        for (i, id) in ATK.iter().enumerate() {
            let a = ent(&sim, *id);
            if idle[i].is_none() && t >= 30 && a.order.is_none() && !fo.dead && first_dmg.is_none() {
                let c = member_capture(&sim, *id);
                print_capture(tag, &c);
                idle[i] = Some(c);
            }
        }
        if fo.dead {
            dead = Some(t);
            break;
        }
    }
    eprintln!("ENGAGEGROUP {tag} first_dmg={first_dmg:?} dead={dead:?}");
    assert!(
        first_dmg.is_some() && dead.is_some(),
        "{tag}: formation group did not kill the foe: first={first_dmg:?} dead={dead:?}"
    );
    for (i, c) in idle.iter().enumerate() {
        if let Some(c) = c {
            assert_eq!(c.attack_target, Some(FOE), "{tag}: member {} lost target: {c:?}", ATK[i]);
        }
    }
}
