//! Package M — Conscription (TEC-09's queue half): the Castle's
//! Imperial research that makes military production buildings
//! work 33% faster. The Treadmill Crane half of the same rule
//! landed with Castle P3c (`university_p3c_tests`); this module
//! pins the queue scalar: ×4/3 progress at the Barracks, Archery
//! Range, Stable and Castle, for unit and research heads alike,
//! carried exactly by the per-building rate remainder — and the
//! documented exclusions (Siege Workshop, Market, Blacksmith,
//! Town Center) at full rate.
//!
//! Fixture style follows `university_p3c_tests`: buildings are
//! placed directly, tech bits are granted or researched through
//! the real command path where the flow itself is the subject.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{conscription_scales, Age, ProdEntry, ResearchBlock, TechId, TechHost};
use crate::{player_slot, Command, Sim};
use fixed::types::I16F16;

fn sim_in_imperial() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Imperial;
    sim.players[1].age = Age::Imperial;
    sim
}

fn placed(sim: &mut Sim, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> u32 {
    let id = 900 + sim.buildings.len() as u32;
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    let hp = sim.building_max_hp(kind, owner);
    sim.buildings.push(Building {
        id,
        kind,
        owner,
        hp: I16F16::from_num(hp),
        cell_x: cell.0,
        cell_y: cell.1,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: kind.build_thirds(),
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        build_rate_remainder: 0,
        queue_rate_remainder: 0,
        gate_along_x: true,
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
    });
    id
}

fn top_up(sim: &mut Sim, owner: u8) {
    for k in [
        ResourceKind::Food,
        ResourceKind::Wood,
        ResourceKind::Gold,
        ResourceKind::Stone,
    ] {
        sim.stockpile_mut(owner).add(k, I16F16::from_num(5000));
    }
}

fn grant(sim: &mut Sim, owner: u8, tech: TechId) {
    let slot = player_slot(owner).expect("owner slot");
    sim.players[slot].techs |= tech.bit();
}

fn run_until(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn enqueue(sim: &mut Sim, building: u32, entry: ProdEntry) {
    let b = sim
        .buildings
        .iter_mut()
        .find(|b| b.id == building)
        .expect("building stands");
    b.queue.push(entry);
}

/// Ticks until the building's queue drains completely (bounded).
fn drain_ticks(sim: &mut Sim, building: u32, cap: u64) -> u64 {
    let mut n = 0;
    while !sim.building_queue(building).is_empty() && n < cap {
        sim.tick();
        n += 1;
    }
    assert!(
        sim.building_queue(building).is_empty(),
        "queue at {building} never drained within {cap} ticks"
    );
    n
}

fn building(sim: &Sim, id: u32) -> &Building {
    sim.buildings.iter().find(|b| b.id == id).expect("building")
}

// ----------------------------------------------------------------
// The tech row and the affected set.

#[test]
fn conscription_row_matches_the_wiki() {
    // Wiki, Conscription (fetched 2026-10-30 for the wave-2
    // audit): Castle, Imperial Age, 150 F / 150 G, 60 s.
    assert_eq!(TechId::Conscription.cost(), (150, 0, 150, 0));
    assert_eq!(TechId::Conscription.ticks(), 60 * 60);
    assert_eq!(
        TechId::Conscription.host(),
        TechHost::Building(BuildingKind::Castle)
    );
    assert_eq!(TechId::Conscription.age_required(), Age::Imperial);
    assert_eq!(TechId::Conscription.requires_tech(), None);
    assert_eq!(TechId::Conscription.tag(), 29);
}

#[test]
fn conscription_scales_exactly_the_military_production_set() {
    let with = TechId::Conscription.bit();
    for kind in [
        BuildingKind::Barracks,
        BuildingKind::ArcheryRange,
        BuildingKind::Stable,
        BuildingKind::Castle,
    ] {
        assert!(conscription_scales(kind, with), "{kind:?} scales");
        assert!(!conscription_scales(kind, 0), "{kind:?} needs the tech");
    }
    // The documented exclusions: the Siege Workshop by name, and
    // every non-military host (Market queues and Blacksmith
    // research included) by the rule's letter.
    for kind in [
        BuildingKind::SiegeWorkshop,
        BuildingKind::Market,
        BuildingKind::Blacksmith,
        BuildingKind::University,
        BuildingKind::House,
    ] {
        assert!(!conscription_scales(kind, with), "{kind:?} never scales");
    }
}

// ----------------------------------------------------------------
// The research flow at the Castle.

#[test]
fn conscription_research_gates_and_flow() {
    let mut sim = sim_in_imperial();
    top_up(&mut sim, OWNER_PLAYER);
    let castle = placed(&mut sim, BuildingKind::Castle, OWNER_PLAYER, (20, 20));
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (30, 20));
    // Host law: the Castle only.
    assert_eq!(
        sim.research_block_at(rax, TechId::Conscription),
        ResearchBlock::Locked
    );
    // Age law: a Castle Age owner is Locked at the Castle itself.
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_at(castle, TechId::Conscription),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Imperial;
    assert_eq!(
        sim.research_block_at(castle, TechId::Conscription),
        ResearchBlock::Ok
    );
    // The real flow: enqueue, pay exactly 150 F / 150 G, and the
    // bit lands after exactly 3600 ticks — the researching
    // Castle's own queue is NOT sped by the tech it is still
    // researching.
    let amt = |sim: &Sim, k: ResourceKind| {
        sim.stockpile_of(OWNER_PLAYER).amount(k).to_num::<i32>()
    };
    let food_before = amt(&sim, ResourceKind::Food);
    let gold_before = amt(&sim, ResourceKind::Gold);
    sim.push_command(Command::ResearchAt {
        building: castle,
        tech: TechId::Conscription,
    });
    run_until(&mut sim, 1);
    assert_eq!(amt(&sim, ResourceKind::Food), food_before - 150);
    assert_eq!(amt(&sim, ResourceKind::Gold), gold_before - 150);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::Conscription));
    run_until(&mut sim, 3598);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::Conscription));
    run_until(&mut sim, 1);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::Conscription));
    assert_eq!(
        sim.research_block_at(castle, TechId::Conscription),
        ResearchBlock::AlreadyDone
    );
}

// ----------------------------------------------------------------
// The rate itself.

#[test]
fn conscription_speeds_a_barracks_queue_by_exactly_a_third() {
    let mut sim = sim_in_imperial();
    top_up(&mut sim, OWNER_PLAYER);
    top_up(&mut sim, OWNER_OPPONENT);
    placed(&mut sim, BuildingKind::House, OWNER_PLAYER, (60, 60));
    let mine = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (20, 20));
    // The sandbox opponent has no opening of its own (no Town
    // Center, cap 0 — a unit head would read as housed forever),
    // so its Castle stands as its population support.
    placed(&mut sim, BuildingKind::Castle, OWNER_OPPONENT, (80, 60));
    let theirs = placed(&mut sim, BuildingKind::Barracks, OWNER_OPPONENT, (80, 20));
    grant(&mut sim, OWNER_PLAYER, TechId::Conscription);
    enqueue(&mut sim, mine, ProdEntry::Unit(EntityKind::Militia));
    enqueue(&mut sim, theirs, ProdEntry::Unit(EntityKind::Militia));
    // Militia train in 1260 ticks. Both queues run side by side:
    // mine must drain at 945 (1260 × 3/4), theirs at the full
    // 1260 — the scalar is the owner's, not the building's.
    let mut my_drain = None;
    let mut their_drain = None;
    for n in 1..=1260 {
        sim.tick();
        if my_drain.is_none() && sim.building_queue(mine).is_empty() {
            my_drain = Some(n);
        }
        if their_drain.is_none() && sim.building_queue(theirs).is_empty() {
            their_drain = Some(n);
        }
    }
    assert_eq!(my_drain, Some(945), "Conscription queue at 3/4 time");
    assert_eq!(their_drain, Some(1260), "unscaled queue at full time");
    // The unscaled building's remainder never woke.
    assert_eq!(building(&sim, theirs).queue_rate_remainder, 0);
}

#[test]
fn conscription_remainder_carries_thirds_exactly() {
    let mut sim = sim_in_imperial();
    grant(&mut sim, OWNER_PLAYER, TechId::Conscription);
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (20, 20));
    placed(&mut sim, BuildingKind::House, OWNER_PLAYER, (60, 60));
    enqueue(&mut sim, rax, ProdEntry::Unit(EntityKind::Militia));
    enqueue(&mut sim, rax, ProdEntry::Unit(EntityKind::Spearman));
    // The accrual pattern over the first three ticks: +1 (r 1),
    // +1 (r 2), +2 (r 0) — four ticks of progress in three ticks.
    sim.tick();
    assert_eq!(building(&sim, rax).train_progress, 1);
    assert_eq!(building(&sim, rax).queue_rate_remainder, 1);
    sim.tick();
    assert_eq!(building(&sim, rax).train_progress, 2);
    assert_eq!(building(&sim, rax).queue_rate_remainder, 2);
    sim.tick();
    assert_eq!(building(&sim, rax).train_progress, 4);
    assert_eq!(building(&sim, rax).queue_rate_remainder, 0);
    // The mixed queue (1260 + 1320 = 2580 ticks of work) drains
    // in exactly 3/4 of the total: the remainder carries across
    // the head change, nothing is lost at the boundary.
    let rest = drain_ticks(&mut sim, rax, 2580);
    assert_eq!(3 + rest, 1935, "2580 × 3/4 across two heads");
}

#[test]
fn conscription_speeds_research_heads_and_the_castle_itself() {
    let mut sim = sim_in_imperial();
    grant(&mut sim, OWNER_PLAYER, TechId::Conscription);
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (20, 20));
    let castle = placed(&mut sim, BuildingKind::Castle, OWNER_PLAYER, (40, 20));
    // A research head in a military building rides the same
    // scalar (TEC-09: "units and techs alike in that building"):
    // Man-at-Arms' 2400 ticks land in 1800.
    enqueue(&mut sim, rax, ProdEntry::Tech(TechId::ManAtArms));
    assert_eq!(drain_ticks(&mut sim, rax, 2400), 1800);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::ManAtArms));
    // And the Castle's own production queue — the Petard's 1500
    // ticks land in 1125.
    enqueue(&mut sim, castle, ProdEntry::Unit(EntityKind::Petard));
    assert_eq!(drain_ticks(&mut sim, castle, 1500), 1125);
}

#[test]
fn conscription_leaves_the_excluded_queues_at_full_rate() {
    let mut sim = sim_in_imperial();
    top_up(&mut sim, OWNER_PLAYER);
    grant(&mut sim, OWNER_PLAYER, TechId::Conscription);
    let shop = placed(&mut sim, BuildingKind::SiegeWorkshop, OWNER_PLAYER, (20, 20));
    let market = placed(&mut sim, BuildingKind::Market, OWNER_PLAYER, (30, 20));
    let smith = placed(&mut sim, BuildingKind::Blacksmith, OWNER_PLAYER, (40, 20));
    placed(&mut sim, BuildingKind::House, OWNER_PLAYER, (60, 60));
    // Siege Workshop (explicitly excluded): the Ram's 2160 ticks.
    enqueue(&mut sim, shop, ProdEntry::Unit(EntityKind::BatteringRam));
    assert_eq!(drain_ticks(&mut sim, shop, 2160), 2160);
    assert_eq!(building(&sim, shop).queue_rate_remainder, 0);
    // Market (not military production): the Trade Cart's 3060.
    enqueue(&mut sim, market, ProdEntry::Unit(EntityKind::TradeCart));
    assert_eq!(drain_ticks(&mut sim, market, 3060), 3060);
    // Blacksmith (research host, not a production building):
    // Fletching's 1800 ticks.
    enqueue(&mut sim, smith, ProdEntry::Tech(TechId::Fletching));
    assert_eq!(drain_ticks(&mut sim, smith, 1800), 1800);
}

// ----------------------------------------------------------------
// Save/restore carries the remainder (NET-09's codec discipline).

#[test]
fn conscription_remainder_survives_a_save_round_trip() {
    let mut sim = sim_in_imperial();
    grant(&mut sim, OWNER_PLAYER, TechId::Conscription);
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (20, 20));
    enqueue(&mut sim, rax, ProdEntry::Unit(EntityKind::Militia));
    // One tick in, the remainder stands at 1 — mid-pattern state
    // a save must not flatten.
    sim.tick();
    assert_eq!(building(&sim, rax).queue_rate_remainder, 1);
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("a save we wrote must load");
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(building(&restored, rax).queue_rate_remainder, 1);
    // Both copies continue bit-identically.
    run_until(&mut sim, 300);
    run_until(&mut restored, 300);
    assert_eq!(restored.state_hash(), sim.state_hash());
}

