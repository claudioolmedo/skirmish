//! Castle P5a tests (2026-10-31): the Monastery (175 W, 40 s,
//! 3×3, 2100 HP flat, 2/9 armor, LoS 6, Castle Age, NO
//! prerequisite building) and the Monk (100 G, 51 s, HP 30,
//! no attack — CLS-08), its healing (MNK-10: range 4 tiles,
//! 150 HP/min exactly, one target, auto-task, the siege/
//! building/enemy exclusions), Sanctity and Fervor (TEC-10's
//! first two rows), and the scripted AI's monk slice.
//!
//! Values: the Castle Age dossier §5.6 (wiki, Monastery / Monk
//! / Sanctity / Fervor / Healing, fetched 2026-10-22 and
//! re-verified 2026-10-30), RULEBOOK MNK-01/10, CLS-08, TEC-10.
//! Fixture style follows `siege_p4a_tests` (direct records;
//! ages set on the player slots).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, TechId};
use crate::{Command, MatchMode, ResearchBlock, Sim, TechHost};
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
        relics: Vec::new(),
        relic_gold_remainder: 0,
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

// ------------------------------------------------------------------
// The building row and its gates.

#[test]
fn monastery_row_matches_the_dossier() {
    let k = BuildingKind::Monastery;
    assert_eq!(k.tag(), 20);
    assert_eq!(BuildingKind::from_tag(20), Some(k));
    assert_eq!(k.wood_cost().to_num::<i32>(), 175);
    assert_eq!(k.build_ticks(), 2400, "40 s at 60 Hz");
    assert_eq!(k.footprint_size(), 3);
    // Flat 2100 at every Age (the infobox's single HP figure).
    assert_eq!(k.max_hp_at(Age::Castle), 2100);
    assert_eq!(k.max_hp_at(Age::Imperial), 2100);
    assert_eq!(k.armor_melee_at(Age::Castle), 2);
    assert_eq!(k.armor_pierce_at(Age::Castle), 9);
    assert_eq!(k.sight_tiles_at(Age::Castle), 6);
    assert_eq!(k.age_required(), Age::Castle);
    assert_eq!(k.requires_building(), None, "no prerequisite building");
    assert!(k.counts_toward_imperial());
    assert!(k.is_production());
    assert!(k.trains(EntityKind::Monk));
    assert!(!k.trains(EntityKind::Militia));
    assert_eq!(k.garrison_capacity(), Some(10));
    assert!(k.is_production_garrison());
    assert!(!k.is_defensive_garrison());
    assert_eq!(k.weapon(), None, "the Monastery never fires");
}

#[test]
fn monastery_build_gate_and_flow() {
    let mut sim = sim_in_castle();
    top_up(&mut sim, OWNER_PLAYER);
    // A legal anchor near the Town Center, found with the sim's
    // own placement validation (the AI's `find_anchor`
    // discipline).
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let mut anchor = None;
    'scan: for r in 3..=10i32 {
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
                if sim.can_build(1, BuildingKind::Monastery, x, y) {
                    anchor = Some((x, y));
                    break 'scan;
                }
            }
        }
    }
    let (x, y) = anchor.expect("no legal Monastery anchor near the TC");
    // Age law: Feudal refuses, Castle accepts — with NO other
    // building standing (the dossier's "Prerequisite: none").
    sim.players[0].age = Age::Feudal;
    assert!(!sim.can_build(1, BuildingKind::Monastery, x, y));
    sim.players[0].age = Age::Castle;
    assert!(sim.can_build(1, BuildingKind::Monastery, x, y));
    let wood_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Wood)
        .to_num::<i32>();
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Monastery,
        x,
        y,
    });
    let done = run_until(&mut sim, 30_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::Monastery && b.complete)
    });
    assert!(done, "the Monastery never completed");
    let wood_after = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Wood)
        .to_num::<i32>();
    assert_eq!(wood_before - wood_after, 175);
}

// ------------------------------------------------------------------
// Training the Monk.

#[test]
fn monk_training_flow_and_row() {
    let mut sim = sim_in_castle();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    let rax_id = rax.id;
    sim.buildings.push(rax);
    // The Monk trains at the Monastery only.
    assert!(!sim.can_train_at(rax_id, EntityKind::Monk));
    assert!(sim.can_train_at(mon_id, EntityKind::Monk));
    let gold_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    sim.push_command(Command::TrainAt {
        building: mon_id,
        kind: EntityKind::Monk,
    });
    sim.tick();
    let gold_after = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    assert_eq!(gold_before - gold_after, 100, "the Monk costs 100 gold");
    // 51 s of production: not out early, out shortly after.
    let early = run_until(&mut sim, 2900, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Monk)
    });
    assert!(!early, "the Monk trained faster than 51 s");
    let out = run_until(&mut sim, 600, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Monk)
    });
    assert!(out, "the Monk never left the Monastery");
    let monk = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Monk)
        .unwrap();
    assert_eq!(monk.hp, I16F16::from_num(30));
    assert_eq!(monk.owner, OWNER_PLAYER);
}

// ------------------------------------------------------------------
// SEL-09: the idle-military key finds the Monk (bot-loop fix,
// 2026-11-01 16:22 — three live sessions trained a Monk and could
// not find it with any in-client tool; the idle query's subject
// list had never been extended to the kind).

#[test]
fn idle_military_key_finds_the_monk() {
    let mut sim = sim_in_castle();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    sim.push_command(Command::TrainAt {
        building: mon_id,
        kind: EntityKind::Monk,
    });
    let out = run_until(&mut sim, 4_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Monk)
    });
    assert!(out, "the Monk never left the Monastery");
    let monk_id = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Monk)
        .unwrap()
        .id;
    // Nobody is wounded in this fixture, so the monk stands idle
    // at its spawn point — and the idle-military key MUST list it.
    assert!(
        sim.idle_military().contains(&monk_id),
        "idle_military() = {:?} does not contain the idle Monk {}",
        sim.idle_military(),
        monk_id
    );
    // The widening is ONLY the idle key's: the Monk stays out of
    // the combat semantics `is_military` gates (no stance, no
    // attack-move, no AI army counts).
    assert!(!EntityKind::Monk.is_military());
    assert!(EntityKind::Monk.is_idle_military());
    assert!(!sim.can_set_stance(monk_id));
}

// ------------------------------------------------------------------
// MNK-10: healing.

#[test]
fn heal_rate_is_exact_and_never_overheals() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 901, EntityKind::Knight, OWNER_PLAYER, (61, 60));
    wound(&mut sim, 901, 1);
    // One second of healing: exactly +2.5 HP.
    run_until(&mut sim, 60, |_| false);
    assert_eq!(ent(&sim, 901).hp, I16F16::from_bits(3 * 65_536 + 32_768));
    // Twenty seconds total: exactly +50 HP, no drift.
    run_until(&mut sim, 1140, |_| false);
    assert_eq!(ent(&sim, 901).hp, I16F16::from_num(51));
    // To full (100): the task completes, the remainder resets,
    // and further ticks change nothing.
    let full = run_until(&mut sim, 2400, |s| ent(s, 901).hp >= I16F16::from_num(100));
    assert!(full, "the Knight never healed to full");
    assert_eq!(ent(&sim, 900).heal_target, None);
    assert_eq!(ent(&sim, 900).heal_remainder, 0);
    run_until(&mut sim, 600, |_| false);
    assert_eq!(ent(&sim, 901).hp, I16F16::from_num(100));
}

#[test]
fn heal_order_chases_an_out_of_range_target() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 901, EntityKind::Knight, OWNER_PLAYER, (68, 60));
    wound(&mut sim, 901, 10);
    assert!(sim.can_heal(900, 901));
    sim.push_command(Command::Heal {
        entity: 900,
        target: 901,
    });
    // While the monk is still out of range (8 cells ≈ 32 units
    // away, range is 16), the Knight's HP does not move; the
    // monk closes the distance and the heal completes the job.
    let mut saw_chase = false;
    let mut healed_while_ordered = false;
    for _ in 0..12_000 {
        sim.tick();
        let monk = ent(&sim, 900);
        let knight = ent(&sim, 901);
        if monk.order.is_some() && monk.heal_target == Some(901) {
            saw_chase = true;
        }
        let d = crate::steer::vec_len(knight.pos_x - monk.pos_x, knight.pos_y - monk.pos_y);
        if d > crate::monks::HEAL_RANGE {
            assert_eq!(
                knight.hp,
                I16F16::from_num(10),
                "healing accrued from outside the range band"
            );
        }
        if knight.hp >= I16F16::from_num(100) {
            healed_while_ordered = true;
            break;
        }
    }
    assert!(saw_chase, "the monk never walked toward its target");
    assert!(healed_while_ordered, "the Knight never healed to full");
    assert_eq!(ent(&sim, 900).heal_target, None);
}

#[test]
fn heal_exclusions_hold() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    // A wounded Battering Ram beside the monk: siege is never
    // healed (MNK-10), by order or by auto-task.
    spawn(&mut sim, 901, EntityKind::BatteringRam, OWNER_PLAYER, (61, 60));
    wound(&mut sim, 901, 50);
    assert!(!sim.can_heal(900, 901));
    // A wounded enemy Knight beside the monk: never healed.
    spawn(&mut sim, 902, EntityKind::Knight, OWNER_OPPONENT, (60, 61));
    wound(&mut sim, 902, 10);
    assert!(!sim.can_heal(900, 902));
    run_until(&mut sim, 600, |_| false);
    assert_eq!(ent(&sim, 901).hp, I16F16::from_num(50));
    assert_eq!(ent(&sim, 902).hp, I16F16::from_num(10));
    assert_eq!(ent(&sim, 900).heal_target, None);
    // The Town Center is a building for this rule: wounded, it
    // is repaired, never healed.
    let tc_id = sim.town_center().id;
    assert!(!sim.can_heal(900, tc_id));
    // A monk cannot heal itself.
    wound(&mut sim, 900, 5);
    assert!(!sim.can_heal(900, 900));
    run_until(&mut sim, 600, |_| false);
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(5));
    // A full-HP ally is not a target.
    spawn(&mut sim, 903, EntityKind::Knight, OWNER_PLAYER, (62, 60));
    assert!(!sim.can_heal(900, 903));
}

#[test]
fn heal_auto_task_picks_nearest_then_lowest_id() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 905, EntityKind::Knight, OWNER_PLAYER, (66, 60));
    spawn(&mut sim, 904, EntityKind::Militia, OWNER_PLAYER, (62, 60));
    wound(&mut sim, 905, 50);
    wound(&mut sim, 904, 20);
    sim.tick();
    assert_eq!(
        ent(&sim, 900).heal_target,
        Some(904),
        "the nearer wounded ally wins the auto-task"
    );
    // Equal distances: the lowest id wins (CMB-08's tie-break).
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 907, EntityKind::Militia, OWNER_PLAYER, (62, 60));
    spawn(&mut sim, 906, EntityKind::Militia, OWNER_PLAYER, (58, 60));
    wound(&mut sim, 907, 20);
    wound(&mut sim, 906, 20);
    sim.tick();
    assert_eq!(ent(&sim, 900).heal_target, Some(906));
}

#[test]
fn healing_never_touches_the_targets_orders() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 901, EntityKind::Militia, OWNER_PLAYER, (65, 60));
    wound(&mut sim, 901, 20);
    // The Militia walks THROUGH the monk's heal band on its own
    // order, with a second order queued behind it.
    let (tx, ty) = sim.grid().cell_center(55, 60);
    sim.push_command(Command::Move {
        entity: 901,
        x: tx,
        y: ty,
    });
    let (qx, qy) = sim.grid().cell_center(55, 55);
    sim.push_command(Command::Queue {
        entity: 901,
        order: crate::QueuedOrder::Move { x: qx, y: qy },
    });
    let mut healed_under_orders = false;
    for _ in 0..1200 {
        sim.tick();
        let m = ent(&sim, 901);
        if m.hp > I16F16::from_num(20) && m.order.is_some() && m.order_queue.len() == 1 {
            healed_under_orders = true;
            break;
        }
        if m.order.is_none() {
            break;
        }
    }
    assert!(
        healed_under_orders,
        "the Militia was never healed while its own orders stood"
    );
}

// ------------------------------------------------------------------
// CLS-08: the monk never attacks; it is a normal target.

#[test]
fn monk_never_attacks_but_is_a_normal_target() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 901, EntityKind::Militia, OWNER_OPPONENT, (61, 60));
    // No order exists that makes a monk strike (CLS-08).
    assert!(!sim.can_attack(900, 901));
    sim.push_command(Command::Attack {
        entity: 900,
        target: 901,
    });
    // The enemy Militia, meanwhile, acquires the monk by the
    // ordinary CMB-08 chain and kills it: the monk is a normal
    // target — it just never answers.
    let mut monk_hp_never_dropped_enemy = false;
    let dead = run_until(&mut sim, 7200, |s| {
        if ent(s, 901).hp < I16F16::from_num(40) {
            monk_hp_never_dropped_enemy = true;
        }
        ent(s, 900).dead
    });
    assert!(dead, "the enemy Militia never killed the monk");
    assert!(
        !monk_hp_never_dropped_enemy,
        "the monk damaged its attacker — it must never strike"
    );
    assert_eq!(ent(&sim, 900).attack_target, None);
}

#[test]
fn light_cavalry_bonus_damage_vs_monk() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(
        &mut sim,
        901,
        EntityKind::LightCavalry,
        OWNER_OPPONENT,
        (61, 60),
    );
    sim.push_command(Command::Attack {
        entity: 901,
        target: 900,
    });
    // The Light Cavalry's row is 7 melee + 10 vs MONK: the first
    // landed hit takes exactly 17 off the monk's 30 (armor 0).
    let hit = run_until(&mut sim, 1200, |s| ent(s, 900).hp < I16F16::from_num(30));
    assert!(hit, "the Light Cavalry never landed a hit");
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(13));
}

// ------------------------------------------------------------------
// TEC-10's first two rows: Sanctity and Fervor.

#[test]
fn sanctity_and_fervor_rows_and_flow() {
    // The table rows (dossier §5.6).
    assert_eq!(TechId::Sanctity.tag(), 32);
    assert_eq!(TechId::Fervor.tag(), 33);
    assert_eq!(TechId::Sanctity.host(), TechHost::Building(BuildingKind::Monastery));
    assert_eq!(TechId::Fervor.host(), TechHost::Building(BuildingKind::Monastery));
    assert_eq!(TechId::Sanctity.age_required(), Age::Castle);
    assert_eq!(TechId::Fervor.age_required(), Age::Castle);
    assert_eq!(TechId::Sanctity.cost(), (0, 0, 175, 0));
    assert_eq!(TechId::Fervor.cost(), (0, 0, 140, 0));
    assert_eq!(TechId::Sanctity.ticks(), 3600);
    assert_eq!(TechId::Fervor.ticks(), 3000);
    // The lookup patches (stats_for): +15 max HP, ×1.15 speed.
    let mut sim = sim_in_castle();
    let base = sim.stats_for(OWNER_PLAYER, EntityKind::Monk);
    assert_eq!(base.max_hp, 30);
    assert_eq!(base.speed, I16F16::from_bits(3_058));
    sim.players[0].techs |= TechId::Sanctity.bit();
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Monk).max_hp, 45);
    sim.players[0].techs |= TechId::Fervor.bit();
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Monk).speed,
        I16F16::from_bits(3_516),
        "3058 × 115 / 100, truncated"
    );
    // The research flow at a real Monastery: host law, gold law,
    // and the standing monk's current-HP grant on completion.
    let mut sim = sim_in_castle();
    top_up(&mut sim, OWNER_PLAYER);
    let mon = placed(&sim, 900, BuildingKind::Monastery, OWNER_PLAYER, (30, 30));
    let mon_id = mon.id;
    sim.buildings.push(mon);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (40, 30));
    let rax_id = rax.id;
    sim.buildings.push(rax);
    assert_eq!(
        sim.research_block_at(rax_id, TechId::Sanctity),
        ResearchBlock::Locked
    );
    assert_eq!(
        sim.research_block_at(mon_id, TechId::Sanctity),
        ResearchBlock::Ok
    );
    spawn(&mut sim, 950, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    let gold_before = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    sim.push_command(Command::ResearchAt {
        building: mon_id,
        tech: TechId::Sanctity,
    });
    sim.tick();
    let gold_after = sim
        .stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Gold)
        .to_num::<i32>();
    assert_eq!(gold_before - gold_after, 175);
    let done = run_until(&mut sim, 4200, |s| s.has_tech(OWNER_PLAYER, TechId::Sanctity));
    assert!(done, "Sanctity never completed");
    assert_eq!(
        ent(&sim, 950).hp,
        I16F16::from_num(45),
        "the standing monk gains the +15 as current HP (Loom's grant discipline)"
    );
}

// ------------------------------------------------------------------
// Save/restore with a heal in flight (NET-09's trailer must
// verify: the monk fields ride the codec like every other).

#[test]
fn save_round_trip_mid_heal() {
    let mut sim = sim_in_castle();
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (60, 60));
    spawn(&mut sim, 901, EntityKind::Knight, OWNER_PLAYER, (61, 60));
    wound(&mut sim, 901, 20);
    run_until(&mut sim, 137, |_| false);
    assert_eq!(ent(&sim, 900).heal_target, Some(901));
    let bytes = sim.save_game();
    let hash = sim.state_hash();
    let mut restored = Sim::load_game(&bytes).expect("the save must load");
    assert_eq!(restored.state_hash(), hash);
    assert_eq!(ent(&restored, 900).heal_target, Some(901));
    // And the heal resumes on the restored sim.
    let full = run_until(&mut restored, 3600, |s| {
        ent(s, 901).hp >= I16F16::from_num(100)
    });
    assert!(full, "the restored heal never completed");
}

// ------------------------------------------------------------------
// The AI slice: build the Monastery, field monks, and the
// monks heal the AI's own wounded through the auto-task.

#[test]
fn ai_builds_monastery_fields_monks_and_they_heal() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    sim.players[1].age = Age::Castle;
    top_up(&mut sim, OWNER_OPPONENT);
    let monastery = run_until(&mut sim, 400_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Monastery && b.complete)
    });
    assert!(monastery, "the Castle-Age AI never raised a Monastery");
    let monk = run_until(&mut sim, 200_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Monk && e.owner == OWNER_OPPONENT && !e.dead)
    });
    assert!(monk, "the AI never fielded a Monk");
    // Wound the AI's villagers: its monk's auto-task (the same
    // MNK-10 path a human's monk uses) must raise one of them.
    let vils: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::Villager && !e.dead)
        .map(|e| e.id)
        .collect();
    assert!(!vils.is_empty());
    for &id in &vils {
        wound(&mut sim, id, 5);
    }
    let healed = run_until(&mut sim, 12_000, |s| {
        vils.iter().any(|&id| ent(s, id).hp > I16F16::from_num(5))
    });
    assert!(healed, "the AI's monk never healed a wounded villager");
}

