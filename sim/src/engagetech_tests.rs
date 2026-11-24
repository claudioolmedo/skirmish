//! ENGAGETECH (2026-11-24): the tech-modified-foe face of
//! the hidden-attacker defect, staged in the sim harness
//! only — the territory engagestall (run #491 follow-up)
//! left open in its handoff, item 2 of "Remaining
//! untested territory".
//!
//! Live #491 (2026-11-23, real client, ?occlog=1): the
//! player's MILITIA (40/40) was right-click-ordered onto
//! an enemy SPEARMAN (45 HP) settled under the player's
//! TC roof. It landed exactly ONE hit — and that hit read
//! 45 -> 42 = 3 damage, where the sim's base-stat strike
//! is 4 (Militia MELEE 4 vs Spearman melee armor 0). A
//! live foe carrying +1 melee armor — e.g. Scale Mail
//! Armor researched by its owner — produces exactly 3.
//! engagestall excluded the base-stat pairing in 8 scenes
//! (no drop, no stall, foe dead on schedule); NO suite
//! had ever staged the tech-modified pairing. This suite
//! stages it: the same scenes, with the foe's owner
//! holding the researched infantry-armor tech.
//!
//! THE MECHANISM (real, not a stat override): researched
//! techs are per-player state — research completion sets
//! the tech's bit on the owning player's record
//! (`players[slot].techs |= TechId::ScaleMail.bit()`, the
//! same write the start package performs for Loom), and
//! `Sim::stats_for` / `unit_stats` then patch infantry
//! armor +1/+1 for that owner only (Scale Mail Armor,
//! study §8; the Spearman is infantry and takes the
//! patch). Combat reads the target's EFFECTIVE armor
//! through that patched row, so the Militia's strike
//! lands for 4 - 1 = 3. Scale Mail is a Feudal Blacksmith
//! tech, so the opponent's recorded age is set to Feudal
//! as well — the exact state a live researched foe's
//! owner would be in. Staging asserts the patched row
//! (opponent Spearman 1/1, player Spearman still 0/0)
//! and the first strike's step (exactly 3) in every
//! scene.
//!
//! EXPECTED DUEL: 45 HP / 3 per strike = 15 strikes on
//! the Militia's 120-tick reload (vs 12 strikes of 4 in
//! the base-stat pairing) — the armor stretches the duel
//! by ~360 ticks. The question per scene is the
//! engagestall question, unchanged: after the Militia's
//! first strike lands on the armored foe settled under
//! the TC roof (foe holding a TC-entity Attack order),
//! does the Militia ever drop its attack target / go
//! idle while both live and the foe takes no further
//! damage? The LIVE SHAPE is the engagestall definition
//! exactly: foe damaged but alive at window end,
//! attacker dropped (attack_target cleared after its
//! first strike, while both live), attacker idle >= 600
//! ticks after the drop, foe HP frozen from the drop on,
//! attacker alive and UNHURT.
//!
//! Scenes (engagestall set, armored foe):
//!   (1) POSITIONS: Spearman holding a TC-entity Attack
//!       order at the TC centre / east edge / SE pillar;
//!       attacker = Militia ordered once from the south,
//!       plus the centre scene ordered from the east.
//!   (2) GROUPED: the Spearman joined under the roof by
//!       a second enemy Scout Cavalry also holding a
//!       TC-entity Attack order — centre (attacker from
//!       the south) and SE pillar (attacker from the
//!       east). (Scale Mail patches infantry only, so the
//!       Scout keeps its base row, exactly as in DE.)
//!   (3) FULL MATCH (AiNormal, AI deciding on its
//!       30-tick cadence throughout): Spearman + Scout
//!       staged on the player's TC, the opponent holding
//!       Scale Mail; a player Militia spawned idle
//!       south-outside is ordered once onto the Spearman.
//!
//! Spawn positions are scene selection only; every order
//! on the path under test is a real `Command`
//! (`can_attack` asserted where the live issue returned
//! ok:true).
//!
//! OUTCOME (measured on base b5e71dd, see handoff
//! `engagetech-build.md`): NO scene reproduces the #491
//! stall shape — drop=None and max_idle_after_drop=0 in
//! all 7 scenes. The armor is verifiably LIVE in every
//! duel: first strike step exactly 3 (the live #491
//! reading), min_step = max_step = 3 across every strike
//! of every scene, and the duel runs the full expected
//! 15 strikes — the Spearman dies at t=1983–2014 (vs
//! t=1503–1639 for the base-stat pairing), the Militia
//! 40/40 throughout. The longer, armor-stretched duel
//! changes nothing about the engagement: the Militia
//! closes to 2.96–3.00, strikes on its reload until the
//! foe dies, never drops, and is never hurt — a
//! TC-ordered foe never retargets, the grouped Scout
//! never leaves its TC order and ends 45/45, and in the
//! full match the AI cadence never re-orders either
//! staged foe. A tech-modified foe is therefore measured
//! and EXCLUDED as the stall mechanism: the +1 armor
//! explains the live hit arithmetic (3, not 4) but does
//! not connect to any drop. Per package discipline NO
//! production fix is made; these pins are the
//! deliverable, and the remaining untested territory is
//! named in the handoff (client order-path state for a
//! military attacker remains the prime suspect).

use crate::entity::{Entity, EntityKind, Task, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Age, Command, MatchMode, Sim, TechId};
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

const FOE: u32 = 9_801; // Spearman, primary target (armored)
const ATK: u32 = 9_802; // player Militia, the ordered attacker
const FOE2: u32 = 9_803; // second enemy under the roof
const MELEE: f64 = 3.0;

/// Stage the researched-tech state on the OPPONENT's
/// player record: Scale Mail Armor researched (Feudal
/// age recorded with it, as a live research would
/// require). This is the real mechanism — the tech bit
/// on the player record is what research completion
/// writes and what `stats_for` reads; there is no
/// per-entity armor override anywhere in the sim.
fn research_scale_mail_for_opponent(sim: &mut Sim, tag: &str) {
    sim.players[1].age = Age::Feudal;
    sim.players[1].techs |= TechId::ScaleMail.bit();
    let s = sim.stats_for(OWNER_OPPONENT, EntityKind::Spearman);
    assert_eq!(
        (s.armor_melee, s.armor_pierce),
        (1, 1),
        "{tag}: opponent Spearman did not take the Scale Mail patch"
    );
    // Per-player state, never global: the player's own
    // Spearman keeps the base row.
    let p = sim.stats_for(OWNER_PLAYER, EntityKind::Spearman);
    assert_eq!(
        (p.armor_melee, p.armor_pierce),
        (0, 0),
        "{tag}: player Spearman must stay on the base row"
    );
    eprintln!(
        "ENGAGETECH {tag} staged: opponent holds Scale Mail (Feudal); \
         opponent Spearman armor = 1/1, player Spearman armor = 0/0; \
         expected strike step = 3, expected duel = 15 strikes from 45 HP"
    );
}

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,    // first tick primary foe HP drops
    first_step: Option<f64>,   // size of the first foe HP downward step
    min_step: f64,             // smallest foe HP downward step in the window
    max_step: f64,             // largest foe HP downward step in the window
    first_hurt: Option<u64>,   // first tick attacker HP drops
    drop_tick: Option<u64>,    // attacker target cleared AFTER first strike, both live
    dead_tick: Option<u64>,    // primary foe died
    atk_dead_tick: Option<u64>, // attacker died
    min_dist: f64,
    max_stall_out: u64,        // pre-damage idle-out-of-reach run (classic shape)
    max_stall_in: u64,         // pre-damage idle-in-reach run
    max_idle_after_drop: u64,  // consecutive idle ticks after a drop, both alive
    foe_target_changes: u32,
    foe2_target_changes: u32,
    atk_strikes: u32,          // primary foe HP downward steps
    foe_strikes: u32,          // attacker HP downward steps (any foe)
    foe2_dmg_steps: u32,       // second foe HP downward steps
    foe_hp_at_drop: Option<f64>,
    atk_hp_at_drop: Option<f64>,
    dmg_after_drop: bool,
    transitions: u32,
    foe_hp_end: f64,
    foe2_hp_end: f64,
    atk_hp_end: f64,
    foe_off_end: (f64, f64),
}

impl Trace {
    /// The classic arrive-and-idle shape (prior suites):
    /// no damage either way and the attacker stalls / drops.
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
    /// The #491 STALL shape, exactly as defined in the
    /// engagestall header: foe damaged but alive at
    /// window end, attacker dropped after striking,
    /// attacker idle >= 600 ticks after the drop, foe HP
    /// frozen from the drop, attacker alive and unhurt.
    fn stall_shape(&self) -> bool {
        self.first_dmg.is_some()
            && self.dead_tick.is_none()
            && self.atk_dead_tick.is_none()
            && self.drop_tick.is_some()
            && self.max_idle_after_drop >= 600
            && !self.dmg_after_drop
            && self.first_hurt.is_none()
    }
    fn reproduces(&self) -> bool {
        self.live_shape() || self.stall_shape()
    }
}

/// Per-tick traced window: the Militia attacker vs the
/// primary (armored) foe, plus the second foe when
/// staged. Logs EVERY transition of (attack_target /
/// has_order / attack_engaged / strike_tick / attack_auto
/// / stance / task) for all traced units, with positions
/// and HPs, and every foe HP step size.
fn run_window(tag: &str, sim: &mut Sim, ptc: u32, foe2: Option<u32>, window: u64) -> Trace {
    let (px, py) = (ent(sim, ptc).pos_x, ent(sim, ptc).pos_y);
    let mut tr = Trace {
        min_dist: f64::MAX,
        min_step: f64::MAX,
        foe2_hp_end: -1.0,
        ..Default::default()
    };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    let mut idle_after_drop = 0u64;
    let mut prev = String::new();
    let mut last_foe_atk = ent(sim, FOE).attack_target;
    let mut last_foe2_atk = foe2.map(|id| ent(sim, id).attack_target);
    let mut last_foe_hp = f(ent(sim, FOE).hp);
    let mut last_foe2_hp = foe2.map(|id| f(ent(sim, id).hp));
    let mut last_atk_hp = f(ent(sim, ATK).hp);
    for t in 0..window {
        sim.tick();
        let a = ent(sim, ATK);
        let fo = ent(sim, FOE);
        let f2 = foe2.map(|id| ent(sim, id));
        if fo.attack_target != last_foe_atk {
            tr.foe_target_changes += 1;
            eprintln!(
                "ENGAGETECH {tag} t={t} FOE TARGET CHANGE {:?} -> {:?} auto={} (#{})",
                last_foe_atk, fo.attack_target, fo.attack_auto, tr.foe_target_changes
            );
            last_foe_atk = fo.attack_target;
        }
        if let (Some(f2), Some(last)) = (f2, last_foe2_atk) {
            if f2.attack_target != last {
                tr.foe2_target_changes += 1;
                eprintln!(
                    "ENGAGETECH {tag} t={t} FOE2 TARGET CHANGE {:?} -> {:?} auto={} (#{})",
                    last, f2.attack_target, f2.attack_auto, tr.foe2_target_changes
                );
                last_foe2_atk = Some(f2.attack_target);
            }
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if f(fo.hp) < last_foe_hp {
            let step = last_foe_hp - f(fo.hp);
            tr.atk_strikes += 1;
            if tr.first_dmg.is_none() {
                tr.first_dmg = Some(t);
                tr.first_step = Some(step);
            }
            tr.min_step = tr.min_step.min(step);
            tr.max_step = tr.max_step.max(step);
            if tr.drop_tick.is_some() {
                tr.dmg_after_drop = true;
            }
            last_foe_hp = f(fo.hp);
        }
        if let (Some(f2), Some(lh)) = (f2, last_foe2_hp) {
            if f(f2.hp) < lh {
                tr.foe2_dmg_steps += 1;
                last_foe2_hp = Some(f(f2.hp));
            }
        }
        if f(a.hp) < last_atk_hp {
            tr.foe_strikes += 1;
            if tr.first_hurt.is_none() {
                tr.first_hurt = Some(t);
                eprintln!(
                    "ENGAGETECH {tag} t={t} ATTACKER FIRST HURT hp={:.0} foe_target={:?} foe2_target={:?}",
                    f(a.hp), fo.attack_target,
                    f2.map(|e| e.attack_target)
                );
            }
            last_atk_hp = f(a.hp);
        }
        // The drop that matters for #491: the target clears
        // AFTER the first strike has landed, while both the
        // attacker and the primary foe live.
        if tr.first_dmg.is_some()
            && tr.drop_tick.is_none()
            && !fo.dead
            && !a.dead
            && a.attack_target != Some(FOE)
        {
            tr.drop_tick = Some(t);
            tr.foe_hp_at_drop = Some(f(fo.hp));
            tr.atk_hp_at_drop = Some(f(a.hp));
            eprintln!(
                "ENGAGETECH {tag} t={t} ATTACKER DROP after first strike: attack_target cleared while both live, foe_hp={:.0} atk_hp={:.0} atk_task={:?} atk_target_now={:?} foe_target={:?}",
                f(fo.hp), f(a.hp), a.task, a.attack_target, fo.attack_target
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
        let f2_sig = match f2 {
            Some(e) => format!(
                " foe2[atk={:?} ab={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}]",
                e.attack_target, e.attack_building, e.order.is_some(), e.attack_engaged,
                e.strike_tick, e.attack_auto, e.stance, e.task, f(e.hp)
            ),
            None => String::new(),
        };
        let sig = format!(
            "atk[atk={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}] foe[atk={:?} ab={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}]{f2_sig}",
            a.attack_target, a.order.is_some(), a.attack_engaged, a.strike_tick, a.attack_auto,
            a.stance, a.task, f(a.hp),
            fo.attack_target, fo.attack_building, fo.order.is_some(), fo.attack_engaged,
            fo.strike_tick, fo.attack_auto, fo.stance, fo.task, f(fo.hp)
        );
        if sig != prev {
            tr.transitions += 1;
            eprintln!(
                "ENGAGETECH {tag} t={t} TRANS {sig} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} tc_hp={:.0}",
                f(a.pos_x - px), f(a.pos_y - py), f(fo.pos_x - px), f(fo.pos_y - py),
                dist, f(ent(sim, ptc).hp)
            );
            prev = sig;
        }
        tr.foe_hp_end = f(fo.hp);
        tr.atk_hp_end = f(a.hp);
        if let Some(e) = f2 {
            tr.foe2_hp_end = f(e.hp);
        }
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
        "ENGAGETECH {tag} END first_dmg={:?} first_step={:?} min_step={:.0} max_step={:.0} first_hurt={:?} drop={:?} foe_hp_at_drop={:?} atk_hp_at_drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} max_idle_after_drop={} foe_target_changes={} foe2_target_changes={} atk_strikes={} foe_strikes={} foe2_dmg_steps={} dmg_after_drop={} transitions={} foe_hp_end={:.0} foe2_hp_end={:.0} atk_hp_end={:.0} foe_off_end=({:.2},{:.2}) live_shape={} stall_shape={}",
        tr.first_dmg, tr.first_step, tr.min_step, tr.max_step, tr.first_hurt, tr.drop_tick, tr.foe_hp_at_drop,
        tr.atk_hp_at_drop, tr.dead_tick, tr.atk_dead_tick, tr.min_dist, tr.max_stall_out,
        tr.max_stall_in, tr.max_idle_after_drop, tr.foe_target_changes,
        tr.foe2_target_changes, tr.atk_strikes, tr.foe_strikes, tr.foe2_dmg_steps,
        tr.dmg_after_drop, tr.transitions, tr.foe_hp_end, tr.foe2_hp_end, tr.atk_hp_end,
        tr.foe_off_end.0, tr.foe_off_end.1, tr.live_shape(), tr.stall_shape()
    );
    tr
}

fn order_attack(sim: &mut Sim, tag: &str) {
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
}

/// Sandbox staging: the given foes spawned at TC-relative
/// offsets, each under a real Attack on the TC entity;
/// ticks until the TC verifiably drains AND every foe
/// holds the TC as its attack target (a foe holds its
/// spawn cell if in TC-stand range, else walks inward to
/// its TC stand — positions recorded). Returns the TC id.
fn sandbox_foes_on_tc(sim: &mut Sim, tag: &str, foes: &[(u32, EntityKind, (f64, f64))]) -> u32 {
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
    for &(id, kind, off) in foes {
        spawn(sim, id, kind, OWNER_OPPONENT,
              tx + I16F16::from_num(off.0), ty + I16F16::from_num(off.1));
    }
    sim.entities.sort_by_key(|e| e.id);
    for &(id, _, _) in foes {
        assert!(sim.can_attack(id, tc_id), "{tag}: can_attack refused foe {id} -> TC");
        sim.push_command(Command::Attack { entity: id, target: tc_id });
    }
    let hp0 = f(ent(sim, tc_id).hp);
    let mut draining = false;
    for _ in 0..7_200 {
        sim.tick();
        let all_on_tc = foes.iter().all(|&(id, _, _)| ent(sim, id).attack_target == Some(tc_id));
        if all_on_tc && f(ent(sim, tc_id).hp) < hp0 {
            draining = true;
            break;
        }
    }
    assert!(draining, "{tag}: foes never settled draining the TC");
    for &(id, _, _) in foes {
        let fo = ent(sim, id);
        eprintln!(
            "ENGAGETECH {tag} foe {id} settled off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0} foe_attack_target={:?}",
            f(fo.pos_x - tx), f(fo.pos_y - ty), f(fo.hp), f(ent(sim, tc_id).hp), fo.attack_target
        );
    }
    tc_id
}

/// Spawn the player Militia idle 20 wu from the primary
/// foe, due south (`from_east == false`) or due east.
fn spawn_attacker(sim: &mut Sim, tag: &str, from_east: bool) {
    let (fx, fy) = (ent(sim, FOE).pos_x, ent(sim, FOE).pos_y);
    let (x, y) = if from_east {
        (fx + I16F16::from_num(20), fy)
    } else {
        (fx, fy + I16F16::from_num(20))
    };
    spawn(sim, ATK, EntityKind::Militia, OWNER_PLAYER, x, y);
    sim.entities.sort_by_key(|e| e.id);
    let a = ent(sim, ATK);
    assert!(a.order.is_none() && a.attack_target.is_none() && a.task == Task::Idle,
        "{tag}: Militia not idle at spawn (ord={} atk={:?} task={:?})",
        a.order.is_some(), a.attack_target, a.task);
    assert!(f(a.hp) == 40.0, "{tag}: Militia not at the live 40 HP (hp={:.0})", f(a.hp));
}

/// Per-scene staging proof: the armor must be LIVE in
/// the duel — the first strike step is exactly 3 (the
/// live #491 reading), not the base-stat 4.
fn check_armor_live(tag: &str, tr: &Trace, failures: &mut Vec<String>) {
    if tr.first_dmg.is_some() && tr.first_step != Some(3.0) {
        failures.push(format!(
            "{tag}: first strike step was {:?}, not 3 — Scale Mail patch not live in the duel: {tr:?}",
            tr.first_step
        ));
    }
}

/// (1) POSITIONS: armored Spearman holding a TC-entity
/// order at the TC centre / east edge / SE pillar; the
/// Militia is ordered once from the south, and at the
/// centre also from the east. Question per scene: after
/// the first (3-damage) strike lands, does the Militia
/// drop / stall unhurt in the longer 15-strike duel?
#[test]
fn militia_vs_armoured_spearman_tc_positions() {
    let mut failures = Vec::new();
    for (name, foe_off, from_east) in [
        ("centre-from-south", (0.0, 0.0), false),
        ("centre-from-east", (0.0, 0.0), true),
        ("east-edge-from-south", (8.0, 0.0), false),
        ("se-pillar-from-south", (6.0, 6.0), false),
    ] {
        let tag = name;
        let mut sim = Sim::new(42);
        research_scale_mail_for_opponent(&mut sim, tag);
        let tc_id = sandbox_foes_on_tc(&mut sim, tag, &[(FOE, EntityKind::Spearman, foe_off)]);
        spawn_attacker(&mut sim, tag, from_east);
        order_attack(&mut sim, tag);
        let tr = run_window(tag, &mut sim, tc_id, None, 7_200);
        check_armor_live(tag, &tr, &mut failures);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "position failures:\n{}", failures.join("\n"));
}

/// (2) GROUPED: the armored Spearman under the roof
/// joined by a second enemy Scout Cavalry also holding
/// a TC-entity Attack order (the neighbour the live
/// reading names). Centre (attacker from the south) and
/// SE pillar (attacker from the east).
#[test]
fn militia_vs_armoured_spearman_grouped_under_tc() {
    let mut failures = Vec::new();
    for (name, foes, from_east) in [
        ("centre-plus-scout", vec![
            (FOE, EntityKind::Spearman, (0.0, 0.0)),
            (FOE2, EntityKind::ScoutCavalry, (4.0, 4.0)),
        ], false),
        ("se-pillar-plus-scout", vec![
            (FOE, EntityKind::Spearman, (6.0, 6.0)),
            (FOE2, EntityKind::ScoutCavalry, (8.0, 8.0)),
        ], true),
    ] {
        let tag = name;
        let mut sim = Sim::new(42);
        research_scale_mail_for_opponent(&mut sim, tag);
        let tc_id = sandbox_foes_on_tc(&mut sim, tag, &foes);
        spawn_attacker(&mut sim, tag, from_east);
        order_attack(&mut sim, tag);
        let tr = run_window(tag, &mut sim, tc_id, Some(FOE2), 7_200);
        check_armor_live(tag, &tr, &mut failures);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "grouped failures:\n{}", failures.join("\n"));
}

/// (3) FULL MATCH: AiNormal with the AI deciding on its
/// 30-tick cadence throughout, the opponent holding
/// Scale Mail. A Spearman and a Scout are staged on the
/// player's TC (spawns + real Attack orders are scene
/// selection; from then on the AI loop owns them — any
/// AI re-order lands inside the traced window). A
/// player Militia spawned idle south-outside (scene
/// selection; idle asserted) is ordered once onto the
/// armored Spearman.
#[test]
fn militia_vs_armoured_spearman_full_match_ai_cadence() {
    let tag = "full-match-ai-cadence";
    let mut sim = Sim::new_match(9, MatchMode::AiNormal);
    research_scale_mail_for_opponent(&mut sim, tag);
    let ptc_id = sim.tc_entity_of(OWNER_PLAYER).unwrap().id;
    let (px, py) = (ent(&sim, ptc_id).pos_x, ent(&sim, ptc_id).pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, px, py);
    spawn(&mut sim, FOE2, EntityKind::ScoutCavalry, OWNER_OPPONENT,
          px + I16F16::from_num(4), py + I16F16::from_num(4));
    sim.entities.sort_by_key(|e| e.id);
    for id in [FOE, FOE2] {
        assert!(sim.can_attack(id, ptc_id), "{tag}: can_attack refused foe {id} -> TC");
        sim.push_command(Command::Attack { entity: id, target: ptc_id });
    }
    let hp0 = f(ent(&sim, ptc_id).hp);
    let mut draining = false;
    for _ in 0..7_200 {
        sim.tick();
        let all_on_tc = [FOE, FOE2].iter().all(|&id| ent(&sim, id).attack_target == Some(ptc_id));
        if all_on_tc && f(ent(&sim, ptc_id).hp) < hp0 {
            draining = true;
            break;
        }
    }
    assert!(draining, "{tag}: staged foes never drained the player TC");
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          px, py + I16F16::from_num(20));
    sim.entities.sort_by_key(|e| e.id);
    let a = ent(&sim, ATK);
    assert!(a.order.is_none() && a.attack_target.is_none() && a.task == Task::Idle,
        "{tag}: Militia not idle at the order moment (ord={} atk={:?} task={:?})",
        a.order.is_some(), a.attack_target, a.task);
    eprintln!(
        "ENGAGETECH {tag} order-moment: foe_off=({:.2},{:.2}) foe_hp={:.0} foe2_hp={:.0} tc_hp={:.0} tick={}",
        f(ent(&sim, FOE).pos_x - px), f(ent(&sim, FOE).pos_y - py),
        f(ent(&sim, FOE).hp), f(ent(&sim, FOE2).hp), f(ent(&sim, ptc_id).hp), sim.tick
    );
    order_attack(&mut sim, tag);
    let tr = run_window(tag, &mut sim, ptc_id, Some(FOE2), 7_200);
    eprintln!(
        "ENGAGETECH {tag} ai-reorders-observed: foe_target_changes={} foe2_target_changes={}",
        tr.foe_target_changes, tr.foe2_target_changes
    );
    let mut failures = Vec::new();
    check_armor_live(tag, &tr, &mut failures);
    if tr.reproduces() {
        failures.push(format!("{tag}: {tr:?}"));
    }
    assert!(failures.is_empty(), "full-match failures:\n{}", failures.join("\n"));
}
