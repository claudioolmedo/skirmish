//! Castle P5f tests: the Imperial Monastery row (TEC-10
//! completion) — Block Printing (TechId 39: +3 conversion
//! range), Illumination (TechId 40: faith recharge 62 s →
//! 33 s), Theocracy (TechId 41: group conversion drains only
//! the lowest-id participant) and Faith (TechId 42: the
//! target owner's units get +4/+4 CI shift; requires Devotion,
//! TEC-07).
//!
//! Values verified 2026-11-03 against the AoE2 wiki
//! (Technology table + Faith article, update 99311 values for
//! Faith). Fixture style follows `monastery_p5b_tests` /
//! `monastery_p5c_tests` (direct records; ages/techs set on the
//! player slots; the monk's HP boosted where a fighting target
//! could end the experiment early).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, TechId};
use crate::{Command, ResearchBlock, Sim, TechHost};
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
/// `monastery_p5c_tests::placed` (complete, full HP).
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

fn sim_in_imperial() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Imperial;
    sim.players[1].age = Age::Imperial;
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

/// A monk (player) and a target (opponent) two tiles apart —
/// well inside conversion range 9. The monk's HP is boosted so
/// a fighting target cannot end the experiment early.
fn monk_and_target(seed: u64, target: EntityKind) -> (Sim, u32, u32) {
    let mut sim = Sim::new(seed);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, target, OWNER_OPPONENT, (62, 60));
    sim.entities.iter_mut().find(|e| e.id == 9001).unwrap().hp = I16F16::from_num(30_000);
    (sim, 9001, 9002)
}

fn ticks_until_converted(sim: &mut Sim, target: u32, cap: u64) -> Option<u64> {
    for n in 1..=cap {
        sim.tick();
        if ent(sim, target).owner == OWNER_PLAYER {
            return Some(n);
        }
    }
    None
}

// ------------------------------------------------------------------
// The tech rows (TEC-10's Imperial row) and their gates.

#[test]
fn imperial_monastery_rows() {
    // Tags append after P5c's Redemption 37 / Heresy 38.
    assert_eq!(TechId::BlockPrinting.tag(), 39);
    assert_eq!(TechId::Illumination.tag(), 40);
    assert_eq!(TechId::Theocracy.tag(), 41);
    assert_eq!(TechId::Faith.tag(), 42);
    assert_eq!(TechId::from_tag(39), Some(TechId::BlockPrinting));
    assert_eq!(TechId::from_tag(40), Some(TechId::Illumination));
    assert_eq!(TechId::from_tag(41), Some(TechId::Theocracy));
    assert_eq!(TechId::from_tag(42), Some(TechId::Faith));
    assert_eq!(TechId::from_tag(43), Some(TechId::BowSaw));
    assert_eq!(TechId::ALL.len(), 48, "the TEC-07 wood chain appends at 43/44, Water W2a's Dock fishing pair at 45/46, Water W3b's Medium Warships at 47");
    // All four: Monastery, Imperial Age.
    for t in [
        TechId::BlockPrinting,
        TechId::Illumination,
        TechId::Theocracy,
        TechId::Faith,
    ] {
        assert_eq!(
            t.host(),
            TechHost::Building(BuildingKind::Monastery),
            "{t:?} host"
        );
        assert_eq!(t.age_required(), Age::Imperial, "{t:?} age");
    }
    assert_eq!(TechId::BlockPrinting.cost(), (0, 0, 200, 0), "200 gold");
    assert_eq!(TechId::BlockPrinting.ticks(), 3300, "55 s at 60 Hz");
    assert_eq!(TechId::Illumination.cost(), (0, 0, 120, 0), "120 gold");
    assert_eq!(TechId::Illumination.ticks(), 3900, "65 s at 60 Hz");
    assert_eq!(TechId::Theocracy.cost(), (0, 0, 200, 0), "200 gold");
    assert_eq!(TechId::Theocracy.ticks(), 4500, "75 s at 60 Hz");
    assert_eq!(
        TechId::Faith.cost(),
        (550, 0, 750, 0),
        "550 food / 750 gold (update 99311)"
    );
    assert_eq!(TechId::Faith.ticks(), 3600, "60 s at 60 Hz");
    // TEC-07: Faith alone chains — from Devotion.
    assert_eq!(TechId::Faith.requires_tech(), Some(TechId::Devotion));
    for t in [
        TechId::BlockPrinting,
        TechId::Illumination,
        TechId::Theocracy,
    ] {
        assert_eq!(t.requires_tech(), None, "{t:?} prerequisite");
    }
}

#[test]
fn imperial_monastery_gates() {
    let mut sim = sim_in_imperial();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    let rax_id = rax.id;
    sim.buildings.push(rax);
    // Imperial at the Monastery: the three unchained rows accept.
    for t in [
        TechId::BlockPrinting,
        TechId::Illumination,
        TechId::Theocracy,
    ] {
        assert_eq!(sim.research_block_at(mon_id, t), ResearchBlock::Ok, "{t:?}");
        assert_eq!(
            sim.research_block_at(rax_id, t),
            ResearchBlock::Locked,
            "{t:?} is Monastery research, not Barracks"
        );
    }
    // Faith is Locked without Devotion, Ok with it.
    assert_eq!(
        sim.research_block_at(mon_id, TechId::Faith),
        ResearchBlock::Locked,
        "Faith locked without Devotion (TEC-07)"
    );
    assert!(!sim.can_research_at(mon_id, TechId::Faith));
    sim.players[0].techs |= TechId::Devotion.bit();
    assert_eq!(
        sim.research_block_at(mon_id, TechId::Faith),
        ResearchBlock::Ok
    );
    assert!(sim.can_research_at(mon_id, TechId::Faith));
    // Every earlier Age refuses all four (Devotion held, so the
    // Age alone is the block under test for Faith).
    for age in [Age::Castle, Age::Feudal, Age::Dark] {
        sim.players[0].age = age;
        for t in [
            TechId::BlockPrinting,
            TechId::Illumination,
            TechId::Theocracy,
            TechId::Faith,
        ] {
            assert_eq!(
                sim.research_block_at(mon_id, t),
                ResearchBlock::Locked,
                "{t:?} refused before the Imperial Age ({age:?})"
            );
        }
    }
}

#[test]
fn block_printing_research_flow_sets_the_flag() {
    let mut sim = sim_in_imperial();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::BlockPrinting));
    let gold_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    sim.push_command(Command::ResearchAt {
        building: mon_id,
        tech: TechId::BlockPrinting,
    });
    sim.tick();
    let gold_after = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    assert_eq!(gold_before - gold_after, 200, "the research costs 200 gold");
    // 55 s of research: not done early, done shortly after.
    for _ in 0..3200 {
        sim.tick();
        assert!(
            !sim.has_tech(OWNER_PLAYER, TechId::BlockPrinting),
            "Block Printing completed faster than 55 s"
        );
    }
    let mut done = false;
    for _ in 0..200 {
        sim.tick();
        if sim.has_tech(OWNER_PLAYER, TechId::BlockPrinting) {
            done = true;
            break;
        }
    }
    assert!(done, "Block Printing never completed");
}

// ------------------------------------------------------------------
// MNK-02 — Illumination: faith regens in exactly 33 s (62 s before).

#[test]
fn illumination_regenerates_faith_in_exactly_33s() {
    // Before: the base 62 s span (3 720 ticks), re-pinned here
    // through the P5f code path (P5b pins it on its own path).
    let (mut sim, monk, _knight) = monk_and_target(7, EntityKind::Knight);
    sim.entities
        .iter_mut()
        .find(|e| e.id == monk)
        .unwrap()
        .faith = 0;
    for _ in 0..3719 {
        sim.tick();
    }
    assert!(
        ent(&sim, monk).faith < 100_000,
        "not full one tick early (62 s)"
    );
    sim.tick();
    assert_eq!(ent(&sim, monk).faith, 100_000, "full at exactly 62 s");

    // After Illumination: full in exactly 33 s (1 980 ticks).
    let (mut sim, monk, _knight) = monk_and_target(7, EntityKind::Knight);
    sim.players[0].techs |= TechId::Illumination.bit();
    sim.entities
        .iter_mut()
        .find(|e| e.id == monk)
        .unwrap()
        .faith = 0;
    for _ in 0..1979 {
        sim.tick();
    }
    assert!(
        ent(&sim, monk).faith < 100_000,
        "not full one tick early (33 s)"
    );
    sim.tick();
    assert_eq!(ent(&sim, monk).faith, 100_000, "full at exactly 33 s");
}

// ------------------------------------------------------------------
// MNK-05 — Faith: the victim's window shifts +4/+4 CI.

#[test]
fn faith_shifts_the_victims_window_by_four_ci() {
    let (mut sim, monk, knight) = monk_and_target(7, EntityKind::Knight);
    // The TARGET's owner holds Faith: +4/+4.
    sim.players[1].techs |= TechId::Faith.bit();
    sim.push_command(Command::Convert {
        entity: monk,
        target: knight,
    });
    sim.tick();
    let m = ent(&sim, monk);
    assert_eq!(m.conversion_start_shift, 4);
    assert_eq!(m.conversion_guarantee_shift, 4);
    // `at` counts from the helper's first tick; the stamp tick
    // above is tick 1 of the engagement (the P5b devotion
    // test's +1 accounting).
    let at = ticks_until_converted(&mut sim, knight, 1000).expect("converts") + 1;
    assert!(
        (675..=975).contains(&at),
        "faith window should be CIs 9–13 (675–975), landed at {at}"
    );
}

// ------------------------------------------------------------------
// MNK-06 — Theocracy: only the lowest-id participant drains.

#[test]
fn theocracy_drains_only_the_lowest_id_participant() {
    let mut sim = Sim::new(7);
    sim.players[0].techs |= TechId::Theocracy.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Monk, OWNER_PLAYER, (60, 62));
    spawn(&mut sim, 9003, EntityKind::Monk, OWNER_PLAYER, (62, 62));
    spawn(&mut sim, 9010, EntityKind::Knight, OWNER_OPPONENT, (61, 61));
    for id in [9001u32, 9002, 9003] {
        sim.entities.iter_mut().find(|e| e.id == id).unwrap().hp = I16F16::from_num(30_000);
        sim.push_command(Command::Convert {
            entity: id,
            target: 9010,
        });
    }
    let at = ticks_until_converted(&mut sim, 9010, 700).expect("knight converts");
    assert!(at <= 675, "group conversion still lands inside one window");
    // The lowest-id participant rests: drained to 0 by the
    // success (a participant processed after the converter in
    // the same tick has regenned at most a tick or two of faith
    // by the time the tick ends — the P5b group test's pin).
    assert!(
        ent(&sim, 9001).faith <= 26,
        "lowest-id participant must rest, faith {}",
        ent(&sim, 9001).faith
    );
    // The others keep their faith in full; their records still
    // clear (the target is theirs now).
    for id in [9002u32, 9003] {
        assert_eq!(
            ent(&sim, id).faith,
            100_000,
            "participant {id} keeps its faith under Theocracy"
        );
        assert_eq!(ent(&sim, id).conversion_target, None);
    }
}

// ------------------------------------------------------------------
// MNK-03 — Block Printing: +3 conversion range.

#[test]
fn block_printing_extends_conversion_start_range_by_three_tiles() {
    // 11 tiles apart (44 world units): beyond the base range 9
    // (36), inside the extended range 12 (48).
    // Without Block Printing: no CI accrues — the monk chases.
    let mut sim = Sim::new(7);
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (71, 60));
    sim.entities.iter_mut().find(|e| e.id == 9001).unwrap().hp = I16F16::from_num(30_000);
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
        "11 tiles accrues no CI at the base range 9"
    );
    assert_eq!(ent(&sim, 9001).ci_ticks, 0);

    // With Block Printing: the monk stands and chants from
    // 11 tiles — CI progress accrues and the conversion lands
    // on the ordinary unit window.
    let mut sim = Sim::new(7);
    sim.players[0].techs |= TechId::BlockPrinting.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (71, 60));
    sim.entities.iter_mut().find(|e| e.id == 9001).unwrap().hp = I16F16::from_num(30_000);
    let (mx, my) = (ent(&sim, 9001).pos_x, ent(&sim, 9001).pos_y);
    sim.push_command(Command::Convert {
        entity: 9001,
        target: 9002,
    });
    for _ in 0..75 {
        sim.tick();
    }
    assert_eq!(
        ent(&sim, 9001).ci_progress,
        1,
        "one CI banked from 11 tiles with Block Printing"
    );
    assert_eq!(
        (ent(&sim, 9001).pos_x, ent(&sim, 9001).pos_y),
        (mx, my),
        "an in-range monk stands — no chase at 11 tiles"
    );
    let at = ticks_until_converted(&mut sim, 9002, 700).expect("knight converts");
    assert!(at + 75 <= 675, "conversion lands inside the unit window");

    // Beyond the extended range (13 tiles = 52 world units):
    // no CI accrues even with Block Printing — the monk chases.
    let mut sim = Sim::new(7);
    sim.players[0].techs |= TechId::BlockPrinting.bit();
    spawn(&mut sim, 9001, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 9002, EntityKind::Knight, OWNER_OPPONENT, (73, 60));
    sim.entities.iter_mut().find(|e| e.id == 9001).unwrap().hp = I16F16::from_num(30_000);
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
        "13 tiles accrues no CI even with Block Printing"
    );
    assert_eq!(ent(&sim, 9001).ci_ticks, 0);
}
