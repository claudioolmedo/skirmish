//! BETWEENTARGETS (2026-11-23): the remaining untested
//! territory of the hidden-attacker defect, named by
//! livematch (items 2 and 5) and villageredge (items 2–3):
//! the foe BETWEEN targets at the villager's order moment,
//! and villager order-path states beyond plain idle.
//!
//! Live shape to reproduce (runs #487/#488): an idle
//! villager right-click-ordered (`Command::Attack`,
//! occlog-verified, no later cancel) onto an enemy
//! under/inside the player's TC roof that is actively
//! draining the TC; the villager walks only to the TC edge
//! and stands idle there, foe at full HP.
//!
//! Scenes:
//!   (1) FOE BETWEEN TARGETS, full match
//!       (`Sim::new_match(9, AiNormal)`, `ai::decide`
//!       ticking on its 30-tick cadence throughout):
//!       the foe is the AI loop's own Scout (entity 28,
//!       auto-acquired TC attack, settled at TC-relative
//!       ≈(0.94, 0.22) under the roof, as in livematch).
//!       (a) TARGET JUST DIED: the settled Scout is ordered
//!           (real `Command::Attack`) onto a 3-HP sacrificial
//!           player villager standing inside the footprint
//!           beside it; the tick the sacrifice dies, the foe
//!           is idle inside the footprint with its next AI
//!           decision ≤30 ticks away — the starting villager
//!           2, parked idle south-outside, attacks it that
//!           same moment.
//!       (b) JUST BECAME IDLE: the settled Scout is stopped
//!           (real `Command::Stop`) inside the footprint and
//!           villager 2 — parked idle east-outside — attacks
//!           it in the SAME command batch, so its order lands
//!           at the application moment the foe is idle.
//!           (Measured: the post-Stop idle never reads
//!           post-tick — CMB-08 re-acquires the TC in the
//!           same tick the Stop applies — so the idle moment
//!           is intra-tick and the batch is the only
//!           deterministic staging of it.)
//!   (2) SHIFT-QUEUED ATTACK (Sandbox, settled Militia on
//!       the TC as in villagerattack/villageredge):
//!       (a) the villager's Attack on the settled foe is
//!           queued (`Command::Queue` / `QueuedOrder::Attack`)
//!           behind a plain Move waypoint;
//!       (b) the Attack on the settled foe is queued behind
//!           an Attack on a 3-HP sacrificial foe standing
//!           outside the footprint — the chain advances to
//!           the settled foe only when the first target dies.
//!   (3) IDLE AFTER A CANCELLED TOWN BELL / GARRISON CYCLE
//!       (Sandbox, settled Militia on the TC):
//!       (a) the villager shelters under a real Town Bell
//!           ring (contained in the TC asserted), the bell
//!           is cancelled (second ring = back-to-work), the
//!           villager returns to its snapshot position and
//!           reads idle — then attacks the settled foe;
//!       (b) the same via a direct `GarrisonTc` →
//!           `UngarrisonTc` cycle.
//!
//! Per-tick tracing (all scenes): every transition of
//! (attack_target / has_order / order_queue len /
//! order_from_queue / attack_engaged / strike_tick /
//! attack_auto / stance / task) for BOTH villager and foe,
//! with positions and HPs, plus a foe attack_target change
//! counter (AI re-orders / retargets), exactly as the
//! livematch suite traces. Spawn positions and the
//! sacrificial units' 3 HP are scene selection only; every
//! order on the path under test is a real `Command`
//! (`can_attack` asserted where the live issue returned
//! ok:true).

use crate::entity::{Entity, EntityKind, QueuedOrder, Task, OWNER_OPPONENT, OWNER_PLAYER};
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
fn set_hp(sim: &mut Sim, id: u32, hp: i32) {
    sim.entities.iter_mut().find(|e| e.id == id).unwrap().hp = I16F16::from_num(hp);
}

const MELEE: f64 = 3.0;
const FOE: u32 = 9_701;
const VIL: u32 = 9_702;
const SAC: u32 = 9_703;

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
    queue_executed_tick: Option<u64>,
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

/// Run the window after the villager's Attack order (or
/// after the queue that carries it is issued), logging
/// EVERY state transition of villager and foe.
fn run_window(tag: &str, sim: &mut Sim, atk: u32, foe: u32, ptc: u32, window: u64) -> Trace {
    run_window_marked(tag, sim, atk, foe, ptc, window, false)
}

/// Window variant: `live_from_start` marks scenes where
/// the villager's Attack on the foe is already the live
/// order at t=0 (plain `Command::Attack`), so a drop is
/// any tick the attack_target is not the foe (queue empty)
/// while the foe lives. Queued scenes pass `false`: the
/// drop guard arms only once the queued attack executes.
fn run_window_marked(
    tag: &str,
    sim: &mut Sim,
    atk: u32,
    foe: u32,
    ptc: u32,
    window: u64,
    live_from_start: bool,
) -> Trace {
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
                "BETWEEN {tag} t={t} FOE TARGET CHANGE {:?} -> {:?} auto={} (#{})",
                last_foe_atk, fo.attack_target, fo.attack_auto, tr.foe_reorders
            );
            last_foe_atk = fo.attack_target;
        }
        if tr.queue_executed_tick.is_none() && a.order_from_queue && a.attack_target == Some(foe) {
            tr.queue_executed_tick = Some(t);
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            tr.first_dmg = Some(t);
        }
        if tr.first_hurt.is_none() && f(a.hp) < atk_hp0 {
            tr.first_hurt = Some(t);
        }
        if tr.drop_tick.is_none()
            && !fo.dead
            && a.attack_target != Some(foe)
            && a.order_queue.is_empty()
            && (live_from_start || tr.queue_executed_tick.is_some())
        {
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
            "vil[atk={:?} ord={} q={} fromq={} eng={} strike={:?} auto={} task={:?}] foe[atk={:?} ab={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?}]",
            a.attack_target, a.order.is_some(), a.order_queue.len(), a.order_from_queue,
            a.attack_engaged, a.strike_tick, a.attack_auto, a.task,
            fo.attack_target, fo.attack_building, fo.order.is_some(), fo.attack_engaged,
            fo.strike_tick, fo.attack_auto, fo.stance, fo.task
        );
        if sig != prev {
            tr.transitions += 1;
            eprintln!(
                "BETWEEN {tag} t={t} TRANS {sig} vil_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} foe_hp={:.0} vil_hp={:.0} tc_hp={:.0}",
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
        "BETWEEN {tag} END first_dmg={:?} first_hurt={:?} drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} foe_reorders={} transitions={} queue_exec={:?} live_shape={}",
        tr.first_dmg, tr.first_hurt, tr.drop_tick, tr.dead_tick, tr.atk_dead_tick,
        tr.min_dist, tr.max_stall_out, tr.max_stall_in, tr.foe_reorders, tr.transitions,
        tr.queue_executed_tick, tr.live_shape()
    );
    tr
}

/// Plain-Attack window: the attack order is live from
/// t=0, so the drop guard is armed from the first tick.
fn run_window_plain(tag: &str, sim: &mut Sim, atk: u32, foe: u32, ptc: u32, window: u64) -> Trace {
    let mut tr = run_window_marked(tag, sim, atk, foe, ptc, window, true);
    if tr.queue_executed_tick.is_none() {
        tr.queue_executed_tick = Some(0);
    }
    tr
}

/// Run a fresh AiNormal match (no harness orders at all)
/// until the AI's own Scout is settled on the player's TC
/// (copied from livematch): attack_target = player TC and
/// the TC verifiably drains over a 120-tick confirmation.
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

/// Park starting villager 2 outside the TC on the named
/// side and wait until it reads idle.
fn park_villager_2(sim: &mut Sim, ptc_id: u32, side: &str) {
    let tc = ent(sim, ptc_id);
    let (px, py) = (tc.pos_x, tc.pos_y);
    let (dx, dy) = if side == "south" { (0, 20) } else { (20, 0) };
    sim.push_command(Command::Move {
        entity: 2,
        x: px + I16F16::from_num(dx),
        y: py + I16F16::from_num(dy),
    });
    for _ in 0..7_200 {
        sim.tick();
        if ent(sim, 2).order.is_none() {
            break;
        }
    }
    let v = ent(sim, 2);
    assert!(
        v.order.is_none() && v.attack_target.is_none() && v.task == Task::Idle,
        "park-{side}: villager 2 not idle after parking (ord={} atk={:?} task={:?})",
        v.order.is_some(), v.attack_target, v.task
    );
}

fn inside_footprint(sim: &Sim, id: u32, ptc: u32) -> bool {
    let tc = ent(sim, ptc);
    let e = ent(sim, id);
    (e.pos_x - tc.pos_x).abs() < I16F16::from_num(8)
        && (e.pos_y - tc.pos_y).abs() < I16F16::from_num(8)
}

/// Ticks until the next `ai::decide` pass (cadence 30 in
/// AiNormal): decide runs at step 6 of a tick whose
/// pre-increment `sim.tick % 30 == 0`.
fn ticks_to_next_decide(sim: &Sim) -> u64 {
    (30 - (sim.tick % 30)) % 30
}

/// (1) FOE BETWEEN TARGETS in a full match: (a) its target
/// just died inside the footprint; (b) it just became idle
/// inside the footprint via Stop. In both, the villager's
/// Attack lands ≤30 ticks before the next AI decision.
#[test]
fn between_targets_full_match_villager_attacks() {
    let mut failures = Vec::new();

    // ---- (a) target just died --------------------------------
    {
        let tag = "between-target-died";
        let mut sim = Sim::new_match(9, MatchMode::AiNormal);
        let ptc_id = sim.tc_entity_of(OWNER_PLAYER).unwrap().id;
        let foe = run_to_ai_scout_settled(&mut sim).expect("AI scout never settled on the TC");
        park_villager_2(&mut sim, ptc_id, "south");
        // Sacrificial player villager beside the foe, inside
        // the footprint, at 3 HP (scene selection).
        let (fx, fy) = (ent(&sim, foe).pos_x, ent(&sim, foe).pos_y);
        spawn(&mut sim, SAC, EntityKind::Villager, OWNER_PLAYER,
              fx + I16F16::from_num(2), fy);
        sim.entities.sort_by_key(|e| e.id);
        set_hp(&mut sim, SAC, 3);
        assert!(inside_footprint(&sim, SAC, ptc_id), "{tag}: sacrifice not inside footprint");
        assert!(sim.can_attack(foe, SAC), "{tag}: can_attack refused foe -> sacrifice");
        sim.push_command(Command::Attack { entity: foe, target: SAC });
        // Tick until the sacrifice is dead AND the foe reads
        // between targets: no attack target, no order, still
        // inside the footprint.
        let mut between: Option<u64> = None;
        for _ in 0..7_200 {
            sim.tick();
            let fo = ent(&sim, foe);
            if ent(&sim, SAC).dead && fo.attack_target.is_none() && fo.order.is_none() {
                between = Some(ticks_to_next_decide(&sim));
                break;
            }
        }
        if between.is_none() {
            failures.push(format!("{tag}: foe never read between-targets after the kill"));
            eprintln!("BETWEEN {tag} STAGING FAILED: no between-targets moment");
        }
        if let Some(gap) = between {
            assert!(inside_footprint(&sim, foe, ptc_id), "{tag}: foe not inside footprint at the between moment");
            eprintln!(
                "BETWEEN {tag} order-moment: foe idle inside footprint, ticks_to_next_ai_decide={gap} tick={}",
                sim.tick
            );
            assert!(sim.can_attack(2, foe), "{tag}: can_attack refused the live order");
            sim.push_command(Command::Attack { entity: 2, target: foe });
            let tr = run_window_plain(tag, &mut sim, 2, foe, ptc_id, 7_200);
            if tr.live_shape() {
                failures.push(format!("{tag}: {tr:?}"));
            }
        }
    }

    // ---- (b) just became idle (Stop) inside the footprint ----
    {
        let tag = "between-just-idle";
        let mut sim = Sim::new_match(9, MatchMode::AiNormal);
        let ptc_id = sim.tc_entity_of(OWNER_PLAYER).unwrap().id;
        let foe = run_to_ai_scout_settled(&mut sim).expect("AI scout never settled on the TC");
        park_villager_2(&mut sim, ptc_id, "east");
        // Measured staging fact (this run): a Stop applied
        // to the settled Scout NEVER reads idle post-tick —
        // CMB-08 auto-acquisition re-acquires the TC in the
        // SAME tick the Stop applies (first post-Stop tick
        // already reads atk=Some(TC), auto=true, and holds
        // for 600/600 ticks). The just-became-idle moment
        // for a foe standing on its acquisition target
        // exists only INTRA-tick, between command
        // application (step 1) and the acquisition phase.
        // The scene therefore issues Stop and the villager's
        // Attack in ONE command batch: at application the
        // foe is idle inside the footprint, and the
        // villager's order lands in that same between-
        // targets window, ≤30 ticks before the next AI
        // decision.
        assert!(inside_footprint(&sim, foe, ptc_id), "{tag}: foe not inside footprint before the batch");
        assert!(ent(&sim, foe).attack_target == Some(ptc_id), "{tag}: foe not on the TC before the batch");
        let gap = ticks_to_next_decide(&sim);
        eprintln!(
            "BETWEEN {tag} order-moment: Stop+Attack in one batch, foe idle at application inside footprint, ticks_to_next_ai_decide={gap} tick={}",
            sim.tick
        );
        assert!(sim.can_attack(2, foe), "{tag}: can_attack refused the live order");
        sim.push_command(Command::Stop { entity: foe });
        sim.push_command(Command::Attack { entity: 2, target: foe });
        let tr = run_window_plain(tag, &mut sim, 2, foe, ptc_id, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    assert!(failures.is_empty(), "between-targets failures:\n{}", failures.join("\n"));
}

/// Sandbox staging shared by scenes (2) and (3): a Militia
/// spawned at the TC centre under a real Attack on the TC
/// entity, verifiably draining before the villager acts.
fn sandbox_settled_foe(sim: &mut Sim) -> u32 {
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
    spawn(sim, FOE, EntityKind::Militia, OWNER_OPPONENT, tx, ty);
    sim.entities.sort_by_key(|e| e.id);
    assert!(sim.can_attack(FOE, tc_id), "can_attack refused foe -> TC");
    sim.push_command(Command::Attack { entity: FOE, target: tc_id });
    let hp0 = f(ent(sim, tc_id).hp);
    let mut draining = false;
    for _ in 0..7_200 {
        sim.tick();
        if ent(sim, FOE).attack_target == Some(tc_id) && f(ent(sim, tc_id).hp) < hp0 {
            draining = true;
            break;
        }
    }
    assert!(draining, "sandbox foe never settled draining the TC");
    tc_id
}

/// (2) SHIFT-QUEUED ATTACK: the attack on the settled foe
/// reaches the villager only through its order queue.
#[test]
fn shift_queued_attack_on_settled_foe() {
    let mut failures = Vec::new();

    // (a) Attack queued behind a Move waypoint.
    {
        let tag = "queue-behind-move";
        let mut sim = Sim::new(42);
        let tc_id = sandbox_settled_foe(&mut sim);
        let (fx, fy) = (ent(&sim, FOE).pos_x, ent(&sim, FOE).pos_y);
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
        sim.entities.sort_by_key(|e| e.id);
        // Waypoint: 8 wu north of the start (toward the foe,
        // still outside the footprint), then the queued
        // attack on the settled foe.
        sim.push_command(Command::Move {
            entity: VIL,
            x: fx,
            y: fy + I16F16::from_num(12),
        });
        sim.push_command(Command::Queue {
            entity: VIL,
            order: QueuedOrder::Attack { target: FOE },
        });
        let tr = run_window(tag, &mut sim, VIL, FOE, tc_id, 7_200);
        if tr.queue_executed_tick.is_none() {
            failures.push(format!("{tag}: queued attack never executed: {tr:?}"));
        }
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    // (b) Attack queued behind an Attack on a sacrificial
    // foe outside the footprint.
    {
        let tag = "queue-behind-attack";
        let mut sim = Sim::new(42);
        let tc_id = sandbox_settled_foe(&mut sim);
        let (fx, fy) = (ent(&sim, FOE).pos_x, ent(&sim, FOE).pos_y);
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
        spawn(&mut sim, SAC, EntityKind::Militia, OWNER_OPPONENT,
              fx + I16F16::from_num(8), fy + I16F16::from_num(20));
        sim.entities.sort_by_key(|e| e.id);
        set_hp(&mut sim, SAC, 3);
        assert!(sim.can_attack(VIL, SAC), "{tag}: can_attack refused villager -> sacrifice");
        assert!(sim.can_attack(VIL, FOE), "{tag}: can_attack refused villager -> foe");
        sim.push_command(Command::Attack { entity: VIL, target: SAC });
        sim.push_command(Command::Queue {
            entity: VIL,
            order: QueuedOrder::Attack { target: FOE },
        });
        // Window over the SACRIFICE first is implicit: the
        // shared window watches the settled foe; the chain
        // execution tick is recorded when the queued attack
        // goes live. The window must outlast the sacrifice
        // fight plus the foe fight.
        let tr = run_window(tag, &mut sim, VIL, FOE, tc_id, 14_400);
        if tr.queue_executed_tick.is_none() {
            failures.push(format!("{tag}: queued attack never executed: {tr:?}"));
        }
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    assert!(failures.is_empty(), "shift-queued failures:\n{}", failures.join("\n"));
}

/// (3) IDLE AFTER A CANCELLED TOWN BELL / GARRISON CYCLE:
/// the villager's idle at the order moment follows a full
/// shelter cycle, not a fresh spawn.
#[test]
fn bell_cycle_then_attack_settled_foe() {
    let mut failures = Vec::new();

    // (a) Town Bell ring -> sheltered -> cancelled
    // (back-to-work) -> returned to snapshot -> idle.
    {
        let tag = "bell-cycle";
        let mut sim = Sim::new(42);
        let tc_id = sandbox_settled_foe(&mut sim);
        let (fx, fy) = (ent(&sim, FOE).pos_x, ent(&sim, FOE).pos_y);
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
        sim.entities.sort_by_key(|e| e.id);
        let (home_x, home_y) = (ent(&sim, VIL).pos_x, ent(&sim, VIL).pos_y);
        // Ring the bell; the villager must shelter in the TC.
        sim.push_command(Command::TownBell { tc: tc_id });
        sim.tick();
        assert!(sim.tc_bell_active(tc_id), "{tag}: bell did not ring");
        let mut sheltered = false;
        for _ in 0..60_000 {
            sim.tick();
            if ent(&sim, VIL).garrisoned_in.is_some() || ent(&sim, VIL).is_contained() {
                sheltered = true;
                break;
            }
        }
        assert!(sheltered, "{tag}: villager never sheltered under the bell");
        // Cancel the bell (second ring = back-to-work).
        sim.push_command(Command::TownBell { tc: tc_id });
        sim.tick();
        assert!(!sim.tc_bell_active(tc_id), "{tag}: bell still active after the cancel ring");
        // The villager returns to its snapshot position and
        // reads idle.
        let mut idle_home = false;
        for _ in 0..7_200 {
            sim.tick();
            let v = ent(&sim, VIL);
            if !v.is_contained()
                && v.garrisoned_in.is_none()
                && v.order.is_none()
                && v.attack_target.is_none()
                && (v.pos_x - home_x).abs() < I16F16::from_num(2)
                && (v.pos_y - home_y).abs() < I16F16::from_num(2)
            {
                idle_home = true;
                break;
            }
        }
        assert!(idle_home, "{tag}: villager never read idle at its snapshot position after the bell cycle");
        eprintln!(
            "BETWEEN {tag} order-moment: villager idle after cancelled bell, bell_snapshot={:?} tick={}",
            ent(&sim, VIL).bell_snapshot, sim.tick
        );
        assert!(sim.can_attack(VIL, FOE), "{tag}: can_attack refused the live order");
        sim.push_command(Command::Attack { entity: VIL, target: FOE });
        let tr = run_window_plain(tag, &mut sim, VIL, FOE, tc_id, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    // (b) Direct garrison cycle: GarrisonTc -> contained ->
    // UngarrisonTc -> idle beside the TC.
    {
        let tag = "garrison-cycle";
        let mut sim = Sim::new(42);
        let tc_id = sandbox_settled_foe(&mut sim);
        let (fx, fy) = (ent(&sim, FOE).pos_x, ent(&sim, FOE).pos_y);
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
        sim.entities.sort_by_key(|e| e.id);
        sim.push_command(Command::GarrisonTc { entity: VIL, tc: tc_id });
        let mut contained = false;
        for _ in 0..7_200 {
            sim.tick();
            if ent(&sim, VIL).is_contained() {
                contained = true;
                break;
            }
        }
        assert!(contained, "{tag}: villager never garrisoned");
        sim.push_command(Command::UngarrisonTc { tc: tc_id });
        let mut idle = false;
        for _ in 0..7_200 {
            sim.tick();
            let v = ent(&sim, VIL);
            if !v.is_contained() && v.garrisoned_in.is_none() && v.order.is_none() && v.attack_target.is_none() {
                idle = true;
                break;
            }
        }
        assert!(idle, "{tag}: villager never read idle after ungarrison");
        eprintln!("BETWEEN {tag} order-moment: villager idle after garrison cycle, tick={}", sim.tick);
        assert!(sim.can_attack(VIL, FOE), "{tag}: can_attack refused the live order");
        sim.push_command(Command::Attack { entity: VIL, target: FOE });
        let tr = run_window_plain(tag, &mut sim, VIL, FOE, tc_id, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    assert!(failures.is_empty(), "bell/garrison-cycle failures:\n{}", failures.join("\n"));
}
