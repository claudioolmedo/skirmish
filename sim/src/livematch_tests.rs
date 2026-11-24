//! LIVEMATCH (2026-11-23): the hidden-attacker defect in a
//! FULL MATCH with the enemy AI loop ACTIVE — the territory
//! villagerattack (d5b1733, 25 scenes) and villageredge
//! (ab86d0c, 21 scenes) named as remaining #1 and could not
//! stage: every foe order there was a bare `Command::Attack`
//! issued by the harness in a Sandbox (`Sim::new`), with no
//! AI economy, no wave bookkeeping, no explored/fog state
//! and no other AI orders ticking.
//!
//! Here every scene runs `Sim::new_match(9, AiNormal)`:
//! `ai::decide` ticks on its 30-tick cadence for the whole
//! scene (economy, scouting, production, defense, waves),
//! and the foe's TC attack is NEVER issued by this harness:
//!   * Scene set 1: the foe is the AI's own starting Scout
//!     (entity 28 — the live #488 id), which the AI loop's
//!     scouting step (`plan_scout`) rides to the expected
//!     enemy start; on arrival it AUTO-ACQUIRES the player's
//!     Town Center (CMB-08 step 2, `attack_auto=true`) and
//!     settles at TC-relative (0.94, 0.22) — essentially the
//!     TC CENTRE, under the roof — draining it (2400 → 2399
//!     confirmed before the villager's order). Its stance is
//!     the starting Scout's StandGround. No harness spawn or
//!     order touches the foe.
//!   * Scene set 2: a six-unit wave staged at the AI's home
//!     (spawn = scene selection only) is launched by the AI
//!     loop's own wave step (`plan_wave`, `next_wave_tick`
//!     reset as scene setting); the wave marches as a Move to
//!     the expected start, and its Militia settle stacked on
//!     one point at TC-relative ≈ (−2.4, 1.4), all
//!     `attack_auto=true` on the TC, TC verifiably draining
//!     (1795 HP at the order moment — the match had been
//!     running live for ~68k ticks, scout included).
//!   * Scene set 3: set 1's scout, but with the player's TC
//!     GARRISONED (two starting villagers inside via real
//!     `GarrisonTc` commands) when the order lands.
//!
//! The attacker is the live unit's provenance: the real
//! starting villager entity 2 (set 1/3), or a villager
//! spawned at the order moment (set 2, as in prior suites).
//! Every villager order is a real `Command::Attack`
//! (`can_attack` asserted, mirroring the live ok:true).
//!
//! Per-tick trace per scene: villager + foe order/state
//! EVERY tick around any stall — transitions of
//! (attack_target / has_order / attack_engaged /
//! strike_tick / attack_auto / stance / task) for BOTH
//! units are logged with positions, plus a foe
//! attack_target change counter (AI re-orders), TC HP,
//! TC garrison count, the AI's explored flag for the TC
//! cell, and `ai.next_wave_tick` at the order moment.
//!
//! OUTCOME (measured on base b33f9d7, see handoff
//! `livematch-build.md`): NO scene reproduces the
//! arrive-and-idle live shape.
//!   Set 1 (Scout, centre, auto, StandGround): villager
//!   closes INTO the footprint to dist 2.96–2.99 from
//!   inside-west / south / east, first damage t=94 / 342 /
//!   327, Scout dead t=1534 / 1782 / 1767, villager 25/25,
//!   foe NEVER transitions (attack_target=Some(TC),
//!   attack_auto=true for the whole fight), max stall =
//!   the 38-tick first-strike windup.
//!   Set 2 (wave Militia, south): villager crosses the
//!   south edge THROUGH the stacked wave cluster to dist
//!   2.98, first damage t=370, foe dead t=1930, villager
//!   25/25, foe re-orders = 0 before its death (the single
//!   counted change is the death-tick clear), max_stall_out
//!   = 0, max_stall_in = 39.
//!   Set 3 (garrisoned TC, Scout, south): TC fire wears
//!   the foe 39 → 15 HP during the garrison/move staging;
//!   the villager still closes to dist 2.99 (stand
//!   (0, 3.06)), the first foe-HP drop in the window is
//!   TC arrows at t=117, and the villager's own first
//!   strike lands at t=342 killing the foe (3 → 0,
//!   engaged=true that tick); villager 25/25, stalls 0/0.
//! Per package discipline NO production fix is made; the
//! remaining untested territory is named in the handoff.

use crate::entity::{Entity, EntityKind, Task, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, MatchMode, Sim};
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

const MELEE: f64 = 3.0;

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,
    first_hurt: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    atk_dead_tick: Option<u64>,
    min_dist: f64,
    max_stall_out: u64,
    max_stall_in: u64,
    foe_reorders: u32,
    transitions: u32,
}

impl Trace {
    fn live_shape(&self) -> bool {
        if self.atk_dead_tick.is_some() {
            return false;
        }
        self.first_dmg.is_none()
            && self.first_hurt.is_none()
            && (self.dead_tick.is_none()
                || self.max_stall_out >= 600
                || self.max_stall_in >= 600
                || self.drop_tick.is_some())
    }
}

/// Run the window after the villager's Attack order, logging
/// EVERY state transition of villager and foe (the per-tick
/// watch the task requires around any stall).
fn run_window(tag: &str, sim: &mut Sim, atk: u32, foe: u32, ptc: u32, window: u64) -> Trace {
    let (px, py) = (ent(sim, ptc).pos_x, ent(sim, ptc).pos_y);
    let foe_hp0 = f(ent(sim, foe).hp);
    let atk_hp0 = f(ent(sim, atk).hp);
    let mut tr = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    let mut prev = String::new();
    let mut last_foe_atk = ent(sim, foe).attack_target;
    for t in 0..window {
        sim.tick();
        let a = ent(sim, atk);
        let fo = ent(sim, foe);
        if fo.attack_target != last_foe_atk {
            tr.foe_reorders += 1;
            eprintln!(
                "LIVEMATCH {tag} t={t} FOE TARGET CHANGE {:?} -> {:?} auto={} (#{}) ",
                last_foe_atk, fo.attack_target, fo.attack_auto, tr.foe_reorders
            );
            last_foe_atk = fo.attack_target;
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            tr.first_dmg = Some(t);
        }
        if tr.first_hurt.is_none() && f(a.hp) < atk_hp0 {
            tr.first_hurt = Some(t);
        }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(foe) {
            tr.drop_tick = Some(t);
        }
        if !fo.dead && !a.dead && a.order.is_none() && tr.first_dmg.is_none() {
            if dist > MELEE {
                stall_out += 1;
                tr.max_stall_out = tr.max_stall_out.max(stall_out);
            } else {
                stall_in += 1;
                tr.max_stall_in = tr.max_stall_in.max(stall_in);
            }
        } else {
            stall_out = 0;
            stall_in = 0;
        }
        let sig = format!(
            "vil[atk={:?} ord={} eng={} strike={:?} auto={} task={:?}] foe[atk={:?} ab={:?} ord={} eng={} auto={} stance={:?} task={:?}]",
            a.attack_target, a.order.is_some(), a.attack_engaged, a.strike_tick, a.attack_auto, a.task,
            fo.attack_target, fo.attack_building, fo.order.is_some(), fo.attack_engaged, fo.attack_auto, fo.stance, fo.task
        );
        if sig != prev {
            tr.transitions += 1;
            eprintln!(
                "LIVEMATCH {tag} t={t} TRANS {sig} vil_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} foe_hp={:.0} vil_hp={:.0} tc_hp={:.0}",
                f(a.pos_x - px), f(a.pos_y - py), f(fo.pos_x - px), f(fo.pos_y - py),
                dist, f(fo.hp), f(a.hp), f(ent(sim, ptc).hp)
            );
            prev = sig;
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
        if a.dead {
            tr.atk_dead_tick = Some(t);
            break;
        }
    }
    eprintln!(
        "LIVEMATCH {tag} END first_dmg={:?} first_hurt={:?} drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} foe_reorders={} transitions={} live_shape={}",
        tr.first_dmg, tr.first_hurt, tr.drop_tick, tr.dead_tick, tr.atk_dead_tick,
        tr.min_dist, tr.max_stall_out, tr.max_stall_in, tr.foe_reorders, tr.transitions, tr.live_shape()
    );
    tr
}

/// Run a fresh AiNormal match (no harness orders at all)
/// until the AI's own Scout is settled on the player's TC:
/// attack_target = player TC and the TC verifiably drains
/// over a 120-tick confirmation while the target holds.
fn run_to_ai_scout_settled(sim: &mut Sim) -> Option<u32> {
    let ptc_id = sim.tc_entity_of(OWNER_PLAYER).unwrap().id;
    for _ in 0..20_000u64 {
        sim.tick();
        let foe = sim
            .entities()
            .iter()
            .find(|e| {
                e.owner == OWNER_OPPONENT
                    && e.kind == EntityKind::ScoutCavalry
                    && e.attack_target == Some(ptc_id)
                    && !e.dead
            })
            .map(|e| e.id);
        if let Some(fid) = foe {
            let hp0 = f(ent(sim, ptc_id).hp);
            for _ in 0..120 {
                sim.tick();
            }
            if f(ent(sim, ptc_id).hp) < hp0 && ent(sim, fid).attack_target == Some(ptc_id) {
                return Some(fid);
            }
        }
    }
    None
}

fn order_moment_facts(sim: &Sim, ptc: u32, foe: u32) {
    let tc = ent(sim, ptc);
    let (cx, cy) = sim.grid.world_to_cell(tc.pos_x, tc.pos_y);
    let fo = ent(sim, foe);
    eprintln!(
        "LIVEMATCH order-moment: foe id={} kind={:?} off=({:.2},{:.2}) hp={:.0} atk={:?} auto={} stance={:?} ord={} | tc_hp={:.0} tc_garrison={} ai_explored_tc_cell={} ai_visible_tc_cell={} next_wave_tick={} tick={}",
        foe, fo.kind, f(fo.pos_x - tc.pos_x), f(fo.pos_y - tc.pos_y), f(fo.hp),
        fo.attack_target, fo.attack_auto, fo.stance, fo.order.is_some(),
        f(tc.hp),
        sim.tc_states.iter().find(|s| s.tc_id == ptc).map(|s| s.garrison.len()).unwrap_or(0),
        sim.cell_explored_by(OWNER_OPPONENT, cx, cy),
        sim.cell_visible_to(OWNER_OPPONENT, cx, cy),
        sim.ai.next_wave_tick, sim.tick
    );
}

/// (1) THE NATURAL RAID: the AI loop's own Scout settles at
/// the player TC centre under an auto-acquired TC attack;
/// the real starting villager 2 attacks it — directly from
/// its inside-west start, and after a real Move that parks
/// it outside the south / east edge (idle asserted at the
/// order moment, as live #488 proved its villager idle).
#[test]
fn live_scout_centre_starting_villager_attacks() {
    let mut failures = Vec::new();
    for approach in ["inside-west", "south-outside", "east-outside"] {
        let tag = format!("scout-{approach}");
        let mut sim = Sim::new_match(9, MatchMode::AiNormal);
        let ptc = sim.tc_entity_of(OWNER_PLAYER).unwrap();
        let (ptc_id, px, py) = (ptc.id, ptc.pos_x, ptc.pos_y);
        let Some(foe) = run_to_ai_scout_settled(&mut sim) else {
            failures.push(format!("{tag}: AI scout never settled on the TC"));
            continue;
        };
        let vil = 2u32;
        assert_eq!(ent(&sim, vil).kind, EntityKind::Villager, "{tag}: entity 2 is not the starting villager");
        if approach != "inside-west" {
            let (dx, dy) = if approach == "south-outside" { (0, 20) } else { (20, 0) };
            sim.push_command(Command::Move {
                entity: vil,
                x: px + I16F16::from_num(dx),
                y: py + I16F16::from_num(dy),
            });
            for _ in 0..7_200 {
                sim.tick();
                if ent(&sim, vil).order.is_none() {
                    break;
                }
            }
        }
        let v = ent(&sim, vil);
        assert!(
            v.order.is_none() && v.attack_target.is_none() && v.task == Task::Idle,
            "{tag}: villager not idle at the order moment (ord={} atk={:?} task={:?})",
            v.order.is_some(), v.attack_target, v.task
        );
        order_moment_facts(&sim, ptc_id, foe);
        assert!(sim.can_attack(vil, foe), "{tag}: can_attack refused the live order");
        sim.push_command(Command::Attack { entity: vil, target: foe });
        let tr = run_window(&tag, &mut sim, vil, foe, ptc_id, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "scout-centre failures:\n{}", failures.join("\n"));
}

/// (2) THE WAVE CLUSTER: six AI soldiers staged at the AI
/// home are launched by the AI loop's own wave step and
/// settle stacked on the TC under auto-acquired TC attacks
/// (the whole wave's separation crowd, the AI still ticking
/// orders for every other unit); a villager spawned south
/// of the target Militia at the order moment attacks it.
#[test]
fn live_wave_cluster_militia_villager_from_south() {
    let tag = "wave-south";
    let mut sim = Sim::new_match(9, MatchMode::AiNormal);
    let ptc = sim.tc_entity_of(OWNER_PLAYER).unwrap();
    let (ptc_id, px, py) = (ptc.id, ptc.pos_x, ptc.pos_y);
    let otc = sim.tc_entity_of(OWNER_OPPONENT).unwrap();
    let (ox, oy) = (otc.pos_x, otc.pos_y);
    let wave_ids: Vec<u32> = (0..6).map(|i| 9700 + i).collect();
    for (i, &id) in wave_ids.iter().enumerate() {
        let kind = if i == 5 { EntityKind::ScoutCavalry } else { EntityKind::Militia };
        spawn(&mut sim, id, kind, OWNER_OPPONENT,
              ox + I16F16::from_num((i as i32 - 3) * 3), oy + I16F16::from_num(6));
    }
    sim.entities.sort_by_key(|e| e.id);
    sim.ai.next_wave_tick = 0;
    // The live match runs until a wave Militia holds its TC
    // attack through a 180-tick drain confirmation.
    let mut foe: Option<u32> = None;
    for _ in 0..120_000u64 {
        sim.tick();
        if sim.tick % 30 != 0 {
            continue;
        }
        if let Some(&cand) = wave_ids.iter().find(|&&id| {
            let e = ent(&sim, id);
            !e.dead && e.kind == EntityKind::Militia && e.attack_target == Some(ptc_id)
        }) {
            let hp0 = f(ent(&sim, ptc_id).hp);
            let mut holds = true;
            for _ in 0..180 {
                sim.tick();
                if ent(&sim, cand).attack_target != Some(ptc_id) {
                    holds = false;
                    break;
                }
            }
            if holds && f(ent(&sim, ptc_id).hp) < hp0 {
                foe = Some(cand);
                break;
            }
        }
    }
    let foe = foe.expect("wave Militia never settled on the player TC");
    // The cluster fact: how many wave mates stand inside the
    // footprint with the foe at the order moment.
    let inside = wave_ids.iter().filter(|&&id| {
        let e = ent(&sim, id);
        !e.dead && (e.pos_x - px).abs() < I16F16::from_num(8) && (e.pos_y - py).abs() < I16F16::from_num(8)
    }).count();
    eprintln!("LIVEMATCH {tag} wave mates inside footprint at order moment: {inside}");
    order_moment_facts(&sim, ptc_id, foe);
    let vil = 9800u32;
    let (fx, fy) = (ent(&sim, foe).pos_x, ent(&sim, foe).pos_y);
    spawn(&mut sim, vil, EntityKind::Villager, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
    sim.entities.sort_by_key(|e| e.id);
    let v = ent(&sim, vil);
    assert!(v.order.is_none() && v.attack_target.is_none(), "{tag}: spawned villager not idle");
    assert!(sim.can_attack(vil, foe), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: vil, target: foe });
    let tr = run_window(tag, &mut sim, vil, foe, ptc_id, 7_200);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (3) GARRISONED TC: set 1's natural Scout, but the two
/// other starting villagers are garrisoned inside the
/// player's TC (real `GarrisonTc` commands, contained
/// asserted) while villager 2 attacks from the south —
/// the TC's garrison weapon state is live throughout.
#[test]
fn live_scout_garrisoned_tc_villager_from_south() {
    let tag = "scout-garrisoned-south";
    let mut sim = Sim::new_match(9, MatchMode::AiNormal);
    let ptc = sim.tc_entity_of(OWNER_PLAYER).unwrap();
    let (ptc_id, px, py) = (ptc.id, ptc.pos_x, ptc.pos_y);
    let foe = run_to_ai_scout_settled(&mut sim).expect("AI scout never settled on the TC");
    // Garrison villagers 1 and 3 (every starting villager but the attacker).
    let others: Vec<u32> = sim.entities().iter()
        .filter(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::Villager && e.id != 2)
        .map(|e| e.id).collect();
    assert_eq!(others.len(), 2, "{tag}: expected two other starting villagers");
    for &v in &others {
        sim.push_command(Command::GarrisonTc { entity: v, tc: ptc_id });
    }
    let mut contained = 0usize;
    for _ in 0..7_200 {
        sim.tick();
        contained = others.iter().filter(|&&v| ent(&sim, v).is_contained()).count();
        if contained == others.len() {
            break;
        }
    }
    assert_eq!(contained, 2, "{tag}: villagers never garrisoned ({contained}/2)");
    // Attacker to the south outside, idle, then Attack.
    let vil = 2u32;
    sim.push_command(Command::Move {
        entity: vil,
        x: px,
        y: py + I16F16::from_num(20),
    });
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, vil).order.is_none() {
            break;
        }
    }
    let v = ent(&sim, vil);
    assert!(v.order.is_none() && v.attack_target.is_none(), "{tag}: villager not idle at order moment");
    order_moment_facts(&sim, ptc_id, foe);
    assert!(sim.can_attack(vil, foe), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: vil, target: foe });
    let tr = run_window(tag, &mut sim, vil, foe, ptc_id, 7_200);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}
