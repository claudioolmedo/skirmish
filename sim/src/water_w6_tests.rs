//! Water W6 tests: the Imperial University ship trio —
//! Dry Dock (50: ships +1 pierce armor ONLY, the current-DE
//! form — update 169123 removed the speed increase; the W6
//! dossier form is corrected by W6fix, see `tech.rs`),
//! Carvel Hull (51: ships +10% speed, stacking with Clinker
//! Construction) and Shipwright (52: ship wood cost −20% and
//! Dock creation +54% faster, unit heads only) — as TEC-06
//! lookup patches keyed on the SHIP armor class, with the
//! chains Dry Dock ← Careening and Carvel Hull ← Clinker
//! Construction (TEC-07). Costs/times pinned at W6 Passo 0
//! from the wiki Technology table (fetched 2026-11-06):
//! Dry Dock 200 F / 400 G / 60 s, Carvel Hull 350 F / 200 W /
//! 60 s, Shipwright 1 000 F / 300 G / 60 s — VERIFIED, unlike
//! the W5 pair's placeholder times.
//!
//! In-crate staging follows the `water_w5_tests` fixture
//! style (`placed` buildings, direct mask writes for the
//! lookup pins, real ResearchAt / TrainAt flows for the
//! charges and timings).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
use crate::tech::{Age, ProdEntry, TechId};
use crate::units::spec;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
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

/// A complete, idle building record at a cell anchor (the
/// `water_w5_tests::placed` fixture's shape, copied).
fn placed(sim: &mut Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint_size();
    for dy in 0..size {
        for dx in 0..size {
            sim.grid.set_dynamic(cell.0 + dx, cell.1 + dy, true);
        }
    }
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
    }
}

fn stock(sim: &Sim, res: ResourceKind) -> f64 {
    f(sim.stockpile_of(OWNER_PLAYER).amount(res))
}

fn add(sim: &mut Sim, res: ResourceKind, n: i32) {
    sim.stockpile_mut(OWNER_PLAYER)
        .add(res, I16F16::from_num(n));
}

const UNI: u32 = 920;
const DOCK: u32 = 910;

// ------------------------------------------------------------------
// The trio's table rows (W6 Passo 0 pins, wiki Technology table).

#[test]
fn trio_table_rows_match_the_wiki() {
    assert_eq!(TechId::ALL.len(), 53);
    for (tech, tag, cost) in [
        (TechId::DryDock, 50, (200, 0, 400, 0)),
        (TechId::CarvelHull, 51, (350, 200, 0, 0)),
        (TechId::Shipwright, 52, (1000, 0, 300, 0)),
    ] {
        assert_eq!(tech.tag(), tag);
        assert_eq!(TechId::from_tag(tag), Some(tech));
        assert_eq!(tech.cost(), cost, "{tech:?} cost");
        assert_eq!(tech.ticks(), 3600, "{tech:?} — 60 s, VERIFIED (W6 Passo 0)");
        assert_eq!(
            tech.host(),
            crate::tech::TechHost::Building(BuildingKind::University),
            "{tech:?} host"
        );
        assert_eq!(tech.age_required(), Age::Imperial, "{tech:?} age");
    }
    assert_eq!(TechId::from_tag(53), None, "52 is the tail");
    // The chains (TEC-07): Dry Dock is Careening's further
    // upgrade, Carvel Hull is Clinker Construction's; Shipwright
    // stands alone.
    assert_eq!(TechId::DryDock.requires_tech(), Some(TechId::Careening));
    assert_eq!(
        TechId::CarvelHull.requires_tech(),
        Some(TechId::ClinkerConstruction)
    );
    assert_eq!(TechId::Shipwright.requires_tech(), None);
}

// ------------------------------------------------------------------
// Dry Dock: the research flow (host / age / prerequisite gates,
// exact charges) and the SHIP-class patch (armor only —
// current DE; update 169123 removed the speed leg, W6fix).

#[test]
fn dry_dock_researches_and_patches_ships() {
    let mut sim = Sim::new(7);
    let uni = placed(&mut sim, UNI, BuildingKind::University, OWNER_PLAYER, (58, 58));
    sim.buildings.push(uni);
    add(&mut sim, ResourceKind::Food, 2_000);
    add(&mut sim, ResourceKind::Gold, 2_000);

    // Castle Age: Dry Dock is Imperial research — locked.
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_at(UNI, TechId::DryDock),
        crate::tech::ResearchBlock::Locked
    );
    sim.players[0].age = Age::Imperial;
    // Imperial but no Careening: the chain gate locks it too.
    assert_eq!(
        sim.research_block_at(UNI, TechId::DryDock),
        crate::tech::ResearchBlock::Locked
    );
    sim.players[0].techs |= TechId::Careening.bit();
    assert_eq!(
        sim.research_block_at(UNI, TechId::DryDock),
        crate::tech::ResearchBlock::Ok
    );
    // Before (Careening alone): Galley armor 3+1, speed base.
    let galley_speed = spec(EntityKind::Galley).speed.to_bits();
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Galley).armor_pierce, 4);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Galley).speed.to_bits(),
        galley_speed
    );
    let (food0, gold0) = (stock(&sim, ResourceKind::Food), stock(&sim, ResourceKind::Gold));
    sim.push_command(Command::ResearchAt {
        building: UNI,
        tech: TechId::DryDock,
    });
    let done = run_until(&mut sim, 5_000, |s| s.has_tech(OWNER_PLAYER, TechId::DryDock));
    assert!(done, "Dry Dock never completed");
    // Exact charges: 200 food, 400 gold.
    assert_eq!(food0 - stock(&sim, ResourceKind::Food), 200.0);
    assert_eq!(gold0 - stock(&sim, ResourceKind::Gold), 400.0);
    // After: a second +1 pierce (Careening and Dry Dock stack)
    // and NO speed change — Dry Dock is armor-only in current
    // DE (update 169123 removed the +15% speed leg; W6fix
    // corrects the dossier form the W6 build shipped).
    let g = sim.stats_for(OWNER_PLAYER, EntityKind::Galley);
    assert_eq!(g.armor_pierce, 5);
    assert_eq!(g.speed.to_bits(), galley_speed);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::FishingShip).armor_pierce,
        3 // 1 base + Careening + Dry Dock
    );
    // Nothing off the SHIP class moves.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Villager).armor_pierce,
        spec(EntityKind::Villager).armor_pierce
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Villager).speed,
        spec(EntityKind::Villager).speed
    );
}

// ------------------------------------------------------------------
// Carvel Hull: the chain gate, and the multiplicative speed
// stacking Clinker → Carvel at lookup (TEC-08); Dry Dock no
// longer contributes speed (armor only, W6fix).

#[test]
fn carvel_hull_stacks_multiplicatively() {
    let mut sim = Sim::new(7);
    let uni = placed(&mut sim, UNI, BuildingKind::University, OWNER_PLAYER, (58, 58));
    sim.buildings.push(uni);
    add(&mut sim, ResourceKind::Food, 2_000);
    add(&mut sim, ResourceKind::Wood, 2_000);
    sim.players[0].age = Age::Imperial;
    // No Clinker Construction yet: the chain locks Carvel Hull.
    assert_eq!(
        sim.research_block_at(UNI, TechId::CarvelHull),
        crate::tech::ResearchBlock::Locked
    );
    sim.players[0].techs |= TechId::ClinkerConstruction.bit();
    assert_eq!(
        sim.research_block_at(UNI, TechId::CarvelHull),
        crate::tech::ResearchBlock::Ok
    );

    let cog = spec(EntityKind::TradeCog).speed.to_bits(); // 7209
    let speed = |s: &Sim| {
        s.stats_for(OWNER_PLAYER, EntityKind::TradeCog)
            .speed
            .to_bits()
    };
    // Clinker alone (the mask as staged): ×11/10.
    assert_eq!(speed(&sim), ((cog as i64 * 11) / 10) as i32);
    // + Carvel: a second ×11/10 over the layered row.
    sim.players[0].techs |= TechId::CarvelHull.bit();
    let two = (((cog as i64 * 11) / 10) * 11) / 10;
    assert_eq!(speed(&sim), two as i32);
    // + Caravan (the Cog's kind patch layers first) and Dry
    // Dock held alongside: Dry Dock contributes NO speed in
    // current DE (armor only — update 169123 removed the
    // ×115/100 leg; W6fix), so the chain is Caravan → Clinker
    // → Carvel only: 7209 → 8650 → 9515 → 10466.
    sim.players[0].techs |= TechId::Caravan.bit() | TechId::DryDock.bit();
    let layered = {
        let mut b = (cog as i64 * 6) / 5;
        b = (b * 11) / 10;
        (b * 11) / 10
    };
    assert_eq!(layered, 10_466);
    assert_eq!(speed(&sim), layered as i32);
    // Carvel alone on a plain hull: exactly ×11/10, no more.
    sim.players[0].techs = TechId::CarvelHull.bit();
    let galley = spec(EntityKind::Galley).speed.to_bits();
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Galley).speed.to_bits(),
        ((galley as i64 * 11) / 10) as i32
    );
}

// ------------------------------------------------------------------
// Shipwright: the lookup pins — the wood discount keyed on the
// SHIP class, and the Dock unit-head rate gate.

#[test]
fn shipwright_lookup_pins() {
    let with = TechId::Shipwright.bit();
    // Wood −20% on every ship row, exact on the roster (all
    // ship wood costs are multiples of 5).
    for (kind, base, discounted) in [
        (EntityKind::FishingShip, 75, 60),
        (EntityKind::TransportShip, 125, 100),
        (EntityKind::Galley, 90, 72),
        (EntityKind::TradeCog, 100, 80),
    ] {
        assert_eq!(spec(kind).wood_cost, base);
        assert_eq!(crate::tech::train_wood_cost(kind, with), discounted, "{kind:?}");
        assert_eq!(crate::tech::train_wood_cost(kind, 0), base, "{kind:?} unresearched");
    }
    // Off-class kinds never discount: the Trade Cart pays full
    // wood under the same mask.
    assert_eq!(
        crate::tech::train_wood_cost(EntityKind::TradeCart, with),
        spec(EntityKind::TradeCart).wood_cost
    );
    // The rate gate: Dock + ship unit head + the tech — and
    // nothing else. Research heads never scale (the article's
    // explicit clause), other buildings never scale.
    assert!(crate::tech::shipwright_scales(
        BuildingKind::Dock,
        ProdEntry::Unit(EntityKind::Galley),
        with
    ));
    assert!(!crate::tech::shipwright_scales(
        BuildingKind::Dock,
        ProdEntry::Tech(TechId::MediumWarships),
        with
    ));
    assert!(!crate::tech::shipwright_scales(
        BuildingKind::Barracks,
        ProdEntry::Unit(EntityKind::Militia),
        with
    ));
    assert!(!crate::tech::shipwright_scales(
        BuildingKind::Dock,
        ProdEntry::Unit(EntityKind::Galley),
        0
    ));
}

// ------------------------------------------------------------------
// Shipwright: the research flow (Imperial gate, exact charges).

#[test]
fn shipwright_research_flow() {
    let mut sim = Sim::new(7);
    let uni = placed(&mut sim, UNI, BuildingKind::University, OWNER_PLAYER, (58, 58));
    sim.buildings.push(uni);
    add(&mut sim, ResourceKind::Food, 3_000);
    add(&mut sim, ResourceKind::Gold, 1_000);
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_at(UNI, TechId::Shipwright),
        crate::tech::ResearchBlock::Locked
    );
    sim.players[0].age = Age::Imperial;
    assert_eq!(
        sim.research_block_at(UNI, TechId::Shipwright),
        crate::tech::ResearchBlock::Ok
    );
    let (food0, gold0) = (stock(&sim, ResourceKind::Food), stock(&sim, ResourceKind::Gold));
    sim.push_command(Command::ResearchAt {
        building: UNI,
        tech: TechId::Shipwright,
    });
    let done = run_until(&mut sim, 5_000, |s| s.has_tech(OWNER_PLAYER, TechId::Shipwright));
    assert!(done, "Shipwright never completed");
    assert_eq!(food0 - stock(&sim, ResourceKind::Food), 1000.0);
    assert_eq!(gold0 - stock(&sim, ResourceKind::Gold), 300.0);
}

// ------------------------------------------------------------------
// Shipwright at the Dock: the discounted charge, the +54%
// creation rate on a ship head, and the unscaled research head.

/// A seed-42 pond Dock with funds, at Feudal Age; `shipwright`
/// pre-grants the tech by mask write (the lookup discipline —
/// the research flow itself is pinned above).
fn pond_dock_sim(shipwright: bool) -> Sim {
    let mut sim = Sim::new(42);
    let anchor = (0..=117u32)
        .flat_map(|ay| (0..=117u32).map(move |ax| (ax, ay)))
        .find(|&(ax, ay)| sim.grid().dock_placement_ok(ax, ay, 3))
        .expect("seed 42 pond offers a Dock site");
    let dock = placed(&mut sim, DOCK, BuildingKind::Dock, OWNER_PLAYER, anchor);
    sim.buildings.push(dock);
    sim.players[0].age = Age::Feudal;
    if shipwright {
        sim.players[0].techs |= TechId::Shipwright.bit();
    }
    add(&mut sim, ResourceKind::Wood, 1_000);
    add(&mut sim, ResourceKind::Gold, 1_000);
    sim
}

fn ticks_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> Option<u64> {
    for n in 1..=cap {
        sim.tick();
        if done(sim) {
            return Some(n);
        }
    }
    None
}

#[test]
fn shipwright_discounts_and_speeds_dock_training() {
    // The discount gates affordability itself: 71 wood cannot
    // train a Galley for a holder (72 can); a non-holder needs
    // the full 90.
    let mut sim = pond_dock_sim(true);
    // Drain the wood stock (the match-start stockpile plus the
    // fixture's grant) to exactly 71: one short of the
    // discounted 72 a Shipwright holder needs for a Galley.
    let drain = stock(&sim, ResourceKind::Wood) as i32 - 71;
    sim.stockpile_mut(OWNER_PLAYER)
        .spend(ResourceKind::Wood, I16F16::from_num(drain));
    assert_eq!(stock(&sim, ResourceKind::Wood), 71.0);
    assert!(!sim.can_train_at(DOCK, EntityKind::Galley));
    add(&mut sim, ResourceKind::Wood, 1); // 72
    assert!(sim.can_train_at(DOCK, EntityKind::Galley));

    // The charge is exactly the discounted 72 wood / 30 gold,
    // and the Galley (2160 base ticks) is born on tick 1403 —
    // the first tick whose ×154/100 accrual reaches 2160
    // (floor(154·1402/100) = 2159, floor(154·1403/100) = 2160).
    let (wood0, gold0) = (stock(&sim, ResourceKind::Wood), stock(&sim, ResourceKind::Gold));
    sim.push_command(Command::TrainAt {
        building: DOCK,
        kind: EntityKind::Galley,
    });
    let born_at = ticks_until(&mut sim, 3_000, |s| {
        s.entities()
            .iter()
            .any(|e: &Entity| e.kind == EntityKind::Galley && !e.dead)
    });
    assert_eq!(born_at, Some(1403), "Shipwright Galley birth tick");
    assert_eq!(wood0 - stock(&sim, ResourceKind::Wood), 72.0);
    assert_eq!(gold0 - stock(&sim, ResourceKind::Gold), 30.0);

    // Control: the same Dock without Shipwright charges 90 and
    // takes the full 2160 ticks.
    let mut sim = pond_dock_sim(false);
    let wood0 = stock(&sim, ResourceKind::Wood);
    sim.push_command(Command::TrainAt {
        building: DOCK,
        kind: EntityKind::Galley,
    });
    let born_at = ticks_until(&mut sim, 3_000, |s| {
        s.entities()
            .iter()
            .any(|e: &Entity| e.kind == EntityKind::Galley && !e.dead)
    });
    assert_eq!(born_at, Some(2160), "unscaled Galley birth tick");
    assert_eq!(wood0 - stock(&sim, ResourceKind::Wood), 90.0);

    // A research head at a Shipwright Dock is NOT scaled: the
    // Medium Warships research (3000 ticks) takes all 3000.
    let mut sim = pond_dock_sim(true);
    sim.players[0].age = Age::Castle;
    add(&mut sim, ResourceKind::Food, 500);
    sim.push_command(Command::ResearchAt {
        building: DOCK,
        tech: TechId::MediumWarships,
    });
    let done_at = ticks_until(&mut sim, 4_000, |s| {
        s.has_tech(OWNER_PLAYER, TechId::MediumWarships)
    });
    assert_eq!(done_at, Some(3000), "research heads never scale under Shipwright");
}
