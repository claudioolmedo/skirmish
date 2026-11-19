//! RETPROBE (2026-11-18): diagnosis probe for the sibling
//! defect the amoveprobe handoff flagged in its "Honesty
//! bounds" — retaliation in `apply_hit` (combat.rs) matches
//! only `Militia | Archer`, so an idle military unit of any
//! other kind that is struck or shot may never answer its
//! attacker.
//!
//! The rule probed against is CMB-09 (docs/MECHANICS-RULEBOOK.md):
//! "Retaliation is gated the same way (No Attack never answers;
//! Stand Ground answers only an in-range attacker)" — stated
//! for military units generally, with no kind carve-out, and
//! the entry's own unmodelled remainder is a Defensive flee
//! clause, not a kind list. The stance defaults it documents
//! ("military units Aggressive") likewise key on
//! `EntityKind::is_military`, whose land roster is the nine
//! kinds swept here. Siege (Mangonel/Scorpion/BatteringRam),
//! the Petard and the ships are NOT probed as victims: CMB-09
//! documents no retaliation for them (CMB-08 gives rams their
//! own buildings-only clause), so they stay outside the fix
//! scope whatever this probe finds.
//!
//! Isolation — the probe must measure RETALIATION
//! (`apply_hit` setting `attack_target`), never CMB-08
//! acquisition, which sets the same field for idle units
//! within LoS. Two independent isolations:
//!
//!   Struck scenes: the attacker is an opponent VILLAGER and
//!   the victim's diplomacy row toward the opponent is set
//!   Neutral (unlocked teams). `can_auto_acquire` under a
//!   Neutral row qualifies military targets only, so the
//!   victim can NEVER auto-acquire the villager — yet a
//!   villager's explicit Attack order still executes (the
//!   opponent's own row stays Enemy; diplomacy_tests (c)).
//!   If the victim's `attack_target` ever becomes the
//!   villager, retaliation put it there. Self-check: the
//!   target must never appear before the victim's first
//!   damage (a participating hit always deals >= 1 through
//!   the CMB-13 floor, so retaliation and first damage are
//!   simultaneous; acquisition would precede damage).
//!
//!   Shot scenes: the attacker is an opponent Scorpion
//!   ordered to Attack the victim from BEYOND the victim's
//!   LoS (Scorpion band 8..28 wu; staged at 24 wu, or 27 wu
//!   for the LoS-24 kinds), so acquisition's radius check
//!   can never reach it. The Light Cavalry (LoS 32) out-sees
//!   every ranged attacker's band, so no shot scene can
//!   isolate it — it is covered by the struck scenes only
//!   (honesty bound, recorded in the handoff). A Mangonel
//!   was the first shooter tried and was rejected by
//!   measurement: one stone deals ~55 damage, killing every
//!   victim kind but the Knight on the first impact, so
//!   retaliation is unobservable — the Knight keeps a
//!   dedicated Mangonel scene (it survives the first stone
//!   and must answer before the second arrives).
//!
//! Every test asserts the CMB-09-expected outcome and reports
//! the measured numbers in its assert messages: ticks with
//! the attacker held as retaliation target (and whether the
//! target was marked `attack_auto`), attacker/victim HP at
//! the end, and the isolation self-check. A red test here IS
//! the diagnosis.

use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, MatchMode, Sim, Stance, TeamSettings, VictorySettings, STANCE_NEUTRAL};
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

fn set_stance(sim: &mut Sim, id: u32, stance: Stance) {
    let e = sim.entities.iter_mut().find(|e| e.id == id).unwrap();
    e.stance = stance;
}

/// An unlocked-teams sim (diplomacy_tests' shape) with the
/// PLAYER's row toward the opponent set to Neutral — the
/// struck scenes' isolation switch.
fn neutral_row_sim() -> Sim {
    let mut sim = Sim::with_settings_teams(
        42,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings {
            teams: [1, 2],
            locked_teams: false,
        },
    );
    assert!(sim.can_set_diplomacy(OWNER_PLAYER, OWNER_OPPONENT, STANCE_NEUTRAL));
    sim.push_command(Command::SetDiplomacy {
        player: OWNER_PLAYER,
        target: OWNER_OPPONENT,
        stance: STANCE_NEUTRAL,
    });
    sim.tick();
    assert_eq!(
        sim.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_NEUTRAL)
    );
    assert!(
        !sim.can_auto_acquire(OWNER_PLAYER, OWNER_OPPONENT, false),
        "isolation broken: a Neutral row must not auto-acquire a villager"
    );
    assert!(
        sim.can_auto_acquire(OWNER_PLAYER, OWNER_OPPONENT, true),
        "staging note: a Neutral row still auto-acquires military targets"
    );
    sim
}

/// A free 7x7 patch of ground far from both Town Centers
/// (diplomacy_tests' `open_patch`, verbatim shape).
fn open_patch(sim: &Sim) -> (u32, u32) {
    let tcs: Vec<(u32, u32)> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::TownCenter)
        .map(|e| sim.grid().world_to_cell(e.pos_x, e.pos_y))
        .collect();
    for cx in 8..104u32 {
        for cy in 8..104u32 {
            if tcs.iter().any(|&(tx, ty)| {
                (cx as i32 - tx as i32)
                    .abs()
                    .max((cy as i32 - ty as i32).abs())
                    < 24
            }) {
                continue;
            }
            let free = (0..7).all(|dx| (0..7).all(|dy| !sim.grid().is_blocked(cx + dx, cy + dy)));
            if free {
                return (cx + 3, cy + 3);
            }
        }
    }
    panic!("no open patch on seed 42");
}

/// A clear land cell near the map centre (amoveprobe's
/// `clear_land_spot`, verbatim shape) — the shot scenes stage
/// on open ground with plain `Sim::new`.
fn clear_land_spot(sim: &Sim) -> (u32, u32) {
    let g = sim.grid();
    for cy in 40..80 {
        for cx in 40..80 {
            let ok = (0..5).all(|dy| {
                (0..5).all(|dx| {
                    let (x, y) = (cx + dx, cy + dy);
                    !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                })
            });
            if ok {
                return (cx + 2, cy + 2);
            }
        }
    }
    panic!("seed 42: no clear land spot near the centre");
}

const VIC: u32 = 9_800;
const VIL: u32 = 9_801;
const MANGO: u32 = 9_802;
const SCORP: u32 = 9_803;

#[derive(Default, Debug)]
struct StruckMeasure {
    retaliation_ticks: u64,
    retaliation_auto: bool,
    target_before_first_damage: bool,
    vil_hp_end: f64,
    vil_dead: bool,
    vic_hp_end: f64,
    vic_hp_start: f64,
}

fn struck_report(kind: EntityKind, m: &StruckMeasure) -> String {
    format!(
        "kind={:?} retaliation_ticks={} retaliation_auto={} target_before_first_damage={} vil_hp_end={:.0} vil_dead={} vic_hp={:.0}->{:.0}",
        kind,
        m.retaliation_ticks,
        m.retaliation_auto,
        m.target_before_first_damage,
        m.vil_hp_end,
        m.vil_dead,
        m.vic_hp_start,
        m.vic_hp_end,
    )
}

/// One struck scene: an idle victim of `kind` (PLAYER, stance
/// `stance`) on an open patch, attacked in melee by an
/// opponent villager under an explicit Attack order, with the
/// player's diplomacy row Neutral (see the module doc for why
/// only retaliation can answer).
fn struck_scene(kind: EntityKind, stance: Stance, ticks: u64) -> StruckMeasure {
    let mut sim = neutral_row_sim();
    let c = open_patch(&sim);
    let (vx, vy) = sim.grid().cell_center(c.0, c.1);
    spawn(&mut sim, VIC, kind, OWNER_PLAYER, vx, vy);
    set_stance(&mut sim, VIC, stance);
    // The villager starts one cell east and walks to melee
    // reach under its explicit order.
    let (ax, ay) = sim.grid().cell_center(c.0 + 1, c.1);
    spawn(&mut sim, VIL, EntityKind::Villager, OWNER_OPPONENT, ax, ay);
    let vic_hp_start = f(ent(&sim, VIC).hp);
    sim.push_command(Command::Attack {
        entity: VIL,
        target: VIC,
    });
    let mut m = StruckMeasure {
        vic_hp_start,
        ..Default::default()
    };
    for _ in 0..ticks {
        sim.tick();
        let v = ent(&sim, VIC);
        if v.attack_target == Some(VIL) {
            m.retaliation_ticks += 1;
            if v.attack_auto {
                m.retaliation_auto = true;
            }
            if f(v.hp) >= vic_hp_start {
                m.target_before_first_damage = true;
            }
        }
    }
    let vil = ent(&sim, VIL);
    m.vil_hp_end = f(vil.hp);
    m.vil_dead = vil.dead;
    m.vic_hp_end = f(ent(&sim, VIC).hp);
    eprintln!("RETPROBE struck {}", struck_report(kind, &m));
    m
}

/// The struck sweep: every standard land military kind, idle
/// at Aggressive (CMB-09's documented default), struck by a
/// villager it cannot auto-acquire. CMB-09's retaliation is
/// documented for military units with no kind carve-out, so
/// every kind must answer: the villager takes damage.
#[test]
fn struck_idle_aggressive_all_land_military_kinds_retaliate() {
    let mut failures: Vec<String> = Vec::new();
    for kind in [
        EntityKind::Militia,
        EntityKind::ManAtArms,
        EntityKind::Archer,
        EntityKind::ScoutCavalry,
        EntityKind::Spearman,
        EntityKind::Skirmisher,
        EntityKind::LightCavalry,
        EntityKind::Knight,
        EntityKind::CavalryArcher,
    ] {
        let m = struck_scene(kind, Stance::Aggressive, 1800);
        assert!(
            !m.target_before_first_damage,
            "isolation broken for {:?}: the target appeared before any damage — acquisition, not retaliation, answered",
            kind
        );
        assert!(
            m.vic_hp_end < m.vic_hp_start,
            "staging broken for {:?}: the villager never landed a hit: {}",
            kind,
            struck_report(kind, &m)
        );
        if !(m.retaliation_ticks > 0 && m.retaliation_auto && (m.vil_dead || m.vil_hp_end < 25.0)) {
            failures.push(struck_report(kind, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "kinds that never retaliated when struck while idle (Aggressive):\n{}",
        failures.join("\n")
    );
}

/// CMB-09's stance gates on a kind the fix must cover: a
/// No Attack Knight never answers (gate intact, before and
/// after any fix), and a Stand Ground Knight answers an
/// in-range attacker (a melee villager's threat point is its
/// own position, inside MELEE_REACH when its hit lands).
#[test]
fn struck_knight_stance_gates_hold() {
    let m = struck_scene(EntityKind::Knight, Stance::NoAttack, 1200);
    assert!(
        m.retaliation_ticks == 0 && m.vil_hp_end >= 25.0 && !m.vil_dead,
        "CMB-09 violated: a No Attack Knight answered its attacker: {}",
        struck_report(EntityKind::Knight, &m)
    );
    assert!(
        m.vic_hp_end < m.vic_hp_start,
        "staging broken: the villager never landed a hit on the No Attack Knight: {}",
        struck_report(EntityKind::Knight, &m)
    );

    let m = struck_scene(EntityKind::Knight, Stance::StandGround, 1800);
    assert!(
        !m.target_before_first_damage,
        "isolation broken for the Stand Ground Knight: {}",
        struck_report(EntityKind::Knight, &m)
    );
    assert!(
        m.retaliation_ticks > 0 && (m.vil_dead || m.vil_hp_end < 25.0),
        "CMB-09 violated: a Stand Ground Knight did not answer an in-range attacker: {}",
        struck_report(EntityKind::Knight, &m)
    );
}

#[derive(Default, Debug)]
struct ShotMeasure {
    retaliation_ticks: u64,
    retaliation_auto: bool,
    target_before_first_damage: bool,
    shooter_hp_end: f64,
    shooter_hp_start: f64,
    shooter_dead: bool,
    vic_hp_end: f64,
    vic_hp_start: f64,
    min_dist: f64,
}

fn shot_report(kind: EntityKind, m: &ShotMeasure) -> String {
    format!(
        "kind={:?} retaliation_ticks={} retaliation_auto={} target_before_first_damage={} shooter_hp={:.0}->{:.0} shooter_dead={} vic_hp={:.0}->{:.0} min_dist={:.2}",
        kind,
        m.retaliation_ticks,
        m.retaliation_auto,
        m.target_before_first_damage,
        m.shooter_hp_start,
        m.shooter_hp_end,
        m.shooter_dead,
        m.vic_hp_start,
        m.vic_hp_end,
        m.min_dist,
    )
}

/// One shot scene: an idle Aggressive victim of `kind`
/// (PLAYER) shot at by an opponent Scorpion under an explicit
/// Attack order, the Scorpion staged `dist_wu` away — inside
/// its [8, 28] band and beyond the victim's LoS, so
/// acquisition cannot answer (module doc). The Scorpion's
/// bolt is an ordinary `apply_hit` (pass-through resolution),
/// ~11 pierce a bolt: every victim kind survives long enough
/// for retaliation to be observable.
fn shot_scene(kind: EntityKind, dist_wu: i32, ticks: u64) -> ShotMeasure {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let (vx, vy) = sim.grid().cell_center(cx, cy);
    spawn(&mut sim, VIC, kind, OWNER_PLAYER, vx, vy);
    set_stance(&mut sim, VIC, Stance::Aggressive);
    spawn(
        &mut sim,
        SCORP,
        EntityKind::Scorpion,
        OWNER_OPPONENT,
        vx,
        vy + I16F16::from_num(dist_wu),
    );
    let vic_hp_start = f(ent(&sim, VIC).hp);
    let shooter_hp_start = f(ent(&sim, SCORP).hp);
    sim.push_command(Command::Attack {
        entity: SCORP,
        target: VIC,
    });
    let mut m = ShotMeasure {
        vic_hp_start,
        shooter_hp_start,
        min_dist: f64::MAX,
        ..Default::default()
    };
    for _ in 0..ticks {
        sim.tick();
        let v = ent(&sim, VIC);
        let g = ent(&sim, SCORP);
        let d = f(steer::vec_len(g.pos_x - v.pos_x, g.pos_y - v.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        if v.attack_target == Some(SCORP) {
            m.retaliation_ticks += 1;
            if v.attack_auto {
                m.retaliation_auto = true;
            }
            if f(v.hp) >= vic_hp_start {
                m.target_before_first_damage = true;
            }
        }
        if g.dead {
            break;
        }
    }
    let g = ent(&sim, SCORP);
    m.shooter_hp_end = f(g.hp);
    m.shooter_dead = g.dead;
    m.vic_hp_end = f(ent(&sim, VIC).hp);
    eprintln!("RETPROBE shot {}", shot_report(kind, &m));
    m
}

/// The shot sweep: the eight kinds a Scorpion can out-range
/// (LoS <= 24). A victim that retaliates rides out toward
/// the Scorpion and damages or kills it; one that does not
/// stands in place and is shot down. The Light Cavalry
/// (LoS 32) cannot be isolated this way — see the module doc.
#[test]
fn shot_idle_aggressive_kinds_retaliate() {
    let mut failures: Vec<String> = Vec::new();
    for (kind, dist_wu) in [
        (EntityKind::Militia, 24),
        (EntityKind::ManAtArms, 24),
        (EntityKind::Archer, 27),
        (EntityKind::ScoutCavalry, 24),
        (EntityKind::Spearman, 24),
        (EntityKind::Skirmisher, 27),
        (EntityKind::Knight, 24),
        (EntityKind::CavalryArcher, 24),
    ] {
        let m = shot_scene(kind, dist_wu, 9000);
        assert!(
            !m.target_before_first_damage,
            "isolation broken for {:?}: the target appeared before any damage — acquisition, not retaliation, answered",
            kind
        );
        assert!(
            m.vic_hp_end < m.vic_hp_start,
            "staging broken for {:?}: the Scorpion never landed a bolt: {}",
            kind,
            shot_report(kind, &m)
        );
        let engaged = m.shooter_dead
            || m.shooter_hp_end < m.shooter_hp_start
            || m.min_dist <= dist_wu as f64 - 6.0;
        if !(m.retaliation_ticks > 0 && m.retaliation_auto && engaged) {
            failures.push(shot_report(kind, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "kinds that never retaliated when shot while idle (Aggressive):\n{}",
        failures.join("\n")
    );
}

/// The Mangonel scene (Knight only): the one victim kind
/// that survives a stone (~55 damage against 100 HP). The
/// Mangonel fires AttackGround on the Knight's starting
/// position from 24 wu — beyond the Knight's LoS (16), inside
/// its own [12, 28] band. A Knight that retaliates leaves
/// the impact point after the first stone and rides the
/// Mangonel down; one that does not is still standing there
/// when the second stone arrives.
#[test]
fn shot_knight_mangonel_retaliates() {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let (vx, vy) = sim.grid().cell_center(cx, cy);
    spawn(&mut sim, VIC, EntityKind::Knight, OWNER_PLAYER, vx, vy);
    set_stance(&mut sim, VIC, Stance::Aggressive);
    spawn(
        &mut sim,
        MANGO,
        EntityKind::Mangonel,
        OWNER_OPPONENT,
        vx,
        vy + I16F16::from_num(24),
    );
    let vic_hp_start = f(ent(&sim, VIC).hp);
    sim.push_command(Command::AttackGround {
        entity: MANGO,
        x: vx,
        y: vy,
    });
    let mut retaliation_ticks = 0u64;
    let mut retaliation_auto = false;
    let mut target_before_first_damage = false;
    let mut min_dist = f64::MAX;
    for _ in 0..3600 {
        sim.tick();
        let v = ent(&sim, VIC);
        let g = ent(&sim, MANGO);
        let d = f(steer::vec_len(g.pos_x - v.pos_x, g.pos_y - v.pos_y));
        if d < min_dist {
            min_dist = d;
        }
        if v.attack_target == Some(MANGO) {
            retaliation_ticks += 1;
            if v.attack_auto {
                retaliation_auto = true;
            }
            if f(v.hp) >= vic_hp_start {
                target_before_first_damage = true;
            }
        }
        if g.dead || v.dead {
            break;
        }
    }
    let g = ent(&sim, MANGO);
    let v = ent(&sim, VIC);
    eprintln!(
        "RETPROBE shot-mango kind=Knight retaliation_ticks={} retaliation_auto={} target_before_first_damage={} mango_hp_end={:.0} mango_dead={} vic_hp={:.0}->{:.0} vic_dead={} min_dist={:.2}",
        retaliation_ticks,
        retaliation_auto,
        target_before_first_damage,
        f(g.hp),
        g.dead,
        vic_hp_start,
        f(v.hp),
        v.dead,
        min_dist,
    );
    assert!(
        !target_before_first_damage,
        "isolation broken for the Knight: the target appeared before any damage"
    );
    assert!(
        f(v.hp) < vic_hp_start,
        "staging broken: the Mangonel never landed a stone on the Knight"
    );
    assert!(
        retaliation_ticks > 0 && retaliation_auto && (g.dead || f(g.hp) < 50.0 || min_dist < 10.0),
        "the Knight never retaliated against the Mangonel that shot it"
    );
}

/// CMB-09's Stand Ground gate on the shot path: a Stand
/// Ground Knight bombarded from 24 wu — far outside its
/// attack range — must NOT answer (the threat point is the
/// Mangonel's release position). This pins the gate the fix
/// must not widen: it passes before and after.
#[test]
fn shot_stand_ground_knight_does_not_answer_out_of_range() {
    // Staged by hand (not via shot_scene, which stages
    // Aggressive): the same bombardment, a Stand Ground victim.
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let (vx, vy) = sim.grid().cell_center(cx, cy);
    spawn(&mut sim, VIC, EntityKind::Knight, OWNER_PLAYER, vx, vy);
    set_stance(&mut sim, VIC, Stance::StandGround);
    spawn(
        &mut sim,
        MANGO,
        EntityKind::Mangonel,
        OWNER_OPPONENT,
        vx,
        vy + I16F16::from_num(24),
    );
    let vic_hp_start = f(ent(&sim, VIC).hp);
    sim.push_command(Command::AttackGround {
        entity: MANGO,
        x: vx,
        y: vy,
    });
    let mut answered = false;
    for _ in 0..1200 {
        sim.tick();
        if ent(&sim, VIC).attack_target == Some(MANGO) {
            answered = true;
        }
    }
    assert!(
        f(ent(&sim, VIC).hp) < vic_hp_start,
        "staging broken: the Mangonel never landed a stone on the Stand Ground Knight"
    );
    assert!(
        !answered,
        "CMB-09 violated: a Stand Ground Knight answered an out-of-range shooter"
    );
}

