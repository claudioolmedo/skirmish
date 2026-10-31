//! HOUSED front (Claudio's decision, 2026-10-27): training is
//! REFUSED at enqueue when the effective population (living +
//! already queued) is at the cap — the unit never enters a queue
//! to stall at 0%, and no cost is charged for the refusal. Queueing
//! exactly up to the cap stays legal, the gate is shared across
//! every production building of the owner, and research is never
//! gated by population.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_PLAYER};
use crate::tech::TechId;
use crate::{Command, ResearchBlock, Sim, TrainBlock};
use fixed::types::I16F16;

/// A complete, idle building record at a cell anchor (the
/// new_units_tests `placed` fixture's shape).
fn placed(sim: &Sim, id: u32, kind: BuildingKind, cell: (u32, u32)) -> Building {
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    Building {
        id,
        kind,
        owner: OWNER_PLAYER,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: cell.0,
        cell_y: cell.1,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: 3,
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

fn food(sim: &Sim) -> I16F16 {
    sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)
}

#[test]
fn tc_refuses_enqueue_at_the_cap_and_charges_nothing() {
    let mut sim = Sim::new(7);
    // Standard start: 3 villagers + Scout = 4 living, cap 5.
    assert_eq!(sim.population(), 4);
    assert_eq!(sim.population_cap(), 5);
    assert!(sim.can_train_villager());

    // The first villager fills the last population slot.
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.train_queue_len(), 1);
    assert_eq!(food(&sim), I16F16::from_num(150));
    assert_eq!(sim.population(), 5, "living + queued is now at the cap");

    // At the cap the next enqueue is REFUSED: not queued, not
    // charged, and the block reason is Housed.
    assert!(!sim.can_train_villager());
    assert_eq!(sim.train_block_reason(), TrainBlock::Housed);
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.train_queue_len(), 1, "housed order entered the queue");
    assert_eq!(food(&sim), I16F16::from_num(150), "refusal charged food");
}

#[test]
fn barracks_shares_the_same_cap_gate() {
    let mut sim = Sim::new(7);
    let rax = placed(&sim, 900, BuildingKind::Barracks, (58, 58));
    sim.buildings.push(rax);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));

    // Queue the TC villager that fills the cap (4 living + 1 queued).
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.population(), 5);

    // Claudio's 2026-10-24 live case: resources are plentiful, yet
    // the Barracks must refuse the Militia while housed.
    assert!(!sim.can_train_at(900, EntityKind::Militia));
    assert_eq!(
        sim.train_at_block(900, EntityKind::Militia),
        TrainBlock::Housed
    );
    let (f0, g0) = (food(&sim), sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold));
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::Militia,
    });
    sim.tick();
    assert_eq!(sim.building_queue(900).len(), 0, "housed Militia was queued");
    assert_eq!(food(&sim), f0, "refusal charged food");
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold),
        g0,
        "refusal charged gold"
    );
}

#[test]
fn queueing_exactly_up_to_the_cap_stays_legal() {
    let mut sim = Sim::new(7);
    // A completed House raises the cap to 10.
    let house = placed(&sim, 901, BuildingKind::House, (64, 58));
    sim.buildings.push(house);
    assert_eq!(sim.population_cap(), 10);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));

    // Six villagers take the effective population 4 -> 10: every
    // one of them must be accepted…
    for i in 0..6 {
        assert!(sim.can_train_villager(), "enqueue {} refused early", i + 1);
        sim.push_command(Command::TrainVillager);
        sim.tick();
    }
    assert_eq!(sim.train_queue_len(), 6);
    assert_eq!(sim.population(), 10);
    assert_eq!(food(&sim), I16F16::from_num(700 - 300));

    // …and the seventh is the housed refusal.
    assert!(!sim.can_train_villager());
    assert_eq!(sim.train_block_reason(), TrainBlock::Housed);
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.train_queue_len(), 6);
}

#[test]
fn research_is_never_gated_by_population() {
    let mut sim = Sim::new(7);
    // Fill the cap with the TC's own queue…
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.train_block_reason(), TrainBlock::Housed);
    // …and Loom (50 gold, the TC's research) still enqueues:
    // population gates units, not ideas.
    assert_eq!(sim.research_block_tc(TechId::Loom), ResearchBlock::Ok);
    assert!(sim.can_research_tc(TechId::Loom));
}
