//! VIC-02 tests (wonder victory): the Wonder building row
//! (1000 W / 1000 S / 1000 G, 3500 s, 5×5, 4800 HP flat, 3/10
//! armor, Imperial Age, no prerequisite) and its countdown —
//! it starts only while a COMPLETED Wonder of one player
//! stands under the `wonder_victory` setting, ticks down from
//! the shared Tiny duration (`WONDER_VICTORY_DURATION_TICKS`,
//! the VIC-02/VIC-03 table: 100 years at 5 s/year), cancels
//! outright when the last Wonder falls or is deleted (never
//! pauses), and expires into the holder's win through
//! `match_state`. Plain Conquest (setting off) never counts
//! down, however many Wonders stand; Sandbox never counts
//! down; a foundation never counts. Fixture style follows
//! `victory_relic_tests` (direct records).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::Age;
use crate::{
    Command, MatchMode, MatchState, Sim, VictorySettings, RELIC_VICTORY_DURATION_TICKS,
    WONDER_VICTORY_DURATION_TICKS,
};
use fixed::types::I16F16;

fn wonder_settings() -> VictorySettings {
    VictorySettings {
        wonder_victory: true,
        ..VictorySettings::conquest()
    }
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

fn tc_cell(sim: &Sim, owner: u8) -> (u32, u32) {
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == EntityKind::TownCenter)
        .unwrap();
    sim.grid().world_to_cell(tc.pos_x, tc.pos_y)
}

/// Add a Wonder for `owner` near their Town Center and return
/// its building id. `complete` false leaves it a foundation.
fn add_wonder(sim: &mut Sim, owner: u8, id: u32, complete: bool) -> u32 {
    let (cx, cy) = tc_cell(sim, owner);
    let mut b = placed(sim, id, BuildingKind::Wonder, owner, (cx + 6, cy + 6));
    if !complete {
        b.complete = false;
        b.progress_thirds = 0;
    }
    sim.buildings.push(b);
    if sim.next_building_id <= id {
        sim.next_building_id = id + 1;
    }
    id
}

fn settle(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn settle_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn wonder_match(seed: u64) -> Sim {
    Sim::new_match_victory(seed, MatchMode::TwoPlayer, wonder_settings())
}

// ------------------------------------------------------------------
// The building row (WONDER-DOSSIER §2).

#[test]
fn wonder_row_matches_the_dossier() {
    let k = BuildingKind::Wonder;
    assert_eq!(k.tag(), 22);
    assert_eq!(BuildingKind::from_tag(22), Some(k));
    assert_eq!(k.wood_cost(), I16F16::from_num(1000));
    assert_eq!(k.stone_cost(), I16F16::from_num(1000));
    assert_eq!(k.gold_cost(), I16F16::from_num(1000));
    // The full 3000 is the VIC-04 Society value of the building.
    assert_eq!(k.total_cost(), I16F16::from_num(3000));
    assert_eq!(k.build_ticks(), 210_000); // 3500 s at 60 Hz
    assert_eq!(k.footprint_size(), 5);
    assert_eq!(k.max_hp(), 4800); // flat — no BLD-11 step
    assert_eq!(k.armor_melee(), 3);
    assert_eq!(k.armor_pierce(), 10);
    assert_eq!(k.sight_tiles_at(Age::Imperial), 8);
    assert_eq!(k.age_required(), Age::Imperial);
    assert_eq!(k.requires_building(), None);
    assert_eq!(k.pop_support(), 0);
}

#[test]
fn gold_cost_is_zero_for_every_existing_kind() {
    // The cost-model extension is inert for the kinds that
    // predate it: their gold leg reads 0 and their totals are
    // unchanged (wood + stone), so their behaviour — placement,
    // refunds, repair drips, razing and conversion credit — is
    // byte-identical to before VIC-02.
    for tag in 0..=21u8 {
        let k = BuildingKind::from_tag(tag).unwrap();
        assert_eq!(k.gold_cost(), I16F16::ZERO, "tag {tag}");
        assert_eq!(k.total_cost(), k.wood_cost() + k.stone_cost(), "tag {tag}");
    }
    assert_eq!(
        BuildingKind::Castle.total_cost(),
        I16F16::from_num(650),
        "the Castle still totals its 650 stone"
    );
    assert_eq!(
        BuildingKind::TownCenter.total_cost(),
        I16F16::from_num(375),
        "the Town Center still totals 275 wood + 100 stone"
    );
}

// ------------------------------------------------------------------
// Placement economy: the three-resource charge and refund.

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

fn amount(sim: &Sim, owner: u8, kind: ResourceKind) -> I16F16 {
    sim.stockpile_of(owner).amount(kind)
}

fn human_villager(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::Villager)
        .unwrap()
        .id
}

/// The first cell near the human Town Center where the sim
/// itself accepts a Wonder (the 5×5 footprint needs clear
/// ground — the scan asks `can_build`, so the fixture can
/// never drift from the placement rules it exercises).
fn buildable_wonder_cell(sim: &Sim) -> (u32, u32) {
    let villager = human_villager(sim);
    let (tcx, tcy) = tc_cell(sim, OWNER_PLAYER);
    for dy in 0..24u32 {
        for dx in 0..24u32 {
            let (x, y) = sim.grid().cell_center(tcx + dx, tcy + dy);
            if sim.can_build(villager, BuildingKind::Wonder, x, y) {
                return (tcx + dx, tcy + dy);
            }
        }
    }
    panic!("no buildable Wonder cell near the Town Center");
}

#[test]
fn placement_charges_all_three_resources_and_delete_refunds_them() {
    let mut sim = wonder_match(42);
    sim.players[0].age = Age::Imperial;
    top_up(&mut sim, OWNER_PLAYER);
    let (wood0, stone0, gold0, food0) = (
        amount(&sim, OWNER_PLAYER, ResourceKind::Wood),
        amount(&sim, OWNER_PLAYER, ResourceKind::Stone),
        amount(&sim, OWNER_PLAYER, ResourceKind::Gold),
        amount(&sim, OWNER_PLAYER, ResourceKind::Food),
    );
    let (cx, cy) = buildable_wonder_cell(&sim);
    let (x, y) = sim.grid().cell_center(cx, cy);
    sim.push_command(Command::Build {
        entity: human_villager(&sim),
        kind: BuildingKind::Wonder,
        x,
        y,
    });
    settle(&mut sim, 2);
    let wonder = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Wonder)
        .expect("the foundation is placed")
        .id;
    assert_eq!(
        amount(&sim, OWNER_PLAYER, ResourceKind::Wood),
        wood0 - I16F16::from_num(1000)
    );
    assert_eq!(
        amount(&sim, OWNER_PLAYER, ResourceKind::Stone),
        stone0 - I16F16::from_num(1000)
    );
    assert_eq!(
        amount(&sim, OWNER_PLAYER, ResourceKind::Gold),
        gold0 - I16F16::from_num(1000)
    );
    assert_eq!(
        amount(&sim, OWNER_PLAYER, ResourceKind::Food),
        food0,
        "the Wonder costs no food"
    );
    // Deleting the foundation refunds all three legs in full
    // (BLD-02's cancellation rule, gold leg included).
    sim.push_command(Command::DeleteBuilding { building: wonder });
    settle(&mut sim, 2);
    assert_eq!(amount(&sim, OWNER_PLAYER, ResourceKind::Wood), wood0);
    assert_eq!(amount(&sim, OWNER_PLAYER, ResourceKind::Stone), stone0);
    assert_eq!(amount(&sim, OWNER_PLAYER, ResourceKind::Gold), gold0);
}

#[test]
fn placement_refuses_when_any_leg_is_short_or_the_age_is_short() {
    let mut sim = wonder_match(42);
    sim.players[0].age = Age::Imperial;
    top_up(&mut sim, OWNER_PLAYER);
    let villager = human_villager(&sim);
    // A cell the sim accepts while every leg is funded — the
    // refusals below are then about the one condition changed.
    let (cx, cy) = buildable_wonder_cell(&sim);
    let (x, y) = sim.grid().cell_center(cx, cy);
    assert!(sim.can_build(villager, BuildingKind::Wonder, x, y));
    // Drain the gold below 1000: the Wonder refuses, and the
    // refusal is atomic — wood and stone stay untouched.
    let gold = amount(&sim, OWNER_PLAYER, ResourceKind::Gold);
    sim.stockpile_mut(OWNER_PLAYER)
        .spend(ResourceKind::Gold, gold - I16F16::from_num(500));
    let (wood0, stone0) = (
        amount(&sim, OWNER_PLAYER, ResourceKind::Wood),
        amount(&sim, OWNER_PLAYER, ResourceKind::Stone),
    );
    assert!(!sim.can_build(villager, BuildingKind::Wonder, x, y));
    sim.push_command(Command::Build {
        entity: villager,
        kind: BuildingKind::Wonder,
        x,
        y,
    });
    settle(&mut sim, 2);
    assert!(sim
        .buildings()
        .iter()
        .all(|b| b.kind != BuildingKind::Wonder));
    assert_eq!(amount(&sim, OWNER_PLAYER, ResourceKind::Wood), wood0);
    assert_eq!(amount(&sim, OWNER_PLAYER, ResourceKind::Stone), stone0);
    // Below Imperial the age gate refuses outright, funds or no
    // funds (BLD-15): the same cell, found under Imperial,
    // refuses once the age drops back to Castle.
    let mut young = wonder_match(42);
    young.players[0].age = Age::Imperial;
    top_up(&mut young, OWNER_PLAYER);
    let villager = human_villager(&young);
    let (cx, cy) = buildable_wonder_cell(&young);
    let (x, y) = young.grid().cell_center(cx, cy);
    young.players[0].age = Age::Castle;
    assert!(!young.can_build(villager, BuildingKind::Wonder, x, y));
}

// ------------------------------------------------------------------
// The countdown.

#[test]
fn duration_is_tiny_from_the_shared_table() {
    // 200 years (Tiny, the table VIC-03 also reads) × 5 s/year
    // × 60 ticks/s — the two victory conditions share the row.
    assert_eq!(WONDER_VICTORY_DURATION_TICKS, 200 * 5 * 60);
    assert_eq!(WONDER_VICTORY_DURATION_TICKS, 60_000);
    assert_eq!(
        WONDER_VICTORY_DURATION_TICKS,
        RELIC_VICTORY_DURATION_TICKS,
        "one shared duration table (lib.rs, VIC-02/VIC-03)"
    );
}

#[test]
fn countdown_starts_only_with_a_completed_wonder() {
    let mut sim = wonder_match(42);
    // A foundation is not a Wonder yet: no countdown, however
    // long it stands.
    let wonder = add_wonder(&mut sim, OWNER_PLAYER, 900, false);
    settle(&mut sim, 120);
    assert_eq!(sim.wonder_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    // Completion starts the countdown on the next tick, at the
    // full duration minus that tick.
    let b = sim.buildings.iter_mut().find(|b| b.id == wonder).unwrap();
    b.complete = true;
    b.progress_thirds = BuildingKind::Wonder.build_thirds();
    settle(&mut sim, 1);
    assert_eq!(
        sim.wonder_countdown(),
        Some((OWNER_PLAYER, WONDER_VICTORY_DURATION_TICKS - 1))
    );
}

#[test]
fn countdown_ticks_down_while_the_wonder_stands() {
    let mut sim = wonder_match(42);
    add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, 600);
    assert_eq!(
        sim.wonder_countdown(),
        Some((OWNER_PLAYER, WONDER_VICTORY_DURATION_TICKS - 600))
    );
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn a_second_wonder_keeps_the_countdown_when_one_falls() {
    // Multiple Wonders are allowed and the census is existence,
    // not identity: losing one of two changes nothing.
    let mut sim = wonder_match(42);
    add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    let (cx, cy) = tc_cell(&sim, OWNER_PLAYER);
    let mut second = placed(&sim, 901, BuildingKind::Wonder, OWNER_PLAYER, (cx + 14, cy + 6));
    second.complete = true;
    sim.buildings.push(second);
    sim.next_building_id = 902;
    settle(&mut sim, 300);
    assert!(sim.wonder_countdown().is_some());
    sim.push_command(Command::DeleteBuilding { building: 900 });
    settle(&mut sim, 2);
    assert_eq!(
        sim.wonder_countdown(),
        Some((OWNER_PLAYER, WONDER_VICTORY_DURATION_TICKS - 302))
    );
}

#[test]
fn countdown_cancels_when_the_wonder_is_deleted() {
    let mut sim = wonder_match(42);
    let wonder = add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, 500);
    assert!(sim.wonder_countdown().is_some());
    // Deletion cancels outright — the countdown is gone, not
    // paused — and a new Wonder restarts it from the full
    // duration.
    sim.push_command(Command::DeleteBuilding { building: wonder });
    settle(&mut sim, 2);
    assert_eq!(sim.wonder_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    add_wonder(&mut sim, OWNER_PLAYER, 901, true);
    settle(&mut sim, 1);
    assert_eq!(
        sim.wonder_countdown(),
        Some((OWNER_PLAYER, WONDER_VICTORY_DURATION_TICKS - 1))
    );
}

#[test]
fn countdown_cancels_when_the_wonder_is_destroyed() {
    let mut sim = wonder_match(42);
    let wonder = add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, 300);
    assert!(sim.wonder_countdown().is_some());
    // Wound the Wonder to a few HP and let an enemy Militia
    // finish it through the ordinary combat path.
    let b = sim.buildings.iter_mut().find(|b| b.id == wonder).unwrap();
    b.hp = I16F16::from_num(3);
    let (cx, cy) = tc_cell(&sim, OWNER_PLAYER);
    let (mx, my) = sim.grid().cell_center(cx + 5, cy + 8);
    sim.entities.push(crate::entity::Entity::spawned(
        9_000,
        EntityKind::Militia,
        OWNER_OPPONENT,
        mx,
        my,
    ));
    sim.push_command(Command::AttackBuilding {
        entity: 9_000,
        building: wonder,
    });
    let gone = settle_until(&mut sim, 3_000, |s| {
        s.buildings().iter().all(|b| b.id != wonder)
    });
    assert!(gone, "the Militia razes the wounded Wonder");
    settle(&mut sim, 2);
    assert_eq!(sim.wonder_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn countdown_expires_into_the_holders_win() {
    let mut sim = wonder_match(42);
    add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, WONDER_VICTORY_DURATION_TICKS - 1);
    assert_eq!(sim.wonder_countdown(), Some((OWNER_PLAYER, 1)));
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    settle(&mut sim, 1);
    assert_eq!(sim.wonder_countdown(), Some((OWNER_PLAYER, 0)));
    assert_eq!(sim.match_state(), MatchState::Victory);
}

#[test]
fn countdown_expires_into_defeat_when_the_opponent_holds() {
    let mut sim = wonder_match(42);
    add_wonder(&mut sim, OWNER_OPPONENT, 901, true);
    settle(&mut sim, WONDER_VICTORY_DURATION_TICKS);
    assert_eq!(sim.wonder_countdown(), Some((OWNER_OPPONENT, 0)));
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

#[test]
fn conquest_never_counts_down_even_with_a_wonder_standing() {
    let mut sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::conquest());
    add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, WONDER_VICTORY_DURATION_TICKS + 10);
    assert_eq!(sim.wonder_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn sandbox_never_counts_down() {
    let mut sim = Sim::with_settings(42, MatchMode::Sandbox, wonder_settings());
    add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, 600);
    assert_eq!(sim.wonder_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn countdown_state_is_hashed_and_deterministic() {
    let mut a = wonder_match(42);
    let mut b = wonder_match(42);
    for sim in [&mut a, &mut b] {
        add_wonder(sim, OWNER_PLAYER, 900, true);
    }
    settle(&mut a, 100);
    settle(&mut b, 100);
    assert_eq!(a.state_hash(), b.state_hash());
    // The accrued countdown is hashed state: the hash moves as
    // the count accrues, so two countdowns can never silently
    // agree (and a replay reproduces every tick of it).
    let h100 = a.state_hash();
    settle(&mut a, 100);
    assert_ne!(a.state_hash(), h100);
}

#[test]
fn countdown_round_trips_through_a_save() {
    let mut sim = wonder_match(42);
    add_wonder(&mut sim, OWNER_PLAYER, 900, true);
    settle(&mut sim, 1_234);
    let before = sim.wonder_countdown();
    assert!(before.is_some());
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("the save restores");
    assert_eq!(restored.victory_settings(), wonder_settings());
    assert_eq!(restored.wonder_countdown(), before);
    // And both continue identically from the restore point.
    settle(&mut sim, 60);
    settle(&mut restored, 60);
    assert_eq!(restored.state_hash(), sim.state_hash());
}

