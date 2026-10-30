//! Package K tests (2026-10-30): tribute (TRD-04) with the
//! Coinage/Banking fee ladder and the diplomacy validations, and
//! the line-of-sight technologies (VIS-03) — Town Watch / Town
//! Patrol over the per-kind building sight table, exclusions
//! included. Expectations are the documented DE values (wiki,
//! Tribute; Coinage; Banking; Town Watch; Town Patrol — the
//! wave-2 fidelity audit's Package K), not recomputed ones.
//! In-crate staging follows the `trade_tests` fixture style.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, ResearchBlock, TechId};
use crate::{Command, Sim};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn held(sim: &Sim, owner: u8, kind: ResourceKind) -> f64 {
    f(sim.stockpile_of(owner).amount(kind))
}

fn give(sim: &mut Sim, owner: u8, kind: ResourceKind, amount: i32) {
    sim.stockpile_mut(owner)
        .add(kind, I16F16::from_num(amount));
}

/// A complete, idle building record at a cell anchor (the
/// `trade_tests::placed` fixture's shape).
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

// ------------------------------------------------------------------
// TRD-04: the transfer itself.

#[test]
fn tribute_pays_amount_plus_fee_and_credits_in_full() {
    let mut sim = Sim::new(7);
    give(&mut sim, OWNER_PLAYER, ResourceKind::Food, 800); // holds 1000
    assert!(sim.can_tribute(OWNER_PLAYER, OWNER_OPPONENT, ResourceKind::Food, 100));
    sim.push_command(Command::Tribute {
        sender: OWNER_PLAYER,
        recipient: OWNER_OPPONENT,
        resource: ResourceKind::Food,
        amount: 100,
    });
    sim.tick();
    // 30% fee: 100 + 30 leaves the sender; the full 100 lands.
    assert_eq!(held(&sim, OWNER_PLAYER, ResourceKind::Food), 870.0);
    assert_eq!(held(&sim, OWNER_OPPONENT, ResourceKind::Food), 300.0);
}

#[test]
fn tribute_fee_rounds_up_to_whole_resources() {
    let mut sim = Sim::new(7);
    give(&mut sim, OWNER_PLAYER, ResourceKind::Food, 800); // holds 1000
    sim.push_command(Command::Tribute {
        sender: OWNER_PLAYER,
        recipient: OWNER_OPPONENT,
        resource: ResourceKind::Food,
        amount: 101,
    });
    sim.tick();
    // 30% of 101 is 30.3 — the adopted reading rounds the fee UP
    // to 31, so 132 leaves and 101 lands.
    assert_eq!(held(&sim, OWNER_PLAYER, ResourceKind::Food), 868.0);
    assert_eq!(held(&sim, OWNER_OPPONENT, ResourceKind::Food), 301.0);
}

#[test]
fn tribute_fee_ladder_coinage_then_banking() {
    let mut sim = Sim::new(7);
    give(&mut sim, OWNER_PLAYER, ResourceKind::Gold, 900); // holds 1000
    assert_eq!(sim.tribute_fee_pct_of(OWNER_PLAYER), 30);
    sim.players[0].techs |= TechId::Coinage.bit();
    assert_eq!(sim.tribute_fee_pct_of(OWNER_PLAYER), 20);
    sim.push_command(Command::Tribute {
        sender: OWNER_PLAYER,
        recipient: OWNER_OPPONENT,
        resource: ResourceKind::Gold,
        amount: 100,
    });
    sim.tick();
    assert_eq!(held(&sim, OWNER_PLAYER, ResourceKind::Gold), 880.0);
    assert_eq!(held(&sim, OWNER_OPPONENT, ResourceKind::Gold), 200.0);
    sim.players[0].techs |= TechId::Banking.bit();
    assert_eq!(sim.tribute_fee_pct_of(OWNER_PLAYER), 0);
    sim.push_command(Command::Tribute {
        sender: OWNER_PLAYER,
        recipient: OWNER_OPPONENT,
        resource: ResourceKind::Gold,
        amount: 100,
    });
    sim.tick();
    // Banking: no fee at all — 100 out, 100 in.
    assert_eq!(held(&sim, OWNER_PLAYER, ResourceKind::Gold), 780.0);
    assert_eq!(held(&sim, OWNER_OPPONENT, ResourceKind::Gold), 300.0);
}

#[test]
fn tribute_refusals_move_nothing() {
    let mut sim = Sim::new(7);
    let before = (
        held(&sim, OWNER_PLAYER, ResourceKind::Food),
        held(&sim, OWNER_OPPONENT, ResourceKind::Food),
    );
    // To oneself, to Gaia, of nothing, and more than amount + fee
    // covers (200 held, 200 sent needs 260): all refused whole.
    for (sender, recipient, amount) in [
        (OWNER_PLAYER, OWNER_PLAYER, 50),
        (OWNER_PLAYER, OWNER_GAIA, 50),
        (OWNER_PLAYER, OWNER_OPPONENT, 0),
        (OWNER_PLAYER, OWNER_OPPONENT, 200),
    ] {
        assert!(!sim.can_tribute(sender, recipient, ResourceKind::Food, amount));
        sim.push_command(Command::Tribute {
            sender,
            recipient,
            resource: ResourceKind::Food,
            amount,
        });
    }
    sim.tick();
    assert_eq!(
        (
            held(&sim, OWNER_PLAYER, ResourceKind::Food),
            held(&sim, OWNER_OPPONENT, ResourceKind::Food)
        ),
        before
    );
}

#[test]
fn tribute_gate_needs_thirty_of_the_resource() {
    let mut sim = Sim::new(7);
    // Spend the starting 200 food down to 29.
    let spent = sim
        .stockpile_mut(OWNER_PLAYER)
        .spend(ResourceKind::Food, I16F16::from_num(171));
    assert!(spent);
    assert_eq!(held(&sim, OWNER_PLAYER, ResourceKind::Food), 29.0);
    // Even a 10-food tribute (13 with fee, easily covered) is
    // refused below the adopted 30 floor.
    assert!(!sim.can_tribute(OWNER_PLAYER, OWNER_OPPONENT, ResourceKind::Food, 10));
    give(&mut sim, OWNER_PLAYER, ResourceKind::Food, 1); // holds 30
    assert!(sim.can_tribute(OWNER_PLAYER, OWNER_OPPONENT, ResourceKind::Food, 10));
}

#[test]
fn tribute_is_deterministic_across_identical_streams() {
    let run = || {
        let mut sim = Sim::new(11);
        give(&mut sim, OWNER_PLAYER, ResourceKind::Wood, 500);
        for t in 0..300 {
            if t == 60 {
                sim.push_command(Command::Tribute {
                    sender: OWNER_PLAYER,
                    recipient: OWNER_OPPONENT,
                    resource: ResourceKind::Wood,
                    amount: 150,
                });
            }
            sim.tick();
        }
        sim.state_hash()
    };
    assert_eq!(run(), run());
}

#[test]
fn coinage_research_lands_and_drops_the_fee() {
    let mut sim = Sim::new(7);
    let market = placed(&sim, 900, BuildingKind::Market, OWNER_PLAYER, (58, 58));
    sim.buildings.push(market);
    // Feudal: Coinage is a Castle Age technology.
    sim.players[0].age = Age::Feudal;
    assert_eq!(
        sim.research_block_at(900, TechId::Coinage),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_at(900, TechId::Coinage),
        ResearchBlock::Ok
    );
    sim.push_command(Command::ResearchAt {
        building: 900,
        tech: TechId::Coinage,
    });
    let mut landed = false;
    for _ in 0..6_000 {
        sim.tick();
        if sim.has_tech(OWNER_PLAYER, TechId::Coinage) {
            landed = true;
            break;
        }
    }
    assert!(landed, "Coinage never completed");
    assert_eq!(sim.tribute_fee_pct_of(OWNER_PLAYER), 20);
}

// ------------------------------------------------------------------
// VIS-03 + TRD-04: the availability gates of the four technologies.

#[test]
fn los_and_fee_tech_availability_gates() {
    let mut sim = Sim::new(7);
    let market = placed(&sim, 900, BuildingKind::Market, OWNER_PLAYER, (58, 58));
    sim.buildings.push(market);
    // Town Watch is Feudal at the Town Center.
    assert_eq!(
        sim.research_block_tc(TechId::TownWatch),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Feudal;
    assert_eq!(sim.research_block_tc(TechId::TownWatch), ResearchBlock::Ok);
    // Town Patrol is Castle, and chains from Town Watch (TEC-07).
    assert_eq!(
        sim.research_block_tc(TechId::TownPatrol),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_tc(TechId::TownPatrol),
        ResearchBlock::Locked
    );
    sim.players[0].techs |= TechId::TownWatch.bit();
    give(&mut sim, OWNER_PLAYER, ResourceKind::Food, 500);
    give(&mut sim, OWNER_PLAYER, ResourceKind::Gold, 500);
    assert_eq!(sim.research_block_tc(TechId::TownPatrol), ResearchBlock::Ok);
    // Banking is Imperial, and chains from Coinage.
    assert_eq!(
        sim.research_block_at(900, TechId::Banking),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Imperial;
    assert_eq!(
        sim.research_block_at(900, TechId::Banking),
        ResearchBlock::Locked
    );
    sim.players[0].techs |= TechId::Coinage.bit();
    assert_eq!(sim.research_block_at(900, TechId::Banking), ResearchBlock::Ok);
    // The tag contract: appended after the University block.
    assert_eq!(TechId::Coinage.tag(), 25);
    assert_eq!(TechId::Banking.tag(), 26);
    assert_eq!(TechId::TownWatch.tag(), 27);
    assert_eq!(TechId::TownPatrol.tag(), 28);
}

// ------------------------------------------------------------------
// VIS-03: the sight table and the live fog discs.

#[test]
fn building_sight_table_rows_and_tech_bonus() {
    let none = 0u64;
    let both = TechId::TownWatch.bit() | TechId::TownPatrol.bit();
    // Documented rows: House 2 (the Town Watch article's "+4
    // triples it"), walls 2, Castle 11, TC 8. Adopted row: the
    // ordinary kinds' 4.
    assert_eq!(BuildingKind::House.sight_tiles_at(Age::Dark), 2);
    assert_eq!(BuildingKind::Mill.sight_tiles_at(Age::Dark), 4);
    assert_eq!(BuildingKind::Castle.sight_tiles_at(Age::Castle), 11);
    assert_eq!(BuildingKind::TownCenter.sight_tiles_at(Age::Dark), 8);
    // +4 per rung on the kinds the technologies raise.
    assert_eq!(BuildingKind::House.sight_tiles_with_techs(Age::Feudal, both), 10);
    assert_eq!(
        BuildingKind::House.sight_tiles_with_techs(Age::Feudal, TechId::TownWatch.bit()),
        6
    );
    assert_eq!(BuildingKind::Castle.sight_tiles_with_techs(Age::Castle, both), 19);
    // The exclusions never rise: walls and Farms.
    assert_eq!(BuildingKind::PalisadeWall.sight_tiles_with_techs(Age::Feudal, both), 2);
    assert_eq!(BuildingKind::StoneWall.sight_tiles_with_techs(Age::Feudal, both), 2);
    assert_eq!(BuildingKind::Farm.sight_tiles_with_techs(Age::Feudal, both), 4);
    assert_eq!(BuildingKind::House.sight_tiles_with_techs(Age::Dark, none), 2);
}

/// The visible run east of a stamp centre on its own row: the
/// integer disc reaches exactly `r` cells there.
fn east_radius(sim: &Sim, owner: u8, centre: (u32, u32)) -> u32 {
    let mut r = 0;
    while sim.cell_visibility(owner, centre.0 + r + 1, centre.1) == 2 {
        r += 1;
    }
    r
}

#[test]
fn los_techs_raise_live_fog_discs() {
    let mut sim = Sim::new(7);
    // Stage an empty stage: every entity but the Town Center
    // dies, so the two discs measured below have one source each.
    let tc_id = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == crate::entity::EntityKind::TownCenter)
        .map(|e| e.id)
        .expect("a Town Center");
    for e in sim.entities.iter_mut() {
        if e.id != tc_id {
            e.dead = true;
        }
    }
    let tc = sim.entities().iter().find(|e| e.id == tc_id).unwrap();
    let tc_cell = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    // A House far east of the Town Center (its disc never meets
    // the TC's), staged directly like every fixture here.
    let house_cell = (tc_cell.0 + 40, tc_cell.1);
    let house = placed(&sim, 901, BuildingKind::House, OWNER_PLAYER, house_cell);
    let house_centre = sim.grid().world_to_cell(house.pos_x, house.pos_y);
    sim.buildings.push(house);

    // One recompute with the staged world (the House record was
    // placed after the creation stamp), then the base rows:
    // TC 8, House 2.
    for _ in 0..20 {
        sim.tick();
    }
    assert_eq!(east_radius(&sim, OWNER_PLAYER, tc_cell), 8);
    assert_eq!(east_radius(&sim, OWNER_PLAYER, house_centre), 2);
    // Town Watch: +4 on both.
    sim.players[0].techs |= TechId::TownWatch.bit();
    for _ in 0..20 {
        sim.tick();
    }
    assert_eq!(east_radius(&sim, OWNER_PLAYER, tc_cell), 12);
    assert_eq!(east_radius(&sim, OWNER_PLAYER, house_centre), 6);
    // Town Patrol: a further +4 on both.
    sim.players[0].techs |= TechId::TownPatrol.bit();
    for _ in 0..20 {
        sim.tick();
    }
    assert_eq!(east_radius(&sim, OWNER_PLAYER, tc_cell), 16);
    assert_eq!(east_radius(&sim, OWNER_PLAYER, house_centre), 10);
}
