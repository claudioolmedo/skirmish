//! Game-mode delta tests (Package I: VIC-09, VIC-10, VIC-08 +
//! AI-09), built on Package C's `VictorySettings` framework.
//!
//! — VIC-09 (Death Match): both players open on the Death
//!   Match stockpiles (20,000 / 20,000 / 10,000 / 5,000); the
//!   rest of the opening is the standard one.
//! — VIC-10 (Empire Wars): both players open in the Feudal Age
//!   with Loom, the pre-built town (Mill, 3 Lumber Camps,
//!   Mining Camp, 6 Houses, 5 Farms, Barracks) and 27 tasked
//!   villagers — 4 gold, 12 wood, 5 food (2 Farms + 3 berries),
//!   6 idle — on the wiki article's declared total of 27 (its
//!   printed distribution sums to 28; see the resolution in
//!   `Sim::apply_empire_wars_start`).
//! — AI-09 + VIC-08 (King of the Hill): a Gaia Monument stands
//!   at the map center, captured by presence on the AI-06
//!   pattern; the holder's countdown (100 years × 5 s on this
//!   Tiny map) wins the match, and a capture under the
//!   100-year floor resets the timer to the floor.
//! Plain Conquest is untouched by all three: the settings and
//! the KotH state join the hash only off-default / in-mode.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, EntityKind, MatchMode, MatchState, NodeKind, ResourceKind, Sim, TechId,
    VictorySettings, KOTH_COUNTDOWN_TICKS, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER,
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

fn stock(sim: &Sim, owner: u8, kind: ResourceKind) -> i32 {
    sim.stockpile_of(owner).amount(kind).to_num::<i32>()
}

fn buildings_of(sim: &Sim, owner: u8, kind: BuildingKind) -> usize {
    sim.buildings()
        .iter()
        .filter(|b| b.owner == owner && b.kind == kind)
        .count()
}

// ---- Settings plumbing -------------------------------------------

#[test]
fn conquest_default_is_untouched_by_package_i() {
    let plain = Sim::with_mode(42, MatchMode::TwoPlayer);
    let explicit = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::conquest());
    assert_eq!(plain.state_hash(), explicit.state_hash());
    assert!(plain.victory_settings().is_default());
    // No Monument stands in a Conquest match.
    assert_eq!(plain.monument_owner(), None);
    assert_eq!(buildings_of(&plain, OWNER_GAIA, BuildingKind::Monument), 0);
}

#[test]
fn mode_settings_round_trip() {
    for settings in [
        VictorySettings::death_match(),
        VictorySettings::empire_wars(),
        VictorySettings::king_of_the_hill(),
    ] {
        let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, settings);
        assert_eq!(sim.victory_settings(), settings);
        assert!(!settings.is_default());
    }
}

// ---- VIC-09 Death Match ------------------------------------------

#[test]
fn death_match_stockpiles_for_both_players() {
    let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::death_match());
    for owner in [OWNER_PLAYER, OWNER_OPPONENT] {
        assert_eq!(stock(&sim, owner, ResourceKind::Food), 20_000);
        assert_eq!(stock(&sim, owner, ResourceKind::Wood), 20_000);
        assert_eq!(stock(&sim, owner, ResourceKind::Gold), 10_000);
        assert_eq!(stock(&sim, owner, ResourceKind::Stone), 5_000);
        // The delta is the stockpile alone: the standard
        // opening (3 villagers, a Scout, the Dark Age) stands.
        assert_eq!(find_all(&sim, owner, EntityKind::Villager).len(), 3);
        assert_eq!(find_all(&sim, owner, EntityKind::ScoutCavalry).len(), 1);
        assert_eq!(sim.age_of(owner), skirmish_sim::Age::Dark);
    }
}

#[test]
fn death_match_stream_is_deterministic_and_distinct() {
    let a = Sim::new_match_victory(7, MatchMode::AiNormal, VictorySettings::death_match());
    let mut b = Sim::new_match_victory(7, MatchMode::AiNormal, VictorySettings::death_match());
    let conquest = Sim::new_match(7, MatchMode::AiNormal);
    assert_ne!(a.state_hash(), conquest.state_hash());
    let mut a = a;
    // Identical commands on both: the streams must stay equal,
    // and the AI must play on the big stockpiles without a
    // hiccup.
    let scout = find_id(&a, OWNER_PLAYER, EntityKind::ScoutCavalry);
    for s in [&mut a, &mut b] {
        s.push_command(Command::Move {
            entity: scout,
            x: I16F16::from_num(20),
            y: I16F16::from_num(20),
        });
    }
    settle(&mut a, 1800);
    settle(&mut b, 1800);
    assert_eq!(a.state_hash(), b.state_hash());
    assert_eq!(a.match_state(), MatchState::Ongoing);
}

// ---- VIC-10 Empire Wars ------------------------------------------

fn assert_empire_wars_start(sim: &Sim, owner: u8) {
    // Feudal Age with Loom (the documented starting conditions).
    assert_eq!(sim.age_of(owner), skirmish_sim::Age::Feudal);
    assert!(sim.has_tech(owner, TechId::Loom));
    // The pre-built town, all complete.
    for (kind, want) in [
        (BuildingKind::Mill, 1),
        (BuildingKind::LumberCamp, 3),
        (BuildingKind::MiningCamp, 1),
        (BuildingKind::House, 6),
        (BuildingKind::Farm, 5),
        (BuildingKind::Barracks, 1),
    ] {
        assert_eq!(
            buildings_of(sim, owner, kind),
            want,
            "owner {owner} kind {kind:?}"
        );
        assert!(sim
            .buildings()
            .iter()
            .filter(|b| b.owner == owner && b.kind == kind)
            .all(|b| b.complete));
    }
    // 27 villagers + 1 Scout; population room 5 (TC) + 6×5 = 35.
    let villagers = find_all(sim, owner, EntityKind::Villager);
    assert_eq!(villagers.len(), 27);
    assert_eq!(find_all(sim, owner, EntityKind::ScoutCavalry).len(), 1);
    assert_eq!(sim.population_cap_of(owner), 35);
    // Standard resources (ECO-17 = the mode's documented ones).
    assert_eq!(stock(sim, owner, ResourceKind::Food), 200);
    assert_eq!(stock(sim, owner, ResourceKind::Wood), 200);
    assert_eq!(stock(sim, owner, ResourceKind::Gold), 100);
    assert_eq!(stock(sim, owner, ResourceKind::Stone), 200);
    // The tasked distribution, read off the assignments the
    // construction installed: 4 gold, 12 wood, 2 farms,
    // 3 berries, 6 with no assignment.
    let mut counts = (0usize, 0usize, 0usize, 0usize, 0usize);
    for e in sim
        .entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == EntityKind::Villager && !e.dead)
    {
        match e.task_node {
            None => counts.4 += 1,
            Some(nid) => match sim.nodes()[nid as usize].kind {
                NodeKind::GoldMine => counts.0 += 1,
                NodeKind::Tree => counts.1 += 1,
                NodeKind::Farm => counts.2 += 1,
                NodeKind::BerryBush => counts.3 += 1,
                other => panic!("unexpected EW task node {other:?}"),
            },
        }
    }
    assert_eq!(counts, (4, 12, 2, 3, 6), "owner {owner} distribution");
}

#[test]
fn empire_wars_opening_composition() {
    for seed in [42u64, 7, 1234] {
        let sim = Sim::new_match_victory(seed, MatchMode::TwoPlayer, VictorySettings::empire_wars());
        assert_empire_wars_start(&sim, OWNER_PLAYER);
        assert_empire_wars_start(&sim, OWNER_OPPONENT);
    }
}

#[test]
fn empire_wars_economy_runs_and_is_deterministic() {
    let mut a = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::empire_wars());
    let mut b = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::empire_wars());
    let conquest = Sim::new_match(42, MatchMode::TwoPlayer);
    assert_ne!(a.state_hash(), conquest.state_hash());
    // A minute of game time: the tasked villagers work their
    // assignments with no orders at all, and resources land.
    settle(&mut a, 3600);
    settle(&mut b, 3600);
    assert_eq!(a.state_hash(), b.state_hash());
    let total = |s: &Sim, o: u8| {
        stock(s, o, ResourceKind::Food)
            + stock(s, o, ResourceKind::Wood)
            + stock(s, o, ResourceKind::Gold)
            + stock(s, o, ResourceKind::Stone)
    };
    assert!(
        total(&a, OWNER_PLAYER) > 800,
        "the tasked economy must gather: {}",
        total(&a, OWNER_PLAYER)
    );
    assert_eq!(a.match_state(), MatchState::Ongoing);
}

#[test]
fn empire_wars_ai_match_runs() {
    let mut a = Sim::new_match_victory(99, MatchMode::AiNormal, VictorySettings::empire_wars());
    let mut b = Sim::new_match_victory(99, MatchMode::AiNormal, VictorySettings::empire_wars());
    settle(&mut a, 1800);
    settle(&mut b, 1800);
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn death_match_overrides_empire_wars_stockpiles_when_combined() {
    // The two deltas compose in construction order: the start
    // package lands, then the stockpile delta wins.
    let settings = VictorySettings {
        death_match: true,
        empire_wars: true,
        ..VictorySettings::conquest()
    };
    let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, settings);
    assert_eq!(find_all(&sim, OWNER_PLAYER, EntityKind::Villager).len(), 27);
    assert_eq!(stock(&sim, OWNER_PLAYER, ResourceKind::Food), 20_000);
}

// ---- AI-09 + VIC-08 King of the Hill ------------------------------

fn monument(sim: &Sim) -> (u32, I16F16, I16F16) {
    let b = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Monument)
        .expect("a Monument stands");
    (b.id, b.pos_x, b.pos_y)
}

#[test]
fn monument_stands_at_the_center_gaia_owned() {
    let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    assert_eq!(buildings_of(&sim, OWNER_GAIA, BuildingKind::Monument), 1);
    assert_eq!(sim.monument_owner(), Some(OWNER_GAIA));
    assert_eq!(sim.koth_ticks_left(), KOTH_COUNTDOWN_TICKS);
    let (_, mx, my) = monument(&sim);
    // Within a few tiles of the exact center (the anchor search
    // starts there; only a mid-map forest may push it off).
    assert!(mx.abs() <= I16F16::from_num(24), "monument x {mx}");
    assert!(my.abs() <= I16F16::from_num(24), "monument y {my}");
    // The ordinary opening stands around it.
    assert_eq!(find_all(&sim, OWNER_PLAYER, EntityKind::Villager).len(), 3);
}

#[test]
fn monument_capture_runs_the_countdown_to_victory() {
    let mut sim =
        Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    let (_, mx, my) = monument(&sim);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    sim.push_command(Command::Move {
        entity: scout,
        x: mx,
        y: my,
    });
    assert!(
        run_until(&mut sim, 12_000, |s| s.monument_owner() == Some(OWNER_PLAYER)),
        "the scout's presence must capture the Monument"
    );
    let at_capture = sim.koth_ticks_left();
    assert!(at_capture <= KOTH_COUNTDOWN_TICKS);
    // The countdown runs while the hold stands…
    settle(&mut sim, 600);
    assert_eq!(sim.koth_ticks_left(), at_capture - 600);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    // …and its expiry is the victory, on the tick.
    assert!(
        run_until(&mut sim, KOTH_COUNTDOWN_TICKS + 120, |s| {
            s.match_state() == MatchState::Victory
        }),
        "holding the Monument for the countdown must win"
    );
    assert_eq!(sim.koth_ticks_left(), 0);
}

#[test]
fn monument_steal_resets_the_timer_to_the_floor() {
    let mut sim =
        Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    let (_, mx, my) = monument(&sim);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    sim.push_command(Command::Move {
        entity: scout,
        x: mx,
        y: my,
    });
    assert!(run_until(&mut sim, 12_000, |s| {
        s.monument_owner() == Some(OWNER_PLAYER)
    }));
    settle(&mut sim, 600);
    let before_steal = sim.koth_ticks_left();
    assert!(before_steal < KOTH_COUNTDOWN_TICKS - 500);
    // The holder leaves; a foe villager walks in and takes it.
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::TownCenter)
        .unwrap();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move {
        entity: scout,
        x: tcx,
        y: tcy,
    });
    let foe_villager = find_id(&sim, OWNER_OPPONENT, EntityKind::Villager);
    sim.push_command(Command::Move {
        entity: foe_villager,
        x: mx,
        y: my,
    });
    assert!(
        run_until(&mut sim, 20_000, |s| {
            s.monument_owner() == Some(OWNER_OPPONENT)
        }),
        "an emptied Monument must fall to the foe's presence"
    );
    // The capture came with under 100 years left: the timer
    // reset to the floor (the full countdown on this map size),
    // less the ticks already run under the new owner.
    let after = sim.koth_ticks_left();
    assert!(after > before_steal, "reset: {before_steal} -> {after}");
    assert!(after >= KOTH_COUNTDOWN_TICKS - 2);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn monument_cannot_be_attacked_deleted_or_built() {
    let mut sim =
        Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    let (mid, mx, my) = monument(&sim);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    // Not attackable — before capture (Gaia) or ever.
    assert!(!sim.can_attack_building(scout, mid));
    sim.push_command(Command::AttackBuilding {
        entity: scout,
        building: mid,
    });
    // Not deletable by the Delete command path.
    sim.push_command(Command::DeleteBuilding { building: mid });
    settle(&mut sim, 30);
    assert_eq!(sim.monument_owner(), Some(OWNER_GAIA));
    let hp = sim
        .buildings()
        .iter()
        .find(|b| b.id == mid)
        .map(|b| b.hp)
        .expect("the Monument still stands");
    assert_eq!(hp, I16F16::from_num(4800));
    // Not buildable: a villager ordered to raise one is refused
    // and no second Monument appears.
    let villager = find_id(&sim, OWNER_PLAYER, EntityKind::Villager);
    let before = sim.buildings().len();
    assert!(!sim.can_build(villager, BuildingKind::Monument, mx, my));
    sim.push_command(Command::Build {
        entity: villager,
        kind: BuildingKind::Monument,
        x: mx,
        y: my,
    });
    settle(&mut sim, 30);
    assert_eq!(sim.buildings().len(), before);
}

#[test]
fn koth_stream_is_deterministic() {
    let mut a =
        Sim::new_match_victory(7, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    let mut b =
        Sim::new_match_victory(7, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    let (_, mx, my) = monument(&a);
    let scout = find_id(&a, OWNER_PLAYER, EntityKind::ScoutCavalry);
    for s in [&mut a, &mut b] {
        s.push_command(Command::Move {
            entity: scout,
            x: mx,
            y: my,
        });
    }
    settle(&mut a, 6000);
    settle(&mut b, 6000);
    assert_eq!(a.state_hash(), b.state_hash());
    assert_eq!(a.monument_owner(), Some(OWNER_PLAYER));
}

// Landing addition (save format v14): a save taken mid-countdown
// in a King of the Hill match must round-trip — the countdown is
// hashed state in KotH matches, so without the v14 block the
// load-time hash check could never pass. (Package I's builder
// tree predated the settings round-trip and wrote no save test.)
#[test]
fn koth_save_round_trips_mid_countdown() {
    let mut sim =
        Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::king_of_the_hill());
    let (_, mx, my) = monument(&sim);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    sim.push_command(Command::Move {
        entity: scout,
        x: mx,
        y: my,
    });
    assert!(run_until(&mut sim, 12_000, |s| {
        s.monument_owner() == Some(OWNER_PLAYER)
    }));
    settle(&mut sim, 600);
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a KotH save we wrote must load");
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(restored.koth_ticks_left(), sim.koth_ticks_left());
    assert_eq!(restored.monument_owner(), Some(OWNER_PLAYER));
    assert_eq!(restored.save_game(), bytes);
}
