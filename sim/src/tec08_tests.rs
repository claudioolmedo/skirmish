//! TEC-08 tests: Siege Engineers (TechId 53: University,
//! Imperial Age, 500 food / 600 wood, 45 s) — the game's first
//! *attack* percentage and the consumer that exercises TEC-08's
//! ordering: the percentage scales the attack value per building
//! class, rounded and stored, BEFORE CMB-01's per-class armor
//! subtraction (wiki, "Armor class", Damage calculation Step 1;
//! Siege Engineers article + note 1; Scorpion article, update
//! 141935 — all fetched 2026-11-16).
//!
//! Fixture style follows `bowsaw_tests` (direct records; ages /
//! techs set on the player slots). The damage pins below are
//! built so pre-armor and post-armor scaling DISAGREE, so the
//! ordering itself is what is pinned, not just the totals.

use crate::build::{Building, BuildingKind};
use crate::classes as cls;
use crate::combat::{compute_damage, ram_building_attacks};
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, TechId, TechHost};
use crate::units::{self, UnitSpec};
use crate::{Command, ResearchBlock, Sim};
use fixed::types::I16F16;

/// A standing building record, copied's shape from
/// `bowsaw_tests::placed` (complete, full HP).
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

/// A sim in `age` with a standing University (id 900) and a
/// Barracks (id 901, the wrong host) for the player, topped up.
fn sim_with_university(age: Age) -> (Sim, u32, u32) {
    let mut sim = Sim::new(7);
    sim.players[0].age = age;
    top_up(&mut sim, OWNER_PLAYER);
    let uni = placed(&sim, 900, BuildingKind::University, OWNER_PLAYER, (30, 30));
    let uni_id = uni.id;
    sim.buildings.push(uni);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    let rax_id = rax.id;
    sim.buildings.push(rax);
    (sim, uni_id, rax_id)
}

fn grant_siege_engineers(sim: &mut Sim) {
    sim.players[0].techs |= TechId::SiegeEngineers.bit();
}

/// A synthetic building target: member of every building class,
/// melee armor 5, pierce armor 6, building armor 3 (a Masonry-
/// grade building armor, so the pre/post-armor orders separate).
fn building_target() -> UnitSpec {
    let mut t = *units::spec(EntityKind::Villager);
    t.classes = cls::mask(&[
        cls::ALL_BUILDINGS,
        cls::STANDARD_BUILDINGS,
        cls::WALLS_AND_GATES,
        cls::CASTLE,
    ]);
    t.armor_melee = 5;
    t.armor_pierce = 6;
    t.armor_building = 3;
    t
}

#[test]
fn siege_engineers_row_is_the_documented_one() {
    let t = TechId::SiegeEngineers;
    assert_eq!(t.tag(), 53);
    assert_eq!(TechId::from_tag(53), Some(t));
    assert_eq!(TechId::ALL.len(), 54);
    assert_eq!(t.host(), TechHost::Building(BuildingKind::University));
    assert_eq!(t.age_required(), Age::Imperial);
    assert_eq!(t.cost(), (500, 600, 0, 0));
    assert_eq!(t.ticks(), 2700); // 45 s
    assert_eq!(t.requires_tech(), None);
}

#[test]
fn siege_engineers_gates() {
    // Castle Age: Imperial research — Locked.
    let (mut sim, uni, rax) = sim_with_university(Age::Castle);
    assert_eq!(
        sim.research_block_at(uni, TechId::SiegeEngineers),
        ResearchBlock::Locked,
        "Siege Engineers locked in Castle"
    );
    // Imperial: available at the University, and only there.
    let (mut sim, uni, rax) = sim_with_university(Age::Imperial);
    assert_eq!(
        sim.research_block_at(uni, TechId::SiegeEngineers),
        ResearchBlock::Ok
    );
    assert!(sim.can_research_at(uni, TechId::SiegeEngineers));
    assert_eq!(
        sim.research_block_at(rax, TechId::SiegeEngineers),
        ResearchBlock::Locked,
        "Siege Engineers is University research, not Barracks"
    );
    let _ = &mut sim;
}

#[test]
fn siege_engineers_research_flow_sets_the_mask_bit() {
    let (mut sim, uni, _rax) = sim_with_university(Age::Imperial);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::SiegeEngineers));
    let food_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Food)
        .to_num::<i32>();
    let wood_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Wood)
        .to_num::<i32>();
    sim.push_command(Command::ResearchAt {
        building: uni,
        tech: TechId::SiegeEngineers,
    });
    sim.tick();
    // Charged at enqueue (TEC-05): 500 food / 600 wood.
    assert_eq!(
        food_before
            - sim
                .stockpile_of(OWNER_PLAYER)
                .amount(ResourceKind::Food)
                .to_num::<i32>(),
        500,
        "the research costs 500 food"
    );
    assert_eq!(
        wood_before
            - sim
                .stockpile_of(OWNER_PLAYER)
                .amount(ResourceKind::Wood)
                .to_num::<i32>(),
        600,
        "the research costs 600 wood"
    );
    // 45 s of research: not done early, done shortly after.
    for _ in 0..2600 {
        sim.tick();
        assert!(
            !sim.has_tech(OWNER_PLAYER, TechId::SiegeEngineers),
            "Siege Engineers completed faster than 45 s"
        );
    }
    let mut done = false;
    for _ in 0..200 {
        sim.tick();
        if sim.has_tech(OWNER_PLAYER, TechId::SiegeEngineers) {
            done = true;
            break;
        }
    }
    assert!(done, "Siege Engineers did not complete after 45 s");
}

#[test]
fn siege_engineers_swaps_the_attack_tables() {
    let (mut sim, _uni, _rax) = sim_with_university(Age::Imperial);
    // Base rows first.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam).attacks,
        &[(cls::MELEE, 2), (cls::STANDARD_BUILDINGS, 150), (cls::SIEGE, 40)]
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel).attacks,
        &[(cls::MELEE, 40), (cls::STANDARD_BUILDINGS, 35), (cls::SIEGE, 12)]
    );
    grant_siege_engineers(&mut sim);
    // Scaled rows: building pairs ×6/5 (×7/5 for the Petard),
    // rounded per class; every other pair untouched.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam).attacks,
        units::BATTERING_RAM_SIEGE_ENGINEERS_ATTACKS
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam).attacks[1],
        (cls::STANDARD_BUILDINGS, 180) // 150 × 6/5
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel).attacks[1],
        (cls::STANDARD_BUILDINGS, 42) // 35 × 6/5
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion).attacks,
        units::SCORPION_SIEGE_ENGINEERS_ATTACKS
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion).attacks[3],
        (cls::STANDARD_BUILDINGS, 4) // 3 × 6/5 = 3.6 → 4, rounded
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Petard).attacks,
        units::PETARD_SIEGE_ENGINEERS_ATTACKS
    );
    // The opponent, who never researched it, keeps the base rows.
    assert_eq!(
        sim.stats_for(OWNER_OPPONENT, EntityKind::BatteringRam).attacks[1],
        (cls::STANDARD_BUILDINGS, 150)
    );
    // A non-siege row is untouched by the layer.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Knight).attacks,
        units::spec(EntityKind::Knight).attacks
    );
    // The Demolition line carries no building-class bonus pair,
    // so its row is unchanged (the +40% is not faked onto its
    // melee base pair).
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::DemolitionShip).attacks,
        units::spec(EntityKind::DemolitionShip).attacks
    );
}

#[test]
fn siege_engineers_range_and_ram_los_legs() {
    let (mut sim, _uni, _rax) = sim_with_university(Age::Imperial);
    let one_tile = I16F16::from_bits(262_144);
    let mangonel = sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel);
    let scorpion = sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion);
    let ram = sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam);
    grant_siege_engineers(&mut sim);
    // Ranged siege +1 range (7 → 8 tiles on both rows).
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel).range_max,
        mangonel.range_max + one_tile
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion).range_max,
        scorpion.range_max + one_tile
    );
    // The ram is melee: no range leg, but +1 LoS (5 → 6 tiles).
    let ram_se = sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam);
    assert_eq!(ram_se.range_max, ram.range_max);
    assert_eq!(ram_se.los, ram.los + one_tile);
    // The Petard is not SIEGE class: neither leg reaches it.
    let petard = sim.stats_for(OWNER_PLAYER, EntityKind::Petard);
    assert_eq!(petard.range_max, I16F16::ZERO);
    assert_eq!(
        petard.los,
        units::spec(EntityKind::Petard).los
    );
}

#[test]
fn siege_engineers_scales_before_armor_subtraction() {
    // TEC-08's ordering, pinned where the two orders disagree:
    // against building armor 3, pre-armor scaling of the Ram's
    // 150 gives (180 − 3) = 177; post-armor scaling would give
    // (150 − 3) × 6/5 = 176.4 → 176. Only 177 is DE's Step 1.
    let (mut sim, _uni, _rax) = sim_with_university(Age::Imperial);
    let target = building_target();
    let ram = sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam);
    assert_eq!(compute_damage(ram.attacks, &target), 147); // (150 − 3)
    let mangonel = sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel);
    assert_eq!(compute_damage(mangonel.attacks, &target), 67); // 35 + (35 − 3)
    let scorpion = sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion);
    assert_eq!(compute_damage(scorpion.attacks, &target), 5); // (11 − 6) + (3 − 3)
    let petard = sim.stats_for(OWNER_PLAYER, EntityKind::Petard);
    assert_eq!(compute_damage(petard.attacks, &target), 1517); // 20 + 900 + (500 − 3) + 100

    grant_siege_engineers(&mut sim);
    let ram = sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam);
    assert_eq!(compute_damage(ram.attacks, &target), 177); // (180 − 3), NOT 176
    let mangonel = sim.stats_for(OWNER_PLAYER, EntityKind::Mangonel);
    assert_eq!(compute_damage(mangonel.attacks, &target), 74); // 35 + (42 − 3)
    // The Scorpion also pins the rounding: its +3 becomes +4
    // (3.6 rounds to 4), so the pair contributes (4 − 3) = 1
    // where post-armor scaling of (3 − 3) would contribute 0.
    let scorpion = sim.stats_for(OWNER_PLAYER, EntityKind::Scorpion);
    assert_eq!(compute_damage(scorpion.attacks, &target), 6); // (11 − 6) + (4 − 3)
    // The Petard at the suicide rate ×7/5, per class:
    // 20 + 1260 + (700 − 3) + 140.
    let petard = sim.stats_for(OWNER_PLAYER, EntityKind::Petard);
    assert_eq!(compute_damage(petard.attacks, &target), 2117);
}

#[test]
fn siege_engineers_composes_with_the_gar10_passenger_bonus() {
    // GAR-10 derives +10 per passenger over the layered table at
    // strike time, so with Siege Engineers a 4-passenger ram
    // strikes buildings at 180 + 40 = 220 on the buildings pair.
    let (mut sim, _uni, _rax) = sim_with_university(Age::Imperial);
    grant_siege_engineers(&mut sim);
    let ram = sim.stats_for(OWNER_PLAYER, EntityKind::BatteringRam);
    let table = ram_building_attacks(EntityKind::BatteringRam, 4, ram.attacks);
    assert_eq!(table[1], (cls::STANDARD_BUILDINGS, 220));
    let target = building_target();
    assert_eq!(compute_damage(&table, &target), 217); // (220 − 3)
}

#[test]
fn siege_engineers_respects_the_frozen_patch_mask() {
    // MNK-12's discipline: a frozen unit's row resolves from its
    // conversion-time mask, never the new owner's set — the SE
    // layer reads the same mask as every other patch.
    let (mut sim, _uni, _rax) = sim_with_university(Age::Imperial);
    grant_siege_engineers(&mut sim);
    let frozen_none = sim.stats_for_masked(OWNER_PLAYER, EntityKind::Mangonel, Some(0));
    assert_eq!(
        frozen_none.attacks[1],
        (cls::STANDARD_BUILDINGS, 35),
        "a unit frozen before the research keeps the base table"
    );
    let frozen_se = sim.stats_for_masked(
        OWNER_PLAYER,
        EntityKind::Mangonel,
        Some(TechId::SiegeEngineers.bit()),
    );
    assert_eq!(frozen_se.attacks[1], (cls::STANDARD_BUILDINGS, 42));
}
