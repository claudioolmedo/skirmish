//! Teams T3a tests (2026-11-05): the diplomacy stance table
//! (NET-11) — `TeamSettings::locked_teams`, the directed
//! `stance` table initialized from T1's teams, the
//! `SetDiplomacy` command (codec tag 50), the one hostile
//! predicate (`can_auto_acquire`) at every CMB-08 consumer,
//! gates (BLD-13), monk heal/conversion (MNK-10/MNK-08), sheep
//! stealing (AI-06), the mutual-Ally vision ruling, and the
//! conditional hash (marker 0xC5) / save v18 discipline:
//! defaults are byte-identical to before, locked matches can
//! never differ, and a stance changed back to its initial
//! value leaves no trace in the stream.
//!
//! Expectations are the dossier's pinned model (TEAMS-SPEC
//! §5 + the execution plan's T3 section), including its two
//! adopted readings: Neutral units and gates (register #9 —
//! neither opens nor holds shut) and conversion of neutral-row
//! players (register #10 — convertible). Gate cases that need
//! a built gate live in `sim/tests/gates.rs`, per the plan.

use crate::entity::{Entity, EntityKind, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{
    Command, MatchMode, Sim, TeamSettings, VictorySettings, STANCE_ALLY, STANCE_ENEMY,
    STANCE_NEUTRAL,
};
use fixed::types::I16F16;

fn unlocked_distinct() -> TeamSettings {
    TeamSettings {
        teams: [1, 2],
        locked_teams: false,
    }
}

fn unlocked_sim(seed: u64) -> Sim {
    Sim::with_settings_teams(
        seed,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        unlocked_distinct(),
    )
}

fn same_team_sim(seed: u64, locked: bool) -> Sim {
    Sim::with_settings_teams(
        seed,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings {
            teams: [1, 1],
            locked_teams: locked,
        },
    )
}

fn set_stance(sim: &mut Sim, player: u8, target: u8, stance: u8) {
    assert!(
        sim.can_set_diplomacy(player, target, stance),
        "SetDiplomacy {player} -> {target} = {stance} must be acceptable"
    );
    sim.push_command(Command::SetDiplomacy {
        player,
        target,
        stance,
    });
    sim.tick();
    assert_eq!(sim.stance_for(player, target), Some(stance));
}

/// A free 7×7 patch of ground far from both Town Centers, for
/// staging spawned entities without interference from the
/// starts (their units, their sheep, their trees).
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
    panic!("no open patch on this seed");
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) -> u32 {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn wound(sim: &mut Sim, id: u32, hp: i32) {
    let e = sim.entities.iter_mut().find(|e| e.id == id).unwrap();
    e.hp = I16F16::from_num(hp);
}

// ------------------------------------------------------------------
// (a) Construction: the table is derived from teams.

#[test]
fn construction_derives_the_table_from_teams() {
    // Locked distinct teams (the default): Enemy both ways.
    let sim = Sim::new_match(7, MatchMode::TwoPlayer);
    assert!(sim.team_settings().locked_teams);
    assert_eq!(
        sim.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_ENEMY)
    );
    assert_eq!(
        sim.stance_for(OWNER_OPPONENT, OWNER_PLAYER),
        Some(STANCE_ENEMY)
    );
    // Self reads Ally (the fixed self cell); Gaia has no stance.
    assert_eq!(
        sim.stance_for(OWNER_PLAYER, OWNER_PLAYER),
        Some(STANCE_ALLY)
    );
    assert_eq!(sim.stance_for(OWNER_PLAYER, OWNER_GAIA), None);
    assert_eq!(sim.stance_for(OWNER_GAIA, OWNER_OPPONENT), None);
    assert!(!sim.stance_differs_from_initial());

    // Same team: Ally both ways.
    let sim = same_team_sim(7, true);
    assert_eq!(
        sim.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_ALLY)
    );
    assert_eq!(
        sim.stance_for(OWNER_OPPONENT, OWNER_PLAYER),
        Some(STANCE_ALLY)
    );

    // NO_TEAM × NO_TEAM: Enemy (unaffiliated is not allied).
    let sim = Sim::with_settings_teams(
        7,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings {
            teams: [crate::NO_TEAM, crate::NO_TEAM],
            ..Default::default()
        },
    );
    assert_eq!(
        sim.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_ENEMY)
    );
    assert_eq!(
        sim.stance_for(OWNER_OPPONENT, OWNER_PLAYER),
        Some(STANCE_ENEMY)
    );
}

// ------------------------------------------------------------------
// (b) Locked refusal — and invalid orders in unlocked matches.

#[test]
fn locked_teams_refuse_stance_changes_and_hash_nothing() {
    let mut a = Sim::new_match(9, MatchMode::TwoPlayer);
    let mut b = Sim::new_match(9, MatchMode::TwoPlayer);
    assert!(!a.can_set_diplomacy(OWNER_PLAYER, OWNER_OPPONENT, STANCE_NEUTRAL));
    a.push_command(Command::SetDiplomacy {
        player: OWNER_PLAYER,
        target: OWNER_OPPONENT,
        stance: STANCE_NEUTRAL,
    });
    for tick in 0..300 {
        let (ha, hb) = (a.tick(), b.tick());
        assert_eq!(
            ha, hb,
            "a refused command changed the stream at tick {tick}"
        );
    }
    assert_eq!(
        a.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_ENEMY)
    );
    assert!(!a.stance_differs_from_initial());
}

#[test]
fn invalid_stance_orders_are_refused_in_unlocked_matches() {
    let mut sim = unlocked_sim(9);
    // Self, Gaia, unknown seats, and an out-of-range tag.
    assert!(!sim.can_set_diplomacy(OWNER_PLAYER, OWNER_PLAYER, STANCE_ALLY));
    assert!(!sim.can_set_diplomacy(OWNER_PLAYER, OWNER_GAIA, STANCE_ALLY));
    assert!(!sim.can_set_diplomacy(OWNER_PLAYER, 99, STANCE_ALLY));
    assert!(!sim.can_set_diplomacy(OWNER_GAIA, OWNER_OPPONENT, STANCE_ALLY));
    assert!(!sim.can_set_diplomacy(OWNER_PLAYER, OWNER_OPPONENT, 3));
    for (player, target, stance) in [
        (OWNER_PLAYER, OWNER_PLAYER, STANCE_ALLY),
        (OWNER_PLAYER, OWNER_GAIA, STANCE_NEUTRAL),
        (OWNER_PLAYER, OWNER_OPPONENT, 3),
    ] {
        sim.push_command(Command::SetDiplomacy {
            player,
            target,
            stance,
        });
    }
    sim.tick();
    assert_eq!(
        sim.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_ENEMY)
    );
    assert!(!sim.stance_differs_from_initial());
}

// ------------------------------------------------------------------
// (c) Neutral: auto-acquisition is military-only; explicit
// orders still execute (manual tasking is the DE behavior).

#[test]
fn neutral_row_auto_acquires_military_only() {
    let mut sim = unlocked_sim(42);
    set_stance(&mut sim, OWNER_PLAYER, OWNER_OPPONENT, STANCE_NEUTRAL);
    // The predicate itself, in its three arms.
    assert!(sim.can_auto_acquire(OWNER_PLAYER, OWNER_OPPONENT, true));
    assert!(!sim.can_auto_acquire(OWNER_PLAYER, OWNER_OPPONENT, false));

    // Stage: my Militia, their Villager CLOSER than their
    // Militia. Under Enemy the chain takes the villager (the
    // closest unit); under Neutral it must skip to the soldier.
    let c = open_patch(&sim);
    let mine = spawn(&mut sim, 9001, EntityKind::Militia, OWNER_PLAYER, c);
    let vil = spawn(
        &mut sim,
        9002,
        EntityKind::Villager,
        OWNER_OPPONENT,
        (c.0 + 1, c.1),
    );
    let foe = spawn(
        &mut sim,
        9003,
        EntityKind::Militia,
        OWNER_OPPONENT,
        (c.0 + 2, c.1),
    );
    sim.tick();
    assert_eq!(
        ent(&sim, mine).attack_target,
        Some(foe),
        "the neutral chain must skip the villager and take the soldier"
    );
    // And over time the villager is never harmed by acquisition.
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(ent(&sim, vil).hp, I16F16::from_num(25));

    // An explicit attack order on the neutral villager is
    // still legal and executes (manual tasking).
    assert!(sim.can_attack(mine, vil));
    sim.push_command(Command::Attack {
        entity: mine,
        target: vil,
    });
    let mut hurt = false;
    for _ in 0..1200 {
        sim.tick();
        if ent(&sim, vil).hp < I16F16::from_num(25) || ent(&sim, vil).dead {
            hurt = true;
            break;
        }
    }
    assert!(
        hurt,
        "an explicit attack on a neutral villager never landed"
    );
}

// ------------------------------------------------------------------
// (d) One-sided Ally: the table is directed.

#[test]
fn one_sided_ally_stops_my_acquisition_not_theirs() {
    let mut sim = unlocked_sim(42);
    set_stance(&mut sim, OWNER_PLAYER, OWNER_OPPONENT, STANCE_ALLY);
    assert_eq!(
        sim.stance_for(OWNER_OPPONENT, OWNER_PLAYER),
        Some(STANCE_ENEMY)
    );
    assert!(!sim.can_auto_acquire(OWNER_PLAYER, OWNER_OPPONENT, true));
    assert!(sim.can_auto_acquire(OWNER_OPPONENT, OWNER_PLAYER, true));

    let c = open_patch(&sim);
    let mine = spawn(&mut sim, 9001, EntityKind::Militia, OWNER_PLAYER, c);
    let foe = spawn(
        &mut sim,
        9002,
        EntityKind::Militia,
        OWNER_OPPONENT,
        (c.0 + 2, c.1),
    );
    // Explicit orders follow the same directed table.
    assert!(
        !sim.can_attack(mine, foe),
        "I may not attack-order my Ally row"
    );
    assert!(
        sim.can_attack(foe, mine),
        "their Enemy row may order onto me"
    );
    for _ in 0..30 {
        sim.tick();
    }
    // Mine never acquired them; theirs acquired mine.
    assert_eq!(ent(&sim, mine).attack_target, None);
    assert_eq!(ent(&sim, foe).attack_target, Some(mine));
    assert_eq!(
        ent(&sim, foe).hp,
        I16F16::from_num(40),
        "my units harmed an Ally row"
    );
}

// ------------------------------------------------------------------
// (f) Heal + conversion eligibility follow the row.

#[test]
fn heal_and_conversion_follow_the_stance_row() {
    // Ally row (same team, locked): my Monk heals their unit,
    // and may not convert it.
    let mut sim = same_team_sim(42, true);
    let c = open_patch(&sim);
    let monk = spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, c);
    let ally = spawn(
        &mut sim,
        9002,
        EntityKind::Militia,
        OWNER_OPPONENT,
        (c.0 + 1, c.1),
    );
    wound(&mut sim, ally, 10);
    assert!(
        sim.can_heal(monk, ally),
        "an Ally-row unit must be healable"
    );
    assert!(
        !sim.can_convert_target(monk, ally),
        "an Ally-row unit is not convertible"
    );
    sim.push_command(Command::Heal {
        entity: monk,
        target: ally,
    });
    let mut healed = false;
    for _ in 0..1200 {
        sim.tick();
        if ent(&sim, ally).hp > I16F16::from_num(10) {
            healed = true;
            break;
        }
    }
    assert!(healed, "the monk never healed the allied unit");

    // Enemy row (the default): heal refused, conversion open.
    let mut sim = Sim::new_match(42, MatchMode::TwoPlayer);
    let c = open_patch(&sim);
    let monk = spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, c);
    let foe = spawn(
        &mut sim,
        9002,
        EntityKind::Militia,
        OWNER_OPPONENT,
        (c.0 + 1, c.1),
    );
    wound(&mut sim, foe, 10);
    assert!(!sim.can_heal(monk, foe));
    assert!(sim.can_convert_target(monk, foe));

    // Neutral row (unlocked): heal refused; conversion stays
    // open — the adopted reading of register #10.
    let mut sim = unlocked_sim(42);
    set_stance(&mut sim, OWNER_PLAYER, OWNER_OPPONENT, STANCE_NEUTRAL);
    let c = open_patch(&sim);
    let monk = spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, c);
    let foe = spawn(
        &mut sim,
        9002,
        EntityKind::Militia,
        OWNER_OPPONENT,
        (c.0 + 1, c.1),
    );
    wound(&mut sim, foe, 10);
    assert!(
        !sim.can_heal(monk, foe),
        "a Neutral-row unit is not healable"
    );
    assert!(
        sim.can_convert_target(monk, foe),
        "a Neutral-row unit stays convertible (adopted reading)"
    );
}

// ------------------------------------------------------------------
// (h) Sheep: an ally's passing unit does not steal; an
// enemy's does.

#[test]
fn sheep_stealing_requires_a_non_ally() {
    // Enemy (default): my sheep, left alone, falls to their
    // passing villager.
    let mut sim = Sim::new_match(42, MatchMode::TwoPlayer);
    let c = open_patch(&sim);
    let sheep = spawn(&mut sim, 9001, EntityKind::Sheep, OWNER_PLAYER, c);
    spawn(
        &mut sim,
        9002,
        EntityKind::Villager,
        OWNER_OPPONENT,
        (c.0 + 1, c.1),
    );
    sim.tick();
    assert_eq!(
        ent(&sim, sheep).owner,
        OWNER_OPPONENT,
        "an enemy must steal"
    );

    // Ally (same team): their villager stands beside my sheep
    // and the sheep stays mine.
    let mut sim = same_team_sim(42, true);
    let c = open_patch(&sim);
    let sheep = spawn(&mut sim, 9001, EntityKind::Sheep, OWNER_PLAYER, c);
    spawn(
        &mut sim,
        9002,
        EntityKind::Villager,
        OWNER_OPPONENT,
        (c.0 + 1, c.1),
    );
    for _ in 0..120 {
        sim.tick();
    }
    assert_eq!(
        ent(&sim, sheep).owner,
        OWNER_PLAYER,
        "an ally must not steal"
    );
}

// ------------------------------------------------------------------
// Vision under unlocked diplomacy: mutual Ally only
// (register #11's adopted ruling).

#[test]
fn unlocked_vision_keys_on_mutual_ally_rows() {
    // Same team, unlocked: the initial rows are mutual Ally,
    // so the shared read unions — stage the foe scout deep in
    // its own half, as in the T1 staged-alliance proof.
    let mut sim = same_team_sim(42, false);
    let scout = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::ScoutCavalry)
        .map(|e| (e.id, e.pos_x, e.pos_y))
        .unwrap();
    sim.push_command(Command::Move {
        entity: scout.0,
        x: scout.1 + I16F16::from_num(8),
        y: scout.2,
    });
    for _ in 0..180 {
        sim.tick();
    }
    let s = ent(&sim, scout.0);
    let (scx, scy) = sim.grid().world_to_cell(s.pos_x, s.pos_y);
    let idx = (scy * crate::GRID_DIM + scx) as usize;
    assert_eq!(
        sim.visibility(OWNER_PLAYER)[idx],
        0,
        "own truth must not show the scout"
    );
    assert_eq!(
        sim.visibility_shared(OWNER_PLAYER)[idx],
        2,
        "mutual Ally must union"
    );

    // I break the pair one-sidedly: vision unshares in BOTH
    // directions (the mutual ruling), even though their row
    // toward me still says Ally.
    set_stance(&mut sim, OWNER_PLAYER, OWNER_OPPONENT, STANCE_ENEMY);
    assert_eq!(
        sim.stance_for(OWNER_OPPONENT, OWNER_PLAYER),
        Some(STANCE_ALLY)
    );
    assert_eq!(
        sim.visibility_shared(OWNER_PLAYER),
        sim.visibility(OWNER_PLAYER).to_vec(),
        "a one-sided Ally must not share vision"
    );
    assert_eq!(
        sim.visibility_shared(OWNER_OPPONENT),
        sim.visibility(OWNER_OPPONENT).to_vec()
    );
}

// ------------------------------------------------------------------
// (g) Determinism + save discipline around a stance change.

#[test]
fn stance_change_back_to_initial_leaves_no_hash_trace() {
    let mut a = unlocked_sim(11);
    let mut b = unlocked_sim(11);
    set_stance(&mut a, OWNER_PLAYER, OWNER_OPPONENT, STANCE_NEUTRAL);
    b.tick();
    assert!(a.stance_differs_from_initial());
    assert_ne!(
        a.state_hash(),
        b.state_hash(),
        "a live stance change must hash"
    );
    set_stance(&mut a, OWNER_PLAYER, OWNER_OPPONENT, STANCE_ENEMY);
    b.tick();
    assert!(!a.stance_differs_from_initial());
    for tick in 0..120 {
        let (ha, hb) = (a.tick(), b.tick());
        assert_eq!(ha, hb, "changed-back stance left a trace at tick {tick}");
    }
}

#[test]
fn save_round_trip_after_a_stance_change() {
    let mut sim = unlocked_sim(11);
    set_stance(&mut sim, OWNER_PLAYER, OWNER_OPPONENT, STANCE_NEUTRAL);
    for _ in 0..60 {
        sim.tick();
    }
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a v18 save we wrote must load");
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(
        restored.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_NEUTRAL)
    );
    assert_eq!(
        restored.stance_for(OWNER_OPPONENT, OWNER_PLAYER),
        Some(STANCE_ENEMY)
    );
    assert!(!restored.team_settings().locked_teams);
    assert_eq!(
        restored.save_game(),
        bytes,
        "re-save must be byte-identical"
    );
}

#[test]
fn save_round_trip_with_a_queued_stance_change() {
    // A save taken between frames, with the change still in
    // the pending-command queue, restores the queue (codec
    // tag 50) — the change then applies identically.
    let mut sim = unlocked_sim(11);
    sim.push_command(Command::SetDiplomacy {
        player: OWNER_PLAYER,
        target: OWNER_OPPONENT,
        stance: STANCE_NEUTRAL,
    });
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("queued-change save must load");
    assert_eq!(restored.state_hash(), sim.state_hash());
    let (ha, hb) = (sim.tick(), restored.tick());
    assert_eq!(ha, hb);
    assert_eq!(
        restored.stance_for(OWNER_PLAYER, OWNER_OPPONENT),
        Some(STANCE_NEUTRAL)
    );
}

#[test]
fn unlocked_default_ids_round_trip_the_lock() {
    // Unlocked with DEFAULT team ids: the settings are
    // non-default through the lock alone, and both the hash
    // and the save must carry that (the extended 0xC4 block).
    let a = unlocked_sim(11);
    let b = Sim::new_match(11, MatchMode::TwoPlayer);
    assert_ne!(a.state_hash(), b.state_hash());
    let restored = Sim::load_game(&a.save_game()).expect("unlocked save must load");
    assert_eq!(restored.state_hash(), a.state_hash());
    assert!(!restored.team_settings().locked_teams);
    assert_eq!(restored.team_settings().teams, [1, 2]);
}
