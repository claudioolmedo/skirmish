//! ENGAGECROWD (2026-11-24): the two territories the
//! engagearcher / engagetech handoffs explicitly left
//! untested for the hidden-attacker / one-hit-then-idle
//! defect, staged in the sim harness only.
//!
//! Part 1 — THE CROWD FACE (engagetech exclusion item 3,
//! engagearcher exclusion item 4): THREE enemy foes at
//! once under / around the player Town Center roof — a
//! Scout Cavalry at the TC centre, a Spearman at the SE
//! pillar, an Archer spawned east (it settles at its
//! ranged stand east of the TC, "around" the roof) —
//! EVERY foe holding a real `Command::Attack` on the TC
//! entity, the TC verifiably draining with all three
//! holding the TC target before the attacker acts. The
//! player's Militia is spawned idle 20 wu south of the
//! PRIMARY foe and ordered ONCE (`Command::Attack`,
//! `can_attack` asserted) onto it, in turn: the Scout,
//! the Spearman, the Archer. Per-tick trace answers:
//! (a) does the Militia ever drop its target / go idle
//! after its first strike while both live, with the
//! foe's HP frozen after the drop (the exact live shape:
//! exactly 1 strike, drop, idle >= 600 ticks, both
//! alive)? and (b) does separation / crowd pressure
//! DISPLACE the attacker out of melee reach after its
//! first strike (max distance to the foe after the first
//! strike, longest consecutive out-of-reach streak while
//! both live, attacker's drift from its first-strike
//! position)?
//!
//! Part 2 — THE NoAttack FACE (engagearcher exclusion
//! item 2, Part B style): an idle defender 12 wu south
//! of the TC while an enemy Scout drains the TC for
//! 3600 ticks, in the two stances Part B did NOT stage:
//! (i) a Militia on explicit NoAttack (real
//! `Command::SetStance`), and (ii) the player's REAL
//! starting Scout Cavalry — the entity `Sim::new`
//! itself spawns — on its documented Dark Age default
//! Stand Ground (never re-stanced, never ordered; it is
//! relocated to the 12-wu-south cell as scene selection
//! only, stance untouched). A freshly-spawned Scout
//! Cavalry is asserted Aggressive in the same scene, so
//! the starting scout's Stand Ground is measured as
//! DISTINCT, not assumed. Documented design says neither
//! defender ever acquires: NoAttack never auto-engages,
//! and Stand Ground never moves to attack (the foe is
//! 12 wu away, out of melee range). The scenes pin
//! whichever reading the sim gives, as staging pins.
//!
//! Spawn positions are scene selection only; every order
//! on the path under test is a real `Command`. Tests
//! only — no production code is changed.
//!
//! OUTCOME (measured on base 19b4269, see handoff
//! `engagecrowd-build.md`): NO scene reproduces either
//! live shape. Part 1: in all 3 crowd scenes the Militia
//! closes to 2.96–2.98, lands its first strike at
//! t=314–344 and keeps striking until the primary foe
//! dies (Scout 12 strikes, dead t=1634; Spearman 12
//! strikes, dead t=1656; Archer 8 strikes of 4 — the
//! #492 arithmetic, 30 -> 26 first step — dead t=1184),
//! drop=None and max_idle_after_drop=0 everywhere, and
//! the DISPLACEMENT readings are exactly zero:
//! max_dist_after_first == min_dist (2.96–2.98),
//! out-of-reach streak 0, drift from the first-strike
//! position 0.00 — separation/crowd pressure never moves
//! the striking attacker at all. The Militia ends 40/40
//! in every scene (no crowd foe ever retargets off the
//! TC; each foe's only target change is its own
//! death-tick clear, the other two foes end untouched).
//! (Settle note: the all-hold + draining gate passes at
//! t=31 with the Archer still walking in at (22.08, 0);
//! it reaches its (15.94, 0) stand during the window —
//! the same stand engagearcher measured — and in the
//! primary-archer scene the Militia runs it down there.)
//! Part 2: BOTH defenders never acquire in 3600 ticks —
//! acquisitions=0, foe untouched 45/45, defender moved
//! 0.00 wu, min_dist stays 12.00, TC loses exactly 30 HP
//! in each window: the NoAttack Militia and the starting
//! Scout Cavalry on its documented Stand Ground default
//! (measured distinct: a freshly-spawned Scout Cavalry
//! in the same scene is Aggressive) behave exactly as
//! documented design says — pinned as staging pins.

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

const FOE_SCOUT: u32 = 9_801; // crowd foe: Scout Cavalry, TC centre
const FOE_SPEAR: u32 = 9_802; // crowd foe: Spearman, SE pillar
const FOE_ARCH: u32 = 9_803; // crowd foe: Archer, east stand
const ATK: u32 = 9_810; // player Militia, the ordered attacker (part 1)
const DEF: u32 = 9_804; // idle defender (part 2, NoAttack Militia)
const FOE_B: u32 = 9_806; // part 2 enemy Scout draining the TC
const FRESH: u32 = 9_807; // part 2 freshly-spawned Scout (distinctness probe)
const MELEE: f64 = 3.0;

// ------------------------------------------------------------------
// Part 1: the crowd under / around the TC roof
// ------------------------------------------------------------------

#[derive(Default, Debug)]
struct TraceC {
    first_dmg: Option<u64>,
    first_hurt: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    atk_dead_tick: Option<u64>,
    min_dist: f64,
    max_idle_after_drop: u64,
    atk_strikes: u32,
    foe_strikes_on_atk: u32, // HP steps of the attacker, any source
    dmg_after_drop: bool,
    foe_hp_at_drop: Option<f64>,
    foe_hp_end: f64,
    atk_hp_end: f64,
    /// Separation / displacement readings, after the
    /// attacker's first strike, while both live:
    max_dist_after_first: f64,
    max_out_of_reach_streak: u64,
    max_drift_from_first_strike_pos: f64,
    /// Per crowd foe (scout, spear, archer): target changes.
    foe_target_changes: [u32; 3],
    /// Settled TC-relative offsets of the three foes.
    settled_offs: [(f64, f64); 3],
    /// End HP of the two non-primary foes.
    other_hp_end: [f64; 2],
    transitions: u32,
}

impl TraceC {
    /// The live shape, exactly: exactly ONE attacker
    /// strike, then the attacker dropped, idle >= 600
    /// ticks after the drop, foe HP frozen, both alive.
    fn stall_shape(&self) -> bool {
        self.atk_strikes == 1
            && self.first_dmg.is_some()
            && self.dead_tick.is_none()
            && self.atk_dead_tick.is_none()
            && self.drop_tick.is_some()
            && self.max_idle_after_drop >= 600
            && !self.dmg_after_drop
    }
    /// The displacement shape: exactly ONE strike ever,
    /// both alive at the window end, and after that
    /// strike the attacker spent >= 600 consecutive ticks
    /// out of melee reach of the foe — pushed out by the
    /// crowd and never closing again.
    fn displaced_shape(&self) -> bool {
        self.atk_strikes == 1
            && self.first_dmg.is_some()
            && self.dead_tick.is_none()
            && self.atk_dead_tick.is_none()
            && self.max_out_of_reach_streak >= 600
    }
    /// The classic arrive-and-idle shape (no strike ever).
    fn live_shape(&self) -> bool {
        self.atk_dead_tick.is_none() && self.first_dmg.is_none() && self.drop_tick.is_some()
    }
    fn reproduces(&self) -> bool {
        self.stall_shape() || self.displaced_shape() || self.live_shape()
    }
}

/// Stage the three-foe crowd: Scout at the TC centre,
/// Spearman at the SE pillar offset (2, 2), Archer 24 wu
/// east — each under a real Attack on the TC entity —
/// ticked until the TC verifiably drains AND every foe
/// holds the TC as its target. Returns the TC id.
fn stage_crowd(tag: &str, sim: &mut Sim) -> u32 {
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);
    spawn(sim, FOE_SCOUT, EntityKind::ScoutCavalry, OWNER_OPPONENT, tx, ty);
    spawn(
        sim,
        FOE_SPEAR,
        EntityKind::Spearman,
        OWNER_OPPONENT,
        tx + I16F16::from_num(2),
        ty + I16F16::from_num(2),
    );
    spawn(
        sim,
        FOE_ARCH,
        EntityKind::Archer,
        OWNER_OPPONENT,
        tx + I16F16::from_num(24),
        ty,
    );
    sim.entities.sort_by_key(|e| e.id);
    for foe in [FOE_SCOUT, FOE_SPEAR, FOE_ARCH] {
        assert!(sim.can_attack(foe, tc_id), "{tag}: can_attack refused foe {foe} -> TC");
        sim.push_command(Command::Attack { entity: foe, target: tc_id });
    }
    let hp0 = f(ent(sim, tc_id).hp);
    let mut settled = false;
    for _ in 0..7_200 {
        sim.tick();
        let all_hold = [FOE_SCOUT, FOE_SPEAR, FOE_ARCH]
            .iter()
            .all(|&id| ent(sim, id).attack_target == Some(tc_id));
        if all_hold && f(ent(sim, tc_id).hp) < hp0 {
            settled = true;
            break;
        }
    }
    assert!(settled, "{tag}: crowd never settled draining the TC");
    for (id, hp) in [(FOE_SCOUT, 45.0), (FOE_SPEAR, 45.0), (FOE_ARCH, 30.0)] {
        let fo = ent(sim, id);
        eprintln!(
            "ENGAGECROWD {tag} foe {id} settled off=({:.2},{:.2}) hp={:.0} target={:?}",
            f(fo.pos_x - tx),
            f(fo.pos_y - ty),
            f(fo.hp),
            fo.attack_target
        );
        // A foe may have taken settle wear from a neighbour;
        // it must still be alive and holding the TC.
        assert!(!fo.dead && f(fo.hp) > 0.0, "{tag}: foe {id} died during settle");
        let _ = hp;
    }
    tc_id
}

fn run_crowd_scene(tag: &str, primary: u32) -> TraceC {
    let mut sim = Sim::new(42);
    let tc_id = stage_crowd(tag, &mut sim);
    let (tx, ty) = (ent(&sim, tc_id).pos_x, ent(&sim, tc_id).pos_y);
    let foes = [FOE_SCOUT, FOE_SPEAR, FOE_ARCH];
    let settled_offs = [
        (f(ent(&sim, FOE_SCOUT).pos_x - tx), f(ent(&sim, FOE_SCOUT).pos_y - ty)),
        (f(ent(&sim, FOE_SPEAR).pos_x - tx), f(ent(&sim, FOE_SPEAR).pos_y - ty)),
        (f(ent(&sim, FOE_ARCH).pos_x - tx), f(ent(&sim, FOE_ARCH).pos_y - ty)),
    ];

    // The Militia: spawned idle 20 wu SOUTH of the primary
    // foe's settled cell, ordered ONCE onto it.
    let (fx, fy) = (ent(&sim, primary).pos_x, ent(&sim, primary).pos_y);
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER, fx, fy + I16F16::from_num(20));
    sim.entities.sort_by_key(|e| e.id);
    {
        let a = ent(&sim, ATK);
        assert!(
            a.order.is_none() && a.attack_target.is_none() && a.task == Task::Idle,
            "{tag}: Militia not idle at spawn (ord={} atk={:?} task={:?})",
            a.order.is_some(),
            a.attack_target,
            a.task
        );
        assert!(f(a.hp) == 40.0, "{tag}: Militia not at 40 HP (hp={:.0})", f(a.hp));
    }
    let primary_hp0 = f(ent(&sim, primary).hp);
    eprintln!(
        "ENGAGECROWD {tag} order-moment: primary={primary} primary_hp={primary_hp0:.0} settled_offs={settled_offs:?} tick={}",
        sim.tick
    );
    assert!(sim.can_attack(ATK, primary), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: primary });

    let mut tr = TraceC { min_dist: f64::MAX, settled_offs, ..Default::default() };
    let mut idle_after_drop = 0u64;
    let mut oor_streak = 0u64;
    let mut prev = String::new();
    let mut last_foe_atk: Vec<Option<u32>> =
        foes.iter().map(|&id| ent(&sim, id).attack_target).collect();
    let mut last_primary_hp = primary_hp0;
    let mut last_atk_hp = 40.0f64;
    let mut first_strike_pos: Option<(I16F16, I16F16)> = None;
    for t in 0..7_200u64 {
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, primary);
        for (i, &id) in foes.iter().enumerate() {
            let cur = ent(&sim, id).attack_target;
            if cur != last_foe_atk[i] {
                tr.foe_target_changes[i] += 1;
                eprintln!(
                    "ENGAGECROWD {tag} t={t} FOE {id} TARGET CHANGE {:?} -> {:?} auto={}",
                    last_foe_atk[i], cur, ent(&sim, id).attack_auto
                );
                last_foe_atk[i] = cur;
            }
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if f(fo.hp) < last_primary_hp {
            tr.atk_strikes += 1;
            if tr.first_dmg.is_none() {
                tr.first_dmg = Some(t);
                first_strike_pos = Some((a.pos_x, a.pos_y));
                eprintln!(
                    "ENGAGECROWD {tag} t={t} FIRST STRIKE primary_hp={:.0} dist={:.2} atk_off=({:.2},{:.2})",
                    f(fo.hp),
                    dist,
                    f(a.pos_x - tx),
                    f(a.pos_y - ty)
                );
            }
            if tr.drop_tick.is_some() {
                tr.dmg_after_drop = true;
            }
            last_primary_hp = f(fo.hp);
        }
        if f(a.hp) < last_atk_hp {
            tr.foe_strikes_on_atk += 1;
            if tr.first_hurt.is_none() {
                tr.first_hurt = Some(t);
                eprintln!("ENGAGECROWD {tag} t={t} ATTACKER FIRST HURT hp={:.0}", f(a.hp));
            }
            last_atk_hp = f(a.hp);
        }
        // Displacement readings, gated on the first strike.
        if tr.first_dmg.is_some() && !fo.dead && !a.dead {
            tr.max_dist_after_first = tr.max_dist_after_first.max(dist);
            if dist > MELEE {
                oor_streak += 1;
                tr.max_out_of_reach_streak = tr.max_out_of_reach_streak.max(oor_streak);
            } else {
                oor_streak = 0;
            }
            if let Some((sx, sy)) = first_strike_pos {
                let drift = f(steer::vec_len(a.pos_x - sx, a.pos_y - sy));
                tr.max_drift_from_first_strike_pos =
                    tr.max_drift_from_first_strike_pos.max(drift);
            }
        }
        if tr.first_dmg.is_some()
            && tr.drop_tick.is_none()
            && !fo.dead
            && !a.dead
            && a.attack_target != Some(primary)
        {
            tr.drop_tick = Some(t);
            tr.foe_hp_at_drop = Some(f(fo.hp));
            eprintln!(
                "ENGAGECROWD {tag} t={t} ATTACKER DROP after first strike: foe_hp={:.0} atk_hp={:.0} atk_task={:?} atk_target_now={:?} dist={:.2}",
                f(fo.hp), f(a.hp), a.task, a.attack_target, dist
            );
        }
        if tr.drop_tick.is_some() && !fo.dead && !a.dead && a.order.is_none() && a.attack_target.is_none() {
            idle_after_drop += 1;
            tr.max_idle_after_drop = tr.max_idle_after_drop.max(idle_after_drop);
        } else {
            idle_after_drop = 0;
        }
        let sig = format!(
            "atk[atk={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}] prim[hp={:.0}] crowd[sc={:?} sp={:?} ar={:?}]",
            a.attack_target, a.order.is_some(), a.attack_engaged, a.strike_tick, a.attack_auto,
            a.stance, a.task, f(a.hp), f(fo.hp),
            ent(&sim, FOE_SCOUT).attack_target, ent(&sim, FOE_SPEAR).attack_target,
            ent(&sim, FOE_ARCH).attack_target
        );
        if sig != prev {
            tr.transitions += 1;
            eprintln!(
                "ENGAGECROWD {tag} t={t} TRANS {sig} atk_off=({:.2},{:.2}) prim_off=({:.2},{:.2}) dist={:.2} tc_hp={:.0}",
                f(a.pos_x - tx), f(a.pos_y - ty), f(fo.pos_x - tx), f(fo.pos_y - ty),
                dist, f(ent(&sim, tc_id).hp)
            );
            prev = sig;
        }
        tr.foe_hp_end = f(fo.hp);
        tr.atk_hp_end = f(a.hp);
        let others: Vec<u32> = foes.iter().copied().filter(|&id| id != primary).collect();
        tr.other_hp_end = [f(ent(&sim, others[0]).hp), f(ent(&sim, others[1]).hp)];
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
        "ENGAGECROWD {tag} END first_dmg={:?} first_hurt={:?} drop={:?} foe_hp_at_drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_idle_after_drop={} atk_strikes={} foe_strikes_on_atk={} dmg_after_drop={} foe_hp_end={:.0} atk_hp_end={:.0} max_dist_after_first={:.2} max_oor_streak={} max_drift={:.2} foe_target_changes={:?} other_hp_end={:?} transitions={} stall_shape={} displaced_shape={} live_shape={}",
        tr.first_dmg, tr.first_hurt, tr.drop_tick, tr.foe_hp_at_drop, tr.dead_tick,
        tr.atk_dead_tick, tr.min_dist, tr.max_idle_after_drop, tr.atk_strikes,
        tr.foe_strikes_on_atk, tr.dmg_after_drop, tr.foe_hp_end, tr.atk_hp_end,
        tr.max_dist_after_first, tr.max_out_of_reach_streak,
        tr.max_drift_from_first_strike_pos, tr.foe_target_changes, tr.other_hp_end,
        tr.transitions, tr.stall_shape(), tr.displaced_shape(), tr.live_shape()
    );
    tr
}

/// (1) The crowd face: three foes under / around the TC
/// roof, all draining; the Militia ordered once onto
/// each primary in turn. A scene that reproduces the
/// one-hit-then-idle shape OR the displacement shape is
/// a FINDING (collected as a failure with the full trace
/// — see the handoff).
#[test]
fn crowd_three_foes_militia_ordered_once() {
    let mut failures = Vec::new();
    for (name, primary) in [
        ("crowd-primary-scout", FOE_SCOUT),
        ("crowd-primary-spearman", FOE_SPEAR),
        ("crowd-primary-archer", FOE_ARCH),
    ] {
        let tr = run_crowd_scene(name, primary);
        if tr.first_dmg.is_none() && !tr.reproduces() {
            failures.push(format!("{name}: Militia never landed a first strike (staging?): {tr:?}"));
        }
        if tr.reproduces() {
            failures.push(format!("{name}: REPRODUCES live shape: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "crowd failures:\n{}", failures.join("\n"));
}

// ------------------------------------------------------------------
// Part 2: NoAttack / starting-scout defenders vs a TC drain
// ------------------------------------------------------------------

#[derive(Default, Debug)]
struct TraceB {
    acquired_tick: Option<u64>,
    acquisitions: u32,
    foe_dmg_steps: u32,
    foe_hp_delta: f64,
    tc_hp_delta: f64,
    def_hp_end: f64,
    foe_hp_end: f64,
    min_dist: f64,
    def_moved: f64,
    stance_seen: Option<Stance>,
}

/// Enemy Scout at the TC centre draining; the defender
/// stands idle 12 wu south of the TC centre and is NEVER
/// ordered. `noattack` spawns a Militia and sets NoAttack
/// by real command; `starting` uses the player's real
/// starting Scout Cavalry (relocated, stance untouched).
fn run_noacquire(tag: &str, starting: bool) -> TraceB {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tc_id, tx, ty) = (tc.id, tc.pos_x, tc.pos_y);

    // The real starting scout: find it before anything
    // else spawns, and read its stance as `Sim::new`
    // left it (the documented Dark Age Stand Ground).
    let start_scout_id = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::ScoutCavalry && e.owner == OWNER_PLAYER)
        .map(|e| e.id)
        .expect("no starting Scout Cavalry in Sim::new(42)");
    let start_scout_stance = ent(&sim, start_scout_id).stance;
    eprintln!(
        "ENGAGECROWD {tag} starting scout id={start_scout_id} stance={start_scout_stance:?} off=({:.2},{:.2})",
        f(ent(&sim, start_scout_id).pos_x - tx),
        f(ent(&sim, start_scout_id).pos_y - ty)
    );

    spawn(&mut sim, FOE_B, EntityKind::ScoutCavalry, OWNER_OPPONENT, tx, ty);
    sim.entities.sort_by_key(|e| e.id);
    assert!(sim.can_attack(FOE_B, tc_id), "{tag}: can_attack refused Scout -> TC");
    sim.push_command(Command::Attack { entity: FOE_B, target: tc_id });
    let hp0 = f(ent(&sim, tc_id).hp);
    let mut draining = false;
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, FOE_B).attack_target == Some(tc_id) && f(ent(&sim, tc_id).hp) < hp0 {
            draining = true;
            break;
        }
    }
    assert!(draining, "{tag}: enemy Scout never settled draining the TC");

    let def_id = if starting {
        // Scene selection only: relocate the REAL starting
        // scout to the 12-wu-south cell. Its stance is NOT
        // touched and it receives NO command of any kind.
        let e = sim.entities.iter_mut().find(|e| e.id == start_scout_id).unwrap();
        e.pos_x = tx;
        e.pos_y = ty + I16F16::from_num(12);
        start_scout_id
    } else {
        spawn(&mut sim, DEF, EntityKind::Militia, OWNER_PLAYER, tx, ty + I16F16::from_num(12));
        sim.entities.sort_by_key(|e| e.id);
        let d = ent(&sim, DEF);
        assert!(
            d.order.is_none() && d.attack_target.is_none() && d.task == Task::Idle,
            "{tag}: defender not idle at spawn (ord={} atk={:?} task={:?})",
            d.order.is_some(),
            d.attack_target,
            d.task
        );
        sim.push_command(Command::SetStance { entity: DEF, stance: Stance::NoAttack });
        sim.tick();
        DEF
    };
    // Distinctness probe (starting scene only): a FRESHLY
    // spawned Scout Cavalry defaults to Aggressive —
    // measured here so the starting scout's stance is a
    // measured distinction, not an assumption. The probe
    // unit stands far east, out of every LoS, and is never
    // ordered; it cannot interfere with the window.
    if starting {
        spawn(
            &mut sim,
            FRESH,
            EntityKind::ScoutCavalry,
            OWNER_PLAYER,
            tx + I16F16::from_num(60),
            ty + I16F16::from_num(60),
        );
        sim.entities.sort_by_key(|e| e.id);
        let fresh_stance = ent(&sim, FRESH).stance;
        eprintln!("ENGAGECROWD {tag} freshly-spawned scout stance={fresh_stance:?}");
        assert_eq!(
            fresh_stance,
            Stance::Aggressive,
            "{tag}: freshly-spawned Scout Cavalry no longer defaults to Aggressive — distinctness premise changed"
        );
        assert_eq!(
            start_scout_stance,
            Stance::StandGround,
            "{tag}: starting scout no longer carries the documented Stand Ground default (saw {start_scout_stance:?})"
        );
    }
    let d = ent(&sim, def_id);
    assert!(d.task == Task::Idle, "{tag}: defender task not Idle at window start (task={:?})", d.task);
    let stance_seen = d.stance;
    let (dx, dy) = (d.pos_x, d.pos_y);
    eprintln!(
        "ENGAGECROWD {tag} window-start: def={def_id} stance={stance_seen:?} def_off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0}",
        f(dx - tx),
        f(dy - ty),
        f(ent(&sim, FOE_B).hp),
        f(ent(&sim, tc_id).hp)
    );

    let tc_start = f(ent(&sim, tc_id).hp);
    let foe_start = f(ent(&sim, FOE_B).hp);
    let mut tr = TraceB { min_dist: f64::MAX, stance_seen: Some(stance_seen), ..Default::default() };
    let mut was_on_foe = ent(&sim, def_id).attack_target == Some(FOE_B);
    if was_on_foe {
        tr.acquired_tick = Some(0);
        tr.acquisitions = 1;
        eprintln!("ENGAGECROWD {tag} t=0 DEFENDER ALREADY ACQUIRED during staging");
    }
    let mut last_foe_hp = foe_start;
    let mut prev = String::new();
    for t in 0..3_600u64 {
        sim.tick();
        let d = ent(&sim, def_id);
        let fo = ent(&sim, FOE_B);
        let on_foe = d.attack_target == Some(FOE_B);
        if on_foe && !was_on_foe {
            tr.acquisitions += 1;
            if tr.acquired_tick.is_none() {
                tr.acquired_tick = Some(t);
            }
            eprintln!(
                "ENGAGECROWD {tag} t={t} DEFENDER ACQUIRED foe auto={} eng={} stance={:?}",
                d.attack_auto, d.attack_engaged, d.stance
            );
        }
        was_on_foe = on_foe;
        let dist = f(steer::vec_len(fo.pos_x - d.pos_x, fo.pos_y - d.pos_y));
        tr.min_dist = tr.min_dist.min(dist);
        if f(fo.hp) < last_foe_hp {
            tr.foe_dmg_steps += 1;
            last_foe_hp = f(fo.hp);
        }
        tr.def_moved = f(steer::vec_len(d.pos_x - dx, d.pos_y - dy));
        let sig = format!(
            "def[atk={:?} ord={} eng={} strike={:?} auto={} stance={:?} task={:?} hp={:.0}] foe[atk={:?} hp={:.0}]",
            d.attack_target, d.order.is_some(), d.attack_engaged, d.strike_tick,
            d.attack_auto, d.stance, d.task, f(d.hp), fo.attack_target, f(fo.hp)
        );
        if sig != prev {
            eprintln!(
                "ENGAGECROWD {tag} t={t} TRANS {sig} def_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) dist={:.2} tc_hp={:.0}",
                f(d.pos_x - tx), f(d.pos_y - ty), f(fo.pos_x - tx), f(fo.pos_y - ty),
                dist, f(ent(&sim, tc_id).hp)
            );
            prev = sig;
        }
        tr.def_hp_end = f(d.hp);
        tr.foe_hp_end = f(fo.hp);
        if fo.dead {
            eprintln!("ENGAGECROWD {tag} t={t} FOE DIED");
            break;
        }
    }
    tr.foe_hp_delta = foe_start - tr.foe_hp_end;
    tr.tc_hp_delta = tc_start - f(ent(&sim, tc_id).hp);
    eprintln!(
        "ENGAGECROWD {tag} END acquired={:?} acquisitions={} foe_dmg_steps={} foe_hp_delta={:.0} tc_hp_delta={:.0} def_hp_end={:.0} foe_hp_end={:.0} min_dist={:.2} def_moved={:.2} stance={:?}",
        tr.acquired_tick, tr.acquisitions, tr.foe_dmg_steps, tr.foe_hp_delta, tr.tc_hp_delta,
        tr.def_hp_end, tr.foe_hp_end, tr.min_dist, tr.def_moved, tr.stance_seen
    );
    tr
}

/// (2) The NoAttack face, pinned as documented design:
/// neither the NoAttack Militia nor the starting Scout
/// Cavalry (Stand Ground by documented default, measured
/// distinct from a freshly-spawned scout's Aggressive)
/// EVER acquires the TC-draining foe in 3600 ticks, the
/// foe is never damaged by the defender, the defender
/// never moves, and the TC keeps draining (the scene
/// stayed live). Any acquisition is a pin failure and
/// is reported in the handoff either way.
#[test]
fn noattack_and_starting_scout_never_acquire() {
    for (name, starting, want_stance) in [
        ("militia-noattack", false, Stance::NoAttack),
        ("starting-scout-standground", true, Stance::StandGround),
    ] {
        let tr = run_noacquire(name, starting);
        eprintln!("ENGAGECROWD {name} RESULT {tr:?}");
        assert_eq!(tr.stance_seen, Some(want_stance), "{name}: defender stance is not the staged one");
        assert!(
            tr.acquired_tick.is_none() && tr.acquisitions == 0,
            "{name}: defender ACQUIRED the TC-draining foe (tick {:?}, {} acquisitions) — \
             contradicts the documented no-auto-engage behaviour for this stance",
            tr.acquired_tick, tr.acquisitions
        );
        assert!(
            tr.foe_dmg_steps == 0 && tr.foe_hp_delta == 0.0,
            "{name}: defender damaged the foe ({} steps, {:.0} dmg) without acquiring?",
            tr.foe_dmg_steps, tr.foe_hp_delta
        );
        assert!(
            tr.def_moved < 0.5,
            "{name}: defender moved {:.2} wu from its idle cell",
            tr.def_moved
        );
        assert!(
            tr.tc_hp_delta > 0.0,
            "{name}: TC did not keep draining during the window (delta {:.0}) — scene went dead",
            tr.tc_hp_delta
        );
    }
}
