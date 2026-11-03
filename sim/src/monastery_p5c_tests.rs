//! Castle P5c tests: the conversion unlocks — Redemption
//! (TechId 37: siege units at range, Rams at contact, buildings
//! adjacent on the slow CI row) and Heresy (TechId 38: units die,
//! buildings are destroyed — MNK-11 as corrected by the P5
//! plan's Passo 0.1), the building arm of the ownership transfer
//! (§3.4: queue/rally persist, passengers eject per Passo 0.2),
//! and the building→unit arm of MNK-07's sticky-CI rule (its
//! test was assigned to this slice by the plan, §5).
//!
//! Values: RULEBOOK §12 (MNK-04/05/07/08/11), dossier §5.6,
//! wiki Redemption / Heresy articles (fetched at build time for
//! Passo 0.1/0.2 — see the handoff note). Fixture style follows
//! `monastery_p5b_tests` (direct records; ages/techs set on the
//! player slots; the monk's HP boosted where a fighting target
//! could end the experiment early).

use crate::build::{Building, BuildingKind};
use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::TechId;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    sim.entities.sort_by_key(|e| e.id);
}

/// A standing building record, copied's shape from
/// `monastery_p5a_tests::placed` (complete, full HP).
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

/// The standard scene: a player Monk (HP boosted) standing in
/// the west ring cell of an opponent Barracks (anchor (62,59),
/// footprint cells x 62..=64, y 59..=61) — footprint distance
/// 2 world units, i.e. ADJACENT — with Redemption granted to
/// the player when `redemption` holds.
fn monk_and_barracks(seed: u64, redemption: bool) -> (Sim, u32, u32) {
    let mut sim = Sim::new(seed);
    if redemption {
        sim.players[0].techs |= TechId::Redemption.bit();
    }
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (61, 60));
    sim.entities
        .iter_mut()
        .find(|e| e.id == 9001)
        .unwrap()
        .hp = I16F16::from_num(30_000);
    let rax = placed(&sim, 900, BuildingKind::Barracks, OWNER_OPPONENT, (62, 59));
    sim.buildings.push(rax);
    (sim, 9001, 900)
}

fn building_owner(sim: &Sim, id: u32) -> Option<u8> {
    sim.buildings().iter().find(|b| b.id == id).map(|b| b.owner)
}

// ------------------------------------------------------------------
// The tech rows (TEC-10's last two Castle entries).

#[test]
fn redemption_and_heresy_rows() {
    // Tags append after P5b's Atonement 35 / Devotion 36.
    assert_eq!(TechId::Redemption.tag(), 37);
    assert_eq!(TechId::Heresy.tag(), 38);
    assert_eq!(TechId::from_tag(37), Some(TechId::Redemption));
    assert_eq!(TechId::from_tag(38), Some(TechId::Heresy));
    assert_eq!(TechId::ALL.len(), 43, "P5f's Imperial row appends at 39–42");
    // Dossier §5.6 rows: Monastery, Castle Age.
    for t in [TechId::Redemption, TechId::Heresy] {
        assert_eq!(
            t.host(),
            crate::TechHost::Building(BuildingKind::Monastery)
        );
        assert_eq!(t.age_required(), crate::tech::Age::Castle);
        assert_eq!(t.requires_tech(), None);
    }
    assert_eq!(TechId::Redemption.cost(), (0, 0, 475, 0), "475 gold");
    assert_eq!(TechId::Redemption.ticks(), 3000, "50 s at 60 Hz");
    assert_eq!(TechId::Heresy.cost(), (0, 0, 1000, 0), "1000 gold");
    assert_eq!(TechId::Heresy.ticks(), 3600, "60 s at 60 Hz");
}

// ------------------------------------------------------------------
// MNK-08's building arm: the eligibility matrix with Redemption.

#[test]
fn building_eligibility_matrix_with_redemption() {
    let (mut sim, monk, rax) = monk_and_barracks(7, true);
    assert!(sim.can_convert_building(monk, rax), "Barracks converts");
    // The never list, with Redemption held: Castle, Monastery,
    // walls, gates, Farms and foundations stay refused.
    for (id, kind) in [
        (901, BuildingKind::Castle),
        (902, BuildingKind::Monastery),
        (903, BuildingKind::StoneWall),
        (904, BuildingKind::PalisadeGate),
        (905, BuildingKind::Farm),
    ] {
        let b = placed(&sim, id, kind, OWNER_OPPONENT, (80, 80));
        sim.buildings.push(b);
        assert!(
            !sim.can_convert_building_target(monk, id),
            "{kind:?} must never convert"
        );
    }
    let mut foundation = placed(&sim, 906, BuildingKind::Barracks, OWNER_OPPONENT, (90, 90));
    foundation.complete = false;
    sim.buildings.push(foundation);
    assert!(
        !sim.can_convert_building_target(monk, 906),
        "a foundation never converts"
    );
    // Without Redemption the whole building arm is closed.
    let (sim2, monk2, rax2) = monk_and_barracks(7, false);
    assert!(!sim2.can_convert_building_target(monk2, rax2));
    assert!(!sim2.can_convert_building(monk2, rax2));
}

// ------------------------------------------------------------------
// MNK-04: the building CI row — adjacent only, CI 15..25, 13%.

#[test]
fn building_conversion_lands_only_in_ci_15_to_25() {
    for seed in 1..=12u64 {
        let (mut sim, monk, rax) = monk_and_barracks(seed, true);
        sim.push_command(Command::ConvertBuilding {
            entity: monk,
            building: rax,
        });
        let mut converted_at = None;
        for n in 1..=1900u64 {
            sim.tick();
            if building_owner(&sim, rax) == Some(OWNER_PLAYER) {
                converted_at = Some(n);
                break;
            }
        }
        let at = converted_at
            .unwrap_or_else(|| panic!("seed {seed}: barracks never converted"));
        assert!(
            (1125..=1875).contains(&at),
            "seed {seed}: building converted at tick {at}, outside CIs 15–25 (1125–1875)"
        );
        assert_eq!(ent(&sim, monk).faith, 0, "success drains faith (MNK-06)");
    }
}

#[test]
fn building_conversion_makes_no_progress_from_range() {
    // The monk starts 9 cells west of the Barracks and must WALK
    // adjacent: while it is still travelling (and for the whole
    // approach), the CI counter does not move — conversion
    // progress is adjacency-only, never range-9 progress.
    let mut sim = Sim::new(7);
    sim.players[0].techs |= TechId::Redemption.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (52, 60));
    let rax = placed(&sim, 900, BuildingKind::Barracks, OWNER_OPPONENT, (62, 59));
    sim.buildings.push(rax);
    sim.push_command(Command::ConvertBuilding {
        entity: 9001,
        building: 900,
    });
    for _ in 0..60 {
        sim.tick();
    }
    let m = ent(&sim, 9001);
    assert_eq!(m.conversion_target_building, Some(900), "task stands");
    assert_eq!(m.ci_progress, 0, "no CI progress while not adjacent");
    assert_eq!(m.ci_ticks, 0, "no in-CI ticks while not adjacent");
    // Left alone, the chase closes and the conversion completes
    // inside the building window plus travel time.
    let mut done = false;
    for _ in 0..4000 {
        sim.tick();
        if building_owner(&sim, 900) == Some(OWNER_PLAYER) {
            done = true;
            break;
        }
    }
    assert!(done, "the chase + chant converts the building");
}

// ------------------------------------------------------------------
// MNK-04's contact/range split for siege units, with Redemption.

#[test]
fn ram_converts_at_contact_only_mangonel_at_range() {
    // Ram: the monk starts 2 tiles away — outside melee reach —
    // so no CI accrues while it closes; the conversion then
    // completes on the ordinary unit window after contact.
    let mut sim = Sim::new(11);
    sim.players[0].techs |= TechId::Redemption.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::BatteringRam, OWNER_OPPONENT, (62, 60));
    sim.entities
        .iter_mut()
        .find(|e| e.id == 9001)
        .unwrap()
        .hp = I16F16::from_num(30_000);
    assert!(sim.can_convert(9001, 9002), "Ram admitted with Redemption");
    sim.push_command(Command::Convert {
        entity: 9001,
        target: 9002,
    });
    for _ in 0..30 {
        sim.tick();
    }
    assert_eq!(
        ent(&sim, 9001).ci_progress,
        0,
        "a Ram at 2 tiles accrues no CI — contact only"
    );
    let mut done = false;
    for _ in 0..3000 {
        sim.tick();
        if ent(&sim, 9002).owner == OWNER_PLAYER {
            done = true;
            break;
        }
    }
    assert!(done, "the Ram converts once the monk is adjacent");

    // Mangonel: an ordinary ranged siege weapon — converts at
    // range on the unit CI window (CIs 5–9), no adjacency owed.
    let mut sim = Sim::new(11);
    sim.players[0].techs |= TechId::Redemption.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Mangonel, OWNER_OPPONENT, (62, 60));
    sim.entities
        .iter_mut()
        .find(|e| e.id == 9001)
        .unwrap()
        .hp = I16F16::from_num(30_000);
    sim.push_command(Command::Convert {
        entity: 9001,
        target: 9002,
    });
    let mut at = None;
    for n in 1..=700u64 {
        sim.tick();
        if ent(&sim, 9002).owner == OWNER_PLAYER {
            at = Some(n);
            break;
        }
    }
    let at = at.expect("Mangonel converts at range");
    assert!(
        (375..=675).contains(&at),
        "Mangonel converted at tick {at}, outside the unit window (375–675)"
    );
}

// ------------------------------------------------------------------
// MNK-07's building→unit arm (the P5c test the plan assigns).

#[test]
fn building_to_unit_switch_resets_ci_unit_switches_do_not() {
    // Building → unit: progress built against the Barracks
    // (13 CIs in, before the first building roll at CI 15)
    // zeroes when the monk is re-tasked onto a unit.
    let (mut sim, monk, rax) = monk_and_barracks(7, true);
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (60, 60));
    sim.push_command(Command::ConvertBuilding {
        entity: monk,
        building: rax,
    });
    for _ in 0..(13 * 75) {
        sim.tick();
    }
    assert_eq!(ent(&sim, monk).ci_progress, 13, "13 building CIs banked");
    sim.push_command(Command::Convert {
        entity: monk,
        target: 9002,
    });
    sim.tick();
    assert_eq!(
        ent(&sim, monk).ci_progress,
        0,
        "building→unit switch resets the counter (update-95810)"
    );

    // Unit → building: the counter SURVIVES the switch.
    let (mut sim, monk, rax) = monk_and_barracks(7, true);
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (60, 60));
    sim.push_command(Command::Convert {
        entity: monk,
        target: 9002,
    });
    for _ in 0..(4 * 75) {
        sim.tick();
    }
    assert_eq!(ent(&sim, monk).ci_progress, 4, "4 unit CIs banked");
    sim.push_command(Command::ConvertBuilding {
        entity: monk,
        building: rax,
    });
    sim.tick();
    assert_eq!(
        ent(&sim, monk).ci_progress,
        4,
        "unit→building switch keeps the counter"
    );
    assert_eq!(ent(&sim, monk).conversion_target_building, Some(rax));
}

// ------------------------------------------------------------------
// §3.4's building arm: queue/rally persist; passengers eject
// with their ownership unchanged (Passo 0.2).

#[test]
fn converted_building_keeps_queue_and_rally_and_ejects_passengers() {
    let (mut sim, monk, rax) = monk_and_barracks(3, true);
    // A queued unit + a rally point on the victim Barracks, and
    // one of the VICTIM's units garrisoned inside it.
    {
        let b = sim
            .buildings
            .iter_mut()
            .find(|b| b.id == rax)
            .unwrap();
        b.queue.push(crate::ProdEntry::Unit(EntityKind::Militia));
        b.rally = Some((I16F16::from_num(100), I16F16::from_num(100)));
    }
    spawn(&mut sim, 9005, EntityKind::Villager, OWNER_OPPONENT, (63, 60));
    {
        let e = sim.entities.iter_mut().find(|e| e.id == 9005).unwrap();
        e.garrisoned_in = Some(GarrisonRef::Building(rax));
        let b = sim
            .buildings
            .iter_mut()
            .find(|b| b.id == rax)
            .unwrap();
        b.garrison.push(9005);
    }
    sim.push_command(Command::ConvertBuilding {
        entity: monk,
        building: rax,
    });
    let mut done = false;
    for _ in 0..1900 {
        sim.tick();
        if building_owner(&sim, rax) == Some(OWNER_PLAYER) {
            done = true;
            break;
        }
    }
    assert!(done, "barracks converted");
    let b = sim.buildings().iter().find(|b| b.id == rax).unwrap();
    assert_eq!(b.queue.len(), 1, "the production queue survives the flip");
    assert!(b.rally.is_some(), "the rally point survives the flip");
    assert!(b.garrison.is_empty(), "passengers left the building");
    let v = ent(&sim, 9005);
    assert_eq!(v.owner, OWNER_OPPONENT, "passenger ownership unchanged");
    assert_eq!(v.garrisoned_in, None, "passenger ejected at the ring");
    assert!(!v.dead, "passenger unharmed");
}

// ------------------------------------------------------------------
// MNK-11: Heresy — units die, buildings are DESTROYED (Passo
// 0.1: the wiki Heresy article, "Converted buildings are
// destroyed", against the rulebook's earlier exempt line).

#[test]
fn heresy_unit_dies_instead_of_converting() {
    let mut sim = Sim::new(7);
    sim.players[1].techs |= TechId::Heresy.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (62, 60));
    sim.entities
        .iter_mut()
        .find(|e| e.id == 9001)
        .unwrap()
        .hp = I16F16::from_num(30_000);
    sim.push_command(Command::Convert {
        entity: 9001,
        target: 9002,
    });
    let mut died = false;
    for _ in 0..800 {
        sim.tick();
        if ent(&sim, 9002).dead {
            died = true;
            break;
        }
    }
    assert!(died, "the Heresy knight dies at the success moment");
    assert_eq!(
        ent(&sim, 9002).owner,
        OWNER_OPPONENT,
        "ownership never moved"
    );
    // The conversion "succeeded" — the monk still rests
    // (MNK-06): faith drained to 0 at the success moment, so by
    // the time the death sweep lands (next tick's combat phase)
    // it holds only a tick or two of regen (27 milli-%/tick),
    // never a full bar.
    assert!(
        ent(&sim, 9001).faith < 1_000,
        "the monk drained at the success moment (faith {})",
        ent(&sim, 9001).faith
    );
}

#[test]
fn heresy_building_is_destroyed_instead_of_converting() {
    let (mut sim, monk, rax) = monk_and_barracks(3, true);
    sim.players[1].techs |= TechId::Heresy.bit();
    sim.push_command(Command::ConvertBuilding {
        entity: monk,
        building: rax,
    });
    let mut gone = false;
    for _ in 0..1900 {
        sim.tick();
        if building_owner(&sim, rax).is_none() {
            gone = true;
            break;
        }
    }
    assert!(gone, "the Heresy barracks is destroyed, not converted");
    assert_eq!(ent(&sim, monk).faith, 0, "the monk still rests");
}
