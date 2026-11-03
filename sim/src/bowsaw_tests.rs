//! TEC-07 wood chain tests: Bow Saw (TechId 43: Lumber Camp,
//! Castle Age, 150 food / 100 wood, 50 s, wood gather +20%,
//! requires Double-Bit Axe) and Two-Man Saw (TechId 44:
//! Lumber Camp, Imperial Age, 300 food / 200 wood, 100 s,
//! wood gather a further +10%, requires Bow Saw) — the two
//! links that complete the Double-Bit Axe → Bow Saw →
//! Two-Man Saw chain named in TEC-07's own text.
//!
//! Values verified 2026-11-03 against the AoE2 wiki
//! (Technology table + per-tech articles). Fixture style
//! follows `monastery_p5f_tests` (direct records; ages/techs
//! set on the player slots). The rows and the gather-rate
//! stacking arithmetic are pinned in `tech::tests`; this file
//! pins the research gates and the queue flow.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::OWNER_PLAYER;
use crate::tech::{Age, TechId};
use crate::{Command, ResearchBlock, Sim};
use fixed::types::I16F16;

/// A standing building record, copied's shape from
/// `monastery_p5f_tests::placed` (complete, full HP).
fn placed(sim: &Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    Building {
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
    }
}

fn top_up(sim: &mut Sim, owner: u8) {
    for kind in [
        ResourceKind::Food,
        ResourceKind::Wood,
        ResourceKind::Gold,
        ResourceKind::Stone,
    ] {
        sim.stockpile_mut(owner).add(kind, I16F16::from_num(5000));
    }
}

/// A sim in `age` with a standing Lumber Camp (id 900) and a
/// Barracks (id 901, the wrong host) for the player, topped up.
fn sim_with_camp(age: Age) -> (Sim, u32, u32) {
    let mut sim = Sim::new(7);
    sim.players[0].age = age;
    top_up(&mut sim, OWNER_PLAYER);
    let camp = placed(&sim, 900, BuildingKind::LumberCamp, OWNER_PLAYER, (30, 30));
    let camp_id = camp.id;
    sim.buildings.push(camp);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    let rax_id = rax.id;
    sim.buildings.push(rax);
    (sim, camp_id, rax_id)
}

#[test]
fn bow_saw_gates() {
    // Feudal: Bow Saw is Castle research — Locked even with
    // Double-Bit Axe held (the Age alone is the block).
    let (mut sim, camp, rax) = sim_with_camp(Age::Feudal);
    sim.players[0].techs |= TechId::DoubleBitAxe.bit();
    assert_eq!(
        sim.research_block_at(camp, TechId::BowSaw),
        ResearchBlock::Locked,
        "Bow Saw locked in Feudal"
    );
    // Castle without Double-Bit Axe: Locked (TEC-07 chain).
    let (mut sim, camp, _rax) = sim_with_camp(Age::Castle);
    assert_eq!(
        sim.research_block_at(camp, TechId::BowSaw),
        ResearchBlock::Locked,
        "Bow Saw locked in Castle without Double-Bit Axe"
    );
    assert!(!sim.can_research_at(camp, TechId::BowSaw));
    // Castle with Double-Bit Axe: available at the Lumber
    // Camp, and only there.
    sim.players[0].techs |= TechId::DoubleBitAxe.bit();
    assert_eq!(
        sim.research_block_at(camp, TechId::BowSaw),
        ResearchBlock::Ok
    );
    assert!(sim.can_research_at(camp, TechId::BowSaw));
    assert_eq!(
        sim.research_block_at(rax, TechId::BowSaw),
        ResearchBlock::Locked,
        "Bow Saw is Lumber Camp research, not Barracks"
    );
}

#[test]
fn two_man_saw_gates() {
    // Castle: Two-Man Saw is Imperial research — Locked even
    // with the whole chain below it held.
    let (mut sim, camp, _rax) = sim_with_camp(Age::Castle);
    sim.players[0].techs |= TechId::DoubleBitAxe.bit() | TechId::BowSaw.bit();
    assert_eq!(
        sim.research_block_at(camp, TechId::TwoManSaw),
        ResearchBlock::Locked,
        "Two-Man Saw locked in Castle"
    );
    // Imperial without Bow Saw: Locked (TEC-07 chain) — even
    // with Double-Bit Axe held.
    let (mut sim, camp, _rax) = sim_with_camp(Age::Imperial);
    sim.players[0].techs |= TechId::DoubleBitAxe.bit();
    assert_eq!(
        sim.research_block_at(camp, TechId::TwoManSaw),
        ResearchBlock::Locked,
        "Two-Man Saw requires Bow Saw in Imperial"
    );
    assert!(!sim.can_research_at(camp, TechId::TwoManSaw));
    // Imperial with Bow Saw: available.
    sim.players[0].techs |= TechId::BowSaw.bit();
    assert_eq!(
        sim.research_block_at(camp, TechId::TwoManSaw),
        ResearchBlock::Ok
    );
    assert!(sim.can_research_at(camp, TechId::TwoManSaw));
}

#[test]
fn bow_saw_research_flow_sets_the_mask_bit() {
    let (mut sim, camp, _rax) = sim_with_camp(Age::Castle);
    sim.players[0].techs |= TechId::DoubleBitAxe.bit();
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::BowSaw));
    let food_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Food)
        .to_num::<i32>();
    let wood_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Wood)
        .to_num::<i32>();
    sim.push_command(Command::ResearchAt {
        building: camp,
        tech: TechId::BowSaw,
    });
    sim.tick();
    // Charged at enqueue (TEC-05): 150 food / 100 wood.
    assert_eq!(
        food_before
            - sim
                .stockpile_of(OWNER_PLAYER)
                .amount(ResourceKind::Food)
                .to_num::<i32>(),
        150,
        "the research costs 150 food"
    );
    assert_eq!(
        wood_before
            - sim
                .stockpile_of(OWNER_PLAYER)
                .amount(ResourceKind::Wood)
                .to_num::<i32>(),
        100,
        "the research costs 100 wood"
    );
    // 50 s of research: not done early, done shortly after.
    for _ in 0..2900 {
        sim.tick();
        assert!(
            !sim.has_tech(OWNER_PLAYER, TechId::BowSaw),
            "Bow Saw completed faster than 50 s"
        );
    }
    let mut done = false;
    for _ in 0..200 {
        sim.tick();
        if sim.has_tech(OWNER_PLAYER, TechId::BowSaw) {
            done = true;
            break;
        }
    }
    assert!(done, "Bow Saw never completed");
    // And its completion unlocks the next link's gate read in
    // Imperial (the chain the flow just walked).
    sim.players[0].age = Age::Imperial;
    assert_eq!(
        sim.research_block_at(camp, TechId::TwoManSaw),
        ResearchBlock::Ok
    );
}
