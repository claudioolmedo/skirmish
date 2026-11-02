//! Castle P5e tests (2026-11-01): Herbal Medicine — the last
//! row of TEC-10's Castle set (Monastery, Castle Age, 200 gold,
//! 35 s) and GAR-04's completion: garrisoned units heal ×6 as
//! fast when the HOST's owner has researched it (Town Center /
//! towers 0.1 → 0.6 HP/s, Castle 0.2 → 1.2 HP/s, production
//! buildings still none), read live in the garrison tick's
//! heal-rate path over the shipped per-entity remainder
//! accumulator, so a whole minute lands exact with no drift.
//!
//! Values: the Castle Age dossier §5.6 / P5 execution plan
//! §1.3 + §5 (wiki, Herbal Medicine, fetched 2026-10-22),
//! RULEBOOK GAR-04 / TEC-10. Fixture style follows
//! `monastery_p5a_tests` / `garrison_tests` (direct records;
//! garrisons staged through the host lists, the GAR-05
//! precedent; ages set on the player slots).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, TechId};
use crate::{Command, ResearchBlock, Sim, TechHost};
use fixed::types::I16F16;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
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

fn run_ticks(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

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

fn sim_in_castle() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Castle;
    sim.players[1].age = Age::Castle;
    sim
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

fn wound(sim: &mut Sim, id: u32, hp: i32) {
    let e = sim.entities.iter_mut().find(|e| e.id == id).unwrap();
    e.hp = I16F16::from_num(hp);
}

/// Stage `id` inside `host` (the GAR-05 `stage_garrison`
/// precedent: the entity record and the host's own list agree).
fn stage(sim: &mut Sim, host: GarrisonRef, id: u32) {
    let ei = sim.entities.iter().position(|e| e.id == id).unwrap();
    sim.entities[ei].garrisoned_in = Some(host);
    match host {
        GarrisonRef::Building(bid) => {
            let bi = sim.buildings.iter().position(|b| b.id == bid).unwrap();
            sim.buildings[bi].garrison.push(id);
        }
        GarrisonRef::TownCenter(tc) => {
            let si = sim.tc_states.iter().position(|s| s.tc_id == tc).unwrap();
            sim.tc_states[si].garrison.push(id);
        }
        GarrisonRef::Unit(_) => unreachable!(),
    }
}

// ------------------------------------------------------------------
// The row and its gates.

#[test]
fn herbal_medicine_row_and_gates() {
    // The table row (dossier §5.6): tag 34 appended after P5a's
    // Sanctity 32 / Fervor 33 — no existing tag moved.
    assert_eq!(TechId::HerbalMedicine.tag(), 34);
    assert_eq!(TechId::from_tag(34), Some(TechId::HerbalMedicine));
    assert_eq!(TechId::ALL.len(), 37, "P5b's Atonement/Devotion append at 35/36");
    assert_eq!(
        TechId::HerbalMedicine.host(),
        TechHost::Building(BuildingKind::Monastery)
    );
    assert_eq!(TechId::HerbalMedicine.age_required(), Age::Castle);
    assert_eq!(TechId::HerbalMedicine.cost(), (0, 0, 200, 0), "200 gold");
    assert_eq!(TechId::HerbalMedicine.ticks(), 2100, "35 s at 60 Hz");
    assert_eq!(TechId::HerbalMedicine.requires_tech(), None);

    // Gate law at a real Monastery: Castle accepts; Dark and
    // Feudal refuse (Locked); the wrong building refuses.
    let mut sim = sim_in_castle();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    let rax_id = rax.id;
    sim.buildings.push(rax);
    assert_eq!(
        sim.research_block_at(mon_id, TechId::HerbalMedicine),
        ResearchBlock::Ok
    );
    assert_eq!(
        sim.research_block_at(rax_id, TechId::HerbalMedicine),
        ResearchBlock::Locked,
        "Herbal Medicine is Monastery research, not Barracks"
    );
    for age in [Age::Feudal, Age::Dark] {
        sim.players[0].age = age;
        assert_eq!(
            sim.research_block_at(mon_id, TechId::HerbalMedicine),
            ResearchBlock::Locked,
            "refused before the Castle Age ({age:?})"
        );
    }
    sim.players[0].age = Age::Castle;
    assert_eq!(
        sim.research_block_at(mon_id, TechId::HerbalMedicine),
        ResearchBlock::Ok
    );
}

// ------------------------------------------------------------------
// GAR-04 completion: the ×6 rates, exact over a whole minute.

#[test]
fn herbal_medicine_multiplies_tc_and_tower_rates_exactly() {
    // Town Center: 0.6 HP/s — a Militia at 1 HP gains exactly
    // 36 HP over 3600 ticks, remainder back to zero (no drift).
    let mut sim = sim_in_castle();
    sim.players[0].techs |= TechId::HerbalMedicine.bit();
    let tc = sim.town_center().id;
    spawn(&mut sim, 900, EntityKind::Militia, OWNER_PLAYER, (60, 60));
    wound(&mut sim, 900, 1);
    stage(&mut sim, GarrisonRef::TownCenter(tc), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(37), "1 + 0.6 HP/s × 60 s");
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);

    // Watch Tower: the same 0.6 HP/s (its base is the TC's 0.1).
    let mut sim = sim_in_castle();
    sim.players[0].techs |= TechId::HerbalMedicine.bit();
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    spawn(&mut sim, 900, EntityKind::Militia, OWNER_PLAYER, (60, 60));
    wound(&mut sim, 900, 1);
    stage(&mut sim, GarrisonRef::Building(900), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(37));
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);
}

#[test]
fn herbal_medicine_multiplies_the_castle_rate_exactly_and_never_overheals() {
    // Castle: 1.2 HP/s — a Knight at 10 HP gains exactly 72 HP
    // over a whole minute.
    let mut sim = sim_in_castle();
    sim.players[0].techs |= TechId::HerbalMedicine.bit();
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    wound(&mut sim, 900, 10);
    stage(&mut sim, GarrisonRef::Building(900), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(82), "10 + 1.2 HP/s × 60 s");
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);

    // No overheal at ×6 either: a Knight at 99 stops at 100 and
    // the accumulator resets, exactly as at the base rate.
    let mut sim = sim_in_castle();
    sim.players[0].techs |= TechId::HerbalMedicine.bit();
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    wound(&mut sim, 900, 99);
    stage(&mut sim, GarrisonRef::Building(900), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(100));
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);
}

#[test]
fn herbal_medicine_leaves_production_garrisons_at_none_and_reads_the_host_owner() {
    // Production buildings heal nobody (GAR-04), and ×6 of
    // zero is zero: a Militia staged in a Barracks gains nothing
    // in a whole minute even with the tech.
    let mut sim = sim_in_castle();
    sim.players[0].techs |= TechId::HerbalMedicine.bit();
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Barracks, OWNER_PLAYER, (58, 58)));
    spawn(&mut sim, 900, EntityKind::Militia, OWNER_PLAYER, (60, 60));
    wound(&mut sim, 900, 10);
    stage(&mut sim, GarrisonRef::Building(900), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(10));

    // The multiplier reads the HOST owner's mask, not the
    // guest's: the player's tech does not speed healing inside
    // the OPPONENT's Castle (the opponent holds no tech) — the
    // staged guest heals at the base 0.2 HP/s, +12 in a minute.
    let mut sim = sim_in_castle();
    sim.players[0].techs |= TechId::HerbalMedicine.bit();
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Castle, OWNER_OPPONENT, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    wound(&mut sim, 900, 10);
    stage(&mut sim, GarrisonRef::Building(900), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(22), "the host owner has no tech: base rate");
}

#[test]
fn without_herbal_medicine_the_base_rates_are_unchanged() {
    // The P3b pins, re-measured through the P5e code path: TC
    // 0.1 HP/s (+6 over a minute), Castle 0.2 HP/s (+12).
    let mut sim = sim_in_castle();
    let tc = sim.town_center().id;
    spawn(&mut sim, 900, EntityKind::Militia, OWNER_PLAYER, (60, 60));
    wound(&mut sim, 900, 1);
    stage(&mut sim, GarrisonRef::TownCenter(tc), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(7));
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);

    let mut sim = sim_in_castle();
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    wound(&mut sim, 900, 10);
    stage(&mut sim, GarrisonRef::Building(900), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(22));
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);
}

// ------------------------------------------------------------------
// The real research flow: enqueue at the Monastery, the flag
// lands at completion, and the rate changes with it.

#[test]
fn herbal_medicine_research_flow_sets_the_flag_and_the_rate() {
    let mut sim = sim_in_castle();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::HerbalMedicine));
    let gold_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    sim.push_command(Command::ResearchAt {
        building: mon_id,
        tech: TechId::HerbalMedicine,
    });
    sim.tick();
    let gold_after = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    assert_eq!(gold_before - gold_after, 200, "the research costs 200 gold");
    // 35 s of research: not done early, done shortly after.
    let early = run_until(&mut sim, 2000, |s| {
        s.has_tech(OWNER_PLAYER, TechId::HerbalMedicine)
    });
    assert!(!early, "Herbal Medicine completed faster than 35 s");
    let done = run_until(&mut sim, 400, |s| {
        s.has_tech(OWNER_PLAYER, TechId::HerbalMedicine)
    });
    assert!(done, "Herbal Medicine never completed");
    // With the flag set, the Castle rate is the ×6 rate: a
    // Knight staged now gains exactly 72 HP over a minute.
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    wound(&mut sim, 900, 10);
    stage(&mut sim, GarrisonRef::Building(901), 900);
    run_ticks(&mut sim, 3600);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(82));
    assert_eq!(ent(&sim, 900).garrison_heal_remainder, 0);
}
