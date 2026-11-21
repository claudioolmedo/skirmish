//! ENGAGEEXEC (2026-11-21): staging matrix for the
//! hidden-attacker EXECUTION layer, per handoff
//! `engageexec-build.md` §4 — the post-engagepersist form:
//! right-click attack returns `ok:true`, the Militia walks up
//! next to an enemy under/behind the Town Center and stays
//! IDLE — combat never starts — while the TC drains.
//!
//! All orders on the path under test are real `Command`s
//! (the foe's Attack on the TC; the attacker's SetStance +
//! Attack). Spawn positions/kinds and placed building records
//! are scene selection only (the engagepersist idiom: a placed
//! record + its dynamic grid cells are exactly the two effects
//! a completed building has in this sim).
//!
//! Scenes (§4):
//!   1. Approach-side sweep, foe settled in the TC disc
//!      draining it — expected: all engage (controls).
//!   2. Control A: foe standing 1 cell clear of the TC
//!      footprint edge, each side — expected: engages.
//!   3. Control B: Attack on the TC entity itself from each
//!      side — expected: TC drains.
//!   4. DISCRIMINANT: foe settled in the disc, then a placed
//!      PalisadeWall footprint is completed OVER the foe's own
//!      cell so its exact cell becomes grid-blocked while the
//!      foe keeps draining; Militia ordered from each side.
//!      §3 capture recorded at the idle moment and at window
//!      end. Branch B predicts arrive-and-idle pre-fix
//!      (attack_target retained, dist > MELEE_REACH, foe cell
//!      blocked/covered, fallback stand point out of reach)
//!      and engagement post-fix.
//!   5. Scene 4 + the player's own villagers standing at the
//!      TC dropoff centre (press-band interaction).
//!
//! OUTCOME (measured on base 8c912d0, see handoff
//! `engageexec2-build.md`): NO branch of §3 confirms.
//!   - Not A: `attack_target` never dropped in any scene
//!     (drop=None everywhere; retained at every capture).
//!   - Not B: the discriminant's precondition is unreachable
//!     in this sim. A foe settled at off (0, 2.93), cell
//!     (25, 82), verifiably draining the TC, has its cell
//!     blocked by a completed PalisadeWall — and on the VERY
//!     NEXT TICK the per-tick grid-integrity pass
//!     (`Sim::displace_out_of_blocked`, nearest open cell
//!     Chebyshev r<=8) teleports it to the centre of cell
//!     (24, 81), off (-4.00, 0.00), move order cleared,
//!     `attack_target` (the TC) retained; it then walks back
//!     toward the TC. At every §3 capture the foe therefore
//!     stands on a FREE cell (`foe_cell_blocked=false`,
//!     covering building None), the exact chase path succeeds,
//!     `chase_retry_tick` is 0 (the fallback never installs),
//!     and the Militia engages from all four sides (first
//!     damage t=376..429, foe dead t=1696..1749). A live foe
//!     can never persist standing on a blocked cell while
//!     draining, so the §2.3 fallback-hold loop cannot be
//!     entered by this geometry.
//!   - Not C: the in-range captures (dist 2.94..2.99 <=
//!     MELEE_REACH) are the ordinary fighting stand — strikes
//!     land and the foe dies in every scene.
//! Per the package discipline, NO production fix is made:
//! this suite + its captures are the deliverable.

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

const FOE: u32 = 9_801;
const ATK: u32 = 9_802;

/// engagepersist's `placed` idiom: a completed building record
/// at anchor cell `(cx, cy)` plus the dynamic grid block its
/// footprint carries in the live sim.
fn place_record(sim: &mut Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) {
    let size = kind.footprint().w;
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    let b = Building {
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
    };
    if kind.blocks_footprint() {
        for (cx, cy) in b.footprint_cells() {
            sim.grid.set_dynamic(cx, cy, true);
        }
    }
    sim.buildings.push(b);
}

/// The §3 capture: everything the three-branch discriminant
/// needs, taken at one instant.
#[derive(Debug, Clone)]
struct Capture {
    tick: u64,
    attack_target: Option<u32>,
    order_target: Option<(f64, f64)>,
    chase_retry_tick: u64,
    dist: f64,
    atk_contained: bool,
    foe_contained: bool,
    foe_cell: (u32, u32),
    foe_cell_blocked: bool,
    covering: Option<u32>,
    foe_hp: f64,
    tc_hp: f64,
}

fn capture(sim: &Sim) -> Capture {
    let a = ent(sim, ATK);
    let fo = ent(sim, FOE);
    let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    let covering = sim
        .buildings
        .iter()
        .find(|b| b.footprint_cells().contains(&cell))
        .map(|b| b.id);
    Capture {
        tick: sim.tick,
        attack_target: a.attack_target,
        order_target: a.order.as_ref().map(|o| (f(o.target_x), f(o.target_y))),
        chase_retry_tick: a.chase_retry_tick,
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_contained: a.is_contained(),
        foe_contained: fo.is_contained(),
        foe_cell: cell,
        foe_cell_blocked: sim.grid.is_blocked(cell.0, cell.1),
        covering,
        foe_hp: f(fo.hp),
        tc_hp: f(ent(sim, 0).hp),
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "ENGAGEEXEC CAPTURE {tag} {when}: tick={} attack_target={:?} order_target={:?} chase_retry_tick={} dist={:.2} atk_contained={} foe_contained={} foe_cell={:?} foe_cell_blocked={} covering_building={:?} foe_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.order_target, c.chase_retry_tick, c.dist,
        c.atk_contained, c.foe_contained, c.foe_cell, c.foe_cell_blocked,
        c.covering, c.foe_hp, c.tc_hp
    );
}

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    max_travel: f64,
    min_dist: f64,
    idle: Option<Capture>,
    end: Option<Capture>,
}

/// Foe Spearman walks in from 20 wu south of the TC centre and
/// attacks the TC by real command; settles 900 ticks (ends in
/// the r<=3 disc, TC verifiably draining — engagepersist
/// idiom). `stage` then applies the scene's placed objects.
/// Attacker Militia spawns at `atk_off` (TC-relative), is set
/// Aggressive and ordered Attack by real commands.
fn disc_scene(
    tag: &str,
    stage: &mut dyn FnMut(&mut Sim),
    atk_off: (f64, f64),
    window: u64,
) -> Trace {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::Spearman,
        OWNER_OPPONENT,
        tcx,
        tcy + I16F16::from_num(20),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 {
        sim.tick();
    }
    let foe = ent(&sim, FOE);
    let d = f(steer::vec_len(foe.pos_x - tcx, foe.pos_y - tcy));
    assert!(d <= 3.5, "{tag}: foe did not settle in the TC disc: dist {d:.2}");
    assert!(
        f(ent(&sim, 0).hp) < 2400.0,
        "{tag}: foe settled but the TC is not draining"
    );
    eprintln!(
        "ENGAGEEXEC {tag} foe settled off=({:.2},{:.2}) dist_to_tc={:.2} cell={:?}",
        f(foe.pos_x - tcx),
        f(foe.pos_y - tcy),
        d,
        sim.grid.world_to_cell(foe.pos_x, foe.pos_y)
    );
    stage(&mut sim);
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        tcx + I16F16::from_num(atk_off.0),
        tcy + I16F16::from_num(atk_off.1),
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let (ax0, ay0) = (ent(&sim, ATK).pos_x, ent(&sim, ATK).pos_y);
    let mut tr = Trace {
        min_dist: f64::MAX,
        ..Default::default()
    };
    for t in 0..window {
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
        let travel = f(steer::vec_len(a.pos_x - ax0, a.pos_y - ay0));
        if travel > tr.max_travel {
            tr.max_travel = travel;
        }
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        if dist < tr.min_dist {
            tr.min_dist = dist;
        }
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            tr.first_dmg = Some(t);
        }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
        }
        // The idle moment: the attacker holds no move order,
        // the foe lives, and no damage has been dealt yet —
        // the arrival state §3 discriminates on.
        if tr.idle.is_none()
            && t >= 30
            && a.order.is_none()
            && !fo.dead
            && tr.first_dmg.is_none()
        {
            let c = capture(&sim);
            print_capture(tag, "idle-moment", &c);
            tr.idle = Some(c);
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
    }
    let c = capture(&sim);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "ENGAGEEXEC {tag} first_dmg={:?} drop={:?} dead={:?} max_travel={:.1} min_dist={:.2}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.max_travel, tr.min_dist
    );
    tr
}

fn assert_engages(tag: &str, tr: &Trace) {
    assert!(
        tr.drop_tick.is_none(),
        "{tag}: attack target dropped at {:?} while foe alive: {tr:?}",
        tr.drop_tick
    );
    assert!(
        tr.first_dmg.is_some(),
        "{tag}: foe never took damage: {tr:?}"
    );
    assert!(tr.dead_tick.is_some(), "{tag}: foe never died: {tr:?}");
}

/// Scene 1 — approach-side sweep, foe settled in the TC disc.
#[test]
fn scene1_approach_side_sweep_engages() {
    let mut failures = Vec::new();
    for (name, off) in [
        ("N", (0.0, -20.0)),
        ("S", (0.0, 20.0)),
        ("E", (20.0, 0.0)),
        ("W", (-20.0, 0.0)),
        ("NE", (20.0, -20.0)),
        ("SW", (-20.0, 20.0)),
    ] {
        let tag = format!("scene1-{name}");
        let tr = disc_scene(&tag, &mut |_| {}, off, 7_200);
        if tr.drop_tick.is_some() || tr.first_dmg.is_none() || tr.dead_tick.is_none() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "scene1 failures:\n{}", failures.join("\n"));
}

/// Scene 2 — Control A: foe standing 1 cell (4 wu) clear of
/// the TC footprint edge (footprint spans tc-6..tc+10 per
/// axis), each side; Militia ordered in from beyond it.
#[test]
fn scene2_control_foe_one_cell_outside_footprint_engages() {
    // (foe offset, attacker offset) per side.
    let sides: &[(&str, (i32, i32), (i32, i32))] = &[
        ("E", (14, 0), (26, 0)),
        ("W", (-10, 0), (-22, 0)),
        ("N", (0, -10), (0, -22)),
        ("S", (0, 14), (0, 26)),
    ];
    let mut failures = Vec::new();
    for (name, foe_off, atk_off) in sides {
        let tag = format!("scene2-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(
            &mut sim,
            FOE,
            EntityKind::Spearman,
            OWNER_OPPONENT,
            tcx + I16F16::from_num(foe_off.0),
            tcy + I16F16::from_num(foe_off.1),
        );
        let fi = sim.entities.iter().position(|e| e.id == FOE).unwrap();
        sim.entities[fi].stance = Stance::StandGround;
        spawn(
            &mut sim,
            ATK,
            EntityKind::Militia,
            OWNER_PLAYER,
            tcx + I16F16::from_num(atk_off.0),
            tcy + I16F16::from_num(atk_off.1),
        );
        sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
        sim.tick();
        assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused");
        sim.push_command(Command::Attack { entity: ATK, target: FOE });
        let foe_hp0 = f(ent(&sim, FOE).hp);
        let mut tr = Trace {
            min_dist: f64::MAX,
            ..Default::default()
        };
        for t in 0..3_600u64 {
            sim.tick();
            let a = ent(&sim, ATK);
            let fo = ent(&sim, FOE);
            let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
            if dist < tr.min_dist {
                tr.min_dist = dist;
            }
            if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
                tr.first_dmg = Some(t);
            }
            if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
                tr.drop_tick = Some(t);
            }
            if fo.dead {
                tr.dead_tick = Some(t);
                break;
            }
        }
        eprintln!(
            "ENGAGEEXEC {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2}",
            tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist
        );
        if tr.drop_tick.is_some() || tr.first_dmg.is_none() || tr.dead_tick.is_none() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "scene2 failures:\n{}", failures.join("\n"));
}

/// Scene 3 — Control B: a player Militia spawned in the
/// opponent TC's disc, one staging per side, ordered Attack
/// on the TC entity itself (the foe's own working
/// configuration, mirrored — engageprobe's control 2).
#[test]
fn scene3_control_attack_on_tc_entity_drains_it() {
    let mut failures = Vec::new();
    for (name, off) in [("E", (3, 0)), ("W", (-3, 0)), ("N", (0, -3)), ("S", (0, 3))] {
        let tag = format!("scene3-{name}");
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
            otc.1 + I16F16::from_num(off.0),
            otc.2 + I16F16::from_num(off.1),
        );
        let hp0 = f(ent(&sim, otc.0).hp);
        sim.push_command(Command::Attack { entity: ATK, target: otc.0 });
        let mut first_dmg = None;
        for t in 0..3_600u64 {
            sim.tick();
            if f(ent(&sim, otc.0).hp) < hp0 {
                first_dmg = Some(t);
                break;
            }
        }
        eprintln!("ENGAGEEXEC {tag} first_tc_dmg={first_dmg:?} tc_hp={:.0}", f(ent(&sim, otc.0).hp));
        if first_dmg.is_none() {
            failures.push(format!("{tag}: TC never drained"));
        }
    }
    assert!(failures.is_empty(), "scene3 failures:\n{}", failures.join("\n"));
}

/// Scene 4's staging: a PalisadeWall completed over the foe's
/// own cell — its exact cell becomes grid-blocked (record +
/// dynamic cell) while the foe, in reach of the TC centre,
/// keeps draining it.
fn block_foe_cell(sim: &mut Sim) {
    let fo = ent(sim, FOE);
    let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    place_record(sim, 9_950, BuildingKind::PalisadeWall, OWNER_PLAYER, cell);
    assert!(
        sim.grid.is_blocked(cell.0, cell.1),
        "scene4 staging broken: foe cell {cell:?} not blocked after placement"
    );
}

/// Scene 4 — THE DISCRIMINANT. Post-fix expectation (the DE
/// behaviour): the Militia reaches a stand point within
/// MELEE_REACH of the foe and the foe takes damage, from
/// every approach side.
#[test]
fn scene4_blocked_foe_cell_discriminant() {
    let mut failures = Vec::new();
    for (name, off) in [
        ("N", (0.0, -24.0)),
        ("S", (0.0, 24.0)),
        ("E", (24.0, 0.0)),
        ("W", (-24.0, 0.0)),
    ] {
        let tag = format!("scene4-{name}");
        let tr = disc_scene(&tag, &mut block_foe_cell, off, 7_200);
        if tr.first_dmg.is_none() {
            failures.push(format!("{tag}: foe never took damage: {tr:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "scene4: blocked-cell foe never engaged:\n{}",
        failures.join("\n")
    );
}

/// Diagnostic (pins the measured mechanism): what happens to
/// a settled, TC-draining foe on the tick after its own cell
/// is blocked by a completed footprint. Measured on the
/// pristine tree: the per-tick grid-integrity pass
/// (`displace_out_of_blocked`) teleports the foe to the centre
/// of the nearest open cell (Chebyshev r<=8) on the very next
/// tick — a live foe can therefore never persist standing on
/// a blocked cell, and the Branch-B precondition of
/// `engageexec-build.md` §3 (foe's exact cell blocked while it
/// keeps draining from that cell) cannot be staged with a
/// §4-sized footprint. The foe's TC attack then re-chases the
/// TC from its displaced cell.
#[test]
fn scene4_probe_foe_displaced_off_blocked_cell() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::Spearman,
        OWNER_OPPONENT,
        tcx,
        tcy + I16F16::from_num(20),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 {
        sim.tick();
    }
    let fo = ent(&sim, FOE);
    let cell0 = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    eprintln!(
        "ENGAGEEXEC probe settled foe off=({:.2},{:.2}) cell={cell0:?}",
        f(fo.pos_x - tcx),
        f(fo.pos_y - tcy)
    );
    block_foe_cell(&mut sim);
    assert!(sim.grid.is_blocked(cell0.0, cell0.1));
    let mut left_at = None;
    for t in 0..10u64 {
        sim.tick();
        let fo = ent(&sim, FOE);
        let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
        eprintln!(
            "ENGAGEEXEC probe t+{} foe off=({:.2},{:.2}) cell={cell:?} cell_blocked={} attack_target={:?}",
            t + 1,
            f(fo.pos_x - tcx),
            f(fo.pos_y - tcy),
            sim.grid.is_blocked(cell.0, cell.1),
            fo.attack_target
        );
        if cell != cell0 && left_at.is_none() {
            left_at = Some(t + 1);
        }
    }
    assert!(
        left_at.is_some(),
        "foe still standing on its blocked cell 10 ticks after placement"
    );
}

/// Scene 5 — scene 4 plus the player's own villagers standing
/// at the TC dropoff centre.
#[test]
fn scene5_blocked_foe_cell_with_dropoff_crowd() {
    let mut stage = |sim: &mut Sim| {
        block_foe_cell(sim);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        for (i, (dx, dy)) in [(0.0, 0.0), (2.0, 0.0), (-2.0, 0.0), (0.0, 2.0)]
            .iter()
            .enumerate()
        {
            spawn(
                sim,
                9_960 + i as u32,
                EntityKind::Villager,
                OWNER_PLAYER,
                tcx + I16F16::from_num(*dx),
                tcy + I16F16::from_num(*dy),
            );
        }
    };
    let mut failures = Vec::new();
    for (name, off) in [("S", (0.0, 24.0)), ("W", (-24.0, 0.0))] {
        let tag = format!("scene5-{name}");
        let tr = disc_scene(&tag, &mut stage, off, 7_200);
        if tr.first_dmg.is_none() {
            failures.push(format!("{tag}: foe never took damage: {tr:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "scene5: blocked-cell foe with dropoff crowd never engaged:\n{}",
        failures.join("\n")
    );
}
