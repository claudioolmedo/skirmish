//! Imperial Age rung tests (bot-loop TEC-01 package, 2026-10-29):
//! the Castle → Imperial advancement at the pinned numbers
//! (1000 food / 800 gold, 190 s at the Town Center — study §7),
//! TEC-02's Imperial gate (a completed Castle alone suffices, or
//! any two Castle-Age buildings), and the documented Imperial
//! stat steps that land with the age (Town Center armor 6/8 per
//! BLD-11, the Market's 3/10 per BLD-19, the Scout line's +2 LoS
//! per-advance step to 10 tiles per CLS-04).
//!
//! In-crate staging (direct age/stock/building records) follows
//! the `castle_p3_tests` fixture style: the full-economy play-up
//! of this path lives in the Castle suite's reach helpers; here
//! the rung itself is measured exactly, in seconds.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_PLAYER};
use crate::tech::{Age, ResearchBlock, TechId};
use crate::{player_slot, Command, Sim};
use fixed::types::I16F16;

/// A complete, idle building record at a cell anchor (the
/// `castle_p3_tests::placed` fixture's shape, completed).
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
    }
}

fn stock(sim: &Sim, kind: ResourceKind) -> I16F16 {
    sim.stockpile_of(OWNER_PLAYER).amount(kind)
}

#[test]
fn imperial_age_gates_cost_timing_and_steps() {
    let mut sim = Sim::new(7);
    // Stage: the player stands in the Castle Age.
    let slot = player_slot(OWNER_PLAYER).expect("player slot");
    sim.players[slot].age = Age::Castle;
    assert_eq!(sim.age_of(OWNER_PLAYER), Age::Castle);

    // TEC-02's Imperial gate: no Castle and no Castle-Age
    // building yet — the advancement is Locked (the gate is
    // checked before funds, so this reads Locked at any stock).
    assert_eq!(sim.imperial_building_count(OWNER_PLAYER), 0);
    assert!(!sim.imperial_gate_met(OWNER_PLAYER));
    assert_eq!(
        sim.research_block_tc(TechId::ImperialAge),
        ResearchBlock::Locked
    );
    assert!(!sim.can_research_tc(TechId::ImperialAge));

    // A completed Castle alone satisfies the gate; with the
    // starting stock (200 F / 100 G) the block becomes NoFunds.
    let castle = placed(&sim, 9001, BuildingKind::Castle, OWNER_PLAYER, (40, 20));
    sim.buildings.push(castle);
    assert!(sim.imperial_gate_met(OWNER_PLAYER));
    assert_eq!(sim.imperial_building_count(OWNER_PLAYER), 1);
    assert_eq!(
        sim.research_block_tc(TechId::ImperialAge),
        ResearchBlock::NoFunds
    );

    // Grant the click and measure it exactly.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(2000));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(2000));
    assert_eq!(
        sim.research_block_tc(TechId::ImperialAge),
        ResearchBlock::Ok
    );
    let (food0, gold0) = (stock(&sim, ResourceKind::Food), stock(&sim, ResourceKind::Gold));
    let t0 = sim.tick_count();
    sim.push_command(Command::ResearchTc {
        tech: TechId::ImperialAge,
    });
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 1);
    assert_eq!(stock(&sim, ResourceKind::Food), food0 - I16F16::from_num(1000));
    assert_eq!(stock(&sim, ResourceKind::Gold), gold0 - I16F16::from_num(800));
    for _ in 0..11398 {
        sim.tick();
    }
    assert_eq!(sim.age_of(OWNER_PLAYER), Age::Castle, "not yet Imperial");
    sim.tick();
    assert_eq!(sim.tick_count() - t0, 11400);
    assert_eq!(sim.age_of(OWNER_PLAYER), Age::Imperial);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::ImperialAge));
    assert!(sim.tech_mask_of(OWNER_PLAYER) & TechId::ImperialAge.bit() != 0);
    assert_eq!(
        sim.research_block_tc(TechId::ImperialAge),
        ResearchBlock::AlreadyDone
    );

    // The documented Imperial steps land with the age.
    let tc_stats = sim.stats_for(OWNER_PLAYER, EntityKind::TownCenter);
    assert_eq!((tc_stats.armor_melee, tc_stats.armor_pierce), (6, 8));
    let scout = sim.stats_for(OWNER_PLAYER, EntityKind::ScoutCavalry);
    assert_eq!(scout.los, I16F16::from_bits(2_621_440)); // 40 wu = 10 tiles
}
