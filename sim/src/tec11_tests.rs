//! TEC-11 tests: the unique-technology pair (TechId 58/59:
//! Drilled March, Castle Age; Siege Carpentry, Imperial Age) —
//! the generic empire's two Castle researches. The pattern is
//! the documented one (exactly one Castle-Age + one
//! Imperial-Age unique technology per empire, researched at
//! the Castle, one-time each — AoE2 wiki, Unique technology;
//! research report `research-de/tec11-net02.md` §A): the costs
//! and times are this game's own content inside the documented
//! envelope, and the effects are original permanent civ-wide
//! modifiers in documented categories — an INFANTRY-class
//! speed percentage (the Husbandry scalar shape) and a
//! SIEGE-class HP percentage (the Masonry shape, with its
//! proportional current-HP completion sweep).
//!
//! Fixture style follows `conscription_tests`: buildings are
//! placed directly, ages are set on the player slots, and the
//! research flow itself runs through the real command path.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, ProdEntry, ResearchBlock, TechId, TechHost};
use crate::units;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn placed(sim: &mut Sim, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> u32 {
    let id = 900 + sim.buildings.len() as u32;
    let size = kind.footprint().w;
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
        repair_gold_remainder: 0,
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
        relics: Vec::new(),
        relic_gold_remainder: 0,
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

fn run_until(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

/// A sim in `age` with a standing Castle (the pair's host) and
/// a Barracks (the wrong host) for the player, topped up.
fn sim_with_castle(age: Age) -> (Sim, u32, u32) {
    let mut sim = Sim::new(7);
    sim.players[0].age = age;
    top_up(&mut sim, OWNER_PLAYER);
    let castle = placed(&mut sim, BuildingKind::Castle, OWNER_PLAYER, (30, 30));
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    (sim, castle, rax)
}

fn amount(sim: &Sim, k: ResourceKind) -> i32 {
    sim.stockpile_of(OWNER_PLAYER).amount(k).to_num::<i32>()
}

#[test]
fn unique_tech_rows_carry_the_documented_shape() {
    // Drilled March: tag 58, Castle host, Castle Age, no
    // prerequisite — 400 F / 300 G (700 total, inside the
    // documented Castle-UT envelope ≈450–1,200), 45 s (inside
    // 30–60 s).
    let dm = TechId::DrilledMarch;
    assert_eq!(dm.tag(), 58);
    assert_eq!(TechId::from_tag(58), Some(dm));
    assert_eq!(dm.host(), TechHost::Building(BuildingKind::Castle));
    assert_eq!(dm.age_required(), Age::Castle);
    assert_eq!(dm.requires_tech(), None);
    assert_eq!(dm.cost(), (400, 0, 300, 0));
    assert_eq!(dm.ticks(), 45 * 60);
    // Siege Carpentry: tag 59, Castle host, Imperial Age, no
    // prerequisite — 600 F / 500 G (1,100 total, inside the
    // documented Imperial-UT envelope ≈800–1,700), 50 s
    // (inside 40–60 s).
    let sc = TechId::SiegeCarpentry;
    assert_eq!(sc.tag(), 59);
    assert_eq!(TechId::from_tag(59), Some(sc));
    assert_eq!(sc.host(), TechHost::Building(BuildingKind::Castle));
    assert_eq!(sc.age_required(), Age::Imperial);
    assert_eq!(sc.requires_tech(), None);
    assert_eq!(sc.cost(), (600, 0, 500, 0));
    assert_eq!(sc.ticks(), 50 * 60);
    // The pair appends at the tail; no earlier tag moved.
    assert_eq!(TechId::ALL.len(), 60);
    assert_eq!(TechId::from_tag(60), None);
    // Queue codes ride the standing 100 + tag contract.
    assert_eq!(ProdEntry::Tech(dm).code(), 158);
    assert_eq!(ProdEntry::Tech(sc).code(), 159);
}

#[test]
fn drilled_march_gates_on_age_and_host() {
    // Feudal Age: the Castle-Age member is locked even at its
    // own host (the fixture places the Castle directly).
    let (mut sim, castle, rax) = sim_with_castle(Age::Feudal);
    assert_eq!(
        sim.research_block_at(castle, TechId::DrilledMarch),
        ResearchBlock::Locked
    );
    // Castle Age: open at the Castle, refused at a Barracks
    // (a foreign host reports Locked, the standing contract).
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_at(castle, TechId::DrilledMarch),
        ResearchBlock::Ok
    );
    assert_eq!(
        sim.research_block_at(rax, TechId::DrilledMarch),
        ResearchBlock::Locked
    );
    // The Imperial member stays locked through the Castle Age.
    assert_eq!(
        sim.research_block_at(castle, TechId::SiegeCarpentry),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Imperial;
    assert_eq!(
        sim.research_block_at(castle, TechId::SiegeCarpentry),
        ResearchBlock::Ok
    );
}

#[test]
fn drilled_march_research_flow_charges_and_lands_on_time() {
    let (mut sim, castle, _) = sim_with_castle(Age::Castle);
    let food_before = amount(&sim, ResourceKind::Food);
    let gold_before = amount(&sim, ResourceKind::Gold);
    sim.push_command(Command::ResearchAt {
        building: castle,
        tech: TechId::DrilledMarch,
    });
    run_until(&mut sim, 1);
    // Charged exactly at enqueue (TEC-05): 400 F / 300 G.
    assert_eq!(amount(&sim, ResourceKind::Food), food_before - 400);
    assert_eq!(amount(&sim, ResourceKind::Gold), gold_before - 300);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::DrilledMarch));
    // A second attempt while it is queued reads AlreadyDone
    // (one-time research, TEC-05's gate).
    assert_eq!(
        sim.research_block_at(castle, TechId::DrilledMarch),
        ResearchBlock::AlreadyDone
    );
    run_until(&mut sim, 2698);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::DrilledMarch));
    run_until(&mut sim, 1);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::DrilledMarch));
    // And it stays one-time after completion.
    assert_eq!(
        sim.research_block_at(castle, TechId::DrilledMarch),
        ResearchBlock::AlreadyDone
    );
}

#[test]
fn drilled_march_speeds_infantry_by_a_tenth_and_nothing_else() {
    let (mut sim, _castle, _) = sim_with_castle(Age::Castle);
    sim.players[0].techs |= TechId::DrilledMarch.bit();
    // Militia: 3,932 bits × 11/10 = 4,325 (truncating).
    let militia = sim.stats_for(OWNER_PLAYER, EntityKind::Militia);
    assert_eq!(militia.speed.to_bits(), 4325);
    // The whole INFANTRY class takes the same scalar off its
    // own base row (Man-at-Arms, Spearman — and the Petard,
    // which carries INFANTRY in this roster's class table).
    for kind in [
        EntityKind::ManAtArms,
        EntityKind::Spearman,
        EntityKind::Petard,
    ] {
        let base = units::spec(kind).speed.to_bits() as i64;
        assert_eq!(
            sim.stats_for(OWNER_PLAYER, kind).speed.to_bits(),
            (base * 11 / 10) as i32,
            "{kind:?} speed"
        );
    }
    // Non-infantry rows are untouched: Archer, Knight, Villager.
    for kind in [
        EntityKind::Archer,
        EntityKind::Knight,
        EntityKind::Villager,
    ] {
        assert_eq!(
            sim.stats_for(OWNER_PLAYER, kind).speed,
            units::spec(kind).speed,
            "{kind:?} speed must not move"
        );
    }
    // The patch is the owner's: the opponent's Militia keeps
    // the base row.
    assert_eq!(
        sim.stats_for(OWNER_OPPONENT, EntityKind::Militia).speed.to_bits(),
        3932
    );
    // And the maximum is not part of the effect.
    assert_eq!(militia.max_hp, units::spec(EntityKind::Militia).max_hp);
}

#[test]
fn siege_carpentry_scales_siege_max_hp_by_a_quarter() {
    let (mut sim, _castle, _) = sim_with_castle(Age::Imperial);
    sim.players[0].techs |= TechId::SiegeCarpentry.bit();
    // Integer maxima, × 5/4 truncating: Ram 175 → 218,
    // Mangonel 50 → 62, Scorpion 40 → 50.
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam).max_hp, 218);
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel).max_hp, 62);
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion).max_hp, 50);
    // The Petard carries INFANTRY, not SIEGE, in this roster's
    // class table — it is outside the patch, as is everything
    // else (Militia shown).
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Petard).max_hp, 50);
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Militia).max_hp, 40);
    // The opponent's Ram keeps the base maximum.
    assert_eq!(
        sim.stats_for(OWNER_OPPONENT, EntityKind::BatteringRam).max_hp,
        175
    );
}

#[test]
fn siege_carpentry_completion_sweep_keeps_the_hp_fraction() {
    let (mut sim, castle, _) = sim_with_castle(Age::Imperial);
    // A full Ram, a Ram at 100/175, and an opponent Ram.
    spawn(&mut sim, 500, EntityKind::BatteringRam, OWNER_PLAYER, (20, 20));
    spawn(&mut sim, 501, EntityKind::BatteringRam, OWNER_PLAYER, (22, 20));
    spawn(&mut sim, 502, EntityKind::BatteringRam, OWNER_OPPONENT, (24, 20));
    let idx = sim.entity_index(501).expect("ram placed");
    sim.entities[idx].hp = I16F16::from_num(100);
    // Research through the real flow: 600 F / 500 G, 50 s.
    let food_before = amount(&sim, ResourceKind::Food);
    let gold_before = amount(&sim, ResourceKind::Gold);
    sim.push_command(Command::ResearchAt {
        building: castle,
        tech: TechId::SiegeCarpentry,
    });
    run_until(&mut sim, 1);
    assert_eq!(amount(&sim, ResourceKind::Food), food_before - 600);
    assert_eq!(amount(&sim, ResourceKind::Gold), gold_before - 500);
    run_until(&mut sim, 2999);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::SiegeCarpentry));
    let hp_of = |sim: &Sim, id: u32| {
        sim.entities[sim.entity_index(id).expect("entity")].hp
    };
    // Full HP lands exactly on the new maximum (218) — the
    // integer-ratio sweep never overshoots it.
    assert_eq!(hp_of(&sim, 500), I16F16::from_num(218));
    // The damaged Ram keeps its fraction: 100 × 218/175 in
    // fixed-point bits (124 whole HP and change).
    let expect_bits = (100i64 * 65536 * 218 / 175) as i32;
    assert_eq!(hp_of(&sim, 501).to_bits(), expect_bits);
    assert_eq!(hp_of(&sim, 501).to_num::<i32>(), 124);
    // The opponent's Ram is untouched.
    assert_eq!(hp_of(&sim, 502), I16F16::from_num(175));
}

#[test]
fn the_pair_shares_the_castle_queue_with_production() {
    let (mut sim, castle, _) = sim_with_castle(Age::Castle);
    placed(&mut sim, BuildingKind::House, OWNER_PLAYER, (60, 60));
    sim.push_command(Command::TrainAt {
        building: castle,
        kind: EntityKind::Petard,
    });
    sim.push_command(Command::ResearchAt {
        building: castle,
        tech: TechId::DrilledMarch,
    });
    run_until(&mut sim, 1);
    // One FIFO (TEC-03): the Petard's unit entry leads, the
    // research (queue code 158) rides behind it.
    let queue = sim.building_queue(castle);
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[0], ProdEntry::Unit(EntityKind::Petard));
    assert_eq!(queue[1], ProdEntry::Tech(TechId::DrilledMarch));
    // The research lands only after the Petard's 1,500 ticks
    // plus its own 2,700: not yet at 1,500 + 2,699, done one
    // tick later.
    run_until(&mut sim, 1500 + 2699 - 1);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::DrilledMarch));
    run_until(&mut sim, 1);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::DrilledMarch));
}

#[test]
fn the_pair_researches_deterministically() {
    let run = || {
        let (mut sim, castle, _) = sim_with_castle(Age::Imperial);
        sim.push_command(Command::ResearchAt {
            building: castle,
            tech: TechId::DrilledMarch,
        });
        run_until(&mut sim, 2700);
        sim.push_command(Command::ResearchAt {
            building: castle,
            tech: TechId::SiegeCarpentry,
        });
        run_until(&mut sim, 3000);
        sim.state_hash()
    };
    assert_eq!(run(), run());
}
