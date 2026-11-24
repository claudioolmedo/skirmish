//! ENGAGEFINISH (2026-11-23): ONE new live face of the
//! hidden-attacker defect, staged in the sim harness only.
//!
//! Live #490 (2026-11-23, real client, proof #490): a
//! Villager right-click-ordered onto an enemy SPEARMAN
//! (45 HP) settled under the player's TC roof ENGAGED and
//! damaged it 45 -> 13, then STOPPED — at the next reading
//! the Spearman was still 13/45 and ALL villagers were idle;
//! the villager never finished it. The prior suites
//! (villagerattack / villageredge / betweentargets) staged
//! foes that do NOT fight back effectively (a foe on a
//! running TC-entity order never retargets, measured in all
//! three) or that die to the villager. This face is
//! different in kind:
//!   (a) foe = Spearman-class: 45 HP, real melee damage
//!       (3 melee, RoF 180 ticks, delay 30) vs the villager's
//!       25 HP / 3 melee / RoF 120 / delay 38 — a foe that
//!       fights back WINS a straight duel (it needs 9 hits,
//!       ~1,470 ticks; the villager needs 15, ~1,718), so the
//!       fight is long and both units take damage;
//!   (b) the foe is actively attacking the TC when ordered;
//!   (c) during the fight the foe FIGHTS BACK (switches its
//!       target to the villager and strikes it) or
//!       REPOSITIONS (AI re-order / target change mid-fight).
//!
//! The question per scene is narrow and traced per tick for
//! BOTH units (task, attack_target, hp, pos, plus order /
//! strike / engaged / auto / stance transitions, in the
//! betweentargets style): does the villager DISENGAGE at
//! any HP threshold, when the foe switches target to the
//! villager, when the foe strikes it, or after N strikes —
//! i.e. does the #490 finish shape appear: foe damaged but
//! alive, villager's attack_target cleared while both live,
//! villager idle >= 600 ticks afterwards, foe HP frozen?
//! Or does the engagement run to a death (either unit's)?
//!
//! Scenes:
//!   (1) CENTRE: Spearman settled at the TC centre draining
//!       the TC under a real Attack order on the TC entity;
//!       villager ordered once from the south. Plus the
//!       fights-back form: Spearman pinned StandGround at
//!       the centre — a foe holding any TC target can never
//!       retaliate (measured: a pinned foe on the footprint
//!       auto-acquires the TC at point blank the same tick),
//!       so the villager is staged adjacent in the same
//!       batch and CMB-08 step (1) gives the foe the
//!       villager as its point-blank target from tick 1;
//!       it holds the villager and fights a real duel.
//!   (2) POSITIONS: the same pair of forms at the east edge
//!       stand and the SE pillar (spawn offsets (8,0) and
//!       (6,6); a TC-ordered foe walks inward to its TC
//!       stand, recorded at the order moment; a pinned foe
//!       holds its stand).
//!   (3) FULL MATCH (AiNormal, AI deciding on its 30-tick
//!       cadence throughout): a Spearman staged on the
//!       player's TC is fought by starting villager 2 while
//!       the AI loop runs — if the AI re-orders the foe
//!       mid-fight (target / order change), does the
//!       villager's target clear or the villager abandon
//!       when the foe changes target/position?
//!   (4) TWO VILLAGERS ordered sequentially (the second
//!       order lands while the first villager is already
//!       fighting): TC-order centre form and fights-back
//!       centre form — does EITHER villager abandon?
//!
//! Spawn positions and the foe's stance pin are scene
//! selection only; every order on the path under test is a
//! real `Command` (`can_attack` asserted where the live
//! issue returned ok:true).
//!
//! OUTCOME (measured on base 84c3d17, see handoff
//! `engagefinish-build.md`): NO scene reproduces the #490
//! finish shape — drop=None and max_idle_after_drop=0 in
//! all 9 scenes. Every named disengagement condition is
//! measured and excluded: no HP-threshold stop (duel
//! villagers fight from 25 HP to death; the two-villager
//! A fights down to 7/25 and keeps striking), not at the
//! foe's target switch (t=0 in the duels, before the
//! villager's first strike), not at the foe's strikes
//! (9 taken, no drop), not after N villager strikes (12
//! landed in each duel, 15 in each TC-order kill). The
//! TC-order Spearman never retargets (as every prior
//! suite found) and dies at t=1940–2043, villager 25/25.
//! A Spearman that DOES hold the villager wins the duel:
//! villager dead t=1469 after landing 12 strikes (foe
//! left at 9/45) — the foe stops taking damage because
//! the villager is DEAD, not idle, the closest measured
//! relative of the live 45 -> 13 reading. Two villagers
//! ordered sequentially kill the fighting-back foe
//! (t=998) with no abandonment by either. In the full
//! match the AI cadence never re-ordered the staged foe
//! during the fight (its only target change is the
//! death-tick clear). Per package discipline NO
//! production fix is made; these pins are the
//! deliverable, and the remaining untested territory is
//! named in the handoff (a foe fighting back while still
//! draining the TC — impossible in this sim, where
//! retaliation requires an idle foe; Loom/tech-modified
//! villager duel arithmetic; the client order path).

use crate::entity::{Entity, EntityKind, Stance, Task, OWNER_OPPONENT, OWNER_PLAYER};
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

const FOE: u32 = 9_701;
const VIL: u32 = 9_702;
const VIL_B: u32 = 9_704;
const MELEE: f64 = 3.0;

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,       // first tick foe HP drops
    first_hurt: Option<u64>,      // first tick villager HP drops
    foe_switch: Option<u64>,      // first tick foe attack_target == villager
    drop_tick: Option<u64>,       // villager attack_target lost while both live
    dead_tick: Option<u64>,       // foe died
    atk_dead_tick: Option<u64>,   // villager died
    min_dist: f64,
    max_stall_out: u64,           // pre-damage idle-out-of-reach run (prior suites)
    max_stall_in: u64,            // pre-damage idle-in-reach run
    max_idle_after_drop: u64,     // consecutive idle ticks after a drop, both alive
    foe_target_changes: u32,
    vil_strikes: u32,             // foe HP downward steps
    foe_strikes: u32,             // villager HP downward steps
    foe_hp_at_drop: Option<f64>,
    vil_hp_at_drop: Option<f64>,
    dmg_after_drop: bool,
    transitions: u32,
    foe_hp_end: f64,
    vil_hp_end: f64,
    foe_off_end: (f64, f64),
}

impl Trace {
    /// The classic arrive-and-idle shape (prior suites): no
    /// damage either way and the villager stalls / drops.
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
    /// The #490 FINISH shape: the villager damaged the foe,
    /// then disengaged while BOTH lived — target cleared,
    /// villager idle >= 600 ticks afterwards, foe HP frozen
    /// from the drop on. A villager that DIES fighting, or a
    /// foe that dies, is an engagement, not this shape.
    fn finish_shape(&self) -> bool {
        self.first_dmg.is_some()
            && self.dead_tick.is_none()
            && self.atk_dead_tick.is_none()
            && self.drop_tick.is_some()
            && self.max_idle_after_drop >= 600
            && !self.dmg_after_drop
    }
    fn reproduces(&self) -> bool {
        self.live_shape() || self.finish_shape()
    }
}

/// Per-tick traced window for one villager vs the foe.
/// Logs EVERY transition of (attack_target / has_order /
/// attack_engaged / strike_tick / attack_auto / stance /
/// task) for BOTH units, with positions and HPs, in the
/// betweentargets style.
fn run_window(tag: &str, sim: &mut Sim, vil: u32, ptc: u32, window: u64) -> Trace {
    let (px, py) = (ent(sim, ptc).pos_x, ent(sim, ptc).pos_y);
    let foe_hp0 = f(ent(sim, FOE).hp);
    let vil_hp0 = f(ent(sim, vil).hp);
    let mut tr = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    let mut idle_after_drop = 0u64;
    let mut prev = String::new();
    let mut last_foe_atk = ent(sim, FOE).attack_target;
    let mut last_foe_hp = foe_hp0;
    let mut last_vil_hp = vil_hp0;
    for t in 0..window {
        sim.tick();
        let a = ent(sim, vil);
        let fo = ent(sim, FOE);
        if fo.attack_target != last_foe_atk {
            tr.foe_target_changes += 1;
            eprintln!(
                "ENGAGEFINISH {tag} t={t} FOE TARGET CHANGE {:?} -> {:?} auto={} (#{})",
                last_foe_atk, fo.attack_target, fo.attack_auto, tr.foe_target_changes
            );
            last_foe_atk = fo.attack_target;
        }
        if tr.foe_switch.is_none() && fo.attack_target == Some(vil) {
            tr.foe_switch = Some(t);
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if f(fo.hp) < last_foe_hp {
            tr.vil_strikes += 1;
            if tr.first_dmg.is_none() {
                tr.first_dmg = Some(t);
            }
            if tr.drop_tick.is_some() {
                tr.dmg_after_drop = true;
            }
            last_foe_hp = f(fo.hp);
        }
        if f(a.hp) < last_vil_hp {
            tr.foe_strikes += 1;
            if tr.first_hurt.is_none() {
                tr.first_hurt = Some(t);
            }
            last_vil_hp = f(a.hp);
        }
        if tr.drop_tick.is_none() && !fo.dead && !a.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
            tr.foe_hp_at_drop = Some(f(fo.hp));
            tr.vil_hp_at_drop = Some(f(a.hp));
            eprintln!(
                "ENGAGEFINISH {tag} t={t} VILLAGER DROP: attack_target cleared while both live, foe_hp={:.0} vil_hp={:.0} vil_task={:?} foe_target={:?}",
                f(fo.hp), f(a.hp), a.task, fo.attack_target
            );
        }
        if tr.drop_tick.is_some() && !fo.dead && !a.dead && a.order.is_none() && a.attack_target.is_none() {
            idle_after_drop += 1;
            tr.max_idle_after_drop = tr.max_idle_after_drop.max(idle_after_drop);
        } else {
            idle_after_drop = 0;
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
            "vil[atk={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}] foe[atk={:?} ab={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}]",
            a.attack_target, a.order.is_some(), a.attack_engaged, a.strike_tick, a.attack_auto,
            a.stance, a.task, f(a.hp),
            fo.attack_target, fo.attack_building, fo.order.is_some(), fo.attack_engaged,
            fo.strike_tick, fo.attack_auto, fo.stance, fo.task, f(fo.hp)
        );
        if sig != prev {
            tr.transitions += 1;
            eprintln!(
                "ENGAGEFINISH {tag} t={t} TRANS {sig} vil_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} tc_hp={:.0}",
                f(a.pos_x - px), f(a.pos_y - py), f(fo.pos_x - px), f(fo.pos_y - py),
                dist, f(ent(sim, ptc).hp)
            );
            prev = sig;
        }
        tr.foe_hp_end = f(fo.hp);
        tr.vil_hp_end = f(a.hp);
        tr.foe_off_end = (f(fo.pos_x - px), f(fo.pos_y - py));
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
        "ENGAGEFINISH {tag} END first_dmg={:?} first_hurt={:?} foe_switch={:?} drop={:?} foe_hp_at_drop={:?} vil_hp_at_drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} max_idle_after_drop={} foe_target_changes={} vil_strikes={} foe_strikes={} dmg_after_drop={} transitions={} foe_hp_end={:.0} vil_hp_end={:.0} foe_off_end=({:.2},{:.2}) live_shape={} finish_shape={}",
        tr.first_dmg, tr.first_hurt, tr.foe_switch, tr.drop_tick, tr.foe_hp_at_drop,
        tr.vil_hp_at_drop, tr.dead_tick, tr.atk_dead_tick, tr.min_dist, tr.max_stall_out,
        tr.max_stall_in, tr.max_idle_after_drop, tr.foe_target_changes, tr.vil_strikes,
        tr.foe_strikes, tr.dmg_after_drop, tr.transitions, tr.foe_hp_end, tr.vil_hp_end,
        tr.foe_off_end.0, tr.foe_off_end.1, tr.live_shape(), tr.finish_shape()
    );
    tr
}

fn order_attack(sim: &mut Sim, tag: &str, vil: u32) {
    assert!(sim.can_attack(vil, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: vil, target: FOE });
}

/// Sandbox staging: Spearman spawned at TC-relative offset
/// `foe_off` under a real Attack on the TC entity; ticks
/// until the TC verifiably drains (the foe holds its spawn
/// cell if in TC-stand range, else walks inward to its TC
/// stand — position recorded). Returns the TC id.
fn sandbox_foe_on_tc(sim: &mut Sim, tag: &str, foe_off: (f64, f64)) -> u32 {
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
    spawn(sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tx + I16F16::from_num(foe_off.0), ty + I16F16::from_num(foe_off.1));
    sim.entities.sort_by_key(|e| e.id);
    assert!(sim.can_attack(FOE, tc_id), "{tag}: can_attack refused foe -> TC");
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
    assert!(draining, "{tag}: foe never settled draining the TC");
    let fo = ent(sim, FOE);
    eprintln!(
        "ENGAGEFINISH {tag} foe settled off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0} foe_attack_target={:?}",
        f(fo.pos_x - tx), f(fo.pos_y - ty), f(fo.hp), f(ent(sim, tc_id).hp), fo.attack_target
    );
    tc_id
}

/// Sandbox staging: the fights-back form. Measured staging
/// fact (this run): a Spearman pinned StandGround ON the
/// footprint is never idle post-tick — CMB-08 step (2)
/// auto-acquires the TC entity at point blank in the same
/// tick (centre: atk=Some(TC); east edge: it acquired a
/// starting unit in reach instead), and a foe holding any
/// attack_target cannot retaliate (the retaliation arm
/// requires attack_target.is_none()). The fights-back foe
/// is therefore staged the one way the sim lets a settled
/// foe hold the VILLAGER as its target from the first
/// tick: foe and villager are spawned in one batch, the
/// villager adjacent (gap 1.5, inside MELEE_REACH 3.0 —
/// gap 2.9 lost the SE-pillar acquisition to a starting
/// unit standing closer to (6,6) than the staged
/// villager) due south of the foe, pinned StandGround — on
/// the first tick CMB-08 step (1) (enemy UNITS before the
/// TC step) acquires the adjacent villager at point blank,
/// auto=true. The villager's own Attack order lands the
/// next tick, before the foe's first strike can land
/// (Spearman delay 30). From then on it is a real duel:
/// the foe holds the villager, strikes it on its 180-tick
/// reload, and repositions onto it if it moves.
fn sandbox_foe_fights_back(sim: &mut Sim, tag: &str, foe_off: (f64, f64), vil: u32) -> u32 {
    // Villager stand: adjacent due south of the foe. At the
    // SE pillar the player's starting Scout (entity 4)
    // stands at TC+(5,5), 1.41 from the foe's (6,6) stand —
    // closer than any adjacent villager stand — and wins
    // the foe's point-blank acquisition; it is therefore
    // parked away north with a real Move order first
    // (bystander scene selection).
    let vil_off = (foe_off.0, foe_off.1 + 1.5);
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
    if foe_off == (6.0, 6.0) {
        sim.push_command(Command::Move { entity: 4, x: tx, y: ty - I16F16::from_num(20) });
        for _ in 0..7_200 {
            sim.tick();
            if ent(sim, 4).order.is_none() {
                break;
            }
        }
        assert!(ent(sim, 4).order.is_none(), "{tag}: starting scout never parked away");
    }
    spawn(sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tx + I16F16::from_num(foe_off.0), ty + I16F16::from_num(foe_off.1));
    spawn(sim, vil, EntityKind::Villager, OWNER_PLAYER,
          tx + I16F16::from_num(vil_off.0), ty + I16F16::from_num(vil_off.1));
    sim.entities.sort_by_key(|e| e.id);
    sim.push_command(Command::SetStance { entity: FOE, stance: Stance::StandGround });
    sim.tick();
    let fo = ent(sim, FOE);
    assert!(fo.stance == Stance::StandGround, "{tag}: stance pin refused ({:?})", fo.stance);
    assert!(fo.attack_target == Some(vil),
        "{tag}: fights-back staging failed — foe acquired {:?}, not the adjacent villager", fo.attack_target);
    let v = ent(sim, vil);
    assert!(v.order.is_none() && v.attack_target.is_none() && v.task == Task::Idle,
        "{tag}: villager not idle at the order moment (ord={} atk={:?} task={:?})",
        v.order.is_some(), v.attack_target, v.task);
    eprintln!(
        "ENGAGEFINISH {tag} foe fights-back staged off=({:.2},{:.2}) foe_hp={:.0} foe_target={:?} auto={} vil_hp={:.0}",
        f(fo.pos_x - tx), f(fo.pos_y - ty), f(fo.hp), fo.attack_target, fo.attack_auto, f(v.hp)
    );
    tc_id
}

fn spawn_villager_south(sim: &mut Sim, id: u32) {
    let (fx, fy) = (ent(sim, FOE).pos_x, ent(sim, FOE).pos_y);
    spawn(sim, id, EntityKind::Villager, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
    sim.entities.sort_by_key(|e| e.id);
    let v = ent(sim, id);
    assert!(v.order.is_none() && v.attack_target.is_none() && v.task == Task::Idle,
        "villager {id} not idle at spawn (ord={} atk={:?} task={:?})",
        v.order.is_some(), v.attack_target, v.task);
}

/// (1) CENTRE: (a) Spearman at the TC centre draining the
/// TC under a real TC-entity order; villager ordered once
/// from the south. (b) The fights-back form: Spearman
/// pinned StandGround at the centre, no TC order — it
/// switches to the villager and strikes it during the
/// fight. Trace answers: disengagement at an HP threshold,
/// at the foe's target switch, at the foe's strike, after
/// N strikes — or a death.
#[test]
fn spearman_tc_centre_villager_finish_or_disengage() {
    let mut failures = Vec::new();

    // (a) TC-order form.
    {
        let tag = "centre-tc-order";
        let mut sim = Sim::new(42);
        let tc_id = sandbox_foe_on_tc(&mut sim, tag, (0.0, 0.0));
        spawn_villager_south(&mut sim, VIL);
        order_attack(&mut sim, tag, VIL);
        let tr = run_window(tag, &mut sim, VIL, tc_id, 7_200);
        if tr.reproduces() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    // (b) Fights-back form (pinned; foe holds the villager
    // as its point-blank acquired target from tick 1).
    {
        let tag = "centre-fights-back";
        let mut sim = Sim::new(42);
        let tc_id = sandbox_foe_fights_back(&mut sim, tag, (0.0, 0.0), VIL);
        order_attack(&mut sim, tag, VIL);
        let tr = run_window(tag, &mut sim, VIL, tc_id, 7_200);
        if tr.foe_switch.is_none() && tr.first_hurt.is_none() {
            failures.push(format!("{tag}: foe never fought back (staging failed): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }

    assert!(failures.is_empty(), "centre failures:\n{}", failures.join("\n"));
}

/// (2) POSITIONS: east edge (8,0) and SE pillar (6,6), each
/// in the TC-order form and the fights-back (pinned) form;
/// villager ordered once from due south of the foe.
#[test]
fn spearman_tc_edge_and_pillar_villager() {
    let mut failures = Vec::new();
    for (name, foe_off) in [("east-edge", (8.0, 0.0)), ("se-pillar", (6.0, 6.0))] {
        // TC-order form.
        {
            let tag = format!("{name}-tc-order");
            let mut sim = Sim::new(42);
            let tc_id = sandbox_foe_on_tc(&mut sim, &tag, foe_off);
            spawn_villager_south(&mut sim, VIL);
            order_attack(&mut sim, &tag, VIL);
            let tr = run_window(&tag, &mut sim, VIL, tc_id, 7_200);
            if tr.reproduces() {
                failures.push(format!("{tag}: {tr:?}"));
            }
        }
        // Fights-back form.
        {
            let tag = format!("{name}-fights-back");
            let mut sim = Sim::new(42);
            let tc_id = sandbox_foe_fights_back(&mut sim, &tag, foe_off, VIL);
            order_attack(&mut sim, &tag, VIL);
            let tr = run_window(&tag, &mut sim, VIL, tc_id, 7_200);
            if tr.foe_switch.is_none() && tr.first_hurt.is_none() {
                failures.push(format!("{tag}: foe never fought back (staging failed): {tr:?}"));
            }
            if tr.reproduces() {
                failures.push(format!("{tag}: {tr:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "edge/pillar failures:\n{}", failures.join("\n"));
}

/// (3) FULL MATCH: AiNormal with the AI deciding on its
/// 30-tick cadence throughout. A Spearman is staged on the
/// player's TC (spawn + real Attack order are scene
/// selection; from then on the AI loop owns it — any AI
/// re-order of the foe lands inside the traced window).
/// Starting villager 2, parked idle south-outside, is
/// ordered once. Question: when/if the foe changes target
/// or position under the AI cadence mid-fight, does the
/// villager's target clear or the villager abandon?
#[test]
fn spearman_full_match_ai_reorder_villager() {
    let tag = "full-match-ai-cadence";
    let mut sim = Sim::new_match(9, MatchMode::AiNormal);
    let ptc_id = sim.tc_entity_of(OWNER_PLAYER).unwrap().id;
    let (px, py) = (ent(&sim, ptc_id).pos_x, ent(&sim, ptc_id).pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, px, py);
    sim.entities.sort_by_key(|e| e.id);
    assert!(sim.can_attack(FOE, ptc_id), "{tag}: can_attack refused foe -> TC");
    sim.push_command(Command::Attack { entity: FOE, target: ptc_id });
    let hp0 = f(ent(&sim, ptc_id).hp);
    let mut draining = false;
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, FOE).attack_target == Some(ptc_id) && f(ent(&sim, ptc_id).hp) < hp0 {
            draining = true;
            break;
        }
    }
    assert!(draining, "{tag}: staged Spearman never drained the player TC");
    // Park starting villager 2 idle south-outside.
    sim.push_command(Command::Move {
        entity: 2,
        x: px,
        y: py + I16F16::from_num(20),
    });
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, 2).order.is_none() {
            break;
        }
    }
    let v = ent(&sim, 2);
    assert!(v.order.is_none() && v.attack_target.is_none() && v.task == Task::Idle,
        "{tag}: villager 2 not idle after parking (ord={} atk={:?} task={:?})",
        v.order.is_some(), v.attack_target, v.task);
    eprintln!(
        "ENGAGEFINISH {tag} order-moment: foe_off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0} tick={}",
        f(ent(&sim, FOE).pos_x - px), f(ent(&sim, FOE).pos_y - py),
        f(ent(&sim, FOE).hp), f(ent(&sim, ptc_id).hp), sim.tick
    );
    assert!(sim.can_attack(2, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: 2, target: FOE });
    let tr = run_window(tag, &mut sim, 2, ptc_id, 7_200);
    eprintln!(
        "ENGAGEFINISH {tag} ai-reorders-observed: foe_target_changes={} foe_switch={:?}",
        tr.foe_target_changes, tr.foe_switch
    );
    assert!(!tr.reproduces(), "{tag}: {tr:?}");
}

/// Two-villager traced window: villager A's Attack is live
/// at t=0; the tick A's first damage lands on the foe,
/// villager B's Attack is issued (the sequential second
/// order). Tracks a drop / finish shape for EITHER villager.
fn run_window_two(tag: &str, sim: &mut Sim, ptc: u32, window: u64) -> (Trace, Trace, Option<u64>) {
    let (px, py) = (ent(sim, ptc).pos_x, ent(sim, ptc).pos_y);
    let mut tra = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut trb = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut second_order_tick: Option<u64> = None;
    let mut last_foe_hp = f(ent(sim, FOE).hp);
    let mut last_a_hp = f(ent(sim, VIL).hp);
    let mut last_b_hp = f(ent(sim, VIL_B).hp);
    let mut last_foe_atk = ent(sim, FOE).attack_target;
    let mut idle_a = 0u64;
    let mut idle_b = 0u64;
    let mut prev = String::new();
    let mut pending_second = false;
    for t in 0..window {
        if pending_second && second_order_tick.is_none() {
            // Issued at this tick boundary — the same
            // application point a command pushed between
            // ticks receives (commands apply at step 1 of
            // the next tick), one tick after A's first
            // damage was observed.
            assert!(sim.can_attack(VIL_B, FOE), "{tag}: can_attack refused B's order");
            sim.push_command(Command::Attack { entity: VIL_B, target: FOE });
            second_order_tick = Some(t);
            eprintln!("ENGAGEFINISH {tag} t={t} SECOND ORDER issued for villager B");
            pending_second = false;
        }
        sim.tick();
        let a = ent(sim, VIL);
        let b = ent(sim, VIL_B);
        let fo = ent(sim, FOE);
        if fo.attack_target != last_foe_atk {
            tra.foe_target_changes += 1;
            trb.foe_target_changes += 1;
            eprintln!(
                "ENGAGEFINISH {tag} t={t} FOE TARGET CHANGE {:?} -> {:?} auto={}",
                last_foe_atk, fo.attack_target, fo.attack_auto
            );
            last_foe_atk = fo.attack_target;
        }
        if f(fo.hp) < last_foe_hp {
            if tra.first_dmg.is_none() {
                tra.first_dmg = Some(t);
                trb.first_dmg = Some(t);
            }
            tra.vil_strikes += 1;
            if tra.drop_tick.is_some() || trb.drop_tick.is_some() {
                tra.dmg_after_drop = true;
                trb.dmg_after_drop = true;
            }
            last_foe_hp = f(fo.hp);
            if second_order_tick.is_none() && !pending_second {
                // The sequential second order: B is ordered
                // when A's first damage lands (issued at the
                // next tick boundary, above).
                pending_second = true;
            }
        }
        if f(a.hp) < last_a_hp {
            tra.foe_strikes += 1;
            if tra.first_hurt.is_none() {
                tra.first_hurt = Some(t);
            }
            last_a_hp = f(a.hp);
        }
        if f(b.hp) < last_b_hp {
            trb.foe_strikes += 1;
            if trb.first_hurt.is_none() {
                trb.first_hurt = Some(t);
            }
            last_b_hp = f(b.hp);
        }
        if tra.foe_switch.is_none() && fo.attack_target == Some(VIL) {
            tra.foe_switch = Some(t);
        }
        if trb.foe_switch.is_none() && fo.attack_target == Some(VIL_B) {
            trb.foe_switch = Some(t);
        }
        let dist_a = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        let dist_b = f(steer::vec_len(fo.pos_x - b.pos_x, fo.pos_y - b.pos_y));
        tra.min_dist = tra.min_dist.min(dist_a);
        trb.min_dist = trb.min_dist.min(dist_b);
        // Drops: only meaningful once that villager's own
        // order is live (A from t=0, B from its order tick).
        if tra.drop_tick.is_none() && !fo.dead && !a.dead && a.attack_target != Some(FOE) {
            tra.drop_tick = Some(t);
            tra.foe_hp_at_drop = Some(f(fo.hp));
            tra.vil_hp_at_drop = Some(f(a.hp));
            eprintln!("ENGAGEFINISH {tag} t={t} VILLAGER A DROP foe_hp={:.0} a_hp={:.0}", f(fo.hp), f(a.hp));
        }
        if second_order_tick.is_some()
            && trb.drop_tick.is_none() && !fo.dead && !b.dead && b.attack_target != Some(FOE)
        {
            trb.drop_tick = Some(t);
            trb.foe_hp_at_drop = Some(f(fo.hp));
            trb.vil_hp_at_drop = Some(f(b.hp));
            eprintln!("ENGAGEFINISH {tag} t={t} VILLAGER B DROP foe_hp={:.0} b_hp={:.0}", f(fo.hp), f(b.hp));
        }
        if tra.drop_tick.is_some() && !fo.dead && !a.dead && a.order.is_none() && a.attack_target.is_none() {
            idle_a += 1;
            tra.max_idle_after_drop = tra.max_idle_after_drop.max(idle_a);
        } else {
            idle_a = 0;
        }
        if trb.drop_tick.is_some() && !fo.dead && !b.dead && b.order.is_none() && b.attack_target.is_none() {
            idle_b += 1;
            trb.max_idle_after_drop = trb.max_idle_after_drop.max(idle_b);
        } else {
            idle_b = 0;
        }
        let sig = format!(
            "A[atk={:?} ord={} eng={} task={:?} hp={:.0}] B[atk={:?} ord={} eng={} task={:?} hp={:.0}] foe[atk={:?} ord={} eng={} auto={} stance={:?} hp={:.0}]",
            a.attack_target, a.order.is_some(), a.attack_engaged, a.task, f(a.hp),
            b.attack_target, b.order.is_some(), b.attack_engaged, b.task, f(b.hp),
            fo.attack_target, fo.order.is_some(), fo.attack_engaged, fo.attack_auto,
            fo.stance, f(fo.hp)
        );
        if sig != prev {
            tra.transitions += 1;
            eprintln!(
                "ENGAGEFINISH {tag} t={t} TRANS {sig} A_off=({:.2},{:.2}) B_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) distA={:.2} distB={:.2} tc_hp={:.0}",
                f(a.pos_x - px), f(a.pos_y - py), f(b.pos_x - px), f(b.pos_y - py),
                f(fo.pos_x - px), f(fo.pos_y - py), dist_a, dist_b, f(ent(sim, ptc).hp)
            );
            prev = sig;
        }
        tra.foe_hp_end = f(fo.hp);
        tra.vil_hp_end = f(a.hp);
        trb.foe_hp_end = f(fo.hp);
        trb.vil_hp_end = f(b.hp);
        if fo.dead {
            tra.dead_tick = Some(t);
            trb.dead_tick = Some(t);
            break;
        }
        if a.dead && tra.atk_dead_tick.is_none() {
            tra.atk_dead_tick = Some(t);
        }
        if b.dead && trb.atk_dead_tick.is_none() {
            trb.atk_dead_tick = Some(t);
        }
        if a.dead && b.dead {
            break;
        }
    }
    eprintln!(
        "ENGAGEFINISH {tag} END second_order={:?} A[first_dmg={:?} first_hurt={:?} drop={:?} idle_after_drop={} dead={:?} hp_end={:.0}] B[first_hurt={:?} drop={:?} idle_after_drop={} dead={:?} hp_end={:.0}] foe[switchA={:?} switchB={:?} target_changes={} hp_end={:.0}]",
        second_order_tick, tra.first_dmg, tra.first_hurt, tra.drop_tick, tra.max_idle_after_drop,
        tra.atk_dead_tick, tra.vil_hp_end, trb.first_hurt, trb.drop_tick, trb.max_idle_after_drop,
        trb.atk_dead_tick, trb.vil_hp_end, tra.foe_switch, trb.foe_switch, tra.foe_target_changes,
        tra.foe_hp_end
    );
    (tra, trb, second_order_tick)
}

/// (4) TWO VILLAGERS ordered sequentially: A is ordered
/// first; B's order lands while A is already fighting
/// (the tick A's first damage lands). TC-order centre form
/// and fights-back centre form. Does EITHER abandon?
#[test]
fn two_villagers_sequential_orders_spearman() {
    let mut failures = Vec::new();
    for (name, fights_back) in [("two-vil-tc-order", false), ("two-vil-fights-back", true)] {
        let tag = name;
        let mut sim = Sim::new(42);
        let tc_id = if fights_back {
            // A is staged adjacent by the fights-back
            // staging itself; B is added below.
            sandbox_foe_fights_back(&mut sim, tag, (0.0, 0.0), VIL)
        } else {
            let tc_id = sandbox_foe_on_tc(&mut sim, tag, (0.0, 0.0));
            spawn_villager_south(&mut sim, VIL);
            tc_id
        };
        // B waits east of the foe, idle, until its order.
        let (fx, fy) = (ent(&sim, FOE).pos_x, ent(&sim, FOE).pos_y);
        spawn(&mut sim, VIL_B, EntityKind::Villager, OWNER_PLAYER,
              fx + I16F16::from_num(20), fy);
        sim.entities.sort_by_key(|e| e.id);
        order_attack(&mut sim, tag, VIL);
        let (tra, trb, second) = run_window_two(tag, &mut sim, tc_id, 7_200);
        if second.is_none() {
            failures.push(format!("{tag}: second order never landed (A never damaged the foe): A={tra:?}"));
        }
        if fights_back && tra.foe_switch.is_none() && trb.foe_switch.is_none()
            && tra.first_hurt.is_none() && trb.first_hurt.is_none()
        {
            failures.push(format!("{tag}: foe never fought back (staging failed)"));
        }
        // Finish shape per villager: that villager damaged
        // is shared (first_dmg is the foe's first HP drop);
        // a villager reproduces if it dropped and then idled
        // >= 600 ticks with no further foe damage, both it
        // and the foe alive at window end.
        for (who, tr) in [("A", &tra), ("B", &trb)] {
            if tr.finish_shape() || tr.live_shape() {
                failures.push(format!("{tag} villager {who}: {tr:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "two-villager failures:\n{}", failures.join("\n"));
}
