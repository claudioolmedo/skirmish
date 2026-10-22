//! Cancel & delete tests (SEL-14, BLD-02, TEC-05) and the
//! building age step-up (BLD-11).
//!
//! The rules under test, from the public record: the wiki Hotkey
//! page (Age of Empires II section) lists "Delete one selected
//! unit or building — Delete", "Delete all selected units or
//! buildings — Shift + Delete" and "Stop selected unit or
//! building — G"; GAP 4.1/5.1 (the project's Phase-0 sources)
//! record that cancelling a foundation refunds 100% and that
//! cancelling any queued item — unit or technology — refunds
//! 100%. BLD-11's numbers are MECHANICS-STUDY §5 (House
//! 550/750/900/900 HP by age, melee armor −2/−1/1/3), confirmed
//! on the wiki House page ("House HP staggered: 550/750/900/900
//! in the Dark/Feudal/Castle/Imperial Age"; update 56005 for the
//! armor row).
//!
//! Everything is driven through ordinary commands — no state is
//! poked. Scenarios run in Sandbox (single seat) except the
//! step-up proof, which needs an enemy Scout to damage a House,
//! so it runs in a TwoPlayer match.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, Entity, EntityKind, MatchMode, NodeKind, ResourceKind, Sim, Task,
    TechId, OWNER_GAIA, OWNER_PLAYER,
};

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

fn villagers_of(sim: &Sim, owner: u8) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == EntityKind::Villager && !e.dead)
        .map(|e| e.id)
        .collect()
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

/// The first anchor cell near entity `center` where `kind` may be
/// built by `entity` (the repair suite's ring scan).
fn open_anchor_near(sim: &Sim, center: u32, entity: u32, kind: BuildingKind) -> (u32, u32) {
    let c = ent(sim, center);
    let (tcx, tcy) = sim.grid().world_to_cell(c.pos_x, c.pos_y);
    for r in 2..=16i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = sim.grid().cell_center(ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable ground near the Town Center");
}

fn build_at(sim: &mut Sim, entity: u32, kind: BuildingKind, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.push_command(Command::Build {
        entity,
        kind,
        x,
        y,
    });
}

/// Raise `kind` near the owner's Town Center with one villager
/// and wait for completion; returns the building id.
fn raise_near(sim: &mut Sim, owner: u8, builder: u32, kind: BuildingKind) -> u32 {
    let tc = find_id(sim, owner, EntityKind::TownCenter);
    let cell = open_anchor_near(sim, tc, builder, kind);
    let before: Vec<u32> = sim.buildings().iter().map(|b| b.id).collect();
    build_at(sim, builder, kind, cell);
    let placed = run_until(sim, 600, |s| {
        s.buildings().iter().any(|b| !before.contains(&b.id))
    });
    assert!(placed, "{kind:?} was never placed");
    let bid = sim
        .buildings()
        .iter()
        .find(|b| !before.contains(&b.id))
        .unwrap()
        .id;
    let done = run_until(sim, 30_000, |s| {
        s.buildings().iter().any(|b| b.id == bid && b.complete)
    });
    assert!(done, "{kind:?} never completed");
    bid
}

fn wood(sim: &Sim) -> f64 {
    f(sim.stockpile().amount(ResourceKind::Wood))
}

fn food(sim: &Sim) -> f64 {
    f(sim.stockpile().amount(ResourceKind::Food))
}

fn gold(sim: &Sim) -> f64 {
    f(sim.stockpile().amount(ResourceKind::Gold))
}

// ---------------------------------------------------------------------
// SEL-14: Stop

#[test]
fn stop_halts_a_moving_unit_where_it_stands() {
    let mut sim = Sim::new(42);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let s = ent(&sim, scout);
    sim.push_command(Command::Move {
        entity: scout,
        x: s.pos_x + I16F16::from_num(60),
        y: s.pos_y,
    });
    // Let it get properly under way, then stop it.
    assert!(run_until(&mut sim, 600, |s| {
        ent(s, scout).order.is_some()
    }));
    sim.tick();
    sim.tick();
    sim.push_command(Command::Stop { entity: scout });
    sim.tick();
    let e = ent(&sim, scout);
    assert!(e.order.is_none(), "Stop must clear the move order");
    assert_eq!(e.task, Task::Idle);
    let (px, py) = (e.pos_x, e.pos_y);
    for _ in 0..300 {
        sim.tick();
    }
    let e = ent(&sim, scout);
    assert!(
        (e.pos_x - px).abs() < I16F16::from_num(1) && (e.pos_y - py).abs() < I16F16::from_num(1),
        "a stopped unit must stay where it stopped"
    );
}

#[test]
fn stop_keeps_the_villagers_carry() {
    let mut sim = Sim::new(42);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let tree = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Tree && n.amount_left > I16F16::from_num(0))
        .unwrap()
        .id;
    sim.push_command(Command::Gather {
        entity: vil,
        node: tree,
    });
    let loaded = run_until(&mut sim, 7_200, |s| ent(s, vil).carry > I16F16::from_num(0));
    assert!(loaded, "the villager never picked anything up");
    let carry = ent(&sim, vil).carry;
    sim.push_command(Command::Stop { entity: vil });
    sim.tick();
    let e = ent(&sim, vil);
    assert_eq!(e.task, Task::Idle, "Stop ends the work task");
    assert_eq!(e.carry, carry, "SEL-14: a stopped villager keeps its carry");
}

// ---------------------------------------------------------------------
// SEL-14: Delete (units)

#[test]
fn delete_kills_a_unit_and_frees_its_population() {
    let mut sim = Sim::new(42);
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let pop_before = sim.living_population();
    sim.push_command(Command::DeleteEntity { entity: scout });
    sim.tick();
    assert!(ent(&sim, scout).dead, "the deleted unit must die");
    assert_eq!(sim.living_population(), pop_before - 1);
}

#[test]
fn delete_refuses_gaia_and_unknown_ids() {
    let mut sim = Sim::new(42);
    let boar = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_GAIA && e.kind == EntityKind::Boar)
        .unwrap()
        .id;
    sim.push_command(Command::DeleteEntity { entity: boar });
    sim.push_command(Command::DeleteEntity { entity: 999_999 });
    sim.tick();
    assert!(
        !ent(&sim, boar).dead,
        "Gaia's wild animals are not the player's to delete"
    );
}

#[test]
fn a_deleted_sheep_leaves_its_carcass() {
    let mut sim = Sim::new(42);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let sheep = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Sheep)
        .unwrap()
        .id;
    // Walk a villager onto the sheep to capture it (AI-06), then
    // delete it: the carcass appears, as for a kill — but no
    // villager starts harvesting it (a delete has no killer).
    let sp = ent(&sim, sheep);
    sim.push_command(Command::Move {
        entity: vil,
        x: sp.pos_x,
        y: sp.pos_y,
    });
    let captured = run_until(&mut sim, 7_200, |s| {
        ent(s, sheep).owner == OWNER_PLAYER
    });
    assert!(captured, "the sheep was never captured");
    let nodes_before = sim.nodes().len();
    sim.push_command(Command::DeleteEntity { entity: sheep });
    sim.tick();
    assert!(ent(&sim, sheep).dead);
    let new_nodes: Vec<_> = sim.nodes()[nodes_before..].to_vec();
    assert_eq!(new_nodes.len(), 1, "exactly one carcass node appears");
    assert_eq!(new_nodes[0].kind, NodeKind::CarcassHerd);
    assert_eq!(
        ent(&sim, vil).task,
        Task::Idle,
        "no killer: the nearby villager must not start harvesting"
    );
}

// ---------------------------------------------------------------------
// SEL-14 + BLD-02: Delete (buildings) and the foundation refund

#[test]
fn cancelling_a_foundation_refunds_the_full_cost() {
    let mut sim = Sim::new(42);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let cell = open_anchor_near(&sim, tc, vil, BuildingKind::House);
    assert_eq!(wood(&sim), 200.0);
    build_at(&mut sim, vil, BuildingKind::House, cell);
    sim.tick();
    let b = sim.buildings().first().expect("the foundation stands");
    assert!(!b.complete);
    let bid = b.id;
    assert_eq!(wood(&sim), 175.0, "placement charges the 25 wood");
    sim.push_command(Command::DeleteBuilding { building: bid });
    sim.tick();
    assert!(sim.buildings().is_empty(), "the foundation is gone");
    assert_eq!(wood(&sim), 200.0, "BLD-02: a cancelled foundation refunds 100%");
}

#[test]
fn deleting_a_completed_building_refunds_nothing_and_drops_its_support() {
    let mut sim = Sim::new(42);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let bid = raise_near(&mut sim, OWNER_PLAYER, vil, BuildingKind::House);
    assert_eq!(wood(&sim), 175.0);
    let cap_before = sim.population_cap();
    sim.push_command(Command::DeleteBuilding { building: bid });
    sim.tick();
    assert!(sim.buildings().is_empty());
    assert_eq!(wood(&sim), 175.0, "SEL-14: a completed building refunds nothing");
    assert_eq!(
        sim.population_cap(),
        cap_before - 5,
        "the House's population support is withdrawn (BLD-08/CMB-15)"
    );
}

// ---------------------------------------------------------------------
// TEC-05: queue cancellation refunds

#[test]
fn cancelling_tc_queue_entries_refunds_each_in_full() {
    let mut sim = Sim::new(42);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    // Two villagers queued: 200 - 2*50 = 100 food.
    sim.push_command(Command::TrainVillager);
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 2);
    assert_eq!(food(&sim), 100.0);
    // Cancel the tail entry first: one refund, progress untouched.
    sim.push_command(Command::CancelQueueTc { tc, index: 1 });
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 1);
    assert_eq!(food(&sim), 150.0);
    // Then the head: the second refund lands the stockpile whole.
    sim.push_command(Command::CancelQueueTc { tc, index: 0 });
    sim.tick();
    assert!(sim.tc_queue().is_empty());
    assert_eq!(food(&sim), 200.0);
    // An out-of-range cancel is ignored whole — no refund, no panic.
    sim.push_command(Command::CancelQueueTc { tc, index: 0 });
    sim.tick();
    assert_eq!(food(&sim), 200.0);
}

#[test]
fn cancelling_a_research_entry_refunds_the_technology_cost() {
    let mut sim = Sim::new(42);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    sim.push_command(Command::ResearchTc {
        tech: TechId::Loom,
    });
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 1);
    assert_eq!(gold(&sim), 50.0, "Loom's 50 gold was charged at enqueue");
    sim.push_command(Command::CancelQueueTc { tc, index: 0 });
    sim.tick();
    assert!(sim.tc_queue().is_empty());
    assert_eq!(gold(&sim), 100.0, "TEC-05: cancelling refunds 100%");
}

#[test]
fn cancelling_a_building_queue_entry_refunds_the_unit_cost() {
    let mut sim = Sim::new(42);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let rax = raise_near(&mut sim, OWNER_PLAYER, vil, BuildingKind::Barracks);
    sim.push_command(Command::TrainAt {
        building: rax,
        kind: EntityKind::Militia,
    });
    sim.tick();
    assert_eq!(sim.building_queue(rax).len(), 1);
    // Militia cost is the current-DE 50 F / 20 G (update 141935).
    assert_eq!(food(&sim), 150.0, "the Militia's 50 food was charged");
    assert_eq!(gold(&sim), 80.0, "the Militia's 20 gold was charged");
    sim.push_command(Command::CancelQueueBuilding {
        building: rax,
        index: 0,
    });
    sim.tick();
    assert!(sim.building_queue(rax).is_empty());
    assert_eq!(food(&sim), 200.0);
    assert_eq!(gold(&sim), 100.0);
}

// ---------------------------------------------------------------------
// BLD-11: building HP/armor step up with the owner's Age

#[test]
fn houses_step_up_with_the_feudal_age() {
    // Pure table checks first (study §5): the Dark row is the
    // base accessor; the Feudal column steps HP and both armors.
    assert_eq!(BuildingKind::House.max_hp_at(Age::Dark), 550);
    assert_eq!(BuildingKind::House.max_hp_at(Age::Feudal), 750);
    assert_eq!(BuildingKind::House.armor_melee_at(Age::Dark), -2);
    assert_eq!(BuildingKind::House.armor_melee_at(Age::Feudal), -1);
    assert_eq!(BuildingKind::House.armor_pierce_at(Age::Dark), 7);
    assert_eq!(BuildingKind::House.armor_pierce_at(Age::Feudal), 8);
    assert_eq!(BuildingKind::Barracks.max_hp_at(Age::Dark), 1200);
    assert_eq!(BuildingKind::Barracks.max_hp_at(Age::Feudal), 1500);
    assert_eq!(BuildingKind::Barracks.armor_melee_at(Age::Feudal), 1);
    assert_eq!(BuildingKind::Barracks.armor_pierce_at(Age::Feudal), 8);
    // Kinds with no pinned per-age row keep their flat value.
    assert_eq!(BuildingKind::Mill.max_hp_at(Age::Feudal), 550);
    assert_eq!(BuildingKind::Farm.max_hp_at(Age::Feudal), 480);
}

#[test]
fn a_house_built_in_dark_reaches_the_feudal_max_and_new_houses_start_there() {
    // TwoPlayer: the opponent's Scout chips the player's Dark-Age
    // House; the player ages up; a villager repairs the House —
    // repair caps at the *current* (Feudal) maximum of 750, not
    // the 550 it was built with (BLD-11 as a lookup, TEC-04). A
    // House placed after the age-up starts at 750 outright.
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    // Park the player's own Scout far away: left near the start
    // it would duel the foe Scout on its way in and spoil the
    // one-sided chipping this test measures.
    let own_scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let own_tc_pos = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    sim.push_command(Command::Move {
        entity: own_scout,
        x: own_tc_pos.pos_x + I16F16::from_num(80),
        y: own_tc_pos.pos_y + I16F16::from_num(80),
    });
    let house = raise_near(&mut sim, OWNER_PLAYER, vils[0], BuildingKind::House);
    {
        let b = sim.buildings().iter().find(|b| b.id == house).unwrap();
        assert_eq!(f(b.hp), 550.0, "a Dark-Age House starts at 550 HP");
    }
    // Chip it with the opponent's Scout (3 melee attack vs the
    // House's -2 Dark melee armor = 5 per hit).
    let foe_scout = find_id(&sim, skirmish_sim::OWNER_OPPONENT, EntityKind::ScoutCavalry);
    sim.push_command(Command::AttackBuilding {
        entity: foe_scout,
        building: house,
    });
    let chipped = run_until(&mut sim, 60_000, |s| {
        f(s.buildings().iter().find(|b| b.id == house).unwrap().hp) <= 500.0
    });
    assert!(chipped, "the Scout never chipped the House");
    // Send the Scout home so the rest of the test is peaceful.
    let foe_tc = find_id(&sim, skirmish_sim::OWNER_OPPONENT, EntityKind::TownCenter);
    let tp = ent(&sim, foe_tc);
    sim.push_command(Command::Move {
        entity: foe_scout,
        x: tp.pos_x,
        y: tp.pos_y,
    });

    // The Feudal recipe: Mill + Lumber Camp (the qualifying
    // pair), berries to 500 food, research. Wood is managed by
    // sending a villager to the trees whenever a build needs it.
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    raise_near(&mut sim, OWNER_PLAYER, vils[1], BuildingKind::Mill);
    // Top the wood back up for the Lumber Camp (200 - 25 - 100 = 75).
    let tree = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Tree && n.amount_left > I16F16::from_num(0))
        .min_by_key(|n| {
            let t = ent(&sim, tc);
            (n.pos_x - t.pos_x).to_bits().abs() + (n.pos_y - t.pos_y).to_bits().abs()
        })
        .unwrap()
        .id;
    sim.push_command(Command::Gather {
        entity: vils[2],
        node: tree,
    });
    let stocked = run_until(&mut sim, 120_000, |s| {
        f(s.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)) >= 100.0
    });
    assert!(stocked, "never re-stocked wood for the Lumber Camp");
    raise_near(&mut sim, OWNER_PLAYER, vils[1], BuildingKind::LumberCamp);
    // Berries with two villagers until 500 food.
    let berry = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::BerryBush && n.amount_left > I16F16::from_num(0))
        .min_by_key(|n| {
            let t = ent(&sim, tc);
            (n.pos_x - t.pos_x).to_bits().abs() + (n.pos_y - t.pos_y).to_bits().abs()
        })
        .unwrap()
        .id;
    sim.push_command(Command::Gather {
        entity: vils[0],
        node: berry,
    });
    sim.push_command(Command::Gather {
        entity: vils[2],
        node: berry,
    });
    let fed = run_until(&mut sim, 240_000, |s| {
        f(s.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)) >= 500.0
    });
    assert!(fed, "never gathered the Feudal food");
    sim.push_command(Command::ResearchTcAt {
        tc,
        tech: TechId::FeudalAge,
    });
    let up = run_until(&mut sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");

    // Repair the chipped House: it must climb past its birth max
    // of 550 and cap exactly at the Feudal 750. The repair drip
    // needs a little wood on hand (about 5 for the missing HP,
    // with margin) — chop first if the stockpile is dry.
    if f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)) < 40.0 {
        sim.push_command(Command::Gather {
            entity: vils[2],
            node: tree,
        });
        let ok = run_until(&mut sim, 120_000, |s| {
            f(s.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)) >= 40.0
        });
        assert!(ok, "never stocked wood for the repair drip");
    }
    sim.push_command(Command::Repair {
        entity: vils[0],
        building: house,
    });
    let healed = run_until(&mut sim, 120_000, |s| {
        f(s.buildings().iter().find(|b| b.id == house).unwrap().hp) >= 750.0
    });
    assert!(
        healed,
        "the repaired House must reach the Feudal max of 750, not stop at 550"
    );

    // And a House placed now starts at the Feudal value outright.
    if f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)) < 25.0 {
        sim.push_command(Command::Gather {
            entity: vils[2],
            node: tree,
        });
        let ok = run_until(&mut sim, 120_000, |s| {
            f(s.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)) >= 25.0
        });
        assert!(ok, "never stocked wood for the second House");
    }
    let cell = open_anchor_near(&sim, tc, vils[1], BuildingKind::House);
    build_at(&mut sim, vils[1], BuildingKind::House, cell);
    sim.tick();
    let fresh = sim
        .buildings()
        .iter()
        .filter(|b| b.kind == BuildingKind::House && b.id != house)
        .last()
        .expect("the second House was placed");
    assert_eq!(f(fresh.hp), 750.0, "a Feudal House starts at 750 HP");
}
