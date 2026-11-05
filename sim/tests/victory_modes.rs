//! Victory-mode tests (Package C: VIC-05, VIC-06, VIC-07).
//!
//! The three quick victory modes, exercised end-to-end through
//! the public API:
//! — VIC-05: a time limit awards the higher VIC-04 total at
//!   expiry; a score target wins the moment it is reached. Both
//!   readings award the higher total; an exact tie plays on.
//! — VIC-06 (Regicide): each player opens with a King; its death
//!   — by combat or by Delete, the death sweep is the one path —
//!   eliminates the owner immediately, whatever else they own.
//! — VIC-07 (Sudden Death): losing the last Town Center
//!   eliminates the owner, no rebuild window.
//! Plain Conquest stays the default and its state stream stays
//! byte-identical (the settings join the hash only off-default).

use fixed::types::I16F16;
use skirmish_sim::{
    unit_spec, Command, EntityKind, MatchMode, MatchState, Sim, TechId, VictorySettings,
    OWNER_OPPONENT, OWNER_PLAYER,
};

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

fn find_all(sim: &Sim, owner: u8, kind: EntityKind) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind && !e.dead)
        .map(|e| e.id)
        .collect()
}

fn is_dead(sim: &Sim, id: u32) -> bool {
    sim.entities().iter().find(|e| e.id == id).unwrap().dead
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn settle(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn regicide() -> VictorySettings {
    VictorySettings {
        regicide: true,
        ..VictorySettings::conquest()
    }
}

fn sudden_death() -> VictorySettings {
    VictorySettings {
        sudden_death: true,
        ..VictorySettings::conquest()
    }
}

// ---- Settings plumbing -------------------------------------------

#[test]
fn conquest_is_the_default_and_its_stream_is_unchanged() {
    let plain = Sim::with_mode(42, MatchMode::TwoPlayer);
    let explicit = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::conquest());
    assert!(plain.victory_settings().is_default());
    assert_eq!(plain.state_hash(), explicit.state_hash());
    // The same commands on both: the streams must stay identical —
    // the golden-hash guarantee for every pre-Package-C match.
    let mut a = plain;
    let mut b = explicit;
    let villager = find_id(&a, OWNER_PLAYER, EntityKind::Villager);
    for s in [&mut a, &mut b] {
        s.push_command(Command::Move {
            entity: villager,
            x: I16F16::from_num(10),
            y: I16F16::from_num(10),
        });
    }
    for _ in 0..600 {
        a.tick();
        b.tick();
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn victory_settings_round_trip_and_are_hashed_state() {
    let settings = VictorySettings {
        time_limit_ticks: 54_000,
        score_target: 900,
        regicide: true,
        sudden_death: true,
        relic_victory: false,
        // Package I: the mode-delta fields ride the same
        // round-trip (off here — this test's subject is the
        // Package C conditions).
        death_match: false,
        empire_wars: false,
        king_of_the_hill: false,
        // VIC-02: the wonder condition rides the same
        // round-trip (off here — this test's subject is the
        // Package C conditions).
        wonder_victory: false,
        // VIC-12 (Teams T2): allied victory rides the same
        // round-trip (off here, same reason).
        allied_victory: false,
    };
    let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, settings);
    assert_eq!(sim.victory_settings(), settings);
    // A match under other settings can never silently agree with
    // a Conquest match on the same seed.
    let conquest = Sim::with_mode(42, MatchMode::TwoPlayer);
    assert_ne!(sim.state_hash(), conquest.state_hash());
    // And two matches under the same settings replay identically.
    let mut twin = Sim::new_match_victory(42, MatchMode::TwoPlayer, settings);
    let mut sim = sim;
    settle(&mut sim, 300);
    settle(&mut twin, 300);
    assert_eq!(sim.state_hash(), twin.state_hash());
}

// ---- VIC-06: Regicide --------------------------------------------

#[test]
fn regicide_opening_spawns_one_king_per_player() {
    let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, regicide());
    for owner in [OWNER_PLAYER, OWNER_OPPONENT] {
        let kings = find_all(&sim, owner, EntityKind::King);
        assert_eq!(kings.len(), 1, "owner {owner}");
        let king = sim.entities().iter().find(|e| e.id == kings[0]).unwrap();
        // The wiki row: 75 HP (see the KING stat table in units.rs).
        assert_eq!(king.hp.to_num::<i32>(), 75);
        assert_eq!(king.kind.tag(), 22);
        // The King occupies a population slot: the Regicide start
        // stands at 5/5 until a House or Castle adds room.
        assert_eq!(sim.population_of(owner), 5);
        assert_eq!(sim.population_cap_of(owner), 5);
    }
    // The row carries the CLS-06 flags; no other row does.
    let row = unit_spec(EntityKind::King);
    assert!(row.regicide_unit);
    assert!(row.immune_conversion);
    assert_eq!(row.regenerates, 0);
    assert!(!unit_spec(EntityKind::Knight).regicide_unit);
    // A plain Conquest match spawns no King at all.
    let plain = Sim::with_mode(42, MatchMode::TwoPlayer);
    assert!(find_all(&plain, OWNER_PLAYER, EntityKind::King).is_empty());
    assert!(find_all(&plain, OWNER_OPPONENT, EntityKind::King).is_empty());
    assert_eq!(plain.population_of(OWNER_PLAYER), 4);
}

#[test]
fn regicide_king_killed_in_combat_eliminates_the_owner() {
    let mut sim = Sim::new_match_victory(7, MatchMode::TwoPlayer, regicide());
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let foe_king = find_id(&sim, OWNER_OPPONENT, EntityKind::King);
    sim.push_command(Command::Attack {
        entity: scout,
        target: foe_king,
    });
    let dead = run_until(&mut sim, 240_000, |s| is_dead(s, foe_king));
    assert!(dead, "the scout never killed the foe King");
    // The foe is out on the King's death — Town Center, villagers
    // and scout all still standing (VIC-06: whatever else they own).
    assert_eq!(sim.match_state(), MatchState::Victory);
    assert!(!is_dead(&sim, find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter)));
    assert_eq!(find_all(&sim, OWNER_OPPONENT, EntityKind::Villager).len(), 3);
}

#[test]
fn regicide_king_deleted_eliminates_the_owner() {
    // The Delete path reaches the same death sweep (a deliberate
    // delete has no killer, but the King still died).
    let mut sim = Sim::new_match_victory(7, MatchMode::TwoPlayer, regicide());
    let human_king = find_id(&sim, OWNER_PLAYER, EntityKind::King);
    sim.push_command(Command::DeleteEntity { entity: human_king });
    settle(&mut sim, 5);
    assert!(is_dead(&sim, human_king));
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

// ---- VIC-07: Sudden Death -----------------------------------------

#[test]
fn sudden_death_deleting_your_last_tc_eliminates_you() {
    // Owner-deleting your own Town Center routes through the same
    // destruction path (SEL-14 / CMB-15) — and in Sudden Death it
    // is the elimination event (VIC-07, no rebuild window).
    let mut sim = Sim::new_match_victory(7, MatchMode::TwoPlayer, sudden_death());
    let human_tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    sim.push_command(Command::DeleteEntity { entity: human_tc });
    settle(&mut sim, 5);
    assert_eq!(sim.match_state(), MatchState::Defeat);

    let mut sim = Sim::new_match_victory(7, MatchMode::TwoPlayer, sudden_death());
    let foe_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    sim.push_command(Command::DeleteEntity { entity: foe_tc });
    settle(&mut sim, 5);
    assert_eq!(sim.match_state(), MatchState::Victory);
}

#[test]
fn sudden_death_off_means_tc_loss_alone_is_not_elimination() {
    // The same Delete under plain Conquest: VIC-01's predicate
    // still sees the villagers, so the match plays on.
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let foe_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    sim.push_command(Command::DeleteEntity { entity: foe_tc });
    settle(&mut sim, 5);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

// ---- VIC-05: time limit and score target ---------------------------
//
// The score lever is Loom, whose numbers VIC-04's tests pin: paying
// its 50 gold drops the human's total by 5 at once (the 10% stock
// term), completing it raises the total by a net +5 (Technology
// +10). The foe idles in TwoPlayer mode, so the two totals diverge
// by exactly that much and nothing else moves them.

#[test]
fn time_limit_awards_the_higher_score_at_expiry() {
    // Trailing at expiry loses: the human pays for Loom just
    // before a 200-tick limit, so at expiry they trail by 5.
    let mut sim = Sim::new_match_victory(
        42,
        MatchMode::TwoPlayer,
        VictorySettings {
            time_limit_ticks: 200,
            ..VictorySettings::conquest()
        },
    );
    settle(&mut sim, 30);
    assert_eq!(
        sim.score_for(OWNER_PLAYER).total,
        sim.score_for(OWNER_OPPONENT).total
    );
    sim.push_command(Command::ResearchTc { tech: TechId::Loom });
    settle(&mut sim, 169); // tick 199: the limit has not expired
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    let decided = run_until(&mut sim, 1_000, |s| s.match_state() != MatchState::Ongoing);
    assert!(decided, "the time limit never decided the match");
    assert!(sim.tick_count() >= 200);
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

#[test]
fn time_limit_leader_wins_only_at_expiry_not_before() {
    // Leading before expiry does not end the match early: the
    // human completes Loom (net +5) long before a 4000-tick limit
    // and the match still runs to the clock.
    let mut sim = Sim::new_match_victory(
        42,
        MatchMode::TwoPlayer,
        VictorySettings {
            time_limit_ticks: 4_000,
            ..VictorySettings::conquest()
        },
    );
    settle(&mut sim, 30);
    sim.push_command(Command::ResearchTc { tech: TechId::Loom });
    let done = run_until(&mut sim, 20_000, |s| s.has_tech(OWNER_PLAYER, TechId::Loom));
    assert!(done, "Loom never completed");
    assert!(
        sim.score_for(OWNER_PLAYER).total > sim.score_for(OWNER_OPPONENT).total,
        "the human must lead after Loom"
    );
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    let decided = run_until(&mut sim, 10_000, |s| s.match_state() != MatchState::Ongoing);
    assert!(decided);
    assert!(sim.tick_count() >= 4_000);
    assert_eq!(sim.match_state(), MatchState::Victory);
}

#[test]
fn time_limit_exact_tie_plays_on_past_expiry() {
    // Nobody acts: the mirrored starts keep the totals identical,
    // so at expiry there is no highest score and the match plays
    // on (VIC-05's recorded tie reading).
    let mut sim = Sim::new_match_victory(
        42,
        MatchMode::TwoPlayer,
        VictorySettings {
            time_limit_ticks: 600,
            ..VictorySettings::conquest()
        },
    );
    settle(&mut sim, 1_200);
    assert_eq!(
        sim.score_for(OWNER_PLAYER).total,
        sim.score_for(OWNER_OPPONENT).total
    );
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn score_target_wins_the_moment_it_is_reached() {
    let mut probe = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::conquest());
    settle(&mut probe, 30);
    let start_total = probe.score_for(OWNER_OPPONENT).total;

    // Target exactly the net Loom gain away: the win must arrive
    // with the research, with no time limit involved at all.
    let mut sim = Sim::new_match_victory(
        42,
        MatchMode::TwoPlayer,
        VictorySettings {
            score_target: start_total + 5,
            ..VictorySettings::conquest()
        },
    );
    settle(&mut sim, 30);
    sim.push_command(Command::ResearchTc { tech: TechId::Loom });
    let won = run_until(&mut sim, 20_000, |s| s.match_state() == MatchState::Victory);
    assert!(won, "reaching the score target never won the match");
    assert!(sim.has_tech(OWNER_PLAYER, TechId::Loom));

    // Control: one point further and the same research is not
    // enough — the match plays on.
    let mut sim = Sim::new_match_victory(
        42,
        MatchMode::TwoPlayer,
        VictorySettings {
            score_target: start_total + 6,
            ..VictorySettings::conquest()
        },
    );
    settle(&mut sim, 30);
    sim.push_command(Command::ResearchTc { tech: TechId::Loom });
    let done = run_until(&mut sim, 20_000, |s| s.has_tech(OWNER_PLAYER, TechId::Loom));
    assert!(done);
    settle(&mut sim, 200);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}
