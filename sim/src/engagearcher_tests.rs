//! ENGAGEARCHER (2026-11-24): a NEW face of the hidden-
//! attacker defect OUTSIDE the TC roof, plus the idle-
//! defender auto-engage question, staged in the sim
//! harness only.
//!
//! Live #492 (2026-11-24, real client): the player's
//! MILITIA was right-click-ordered onto an enemy ARCHER
//! standing EAST of the TC in OPEN ground (not under the
//! roof / footprint). Result: exactly ONE hit (Archer
//! 30 -> 26 — Militia attack 4 vs Archer melee armour 0,
//! the sim's own arithmetic), then BOTH units stood idle
//! side-by-side for 60+ real seconds with no further
//! damage; the Archer later walked away. Also observed
//! live: the player's idle Scout Cavalry and Militia near
//! the TC NEVER auto-engaged an enemy Scout draining
//! the TC.
//!
//! Part A stages the Archer face in the sim: an enemy
//! Archer in open ground east of the TC holding (in
//! turn) a real Attack on the TC entity (its live likely
//! state), an Attack on a villager, no order (idle), a
//! plain Move, and an AttackMove; a player Militia
//! spawned idle ~20 wu away is ordered ONCE with
//! `Command::Attack` on the Archer (`can_attack`
//! asserted). Per-tick trace (engagestall style): strikes
//! both ways, attack_target / has_order transitions, the
//! drop tick (target cleared after the first strike while
//! both live), max idle after the drop. The LIVE SHAPE is
//! exact: exactly 1 attacker strike, then the attacker
//! dropped, idle >= 600 ticks after the drop, foe HP
//! frozen after that strike, both alive.
//!
//! Part B stages auto-engage: a player Militia and a
//! Scout Cavalry (each stance the sim supports — the
//! freshly-spawned default, explicit Aggressive, plus
//! Defensive and StandGround) stand idle a few tiles
//! south of the TC while an enemy Scout holds a
//! TC-entity Attack order draining the TC. Over ~3600
//! ticks: does the idle defender ever acquire / engage
//! the foe on its own? Acquisitions, defender strikes,
//! and the TC HP delta are recorded. In the DE an idle
//! military unit on Aggressive (the documented default
//! for trained military) auto-engages enemies in its
//! line of sight; a defender that never acquires a foe
//! draining its own TC a few tiles away, on the default
//! or Aggressive stance, reproduces the live reading as
//! a sim behaviour.
//!
//! Spawn positions are scene selection only; every order
//! on the path under test is a real `Command`.
//! Tests only — no production code is changed.
//!
//! OUTCOME (measured on base b5e71dd, see handoff
//! `engagearcher-build.md`): NO scene reproduces either
//! live reading. Part A: in all 5 scenes the Militia
//! closes to ~2.96, lands its first strike at t=279–1396
//! and keeps striking until the Archer dies (8 strikes
//! of 4, t=1119–2236), drop=None everywhere — against
//! the TC-ordered Archer it stays 40/40 (the Archer never
//! retargets, exactly as the TC-ordered Spearman never
//! did); against the idle / AttackMove / fleeing Archers
//! it takes return fire and ends at 16 HP, and it runs
//! down a Move-ordered Archer that walks 65 wu east
//! during the chase. Part B: idle Militia and Scout
//! Cavalry defenders on the freshly-spawned default
//! stance (Aggressive — measured identical to explicit
//! Aggressive, and to Defensive at this distance)
//! acquire the TC-draining Scout at t=0 (Aggressive:
//! inside the single SetStance tick), close ~9 wu and
//! kill it (Militia 12 steps / 48 dmg; Scout 15 steps /
//! 45 dmg), the TC losing only 12–15 HP meanwhile. The
//! ONE no-engage scene is StandGround — by documented
//! design (no movement to attack; the foe stands 12 wu
//! away, out of range; TC −30 HP over the window). That
//! is directly relevant to half the live reading: the
//! STARTING Scout Cavalry's documented Dark Age default
//! is Stand Ground, so a live idle starting Scout not
//! engaging is by design; a live idle TRAINED Militia
//! (Aggressive by default) not engaging is NOT
//! reproduced — in the sim it engages on the first tick.

use crate::entity::{Entity, EntityKind, Stance, Task, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, Sim};
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

const FOE: u32 = 9_801; // enemy Archer (part A) / enemy Scout (part B)
const ATK: u32 = 9_802; // player Militia, the ordered attacker (part A)
const VIL: u32 = 9_803; // player Villager, the Archer's target (variant)
const DEF: u32 = 9_804; // idle defender (part B)
const MELEE: f64 = 3.0;

// ------------------------------------------------------------------
// Part A trace
// ------------------------------------------------------------------

#[derive(Default, Debug)]
struct TraceA {
    first_dmg: Option<u64>,
    first_hurt: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    atk_dead_tick: Option<u64>,
    min_dist: f64,
    max_idle_after_drop: u64,
    foe_target_changes: u32,
    atk_strikes: u32,
    foe_strikes: u32,
    foe_hp_at_drop: Option<f64>,
    dmg_after_drop: bool,
    transitions: u32,
    foe_hp_end: f64,
    atk_hp_end: f64,
    foe_off_start: (f64, f64),
    foe_off_end: (f64, f64),
    foe_walked_away: f64, // distance foe moved from its order-moment cell
}

impl TraceA {
    /// The #492 shape, exactly: exactly ONE attacker
    /// strike, foe alive, attacker dropped after that
    /// strike, idle >= 600 ticks after the drop, foe HP
    /// frozen from the drop on, attacker alive.
    fn stall_shape(&self) -> bool {
        self.atk_strikes == 1
            && self.first_dmg.is_some()
            && self.dead_tick.is_none()
            && self.atk_dead_tick.is_none()
            && self.drop_tick.is_some()
            && self.max_idle_after_drop >= 600
            && !self.dmg_after_drop
    }
    /// The classic arrive-and-idle shape (no strike ever).
    fn live_shape(&self) -> bool {
        self.atk_dead_tick.is_none()
            && self.first_dmg.is_none()
            && self.drop_tick.is_some()
    }
    fn reproduces(&self) -> bool {
        self.stall_shape() || self.live_shape()
    }
}

fn run_window_a(tag: &str, sim: &mut Sim, tc: u32, window: u64) -> TraceA {
    let (px, py) = (ent(sim, tc).pos_x, ent(sim, tc).pos_y);
    let foe0 = ent(sim, FOE);
    let mut tr = TraceA {
        min_dist: f64::MAX,
        foe_off_start: (f(foe0.pos_x - px), f(foe0.pos_y - py)),
        ..Default::default()
    };
    let (sx, sy) = (foe0.pos_x, foe0.pos_y);
    let mut idle_after_drop = 0u64;
    let mut prev = String::new();
    let mut last_foe_atk = ent(sim, FOE).attack_target;
    let mut last_foe_hp = f(ent(sim, FOE).hp);
    let mut last_atk_hp = f(ent(sim, ATK).hp);
    for t in 0..window {
        sim.tick();
        let a = ent(sim, ATK);
        let fo = ent(sim, FOE);
        if fo.attack_target != last_foe_atk {
            tr.foe_target_changes += 1;
            eprintln!(
                "ENGAGEARCHER {tag} t={t} FOE TARGET CHANGE {:?} -> {:?} auto={} (#{})",
                last_foe_atk, fo.attack_target, fo.attack_auto, tr.foe_target_changes
            );
            last_foe_atk = fo.attack_target;
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if f(fo.hp) < last_foe_hp {
            tr.atk_strikes += 1;
            if tr.first_dmg.is_none() {
                tr.first_dmg = Some(t);
            }
            if tr.drop_tick.is_some() {
                tr.dmg_after_drop = true;
            }
            last_foe_hp = f(fo.hp);
        }
        if f(a.hp) < last_atk_hp {
            tr.foe_strikes += 1;
            if tr.first_hurt.is_none() {
                tr.first_hurt = Some(t);
                eprintln!(
                    "ENGAGEARCHER {tag} t={t} ATTACKER FIRST HURT hp={:.0} foe_target={:?}",
                    f(a.hp), fo.attack_target
                );
            }
            last_atk_hp = f(a.hp);
        }
        if tr.first_dmg.is_some()
            && tr.drop_tick.is_none()
            && !fo.dead
            && !a.dead
            && a.attack_target != Some(FOE)
        {
            tr.drop_tick = Some(t);
            tr.foe_hp_at_drop = Some(f(fo.hp));
            eprintln!(
                "ENGAGEARCHER {tag} t={t} ATTACKER DROP after first strike: foe_hp={:.0} atk_hp={:.0} atk_task={:?} atk_target_now={:?} foe_target={:?}",
                f(fo.hp), f(a.hp), a.task, a.attack_target, fo.attack_target
            );
        }
        if tr.drop_tick.is_some() && !fo.dead && !a.dead && a.order.is_none() && a.attack_target.is_none() {
            idle_after_drop += 1;
            tr.max_idle_after_drop = tr.max_idle_after_drop.max(idle_after_drop);
        } else {
            idle_after_drop = 0;
        }
        let sig = format!(
            "atk[atk={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}] foe[atk={:?} ab={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}]",
            a.attack_target, a.order.is_some(), a.attack_engaged, a.strike_tick, a.attack_auto,
            a.stance, a.task, f(a.hp),
            fo.attack_target, fo.attack_building, fo.order.is_some(), fo.attack_engaged,
            fo.strike_tick, fo.attack_auto, fo.stance, fo.task, f(fo.hp)
        );
        if sig != prev {
            tr.transitions += 1;
            eprintln!(
                "ENGAGEARCHER {tag} t={t} TRANS {sig} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} tc_hp={:.0}",
                f(a.pos_x - px), f(a.pos_y - py), f(fo.pos_x - px), f(fo.pos_y - py),
                dist, f(ent(sim, tc).hp)
            );
            prev = sig;
        }
        tr.foe_hp_end = f(fo.hp);
        tr.atk_hp_end = f(a.hp);
        tr.foe_off_end = (f(fo.pos_x - px), f(fo.pos_y - py));
        tr.foe_walked_away = f(steer::vec_len(fo.pos_x - sx, fo.pos_y - sy));
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
        "ENGAGEARCHER {tag} END first_dmg={:?} first_hurt={:?} drop={:?} foe_hp_at_drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_idle_after_drop={} foe_target_changes={} atk_strikes={} foe_strikes={} dmg_after_drop={} transitions={} foe_hp_end={:.0} atk_hp_end={:.0} foe_off_start=({:.2},{:.2}) foe_off_end=({:.2},{:.2}) foe_walked_away={:.2} stall_shape={} live_shape={}",
        tr.first_dmg, tr.first_hurt, tr.drop_tick, tr.foe_hp_at_drop, tr.dead_tick,
        tr.atk_dead_tick, tr.min_dist, tr.max_idle_after_drop, tr.foe_target_changes,
        tr.atk_strikes, tr.foe_strikes, tr.dmg_after_drop, tr.transitions, tr.foe_hp_end,
        tr.atk_hp_end, tr.foe_off_start.0, tr.foe_off_start.1, tr.foe_off_end.0,
        tr.foe_off_end.1, tr.foe_walked_away, tr.stall_shape(), tr.live_shape()
    );
    tr
}

/// Spawn the Militia idle 20 wu WEST of the Archer (the
/// TC side — the live approach direction) and order it
/// once onto the Archer.
fn spawn_and_order_militia(sim: &mut Sim, tag: &str) {
    let (fx, fy) = (ent(sim, FOE).pos_x, ent(sim, FOE).pos_y);
    spawn(sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          fx - I16F16::from_num(20), fy);
    sim.entities.sort_by_key(|e| e.id);
    let a = ent(sim, ATK);
    assert!(a.order.is_none() && a.attack_target.is_none() && a.task == Task::Idle,
        "{tag}: Militia not idle at spawn (ord={} atk={:?} task={:?})",
        a.order.is_some(), a.attack_target, a.task);
    assert!(f(a.hp) == 40.0, "{tag}: Militia not at 40 HP (hp={:.0})", f(a.hp));
    let foe = ent(sim, FOE);
    assert!(f(foe.hp) == 30.0, "{tag}: Archer not at the live 30 HP (hp={:.0})", f(foe.hp));
    eprintln!(
        "ENGAGEARCHER {tag} order-moment: foe_off=({:.2},{:.2}) foe_hp={:.0} foe_target={:?} foe_task={:?} tick={}",
        f(foe.pos_x), f(foe.pos_y), f(foe.hp), foe.attack_target, foe.task, sim.tick
    );
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
}

/// (A1) The live scene: Archer in open ground EAST of
/// the TC holding a real Attack on the TC entity. It is
/// first ticked until the TC verifiably drains and the
/// Archer holds the TC as its target; its settled cell
/// must be EAST of and OUTSIDE the TC footprint
/// (half-extent 3.6 wu).
#[test]
fn militia_vs_archer_holding_tc_attack() {
    let mut failures = Vec::new();
    for (name, from_south) in [("tc-attack-from-west", false), ("tc-attack-again", true)] {
        let tag = name;
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Archer, OWNER_OPPONENT,
              tx + I16F16::from_num(24), ty);
        sim.entities.sort_by_key(|e| e.id);
        assert!(sim.can_attack(FOE, tc_id), "{tag}: can_attack refused Archer -> TC");
        sim.push_command(Command::Attack { entity: FOE, target: tc_id });
        let hp0 = f(ent(&sim, tc_id).hp);
        let mut draining = false;
        for _ in 0..7_200 {
            sim.tick();
            if ent(&sim, FOE).attack_target == Some(tc_id) && f(ent(&sim, tc_id).hp) < hp0 {
                draining = true;
                break;
            }
        }
        assert!(draining, "{tag}: Archer never settled draining the TC");
        let fo = ent(&sim, FOE);
        let off = (f(fo.pos_x - tx), f(fo.pos_y - ty));
        eprintln!(
            "ENGAGEARCHER {tag} archer settled off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0}",
            off.0, off.1, f(fo.hp), f(ent(&sim, tc_id).hp)
        );
        assert!(off.0 > 3.6, "{tag}: Archer settled INSIDE the TC footprint (off={off:?}) — not the open-ground scene");
        if from_south {
            // Second run of the same scene for determinism
            // of the reading (identical staging, identical
            // expectation); the Militia still spawns west.
            let _ = from_south;
        }
        spawn_and_order_militia(&mut sim, tag);
        let tr = run_window_a(tag, &mut sim, tc_id, 7_200);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: REPRODUCES #492 shape: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "archer-tc failures:\n{}", failures.join("\n"));
}

/// (A2) Variants: the Archer holds an Attack on a
/// villager; the Archer is idle; the Archer is under a
/// plain Move east (walking away); the Archer is under
/// an AttackMove west toward the TC.
#[test]
fn militia_vs_archer_variants() {
    let mut failures = Vec::new();

    // --- Archer holding Attack on a villager ---
    {
        let tag = "archer-on-villager";
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Archer, OWNER_OPPONENT,
              tx + I16F16::from_num(24), ty);
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tx + I16F16::from_num(36), ty);
        sim.entities.sort_by_key(|e| e.id);
        assert!(sim.can_attack(FOE, VIL), "{tag}: can_attack refused Archer -> Villager");
        sim.push_command(Command::Attack { entity: FOE, target: VIL });
        let vhp0 = f(ent(&sim, VIL).hp);
        let mut engaged = false;
        for _ in 0..7_200 {
            sim.tick();
            if ent(&sim, FOE).attack_target == Some(VIL) && f(ent(&sim, VIL).hp) < vhp0 {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "{tag}: Archer never engaged the villager");
        spawn_and_order_militia(&mut sim, tag);
        let tr = run_window_a(tag, &mut sim, tc_id, 7_200);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: REPRODUCES #492 shape: {tr:?}"));
        }
    }

    // --- Archer idle in open ground ---
    {
        let tag = "archer-idle";
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
        // Spawned 40 wu east: at 24 wu the Archer's 24-wu
        // LoS reaches the player's starting units and it
        // auto-acquires one within 60 ticks (measured) —
        // at 40 wu it is genuinely idle in open ground.
        spawn(&mut sim, FOE, EntityKind::Archer, OWNER_OPPONENT,
              tx + I16F16::from_num(40), ty);
        sim.entities.sort_by_key(|e| e.id);
        for _ in 0..60 {
            sim.tick();
        }
        let fo = ent(&sim, FOE);
        assert!(fo.order.is_none() && fo.attack_target.is_none(),
            "{tag}: Archer not idle at staging (ord={} atk={:?})", fo.order.is_some(), fo.attack_target);
        spawn_and_order_militia(&mut sim, tag);
        let tr = run_window_a(tag, &mut sim, tc_id, 7_200);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: REPRODUCES #492 shape: {tr:?}"));
        }
    }

    // --- Archer under a plain Move (walking east, away) ---
    {
        let tag = "archer-moving-east";
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Archer, OWNER_OPPONENT,
              tx + I16F16::from_num(24), ty);
        sim.entities.sort_by_key(|e| e.id);
        sim.push_command(Command::Move {
            entity: FOE,
            x: tx + I16F16::from_num(96),
            y: ty,
        });
        for _ in 0..30 {
            sim.tick();
        }
        assert!(ent(&sim, FOE).order.is_some(), "{tag}: Archer not under a Move order");
        spawn_and_order_militia(&mut sim, tag);
        let tr = run_window_a(tag, &mut sim, tc_id, 7_200);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: REPRODUCES #492 shape: {tr:?}"));
        }
    }

    // --- Archer under an AttackMove west toward the TC ---
    {
        let tag = "archer-attackmove-west";
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Archer, OWNER_OPPONENT,
              tx + I16F16::from_num(40), ty);
        sim.entities.sort_by_key(|e| e.id);
        assert!(sim.can_attack_move(FOE), "{tag}: can_attack_move refused the Archer");
        sim.push_command(Command::AttackMove { entity: FOE, x: tx, y: ty });
        for _ in 0..30 {
            sim.tick();
        }
        spawn_and_order_militia(&mut sim, tag);
        let tr = run_window_a(tag, &mut sim, tc_id, 7_200);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{tag}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{tag}: REPRODUCES #492 shape: {tr:?}"));
        }
    }

    assert!(failures.is_empty(), "archer-variant failures:\n{}", failures.join("\n"));
}

// ------------------------------------------------------------------
// Part B: idle defenders vs an enemy Scout draining the TC
// ------------------------------------------------------------------

#[derive(Default, Debug)]
struct TraceB {
    acquired_tick: Option<u64>,
    acquisitions: u32,
    def_strikes: u32, // defender strike_tick firings while target == FOE in reach
    foe_dmg_steps: u32, // foe HP downward steps inside the window
    foe_hp_delta: f64,
    tc_hp_delta: f64,
    def_hp_end: f64,
    foe_hp_end: f64,
    min_dist: f64,
    def_moved: f64, // distance the defender moved from its idle cell
    stance_seen: Option<Stance>,
}

/// Stage the enemy Scout draining the player TC, spawn
/// the idle defender 12 wu (3 tiles) south of the TC
/// centre, optionally set its stance, then watch 3600
/// ticks with NO order ever given to the defender.
fn run_autoengage(tag: &str, kind: EntityKind, stance: Option<Stance>) -> TraceB {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tx, ty);
    sim.entities.sort_by_key(|e| e.id);
    assert!(sim.can_attack(FOE, tc_id), "{tag}: can_attack refused Scout -> TC");
    sim.push_command(Command::Attack { entity: FOE, target: tc_id });
    let hp0 = f(ent(&sim, tc_id).hp);
    let mut draining = false;
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, FOE).attack_target == Some(tc_id) && f(ent(&sim, tc_id).hp) < hp0 {
            draining = true;
            break;
        }
    }
    assert!(draining, "{tag}: enemy Scout never settled draining the TC");
    let foe_settled = (f(ent(&sim, FOE).pos_x - tx), f(ent(&sim, FOE).pos_y - ty));

    spawn(&mut sim, DEF, kind, OWNER_PLAYER, tx, ty + I16F16::from_num(12));
    sim.entities.sort_by_key(|e| e.id);
    {
        let d = ent(&sim, DEF);
        assert!(d.order.is_none() && d.attack_target.is_none() && d.task == Task::Idle,
            "{tag}: defender not idle at spawn (ord={} atk={:?} task={:?})",
            d.order.is_some(), d.attack_target, d.task);
    }
    if let Some(s) = stance {
        sim.push_command(Command::SetStance { entity: DEF, stance: s });
        sim.tick();
        // NOTE: on an engaging stance the defender may
        // already have auto-acquired the foe during this
        // single tick (measured for Aggressive) — that IS
        // an acquisition at t=0, recorded by the window
        // from its initial state, not a staging failure.
    }
    let d = ent(&sim, DEF);
    assert!(d.task == Task::Idle,
        "{tag}: defender task not Idle at window start (task={:?})", d.task);
    let stance_seen = d.stance;
    let (dx, dy) = (d.pos_x, d.pos_y);
    eprintln!(
        "ENGAGEARCHER {tag} window-start: kind={kind:?} stance={stance_seen:?} (explicit={stance:?}) def_off=(0,12) foe_settled_off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0}",
        foe_settled.0, foe_settled.1, f(ent(&sim, FOE).hp), f(ent(&sim, tc_id).hp)
    );

    let tc_start = f(ent(&sim, tc_id).hp);
    let foe_start = f(ent(&sim, FOE).hp);
    let mut tr = TraceB { min_dist: f64::MAX, stance_seen: Some(stance_seen), ..Default::default() };
    let mut was_on_foe = ent(&sim, DEF).attack_target == Some(FOE);
    if was_on_foe {
        tr.acquired_tick = Some(0);
        tr.acquisitions = 1;
        eprintln!("ENGAGEARCHER {tag} t=0 DEFENDER ALREADY ACQUIRED during the stance tick");
    }
    let mut last_strike_tick = ent(&sim, DEF).strike_tick;
    let mut last_foe_hp = foe_start;
    let mut prev = String::new();
    for t in 0..3_600u64 {
        sim.tick();
        let d = ent(&sim, DEF);
        let fo = ent(&sim, FOE);
        let on_foe = d.attack_target == Some(FOE);
        if on_foe && !was_on_foe {
            tr.acquisitions += 1;
            if tr.acquired_tick.is_none() {
                tr.acquired_tick = Some(t);
            }
            eprintln!(
                "ENGAGEARCHER {tag} t={t} DEFENDER ACQUIRED foe auto={} eng={} stance={:?}",
                d.attack_auto, d.attack_engaged, d.stance
            );
        }
        was_on_foe = on_foe;
        let dist = f(steer::vec_len(fo.pos_x - d.pos_x, fo.pos_y - d.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if f(fo.hp) < last_foe_hp {
            // A foe HP step inside the window: the staged
            // defender is the only player unit that can be
            // striking (the starting Scout is StandGround
            // ~7 wu away and never chases; villagers do not
            // auto-attack) — counted as a defender damage
            // step alongside the strike_tick firings.
            tr.foe_dmg_steps += 1;
            last_foe_hp = f(fo.hp);
        }
        if d.strike_tick != last_strike_tick {
            if on_foe && dist <= MELEE + 1.0 {
                tr.def_strikes += 1;
            }
            last_strike_tick = d.strike_tick;
        }
        tr.def_moved = f(steer::vec_len(d.pos_x - dx, d.pos_y - dy));
        let sig = format!(
            "def[atk={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}] foe[atk={:?} hp={:.0}]",
            d.attack_target, d.order.is_some(), d.attack_engaged, d.strike_tick,
            d.attack_auto, d.stance, d.task, f(d.hp), fo.attack_target, f(fo.hp)
        );
        if sig != prev {
            eprintln!(
                "ENGAGEARCHER {tag} t={t} TRANS {sig} def_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} tc_hp={:.0}",
                f(d.pos_x - tx), f(d.pos_y - ty), f(fo.pos_x - tx), f(fo.pos_y - ty),
                dist, f(ent(&sim, tc_id).hp)
            );
            prev = sig;
        }
        tr.def_hp_end = f(d.hp);
        tr.foe_hp_end = f(fo.hp);
        if fo.dead {
            eprintln!("ENGAGEARCHER {tag} t={t} FOE DIED");
            break;
        }
    }
    tr.foe_hp_delta = foe_start - tr.foe_hp_end;
    tr.tc_hp_delta = tc_start - f(ent(&sim, tc_id).hp);
    eprintln!(
        "ENGAGEARCHER {tag} END acquired={:?} acquisitions={} def_strikes={} foe_dmg_steps={} foe_hp_delta={:.0} tc_hp_delta={:.0} def_hp_end={:.0} foe_hp_end={:.0} min_dist={:.2} def_moved={:.2} stance={:?}",
        tr.acquired_tick, tr.acquisitions, tr.def_strikes, tr.foe_dmg_steps, tr.foe_hp_delta,
        tr.tc_hp_delta, tr.def_hp_end, tr.foe_hp_end, tr.min_dist, tr.def_moved,
        tr.stance_seen
    );
    tr
}

/// (B) Auto-engage: idle Militia and Scout Cavalry
/// defenders, default stance and explicit Aggressive
/// (plus Defensive / StandGround for the Militia, whose
/// documented behaviour bounds the expectation). A
/// default/Aggressive defender that NEVER acquires the
/// TC-draining foe reproduces the live #492 reading.
#[test]
fn idle_defenders_vs_tc_draining_scout() {
    let mut failures = Vec::new();
    for (name, kind, stance, must_engage) in [
        ("militia-default", EntityKind::Militia, None, true),
        ("militia-aggressive", EntityKind::Militia, Some(Stance::Aggressive), true),
        ("scout-default", EntityKind::ScoutCavalry, None, true),
        ("scout-aggressive", EntityKind::ScoutCavalry, Some(Stance::Aggressive), true),
        ("militia-defensive", EntityKind::Militia, Some(Stance::Defensive), false),
        ("militia-standground", EntityKind::Militia, Some(Stance::StandGround), false),
    ] {
        let tr = run_autoengage(name, kind, stance);
        eprintln!("ENGAGEARCHER {name} RESULT {tr:?} must_engage={must_engage}");
        if must_engage && tr.acquired_tick.is_none() {
            failures.push(format!(
                "{name}: idle defender NEVER auto-acquired the TC-draining foe in 3600 ticks \
                 (stance={:?}, min_dist={:.2}, tc_hp_delta={:.0}, foe_hp_delta={:.0}) — \
                 reproduces the live #492 no-auto-engage reading",
                tr.stance_seen, tr.min_dist, tr.tc_hp_delta, tr.foe_hp_delta
            ));
        }
    }
    assert!(failures.is_empty(), "auto-engage failures:\n{}", failures.join("\n"));
}
