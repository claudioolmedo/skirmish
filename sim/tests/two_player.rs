//! Two-player mode tests (M8): the mirrored match spawns exactly as
//! in the AI modes, but NO script drives the second seat — over
//! thousands of ticks the opponent's economy, queue, and army stay
//! precisely at their starting state until a command says otherwise.
//! Both seats must be fully playable through the per-seat command
//! forms lockstep uses (`TrainVillagerAt`, `ResearchTcAt`,
//! `SetRallyTc`), and two instances fed the same interleaved command
//! stream stay bit-identical.

use fixed::types::I16F16;
use skirmish_sim::{
    Command, EntityKind, MatchMode, Sim, TechId, OWNER_OPPONENT, OWNER_PLAYER,
};

fn fx(v: f64) -> I16F16 {
    I16F16::from_num(v)
}

fn tc_of(sim: &Sim, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == owner)
        .map(|e| e.id)
        .expect("a Town Center for the seat")
}

#[test]
fn two_player_mode_is_tag_3_and_spawns_the_mirror_start() {
    assert_eq!(MatchMode::TwoPlayer.tag(), 3);
    assert_eq!(MatchMode::from_tag(3), Some(MatchMode::TwoPlayer));
    assert!(MatchMode::TwoPlayer.is_match());
    let sim = Sim::with_mode(42, MatchMode::TwoPlayer);
    assert_eq!(sim.population_of(OWNER_PLAYER), 4);
    assert_eq!(sim.population_of(OWNER_OPPONENT), 4);
    let _ = tc_of(&sim, OWNER_OPPONENT);
}

#[test]
fn no_script_drives_the_second_seat() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let foe_tc = tc_of(&sim, OWNER_OPPONENT);
    let start = sim.stockpile_of(OWNER_OPPONENT).amounts();
    // 10 in-game minutes with no commands at all: in an AI mode the
    // opponent would have trained villagers, built, and banked
    // resources by now. Here nothing may move.
    for _ in 0..36_000 {
        sim.tick();
    }
    assert_eq!(sim.stockpile_of(OWNER_OPPONENT).amounts(), start);
    assert_eq!(sim.population_of(OWNER_OPPONENT), 4);
    assert!(sim.tc_queue_of(foe_tc).is_empty());
    assert_eq!(sim.match_state(), skirmish_sim::MatchState::Ongoing);
}

#[test]
fn the_second_seat_plays_through_per_seat_commands() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let foe_tc = tc_of(&sim, OWNER_OPPONENT);
    // Rally the guest Town Center, then train at it: the new
    // villager must belong to the second seat and walk to the rally.
    let (rx, ry) = {
        let tc = &sim.entities()[foe_tc as usize];
        (tc.pos_x + fx(20.0), tc.pos_y)
    };
    sim.push_command(Command::SetRallyTc { tc: foe_tc, x: rx, y: ry });
    sim.push_command(Command::TrainVillagerAt { tc: foe_tc });
    sim.tick();
    assert_eq!(sim.tc_queue_of(foe_tc).len(), 1);
    let rally = sim.tc_rally_of(foe_tc).expect("rally stored");
    assert_eq!(rally.0, rx);
    // Villager training takes 1500 ticks; give it room to finish and
    // start walking.
    for _ in 0..1_600 {
        sim.tick();
    }
    assert_eq!(sim.population_of(OWNER_OPPONENT), 5);
    let vil = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == OWNER_OPPONENT && !e.dead)
        .max_by_key(|e| e.id)
        .expect("the new villager");
    assert!(
        vil.order.is_some() || vil.pos_x != sim.entities()[foe_tc as usize].pos_x,
        "the new villager heads for the rally point"
    );
    // Research at the guest Town Center works too (Loom: 50 gold).
    sim.push_command(Command::ResearchTcAt { tc: foe_tc, tech: TechId::Loom });
    sim.tick();
    assert_eq!(sim.tc_queue_of(foe_tc).len(), 1);
}

#[test]
fn interleaved_command_streams_stay_bit_identical() {
    // The lockstep contract itself: two instances, the same seed,
    // the same commands pushed in the same order (both seats mixed),
    // produce the same hash at every tick.
    let mut a = Sim::with_mode(99, MatchMode::TwoPlayer);
    let mut b = Sim::with_mode(99, MatchMode::TwoPlayer);
    let tc_a = tc_of(&a, OWNER_PLAYER);
    let tc_b = tc_of(&a, OWNER_OPPONENT);
    let script: Vec<(u64, Command)> = vec![
        (0, Command::TrainVillagerAt { tc: tc_a }),
        (0, Command::TrainVillagerAt { tc: tc_b }),
        (30, Command::ResearchTcAt { tc: tc_b, tech: TechId::Loom }),
        (60, Command::SetRallyTc { tc: tc_a, x: fx(0.0), y: fx(-40.0) }),
    ];
    for t in 0..3_000u64 {
        for (at, cmd) in &script {
            if *at == t {
                a.push_command(cmd.clone());
                b.push_command(cmd.clone());
            }
        }
        assert_eq!(a.tick(), b.tick(), "hash diverged at tick {t}");
    }
}
