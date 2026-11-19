//! ENGAGEPROBE (2026-11-19): engagement probe for the
//! occluded-attacker defect, bot-loop run #434 — the MILITARY
//! form of the freeze.
//!
//! Live capture (run #434, solo vs AI·Normal, real client,
//! current build): the pick/issue layers are proven — a
//! right-click low on the red outline of an enemy Scout hidden
//! under the player's Town Center roof issued
//! `pick {picked:28, afterGate:28, fromOutline:false,
//! recordIds:[2,28,56,4,1,3], pickedOwner:2}` →
//! `issue {attack e:4 tg:28, ok:true}`. The player's Scout
//! Cavalry (Aggressive stance set explicitly, stance issue
//! ok:true) was ordered onto the occluded enemy Scout standing
//! inside/under the TC footprint and draining the TC. The
//! ordered Scout did NOT fight: it stayed by/under the TC, was
//! idle again within ~40 s (idle-military indicator back to 1),
//! HP 45/45 at every check over 20+ minutes, dealt and took NO
//! damage. The enemy Scout, from that same position, DOES
//! damage the TC (~0.5 HP/s). A second visit repeated the
//! shape: both Scouts' outlines under the roof, the player's
//! Scout idle beside the enemy.
//!
//! occorder9 (run #432/#433) staged a Villager attacker and a
//! foe ordered onto the TC from tc+8 and found engagement in
//! every scene — but it did NOT stage:
//!   - the foe's SETTLED position: a unit attacking the TC
//!     *entity* must stand within MELEE_REACH (3.0) of the TC
//!     entity's centre to land a strike at all, so a foe that
//!     is verifiably draining the TC stands inside the disc
//!     r ≤ 3 around the TC centre — not at tc+8;
//!   - the attacker starting INSIDE the footprint (live
//!     visit 2: the attacker's own outline was under the roof);
//!   - a Scout-vs-Scout pairing in an AiNormal match (the AI
//!     opponent's own starting Scout as the foe);
//!   - every footprint position class (centre / inner / mid /
//!     edge ring / corners) as the foe's standing position;
//!   - a body ring of the player's own villagers around the
//!     foe at the TC centre (the dropoff crowd of a live TC).
//!
//! This suite stages exactly those, through the real Command
//! path only (orders under test are `Command`s; spawn staging
//! follows the amoveprobe/occorder9 discipline — the foe's
//! kind/position/stance is scene selection, every order on the
//! path under test is a real Command, and the foe's TC attack
//! in the disc scenes is itself command-issued).
//!
//! Candidates discriminated:
//!   (a) approach/stand-cell search fails — every cell within
//!       weapon range of the target is inside the footprint /
//!       blocked, and the give-up path silently clears the
//!       order to Idle (`attack_step`'s chase arm:
//!       `set_move_order` false → `clear_combat`);
//!   (b) the target is "contained" and the combat tick skips
//!       it — note `is_contained()` is garrison/transport
//!       only, NOT footprint membership; the disc/grid scenes
//!       test the footprint form, occorder9 the garrison form;
//!   (c) range measured to the target's centre vs footprint
//!       edge and never satisfied at the closest reachable
//!       stand cell;
//!   (d) the attacker's OWN position inside/at the footprint
//!       breaks its attack execution;
//!   (e) something else the measurements show.
//!
//! Measurement per scene: min attacker↔foe distance,
//! ticks-to-idle (first tick the attack target clears while
//! both units live — the live "idle again within ~40 s"
//! signature, 40 s ≈ 2,400 ticks), foe dead tick, end HPs,
//! end offsets from the TC centre, TC HP delta over the
//! window.
//!
//! OUTCOME (measured, see handoff `engageprobe-build.md`):
//!   - EVERY footprint geometry engages: the foe at all 16
//!     footprint position classes (centre / disc / inner /
//!     mid / edge ring / corners), the attacker starting at
//!     every inside-footprint class, Scout/Militia/Spearman
//!     attackers, TwoPlayer and AiNormal real-command scenes
//!     with the foe settled at its true TC-attacker stand
//!     (off (0.7,−2.9), 2.94 from the TC centre). Candidates
//!     (a), (b)-footprint-form, (c) and (d) are DISPROVEN.
//!   - ONE staged configuration reproduces the live
//!     non-engagement's visible shape: the foe ENGAGED ON THE
//!     TC (draining it at the live rate, −30 HP / 3,600
//!     ticks = 0.5 HP/s) and ringed by standing bodies at
//!     r≈3. The attacker is deflected by separation + MOV-03
//!     bump repaths into pockets 4.08–5.04 from the target —
//!     outside MELEE_REACH — and orbits there with the attack
//!     order HELD for the full 14,400-tick live window: zero
//!     damage either way, both HPs full, TC draining (−120).
//!     The ring need not be complete (west-gap variant froze
//!     bit-identically) and the starting bodies are not
//!     required (ablation froze). The freeze needs BOTH
//!     ingredients: with a StandGround foe the same ring
//!     ENGAGES — because the foe kills the two ring villagers
//!     at exactly 3.0 from it (its acquisition boundary),
//!     opening the attacker's fighting pocket; a TC-engaged
//!     foe never clears bodies. A sparser centre cluster and
//!     the same ring on open ground both engage.
//!   - ONE live observable is NOT matched: the live Scout
//!     read as IDLE within ~40 s (idle-military 0→1, which
//!     requires attack_target cleared); in every staged
//!     freeze the order is HELD, never cleared. No staged
//!     configuration reproduced the clear-to-idle. Per the
//!     assignment's discipline this package therefore makes
//!     NO production fix: the staged freeze is pinned here
//!     by the failing crowd tests as the proven mechanism
//!     family, and the handoff names the discriminating live
//!     capture the next vigia must take (the ordered unit's
//!     sim-side attack_target/order state at freeze time).

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

const FOE: u32 = 9_901;
const ATK: u32 = 9_902;

#[derive(Default, Debug)]
struct Engage {
    min_dist: f64,
    idle_tick: Option<u64>,
    held_ticks: u64,
    foe_dead_tick: Option<u64>,
    foe_hp_end: f64,
    atk_hp_end: f64,
    atk_end_off: (f64, f64),
    foe_end_off: (f64, f64),
    tc_hp_delta: f64,
}

fn report(tag: &str, m: &Engage) -> String {
    format!(
        "{tag} min_dist={:.2} idle_tick={:?} held={} foe_dead_tick={:?} foe_hp_end={:.0} atk_hp_end={:.0} atk_end_off=({:.1},{:.1}) foe_end_off=({:.1},{:.1}) tc_hp_delta={:.0}",
        m.min_dist, m.idle_tick, m.held_ticks, m.foe_dead_tick, m.foe_hp_end,
        m.atk_hp_end, m.atk_end_off.0, m.atk_end_off.1, m.foe_end_off.0,
        m.foe_end_off.1, m.tc_hp_delta
    )
}

/// Measure an already-issued Attack order (attacker `atk`,
/// target `foe`) over `window` ticks. TC offsets are measured
/// against entity 0 (the player's Town Center).
fn measure(sim: &mut Sim, atk: u32, foe: u32, window: u64) -> Engage {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let tc_hp0 = f(tc.hp);
    let mut m = Engage {
        min_dist: f64::MAX,
        ..Default::default()
    };
    for t in 0..window {
        sim.tick();
        let a = ent(sim, atk);
        let fo = ent(sim, foe);
        let d = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        if a.attack_target == Some(foe) {
            m.held_ticks += 1;
        } else if m.idle_tick.is_none() && !fo.dead && !a.dead {
            m.idle_tick = Some(t);
        }
        if fo.dead {
            m.foe_dead_tick = Some(t);
            break;
        }
        if a.dead {
            break;
        }
    }
    let a = ent(sim, atk);
    let fo = ent(sim, foe);
    m.foe_hp_end = f(fo.hp);
    m.atk_hp_end = f(a.hp);
    m.atk_end_off = (f(a.pos_x - tcx), f(a.pos_y - tcy));
    m.foe_end_off = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    m.tc_hp_delta = f(ent(sim, 0).hp) - tc_hp0;
    m
}

fn engaged(m: &Engage, foe_max_hp: f64) -> bool {
    m.foe_dead_tick.is_some() || m.foe_hp_end < foe_max_hp
}

// ---------------------------------------------------------------------
// Setup A — fully real scenes: the opponent's own starting
// Scout rides in by commands, attacks the player's TC entity
// (target 0) and settles where a TC-attacker must stand; the
// player's own starting Scout (id 4, the live attacker's id
// class) is Moved to a start class, set Aggressive by command
// (the live sequence), and ordered Attack on the foe.
// ---------------------------------------------------------------------

fn real_scene(mode: MatchMode, tag: &str, atk_start_off: (i32, i32)) -> Engage {
    let mut sim = Sim::new_match(42, mode);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::ScoutCavalry)
        .unwrap()
        .id;
    // The foe rides to the east side and attacks the TC.
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
    // Let the foe settle into its striking stand and record it:
    // this is the position the live foe must occupy (it is
    // verifiably draining the TC at ~0.5 HP/s in the capture).
    for _ in 0..600 {
        sim.tick();
    }
    let fo = ent(&sim, foe);
    let foe_off = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    let foe_dist_tc = f(steer::vec_len(fo.pos_x - tcx, fo.pos_y - tcy));
    eprintln!(
        "ENGAGEPROBE {tag} foe-settled off=({:.1},{:.1}) dist_to_tc_centre={:.2} foe_attack_target={:?}",
        foe_off.0, foe_off.1, foe_dist_tc, fo.attack_target
    );
    // The attacker takes its start class by Move, as a player
    // would position it, then the live order sequence:
    // SetStance Aggressive, then Attack.
    let start = (
        tcx + I16F16::from_num(atk_start_off.0),
        tcy + I16F16::from_num(atk_start_off.1),
    );
    sim.push_command(Command::Move { entity: 4, x: start.0, y: start.1 });
    let settled = run_until(&mut sim, 20_000, |s| {
        let e = ent(s, 4);
        e.order.is_none()
            && steer::vec_len(e.pos_x - start.0, e.pos_y - start.1) < I16F16::from_num(4)
    });
    assert!(settled, "{tag}: the attacker never settled at its start class");
    sim.push_command(Command::SetStance { entity: 4, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(4, foe), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: 4, target: foe });
    let m = measure(&mut sim, 4, foe, 7_200);
    eprintln!("ENGAGEPROBE {}", report(tag, &m));
    m
}

/// Live visit shape, TwoPlayer: attacker starts west outside
/// the footprint (occorder9's geometry, now Scout-vs-Scout
/// with the foe SETTLED on the TC, stance set by command).
#[test]
fn real_twoplayer_scout_from_west_engages_settled_tc_attacker() {
    let m = real_scene(MatchMode::TwoPlayer, "real-2p-west", (-8, 0));
    assert!(
        engaged(&m, 45.0),
        "no engagement from west start: {}",
        report("real-2p-west", &m)
    );
}

/// The live mode itself: solo vs AI·Normal, foe = the AI's own
/// starting Scout (driven here by commands for staging; the AI
/// runs throughout). Attacker from the west.
#[test]
fn real_ainormal_scout_from_west_engages_settled_tc_attacker() {
    let m = real_scene(MatchMode::AiNormal, "real-ai-west", (-8, 0));
    assert!(
        engaged(&m, 45.0),
        "no engagement in AiNormal from west start: {}",
        report("real-ai-west", &m)
    );
}

/// Candidate (d), live visit 2's shape: the attacker starts
/// INSIDE the footprint, west inner cell — its outline under
/// the roof, the foe settled at the centre disc.
#[test]
fn real_twoplayer_scout_from_inside_west_engages() {
    let m = real_scene(MatchMode::TwoPlayer, "real-2p-inside-west", (-5, 0));
    assert!(
        engaged(&m, 45.0),
        "no engagement from inside-west start: {}",
        report("real-2p-inside-west", &m)
    );
}

/// Candidate (d): the attacker starts inside the footprint on
/// the EAST edge ring, beyond the foe (foe between attacker
/// and the TC centre approach side).
#[test]
fn real_twoplayer_scout_from_inside_east_engages() {
    let m = real_scene(MatchMode::TwoPlayer, "real-2p-inside-east", (9, 0));
    assert!(
        engaged(&m, 45.0),
        "no engagement from inside-east start: {}",
        report("real-2p-inside-east", &m)
    );
}

/// Candidate (d): the attacker starts inside the footprint on
/// the south edge ring.
#[test]
fn real_twoplayer_scout_from_inside_south_engages() {
    let m = real_scene(MatchMode::TwoPlayer, "real-2p-inside-south", (0, 9));
    assert!(
        engaged(&m, 45.0),
        "no engagement from inside-south start: {}",
        report("real-2p-inside-south", &m)
    );
}

/// Candidate (d), closest form: the attacker is Moved onto the
/// foe's own settled spot first (both outlines on the same
/// under-roof cell, as in live visit 2), then ordered Attack.
#[test]
fn real_twoplayer_scout_overlapping_foe_engages() {
    let mut sim = Sim::new_match(42, MatchMode::TwoPlayer);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::ScoutCavalry)
        .unwrap()
        .id;
    let porch = (tcx + I16F16::from_num(8), tcy);
    sim.push_command(Command::Move { entity: foe, x: porch.0, y: porch.1 });
    assert!(run_until(&mut sim, 120_000, |s| {
        let e = ent(s, foe);
        e.order.is_none()
            && steer::vec_len(e.pos_x - porch.0, e.pos_y - porch.1) < I16F16::from_num(4)
    }));
    sim.push_command(Command::Attack { entity: foe, target: 0 });
    assert!(run_until(&mut sim, 20_000, |s| ent(s, 0).hp < I16F16::from_num(2400)));
    for _ in 0..600 {
        sim.tick();
    }
    let (fx, fy) = (ent(&sim, foe).pos_x, ent(&sim, foe).pos_y);
    sim.push_command(Command::Move { entity: 4, x: fx, y: fy });
    let settled = run_until(&mut sim, 20_000, |s| {
        let e = ent(s, 4);
        e.order.is_none()
            && steer::vec_len(e.pos_x - fx, e.pos_y - fy) < I16F16::from_num(4)
    });
    assert!(settled, "overlap: the attacker never reached the foe's spot");
    sim.push_command(Command::SetStance { entity: 4, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: 4, target: foe });
    let m = measure(&mut sim, 4, foe, 7_200);
    eprintln!("ENGAGEPROBE {}", report("real-2p-overlap", &m));
    assert!(
        engaged(&m, 45.0),
        "no engagement from overlapping start: {}",
        report("real-2p-overlap", &m)
    );
}

// ---------------------------------------------------------------------
// Setup B — spawn-staged foe grid: EVERY footprint position
// class. The TC footprint spans tc−6 .. tc+10 on each axis
// (4×4 cells of 4 wu; the TC entity stands on its (1,1) cell
// centre). The foe is a Scout (the live kind), Stand Ground
// (pinned: it fights what reaches it, never walks out to
// rescue the engagement). The attacker is a Scout spawned
// west outside the footprint (tc−16) and ordered Attack.
// ---------------------------------------------------------------------

fn grid_scene(tag: &str, foe_off: (i32, i32), atk_kind: EntityKind) -> Engage {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe_pos = (
        tcx + I16F16::from_num(foe_off.0),
        tcy + I16F16::from_num(foe_off.1),
    );
    spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = Stance::StandGround;
    spawn(
        &mut sim,
        ATK,
        atk_kind,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report(tag, &m));
    m
}

/// The foe grid, Scout attacker (the live pairing). One test
/// so the per-class measurements print together; the assert
/// names every failing class.
#[test]
fn foe_grid_scout_attacker_engages_every_footprint_class() {
    let classes: &[(&str, (i32, i32))] = &[
        ("centre", (0, 0)),
        ("disc-e", (3, 0)),
        ("disc-n", (0, -3)),
        ("inner-e", (2, 0)),
        ("inner-w", (-2, 0)),
        ("inner-s", (0, 2)),
        ("mid-se", (4, 4)),
        ("mid-nw", (-4, -4)),
        ("edge-e", (9, 0)),
        ("edge-w", (-5, 0)),
        ("edge-n", (0, -5)),
        ("edge-s", (0, 9)),
        ("corner-se", (9, 9)),
        ("corner-nw", (-5, -5)),
        ("corner-ne", (9, -5)),
        ("corner-sw", (-5, 9)),
    ];
    let mut failures: Vec<String> = Vec::new();
    for (name, off) in classes {
        let tag = format!("grid-scout-{name}");
        let m = grid_scene(&tag, *off, EntityKind::ScoutCavalry);
        if !engaged(&m, 45.0) {
            failures.push(report(&tag, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "footprint classes where a Scout attacker never engaged:\n{}",
        failures.join("\n")
    );
}

/// The foe grid's key classes, Militia and Spearman attackers
/// (the task's attacker-kind spread).
#[test]
fn foe_grid_militia_and_spearman_engage_key_classes() {
    let mut failures: Vec<String> = Vec::new();
    for (kind, kname) in [
        (EntityKind::Militia, "militia"),
        (EntityKind::Spearman, "spearman"),
    ] {
        for (name, off) in [("centre", (0, 0)), ("disc-e", (3, 0)), ("edge-e", (9, 0)), ("corner-nw", (-5, -5))] {
            let tag = format!("grid-{kname}-{name}");
            let m = grid_scene(&tag, off, kind);
            if !engaged(&m, 45.0) {
                failures.push(report(&tag, &m));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "classes where a Militia/Spearman attacker never engaged:\n{}",
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------
// Setup B2 — the foe IN the TC-attacker disc, actively
// draining the TC by a real Attack command (spawn-staged
// position, command-issued TC attack), attacker spawned at
// INSIDE-footprint start classes (candidate (d) in spawn
// form, all classes in one sweep).
// ---------------------------------------------------------------------

#[test]
fn attacker_inside_footprint_classes_engage_disc_foe_draining_tc() {
    let mut failures: Vec<String> = Vec::new();
    for (name, off) in [
        ("west-inner", (-5, 0)),
        ("east-edge", (9, 0)),
        ("north-edge", (0, -5)),
        ("south-edge", (0, 9)),
        ("centre", (0, 0)),
        ("foe-spot", (3, 0)),
        ("corner-nw", (-5, -5)),
        ("corner-se", (9, 9)),
    ] {
        let tag = format!("inside-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        // Foe in the disc, draining the TC by command.
        spawn(
            &mut sim,
            FOE,
            EntityKind::ScoutCavalry,
            OWNER_OPPONENT,
            tcx + I16F16::from_num(3),
            tcy,
        );
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        let draining = run_until(&mut sim, 5_000, |s| ent(s, 0).hp < I16F16::from_num(2400));
        assert!(draining, "{tag}: staging broken — disc foe never drained the TC");
        // Attacker spawned at the inside class, Aggressive by
        // command (the live sequence), then Attack.
        spawn(
            &mut sim,
            ATK,
            EntityKind::ScoutCavalry,
            OWNER_PLAYER,
            tcx + I16F16::from_num(off.0),
            tcy + I16F16::from_num(off.1),
        );
        sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
        sim.tick();
        assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused");
        sim.push_command(Command::Attack { entity: ATK, target: FOE });
        let m = measure(&mut sim, ATK, FOE, 3_600);
        eprintln!("ENGAGEPROBE {}", report(&tag, &m));
        if !engaged(&m, 45.0) {
            failures.push(report(&tag, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "inside-footprint attacker classes that never engaged the disc foe:\n{}",
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------
// Setup C — controls.
// ---------------------------------------------------------------------

/// Control 1: the same foe Scout standing 1 cell CLEAR of the
/// footprint's east edge (edge at tc+10; foe at tc+14) must
/// engage normally.
#[test]
fn control_foe_one_cell_outside_footprint_engages() {
    let m = grid_scene("control-outside-east", (14, 0), EntityKind::ScoutCavalry);
    assert!(
        engaged(&m, 45.0),
        "CONTROL FAILED — foe outside the footprint did not engage: {}",
        report("control-outside-east", &m)
    );
}

/// Control 2: a friendly unit ordered to attack a Town Center
/// itself from inside/at its footprint — the enemy's own
/// working configuration, mirrored: a player Militia spawned
/// inside the OPPONENT's TC footprint disc, ordered Attack on
/// the opponent TC entity, must drain it.
#[test]
fn control_unit_inside_footprint_drains_enemy_tc() {
    let mut sim = Sim::new_match(42, MatchMode::TwoPlayer);
    let otc = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::TownCenter)
        .map(|e| (e.id, e.pos_x, e.pos_y))
        .expect("opponent TC exists");
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        otc.1 + I16F16::from_num(3),
        otc.2,
    );
    let hp0 = f(ent(&sim, otc.0).hp);
    sim.push_command(Command::Attack { entity: ATK, target: otc.0 });
    let mut drained = false;
    for _ in 0..3_600 {
        sim.tick();
        if f(ent(&sim, otc.0).hp) < hp0 {
            drained = true;
            break;
        }
    }
    let a = ent(&sim, ATK);
    eprintln!(
        "ENGAGEPROBE control-attack-tc tc_hp {hp0:.0} -> {:.0} atk_off=({:.1},{:.1}) atk_target={:?}",
        f(ent(&sim, otc.0).hp),
        f(a.pos_x - otc.1),
        f(a.pos_y - otc.2),
        a.attack_target
    );
    assert!(drained, "CONTROL FAILED — unit inside the footprint never drained the enemy TC");
}

// ---------------------------------------------------------------------
// Setup E — the dropoff crowd: the foe in the disc draining
// the TC, ringed by the player's own idle villagers standing
// at the TC centre (the live TC's traffic), attacker Scout
// from the west.
// ---------------------------------------------------------------------

#[test]
fn crowd_ring_around_disc_foe_does_not_strand_scout() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(3),
        tcy,
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    // Eight villagers ring the foe at r=3 (continuous stand
    // positions, spawn-staged as scene selection).
    let ring: [(f64, f64); 8] = [
        (3.0, 0.0),
        (2.12, 2.12),
        (0.0, 3.0),
        (-2.12, 2.12),
        (-3.0, 0.0),
        (-2.12, -2.12),
        (0.0, -3.0),
        (2.12, -2.12),
    ];
    for (i, (dx, dy)) in ring.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(3) + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    let draining = run_until(&mut sim, 5_000, |s| ent(s, 0).hp < I16F16::from_num(2400));
    assert!(draining, "crowd: staging broken — disc foe never drained the TC");
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("crowd-ring", &m));
    assert!(
        engaged(&m, 45.0),
        "the villager ring stranded the scout: {}",
        report("crowd-ring", &m)
    );
}

// ---------------------------------------------------------------------
// Diagnostic (always passes): the pathing question behind
// candidate (a), asked directly for every staged geometry —
// can the chase's `set_move_order` path from the attacker's
// start to the foe's exact position at all?
// ---------------------------------------------------------------------

#[test]
fn probe_pathing_to_foe_positions() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    let ai = sim.entities.iter().position(|e| e.id == ATK).unwrap();
    for (name, off) in [
        ("centre", (0, 0)),
        ("disc-e", (3, 0)),
        ("edge-e", (9, 0)),
        ("edge-w", (-5, 0)),
        ("corner-se", (9, 9)),
        ("corner-nw", (-5, -5)),
        ("outside-e", (14, 0)),
    ] {
        let to = (
            tcx + I16F16::from_num(off.0),
            tcy + I16F16::from_num(off.1),
        );
        let cell = sim.grid().world_to_cell(to.0, to.1);
        let ok = sim.set_move_order(ai, to.0, to.1);
        sim.entities[ai].order = None;
        eprintln!(
            "ENGAGEPROBE pathing {name}: goal_cell={cell:?} blocked={} set_move_order={ok}",
            sim.grid().is_blocked(cell.0, cell.1)
        );
    }
    // From inside the footprint (candidate (d)'s start cells)
    // to the disc foe's position.
    for (name, off) in [("west-inner", (-5, 0)), ("east-edge", (9, 0)), ("centre", (0, 0))] {
        sim.entities[ai].pos_x = tcx + I16F16::from_num(off.0);
        sim.entities[ai].pos_y = tcy + I16F16::from_num(off.1);
        let to = (tcx + I16F16::from_num(3), tcy);
        let ok = sim.set_move_order(ai, to.0, to.1);
        sim.entities[ai].order = None;
        eprintln!("ENGAGEPROBE pathing from-inside {name}: set_move_order={ok}");
    }
    let _ = OWNER_PLAYER;
}

// ---------------------------------------------------------------------
// Setup E2 — the crowd freeze, discriminated. The r=3 ring of
// eight villagers around a disc foe froze the Scout at
// min_dist 4.12 with the attack target HELD for the whole
// 3,600-tick window (no damage either way, TC draining). The
// variants below measure: persistence over the full live
// window (14,400 ticks) with a state trace; ring density;
// a gap on the approach side; the realistic TC-centre cluster
// (foe at its real settled disc position south of centre,
// villagers clustered at the dropoff centre); and the same
// ring on open ground (generality).
// ---------------------------------------------------------------------

/// Stage: foe Scout at `foe_off` draining the TC by command;
/// villagers at the given absolute TC offsets; attacker Scout
/// spawned at `atk_off`, set Aggressive, ordered Attack.
fn crowd_scene(
    tag: &str,
    foe_off: (i32, i32),
    villagers: &[(f64, f64)],
    atk_off: (i32, i32),
    window: u64,
    trace: bool,
) -> Engage {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(foe_off.0),
        tcy + I16F16::from_num(foe_off.1),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for (i, (dx, dy)) in villagers.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    let draining = run_until(&mut sim, 5_000, |s| ent(s, 0).hp < I16F16::from_num(2400));
    assert!(draining, "{tag}: staging broken — foe never drained the TC");
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
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    // Measured window with an optional state trace.
    let tc_hp0 = f(ent(&sim, 0).hp);
    let mut m = Engage {
        min_dist: f64::MAX,
        ..Default::default()
    };
    for t in 0..window {
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
        let d = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        if a.attack_target == Some(FOE) {
            m.held_ticks += 1;
        } else if m.idle_tick.is_none() && !fo.dead && !a.dead {
            m.idle_tick = Some(t);
        }
        if trace && t % 600 == 0 {
            eprintln!(
                "ENGAGEPROBE trace {tag} t={t} atk_off=({:.1},{:.1}) dist={:.2} target={:?} order={} bump_wait={:?} blocked_ticks={:?} foe_hp={:.0}",
                f(a.pos_x - tcx),
                f(a.pos_y - tcy),
                d,
                a.attack_target,
                a.order.is_some(),
                a.order.as_ref().map(|o| o.bump_wait),
                a.order.as_ref().map(|o| o.blocked_ticks),
                f(fo.hp),
            );
        }
        if fo.dead {
            m.foe_dead_tick = Some(t);
            break;
        }
        if a.dead {
            break;
        }
    }
    let a = ent(&sim, ATK);
    let fo = ent(&sim, FOE);
    m.foe_hp_end = f(fo.hp);
    m.atk_hp_end = f(a.hp);
    m.atk_end_off = (f(a.pos_x - tcx), f(a.pos_y - tcy));
    m.foe_end_off = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    m.tc_hp_delta = f(ent(&sim, 0).hp) - tc_hp0;
    eprintln!("ENGAGEPROBE {}", report(tag, &m));
    m
}

fn ring_offsets(foe_off: (f64, f64), r: f64, skip: &[usize]) -> Vec<(f64, f64)> {
    let base: [(f64, f64); 8] = [
        (1.0, 0.0),
        (0.7071, 0.7071),
        (0.0, 1.0),
        (-0.7071, 0.7071),
        (-1.0, 0.0),
        (-0.7071, -0.7071),
        (0.0, -1.0),
        (0.7071, -0.7071),
    ];
    base.iter()
        .enumerate()
        .filter(|(i, _)| !skip.contains(i))
        .map(|(_, (ux, uy))| (foe_off.0 + ux * r, foe_off.1 + uy * r))
        .collect()
}

/// The full live window with trace: does the ring freeze
/// persist 14,400 ticks, and in what state does the attacker
/// sit (live order held? bump wait? order present)?
#[test]
fn crowd_ring_freeze_persists_full_live_window() {
    let vils = ring_offsets((3.0, 0.0), 3.0, &[]);
    let m = crowd_scene("crowd-ring-14400", (3, 0), &vils, (-16, 0), 14_400, true);
    assert!(
        engaged(&m, 45.0),
        "ring freeze persisted the full live window: {}",
        report("crowd-ring-14400", &m)
    );
}

/// Ring with the two WEST bodies removed (the approach side
/// open — the attacker comes from the west).
#[test]
fn crowd_ring_gap_on_approach_side() {
    // base indices: 3 = NW, 4 = W, 5 = SW — remove W and SW.
    let vils = ring_offsets((3.0, 0.0), 3.0, &[4, 5]);
    let m = crowd_scene("crowd-ring-gap-west", (3, 0), &vils, (-16, 0), 3_600, true);
    assert!(
        engaged(&m, 45.0),
        "ring with a west gap still stranded the scout: {}",
        report("crowd-ring-gap-west", &m)
    );
}

/// Only the four WEST-semicircle bodies (the approach lane
/// blocked, the foe's far side open).
#[test]
fn crowd_ring_west_semicircle_only() {
    let vils = ring_offsets((3.0, 0.0), 3.0, &[0, 1, 7]);
    let m = crowd_scene("crowd-ring-west-half", (3, 0), &vils, (-16, 0), 3_600, true);
    assert!(
        engaged(&m, 45.0),
        "west-semicircle bodies stranded the scout: {}",
        report("crowd-ring-west-half", &m)
    );
}

/// The realistic live TC crowd: the foe at its real settled
/// disc position (south of centre, as measured in Setup A:
/// off (0.7,−2.9)), six villagers clustered at the dropoff
/// centre around (0,0); attacker from the west and from the
/// south in two stagings.
#[test]
fn crowd_centre_cluster_with_settled_foe() {
    let cluster: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (2.0, 0.0),
        (-2.0, 0.0),
        (0.0, 2.0),
        (2.0, 2.0),
        (-2.0, -2.0),
        (0.0, -2.0),
        (-2.0, 2.0),
    ];
    let mut failures: Vec<String> = Vec::new();
    for (name, atk_off) in [("west", (-16, 0)), ("south", (0, 16))] {
        let tag = format!("crowd-cluster-{name}");
        let m = crowd_scene(&tag, (1, -3), &cluster, atk_off, 3_600, false);
        if !engaged(&m, 45.0) {
            failures.push(report(&tag, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "centre-cluster crowd stranded the scout:\n{}",
        failures.join("\n")
    );
}

/// Generality: the same r=3 ring around a foe on OPEN ground
/// (no TC anywhere near) — is the freeze a TC phenomenon or a
/// general body-ring phenomenon?
#[test]
fn crowd_ring_on_open_ground() {
    let mut sim = Sim::new(42);
    // A clear spot near the map centre (amoveprobe's helper
    // shape, inlined).
    let (ox, oy) = {
        let g = sim.grid();
        let mut found = None;
        'outer: for cy in 40..80 {
            for cx in 40..80 {
                let ok = (0..7).all(|dy| {
                    (0..7).all(|dx| {
                        let (x, y) = (cx + dx, cy + dy);
                        !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                    })
                });
                if ok {
                    found = Some(g.cell_center(cx + 3, cy + 3));
                    break 'outer;
                }
            }
        }
        found.expect("seed 42: no clear spot")
    };
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, ox, oy);
    let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = Stance::StandGround;
    let ring: [(f64, f64); 8] = [
        (3.0, 0.0),
        (2.12, 2.12),
        (0.0, 3.0),
        (-2.12, 2.12),
        (-3.0, 0.0),
        (-2.12, -2.12),
        (0.0, -3.0),
        (2.12, -2.12),
    ];
    for (i, (dx, dy)) in ring.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            ox + I16F16::from_num(*dx),
            oy + I16F16::from_num(*dy),
        );
    }
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        ox - I16F16::from_num(24),
        oy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-open-ground", &m));
    assert!(
        engaged(&m, 45.0),
        "open-ground ring stranded the scout: {}",
        report("ring-open-ground", &m)
    );
}

/// Ablation: the full r=3 ring at the TC, but the player's
/// starting villagers and Scout are first Moved far away by
/// real commands — is the freeze the ring alone, or the ring
/// plus the TC-centre starting bodies' geometry?
#[test]
fn crowd_ring_tc_without_starting_bodies() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    // Move the starting units (villagers 1..=3, scout 4) far
    // south-west, out of the scene.
    for id in [1u32, 2, 3, 4] {
        sim.push_command(Command::Move {
            entity: id,
            x: tcx - I16F16::from_num(60),
            y: tcy + I16F16::from_num(60),
        });
    }
    let gone = run_until(&mut sim, 20_000, |s| {
        [1u32, 2, 3, 4].iter().all(|&id| ent(s, id).order.is_none())
    });
    assert!(gone, "ablation staging: starting units never left");
    let vils = ring_offsets((3.0, 0.0), 3.0, &[]);
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(3),
        tcy,
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for (i, (dx, dy)) in vils.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    let draining = run_until(&mut sim, 5_000, |s| ent(s, 0).hp < I16F16::from_num(2400));
    assert!(draining, "ablation: foe never drained the TC");
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-no-starters", &m));
    assert!(
        engaged(&m, 45.0),
        "ring alone at the TC (no starting bodies) stranded the scout: {}",
        report("ring-no-starters", &m)
    );
}

/// Cross 1: the full ring at the TC, but the foe is a
/// StandGround Scout NOT attacking the TC (pinned in place).
/// If this engages, the foe's TC-engagement is part of the
/// freeze; if it freezes, the ring geometry at the TC is
/// sufficient on its own.
#[test]
fn crowd_ring_tc_standground_foe() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    for id in [1u32, 2, 3, 4] {
        sim.push_command(Command::Move {
            entity: id,
            x: tcx - I16F16::from_num(60),
            y: tcy + I16F16::from_num(60),
        });
    }
    assert!(run_until(&mut sim, 20_000, |s| {
        [1u32, 2, 3, 4].iter().all(|&id| ent(s, id).order.is_none())
    }));
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(3),
        tcy,
    );
    let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = Stance::StandGround;
    let vils = ring_offsets((3.0, 0.0), 3.0, &[]);
    for (i, (dx, dy)) in vils.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-tc-standground-foe", &m));
    for i in 0..8u32 {
        let v = ent(&sim, 9_910 + i);
        eprintln!(
            "ENGAGEPROBE ring-villager {i} hp={:.0} dead={} off=({:.1},{:.1})",
            f(v.hp), v.dead, f(v.pos_x - tcx), f(v.pos_y - tcy)
        );
    }
    assert!(
        engaged(&m, 45.0),
        "ring at TC stranded the scout even against a stand-ground foe: {}",
        report("ring-tc-standground-foe", &m)
    );
}

/// Cross 2: the full ring on open ground around a Scout foe
/// (the live kind), StandGround — the open-ground Militia
/// scene engaged; does the kind change it?
#[test]
fn crowd_ring_open_ground_scout_foe() {
    let mut sim = Sim::new(42);
    let (ox, oy) = {
        let g = sim.grid();
        let mut found = None;
        'outer: for cy in 40..80 {
            for cx in 40..80 {
                let ok = (0..7).all(|dy| {
                    (0..7).all(|dx| {
                        let (x, y) = (cx + dx, cy + dy);
                        !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                    })
                });
                if ok {
                    found = Some(g.cell_center(cx + 3, cy + 3));
                    break 'outer;
                }
            }
        }
        found.expect("seed 42: no clear spot")
    };
    spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, ox, oy);
    let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = Stance::StandGround;
    let ring: [(f64, f64); 8] = [
        (3.0, 0.0),
        (2.12, 2.12),
        (0.0, 3.0),
        (-2.12, 2.12),
        (-3.0, 0.0),
        (-2.12, -2.12),
        (0.0, -3.0),
        (2.12, -2.12),
    ];
    for (i, (dx, dy)) in ring.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            ox + I16F16::from_num(*dx),
            oy + I16F16::from_num(*dy),
        );
    }
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        ox - I16F16::from_num(24),
        oy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-open-scout", &m));
    assert!(
        engaged(&m, 45.0),
        "open-ground ring stranded the scout against a scout foe: {}",
        report("ring-open-scout", &m)
    );
}

/// Isolation E: StandGround foe at the TC ring, with ONLY
/// its `suppress_separation` flag set (the flag an in-range
/// melee attacker carries — set here as foe scene state, not
/// on the path under test). Does the flag alone freeze the
/// attacker?
#[test]
fn crowd_ring_tc_foe_suppress_flag_only() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    for id in [1u32, 2, 3, 4] {
        sim.push_command(Command::Move {
            entity: id,
            x: tcx - I16F16::from_num(60),
            y: tcy + I16F16::from_num(60),
        });
    }
    assert!(run_until(&mut sim, 20_000, |s| {
        [1u32, 2, 3, 4].iter().all(|&id| ent(s, id).order.is_none())
    }));
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(3),
        tcy,
    );
    let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = Stance::StandGround;
    sim.entities[fi].suppress_separation = true;
    let vils = ring_offsets((3.0, 0.0), 3.0, &[]);
    for (i, (dx, dy)) in vils.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-tc-foe-suppress", &m));
    assert!(
        engaged(&m, 45.0),
        "suppress-flag foe stranded the scout: {}",
        report("ring-tc-foe-suppress", &m)
    );
}

/// Isolation F: the foe is ENGAGED in combat (real Attack
/// command) — but on a sheep standing at its feet, not on
/// the TC. Ring at the TC as before. If this freezes,
/// "foe engaged" is the ingredient; if it engages, the
/// TC-target engagement specifically is.
#[test]
fn crowd_ring_tc_foe_engaged_on_sheep() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    for id in [1u32, 2, 3, 4] {
        sim.push_command(Command::Move {
            entity: id,
            x: tcx - I16F16::from_num(60),
            y: tcy + I16F16::from_num(60),
        });
    }
    assert!(run_until(&mut sim, 20_000, |s| {
        [1u32, 2, 3, 4].iter().all(|&id| ent(s, id).order.is_none())
    }));
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(3),
        tcy,
    );
    // A sheep at the foe's feet (foe + (1,0)) for it to fight.
    spawn(
        &mut sim,
        9_930,
        EntityKind::Sheep,
        OWNER_PLAYER,
        tcx + I16F16::from_num(4),
        tcy,
    );
    sim.push_command(Command::Attack { entity: FOE, target: 9_930 });
    // Let the foe settle into its engagement (sheep has 100
    // HP in this roster's terms — it survives a while; the
    // foe's engaged state, not the kill, is the staging).
    for _ in 0..120 {
        sim.tick();
    }
    let vils = ring_offsets((3.0, 0.0), 3.0, &[]);
    for (i, (dx, dy)) in vils.iter().enumerate() {
        // Skip the ring slot the sheep occupies (east, index 0
        // sits at foe+(3,0) = tc+(6,0); the sheep is at
        // tc+(4,0), a different point — keep all slots).
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-tc-foe-on-sheep", &m));
    assert!(
        engaged(&m, 45.0),
        "sheep-engaged foe stranded the scout: {}",
        report("ring-tc-foe-on-sheep", &m)
    );
}

/// Isolation H: foe attacking the TC and ALREADY draining,
/// ring spawned only afterwards, attacker ordered
/// immediately. Separates "foe engaged on the TC" from
/// "ring present while the foe's first strikes land".
#[test]
fn crowd_ring_spawned_after_foe_draining() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    for id in [1u32, 2, 3, 4] {
        sim.push_command(Command::Move {
            entity: id,
            x: tcx - I16F16::from_num(60),
            y: tcy + I16F16::from_num(60),
        });
    }
    assert!(run_until(&mut sim, 20_000, |s| {
        [1u32, 2, 3, 4].iter().all(|&id| ent(s, id).order.is_none())
    }));
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(3),
        tcy,
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    let draining = run_until(&mut sim, 5_000, |s| ent(s, 0).hp < I16F16::from_num(2400));
    assert!(draining, "H: foe never drained the TC");
    // The foe is verifiably engaged on the TC. NOW the ring.
    let vils = ring_offsets((3.0, 0.0), 3.0, &[]);
    for (i, (dx, dy)) in vils.iter().enumerate() {
        spawn(
            &mut sim,
            9_910 + i as u32,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(*dx),
            tcy + I16F16::from_num(*dy),
        );
    }
    spawn(
        &mut sim,
        ATK,
        EntityKind::ScoutCavalry,
        OWNER_PLAYER,
        tcx - I16F16::from_num(16),
        tcy,
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let m = measure(&mut sim, ATK, FOE, 3_600);
    eprintln!("ENGAGEPROBE {}", report("ring-after-draining", &m));
    assert!(
        engaged(&m, 45.0),
        "ring spawned after draining still stranded the scout: {}",
        report("ring-after-draining", &m)
    );
}
