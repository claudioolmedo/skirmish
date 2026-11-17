//! BLD-18 completion (2026-11-16) — the Watch Tower line
//! completed against the documented DE values (sk-08 research,
//! approved by Claudio): the Guard Tower / Keep upgrade kinds
//! (tags 25/26) with their University techs (tags 54/55) and
//! mass conversion on completion, the Outpost corrections
//! (LoS 6/8/10/12 by Age, garrison capacity 0, pierce armor 0),
//! Arrowslits (tag 56, +1/+2/+3 by tier), and the garrison
//! arrows' secondary attack table (+5 vs Stone Defense).
//!
//! Fixture style follows `university_p3c_tests`: buildings and
//! units are placed directly; tech bits are granted or applied
//! through `apply_research` where the sweep itself is the
//! subject, and researched through the real command path where
//! the flow is.

use crate::build::{Building, BuildingKind};
use crate::classes;
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, ResearchBlock, TechId, TechHost};
use crate::{player_slot, Command, Sim};
use fixed::types::I16F16;

fn sim_in_castle() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Castle;
    sim.players[1].age = Age::Castle;
    sim
}

fn sim_in_imperial() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Imperial;
    sim.players[1].age = Age::Imperial;
    sim
}

fn placed(sim: &mut Sim, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> u32 {
    let id = 900 + sim.buildings.len() as u32;
    let size = kind.footprint_size();
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
    });
    id
}

fn spawn(sim: &mut Sim, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) -> u32 {
    let id = 7000 + sim.entities.len() as u32;
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

fn wu(n: i32) -> I16F16 {
    I16F16::from_num(n)
}

fn run(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
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

fn grant(sim: &mut Sim, owner: u8, tech: TechId) {
    let slot = player_slot(owner).expect("owner slot");
    sim.players[slot].techs |= tech.bit();
}

fn kind_of(sim: &Sim, id: u32) -> BuildingKind {
    sim.buildings
        .iter()
        .find(|b| b.id == id)
        .expect("building stands")
        .kind
}

fn building_hp(sim: &Sim, id: u32) -> i32 {
    sim.buildings
        .iter()
        .find(|b| b.id == id)
        .expect("building stands")
        .hp
        .to_num::<i32>()
}

fn building_pos(sim: &Sim, id: u32) -> (I16F16, I16F16) {
    let b = sim
        .buildings
        .iter()
        .find(|b| b.id == id)
        .expect("building stands");
    (b.pos_x, b.pos_y)
}

fn top_up(sim: &mut Sim) {
    for k in [
        ResourceKind::Food,
        ResourceKind::Wood,
        ResourceKind::Gold,
        ResourceKind::Stone,
    ] {
        sim.stockpiles[0].add(k, I16F16::from_num(5000));
        sim.stockpiles[1].add(k, I16F16::from_num(5000));
    }
}

// ----------------------------------------------------------------
// The data rows.

#[test]
fn tower_line_rows_match_the_documented_values() {
    // Guard Tower (wiki Building comparative table, sk-08).
    let g = BuildingKind::GuardTower;
    assert_eq!(g.tag(), 25);
    assert_eq!(BuildingKind::from_tag(25), Some(g));
    assert_eq!(g.wood_cost().to_num::<i32>(), 35);
    assert_eq!(g.stone_cost().to_num::<i32>(), 125);
    assert_eq!(g.build_ticks(), 4800, "80 s at 60 Hz");
    assert_eq!(g.footprint_size(), 1);
    assert_eq!(g.max_hp_at(Age::Castle), 1500);
    assert_eq!(g.max_hp_at(Age::Imperial), 1500, "flat row — the kind is the tier");
    assert_eq!(g.armor_melee_at(Age::Castle), 2);
    assert_eq!(g.armor_pierce_at(Age::Castle), 8);
    assert_eq!(g.sight_tiles_at(Age::Castle), 10);
    assert_eq!(g.age_required(), Age::Castle);
    assert!(g.is_tower());
    let w = g.weapon().expect("armed");
    assert_eq!(
        w.attacks,
        &[
            (classes::PIERCE, 7),
            (classes::SHIP, 7),
            (classes::SPEARMAN, 2)
        ],
        "Guard: 7 pierce, +7 vs Ship (current DE), +2 vs Spearman"
    );
    assert_eq!(w.reload_ticks, 120);
    assert_eq!(w.volley, 1);

    // Keep (same sources).
    let k = BuildingKind::Keep;
    assert_eq!(k.tag(), 26);
    assert_eq!(BuildingKind::from_tag(26), Some(k));
    assert_eq!(k.wood_cost().to_num::<i32>(), 35);
    assert_eq!(k.stone_cost().to_num::<i32>(), 125);
    assert_eq!(k.build_ticks(), 4800);
    assert_eq!(k.footprint_size(), 1);
    assert_eq!(k.max_hp_at(Age::Imperial), 2250);
    assert_eq!(k.armor_melee_at(Age::Imperial), 3);
    assert_eq!(k.armor_pierce_at(Age::Imperial), 9);
    assert_eq!(k.sight_tiles_at(Age::Imperial), 10);
    assert_eq!(k.age_required(), Age::Imperial);
    assert!(k.is_tower());
    let w = k.weapon().expect("armed");
    assert_eq!(
        w.attacks,
        &[
            (classes::PIERCE, 8),
            (classes::SHIP, 10),
            (classes::SPEARMAN, 2)
        ],
        "Keep: 8 pierce, +10 vs Ship, +2 vs Spearman"
    );

    // The Watch Tower's own row is untouched.
    let t = BuildingKind::WatchTower;
    assert!(t.is_tower());
    assert_eq!(t.max_hp_at(Age::Feudal), 850);
    assert_eq!(t.max_hp_at(Age::Castle), 1020);
    assert_eq!(t.armor_melee_at(Age::Feudal), 1);
    assert_eq!(t.armor_pierce_at(Age::Feudal), 7);
    assert_eq!(t.sight_tiles_at(Age::Feudal), 10);
}

#[test]
fn outpost_rows_match_the_documented_values() {
    let o = BuildingKind::Outpost;
    // LoS 6/8/10/12 by Age (comparative table; Tower article).
    assert_eq!(o.sight_tiles_at(Age::Dark), 6);
    assert_eq!(o.sight_tiles_at(Age::Feudal), 8);
    assert_eq!(o.sight_tiles_at(Age::Castle), 10);
    assert_eq!(o.sight_tiles_at(Age::Imperial), 12);
    // Armor 0/0 — the pierce row is explicit, not the generic
    // fall-through 7 it read before this landing.
    assert_eq!(o.armor_melee_at(Age::Dark), 0);
    assert_eq!(o.armor_pierce_at(Age::Dark), 0);
    assert_eq!(o.armor_pierce_at(Age::Imperial), 0);
    assert_eq!(o.max_hp_at(Age::Dark), 500);
    // Garrison capacity 0 (wiki, Outpost — "Outposts cannot
    // garrison units"): no capacity, not a defensive garrison,
    // admits nobody, heals nobody — and so it is out of the
    // Town Bell's shelter scan, which reads the same column.
    assert_eq!(o.garrison_capacity(), None);
    assert!(!o.is_defensive_garrison());
    assert!(!o.admits_defensive(EntityKind::Villager));
    assert_eq!(o.garrison_heal_hp_per_min(), 0);
    assert!(!o.is_tower());
    assert!(o.weapon().is_none());
}

#[test]
fn tower_tech_rows_match_the_documented_values() {
    // Guard Tower: University, Castle, 100 F / 250 W, 30 s.
    let gt = TechId::GuardTower;
    assert_eq!(gt.tag(), 54); // renumbered +1 at integration (Siege Engineers holds 53)
    assert_eq!(gt.host(), TechHost::Building(BuildingKind::University));
    assert_eq!(gt.age_required(), Age::Castle);
    assert_eq!(gt.requires_tech(), None);
    assert_eq!(gt.cost(), (100, 250, 0, 0));
    assert_eq!(gt.ticks(), 1800);
    // Keep: University, Imperial, 500 F / 350 W, 75 s, chains
    // from Guard Tower.
    let kp = TechId::Keep;
    assert_eq!(kp.tag(), 55);
    assert_eq!(kp.host(), TechHost::Building(BuildingKind::University));
    assert_eq!(kp.age_required(), Age::Imperial);
    assert_eq!(kp.requires_tech(), Some(TechId::GuardTower));
    assert_eq!(kp.cost(), (500, 350, 0, 0));
    assert_eq!(kp.ticks(), 4500);
    // Arrowslits: University, Imperial, 250 F / 250 W, 25 s.
    let ar = TechId::Arrowslits;
    assert_eq!(ar.tag(), 56);
    assert_eq!(ar.host(), TechHost::Building(BuildingKind::University));
    assert_eq!(ar.age_required(), Age::Imperial);
    assert_eq!(ar.requires_tech(), None);
    assert_eq!(ar.cost(), (250, 250, 0, 0));
    assert_eq!(ar.ticks(), 1500);
}

// ----------------------------------------------------------------
// Research gates and the real flow.

#[test]
fn guard_tower_research_gates_and_flow() {
    let mut sim = sim_in_castle();
    top_up(&mut sim);
    let uni = placed(&mut sim, BuildingKind::University, OWNER_PLAYER, (20, 20));
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (30, 20));
    // Host law: the trio lives at the University only.
    for t in [TechId::GuardTower, TechId::Keep, TechId::Arrowslits] {
        assert_eq!(
            sim.research_block_at(rax, t),
            ResearchBlock::Locked,
            "foreign host refuses {:?}",
            t
        );
    }
    // Age law: Guard Tower opens in Castle; Keep and Arrowslits
    // stay Locked until Imperial.
    assert_eq!(sim.research_block_at(uni, TechId::GuardTower), ResearchBlock::Ok);
    assert_eq!(sim.research_block_at(uni, TechId::Keep), ResearchBlock::Locked);
    assert_eq!(sim.research_block_at(uni, TechId::Arrowslits), ResearchBlock::Locked);
    sim.players[0].age = Age::Feudal;
    assert_eq!(sim.research_block_at(uni, TechId::GuardTower), ResearchBlock::Locked);
    sim.players[0].age = Age::Castle;
    // Prerequisite law: in Imperial without Guard Tower, Keep
    // is Locked; with it, open.
    sim.players[0].age = Age::Imperial;
    assert_eq!(sim.research_block_at(uni, TechId::Keep), ResearchBlock::Locked);
    grant(&mut sim, OWNER_PLAYER, TechId::GuardTower);
    assert_eq!(sim.research_block_at(uni, TechId::Keep), ResearchBlock::Ok);
    sim.players[0].techs = 0;
    sim.players[0].age = Age::Castle;

    // The real flow: enqueue Guard Tower, pay exactly
    // 100 F / 250 W, and the bit lands after 1800 ticks.
    let amt = |sim: &Sim, k: ResourceKind| {
        sim.stockpile_of(OWNER_PLAYER).amount(k).to_num::<i32>()
    };
    let (f0, w0) = (amt(&sim, ResourceKind::Food), amt(&sim, ResourceKind::Wood));
    let tower = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
    sim.push_command(Command::ResearchAt {
        building: uni,
        tech: TechId::GuardTower,
    });
    run(&mut sim, 1);
    assert_eq!(amt(&sim, ResourceKind::Food), f0 - 100);
    assert_eq!(amt(&sim, ResourceKind::Wood), w0 - 250);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::GuardTower));
    run(&mut sim, 1700);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::GuardTower));
    run(&mut sim, 200);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::GuardTower));
    // …and the standing Watch Tower converted on completion.
    assert_eq!(kind_of(&sim, tower), BuildingKind::GuardTower);
    assert_eq!(
        sim.research_block_at(uni, TechId::GuardTower),
        ResearchBlock::AlreadyDone
    );
}

// ----------------------------------------------------------------
// The conversion sweeps.

#[test]
fn guard_tower_converts_standing_towers_keeping_hp_and_garrison() {
    let mut sim = sim_in_castle();
    let t1 = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
    let t2 = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (52, 40));
    let foe = placed(&mut sim, BuildingKind::WatchTower, OWNER_OPPONENT, (64, 40));
    // Damage the second tower: its current HP must survive the
    // conversion exactly (the maximum rises; no heal).
    let hp_full = building_hp(&sim, t1);
    {
        let bi = sim.building_index(t2).expect("tower stands");
        sim.buildings[bi].hp = wu(400);
    }
    // Garrison a villager in the first tower through the real
    // command path — the garrison rides the conversion.
    let (tx, ty) = building_pos(&sim, t1);
    let vil = spawn(&mut sim, EntityKind::Villager, OWNER_PLAYER, tx + wu(6), ty + wu(6));
    sim.push_command(Command::GarrisonBuilding {
        entity: vil,
        building: t1,
    });
    assert!(
        run_until(&mut sim, 20_000, |s| s.garrison_count_building(t1) >= 1),
        "the villager never garrisoned"
    );
    sim.apply_research(OWNER_PLAYER, TechId::GuardTower);
    assert_eq!(kind_of(&sim, t1), BuildingKind::GuardTower);
    assert_eq!(kind_of(&sim, t2), BuildingKind::GuardTower);
    assert_eq!(kind_of(&sim, foe), BuildingKind::WatchTower, "the foe's tower is not ours to upgrade");
    assert_eq!(building_hp(&sim, t1), hp_full, "undamaged tower: HP kept");
    assert_eq!(building_hp(&sim, t2), 400, "damaged tower: HP kept, no heal");
    assert_eq!(sim.building_max_hp(BuildingKind::GuardTower, OWNER_PLAYER), 1500);
    assert_eq!(sim.garrison_count_building(t1), 1, "the garrison rides the conversion");
    // The arrows recompute on the Guard weapon's own DPS:
    // 1 villager (2.5 DPS) against the Guard row (7 pierce /
    // 2 s = 3.5 DPS) adds nothing yet.
    assert_eq!(sim.garrison_arrows_building(t1), 1);
}

#[test]
fn keep_converts_only_guard_towers() {
    let mut sim = sim_in_imperial();
    let watch = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
    let guard = placed(&mut sim, BuildingKind::GuardTower, OWNER_PLAYER, (52, 40));
    sim.apply_research(OWNER_PLAYER, TechId::Keep);
    assert_eq!(kind_of(&sim, watch), BuildingKind::WatchTower, "Keep skips the Watch tier");
    assert_eq!(kind_of(&sim, guard), BuildingKind::Keep);
    assert_eq!(sim.building_max_hp(BuildingKind::Keep, OWNER_PLAYER), 2250);
}

#[test]
fn tower_placed_after_research_is_born_at_the_tier() {
    let mut sim = sim_in_castle();
    top_up(&mut sim);
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .map(|e| e.id)
        .expect("a starting villager");
    let idx = sim.entity_index(vil).expect("villager stands");
    grant(&mut sim, OWNER_PLAYER, TechId::GuardTower);
    let b1 = sim
        .place_building(idx, BuildingKind::WatchTower, 60, 60)
        .expect("placement spends and registers");
    assert_eq!(kind_of(&sim, b1), BuildingKind::GuardTower);
    grant(&mut sim, OWNER_PLAYER, TechId::Keep);
    let b2 = sim
        .place_building(idx, BuildingKind::WatchTower, 64, 60)
        .expect("placement spends and registers");
    assert_eq!(kind_of(&sim, b2), BuildingKind::Keep);
}

// ----------------------------------------------------------------
// Combat: the tiers fire their own rows; patches layer.

fn tower_duel(kind: BuildingKind, age: Age) -> i32 {
    let mut sim = Sim::new(7);
    sim.players[0].age = age;
    sim.players[1].age = age;
    let tw = placed(&mut sim, kind, OWNER_PLAYER, (40, 40));
    let (bx, by) = building_pos(&sim, tw);
    // Seven tiles east of the tower's edge: inside the 1–8
    // band, far enough that the militia's approach cannot
    // close into the dead zone inside the window.
    let m = spawn(
        &mut sim,
        EntityKind::Militia,
        OWNER_OPPONENT,
        bx + wu(32),
        by,
    );
    run(&mut sim, 300);
    let mi = sim.entity_index(m).expect("militia placed");
    sim.entities[mi].hp.to_num::<i32>()
}

#[test]
fn tower_tiers_deal_their_own_damage() {
    // Militia: 40 HP, 1 pierce armor. Per hit: Watch 5−1=4,
    // Guard 7−1=6, Keep 8−1=7 — the same window must order
    // the three rows strictly.
    let watch = tower_duel(BuildingKind::WatchTower, Age::Castle);
    let guard = tower_duel(BuildingKind::GuardTower, Age::Castle);
    let keep = tower_duel(BuildingKind::Keep, Age::Imperial);
    assert!(watch < 40, "the Watch Tower fires: {watch}");
    assert!(guard < watch, "Guard out-damages Watch: {guard} < {watch}");
    assert!(keep < guard, "Keep out-damages Guard: {keep} < {guard}");
}

#[test]
fn arrowslits_and_fletching_layer_per_tier() {
    let mut sim = sim_in_imperial();
    let pierce = |sim: &Sim, kind: BuildingKind| {
        sim.building_weapon(OWNER_PLAYER, kind)
            .expect("armed")
            .attacks
            .iter()
            .find(|(c, _)| *c == classes::PIERCE)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    };
    // Base rows.
    assert_eq!(pierce(&sim, BuildingKind::WatchTower), 5);
    assert_eq!(pierce(&sim, BuildingKind::GuardTower), 7);
    assert_eq!(pierce(&sim, BuildingKind::Keep), 8);
    // Arrowslits: +1/+2/+3 by tier.
    grant(&mut sim, OWNER_PLAYER, TechId::Arrowslits);
    assert_eq!(pierce(&sim, BuildingKind::WatchTower), 6);
    assert_eq!(pierce(&sim, BuildingKind::GuardTower), 9);
    assert_eq!(pierce(&sim, BuildingKind::Keep), 11);
    // Fletching composes additively on top, line-wide, and
    // still brings its +1 range to every tier.
    grant(&mut sim, OWNER_PLAYER, TechId::Fletching);
    assert_eq!(pierce(&sim, BuildingKind::WatchTower), 7);
    assert_eq!(pierce(&sim, BuildingKind::GuardTower), 10);
    assert_eq!(pierce(&sim, BuildingKind::Keep), 12);
    let w = sim.building_weapon(OWNER_PLAYER, BuildingKind::GuardTower).expect("armed");
    assert_eq!(w.range_max, wu(36), "Fletching: 8 + 1 tiles on the Guard tier too");
    // The anti-ship / anti-spearman pairs never move.
    assert_eq!(
        w.attacks,
        &[
            (classes::PIERCE, 10),
            (classes::SHIP, 7),
            (classes::SPEARMAN, 2)
        ]
    );
}

#[test]
fn murder_holes_guard_tower_shoots_a_hugging_attacker() {
    let scenario = |murder_holes: bool| -> i32 {
        let mut sim = sim_in_castle();
        if murder_holes {
            grant(&mut sim, OWNER_PLAYER, TechId::MurderHoles);
        }
        let tw = placed(&mut sim, BuildingKind::GuardTower, OWNER_PLAYER, (40, 40));
        let (bx, by) = building_pos(&sim, tw);
        let v = spawn(
            &mut sim,
            EntityKind::Militia,
            OWNER_OPPONENT,
            bx,
            by + wu(4),
        );
        run(&mut sim, 300);
        let vi = sim.entity_index(v).expect("militia placed");
        sim.entities[vi].hp.to_num::<i32>()
    };
    assert_eq!(scenario(false), 40, "inside the band: never shot");
    assert!(scenario(true) < 40, "Murder Holes generalizes to the Guard tier");
}

// ----------------------------------------------------------------
// Garrison: the Outpost refuses; the tiers keep the caps.

#[test]
fn outpost_refuses_garrison_at_the_command_gate() {
    let mut sim = Sim::new(7);
    let outpost = placed(&mut sim, BuildingKind::Outpost, OWNER_PLAYER, (58, 58));
    let tower = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (70, 58));
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .map(|e| e.id)
        .expect("a starting villager");
    assert!(!sim.can_garrison_building(vil, outpost), "the Outpost shelters nobody");
    assert!(sim.can_garrison_building(vil, tower), "the tower still shelters foot units");
}

#[test]
fn garrison_arrow_caps_follow_the_tier_dps() {
    // Five villagers inside (the capacity): contributed
    // 5 × 2.5 = 12.5 DPS. Extra arrows = floor(12.5 / own
    // DPS): Watch (2.5/s) → +4 capped = 5 total; Guard
    // (3.5/s) → +3 = 4 total; Keep (4/s) → +3 = 4 total.
    let scenario = |kind: BuildingKind| -> (u32, u32) {
        let mut sim = sim_in_castle();
        let t = placed(&mut sim, kind, OWNER_PLAYER, (58, 58));
        let (tx, ty) = building_pos(&sim, t);
        for i in 0..6 {
            let v = spawn(
                &mut sim,
                EntityKind::Villager,
                OWNER_PLAYER,
                tx + wu(6 + i * 2),
                ty + wu(6),
            );
            sim.push_command(Command::GarrisonBuilding {
                entity: v,
                building: t,
            });
        }
        assert!(
            run_until(&mut sim, 40_000, |s| s.garrison_count_building(t) >= 5),
            "the tower filled"
        );
        run(&mut sim, 600);
        (sim.garrison_count_building(t), sim.garrison_arrows_building(t))
    };
    assert_eq!(scenario(BuildingKind::WatchTower), (5, 5), "capacity 5, arrows capped at 5");
    assert_eq!(scenario(BuildingKind::GuardTower), (5, 4), "the Guard DPS divisor bites");
    assert_eq!(scenario(BuildingKind::Keep), (5, 4), "the Keep DPS divisor bites");
}

// ----------------------------------------------------------------
// G7: garrison arrows fly the secondary table.

#[test]
fn garrison_arrows_carry_the_stone_defense_bonus() {
    let mut sim = sim_in_castle();
    let t = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
    let (tx, ty) = building_pos(&sim, t);
    // Fill the tower: 5 villagers → 5 arrows per volley.
    for i in 0..5 {
        let v = spawn(
            &mut sim,
            EntityKind::Villager,
            OWNER_PLAYER,
            tx + wu(6 + i * 2),
            ty + wu(6),
        );
        sim.push_command(Command::GarrisonBuilding {
            entity: v,
            building: t,
        });
    }
    assert!(run_until(&mut sim, 40_000, |s| s.garrison_count_building(t) >= 5));
    assert_eq!(sim.garrison_arrows_building(t), 5);
    // An enemy Stone Wall stands in the band — the tower's
    // only target (its own garrison is contained, never
    // targeted).
    let wall = placed(&mut sim, BuildingKind::StoneWall, OWNER_OPPONENT, (48, 40));
    let _ = wall;
    // Wait for the first volley to be in flight, then read the
    // projectile tables: the tower's own arrow flies the main
    // table; the four garrison arrows fly the secondary table
    // (the main table + the +5 vs Stone Defense pair).
    assert!(
        run_until(&mut sim, 600, |s| {
            s.projectiles.iter().any(|p| p.attacker_building == Some(t))
        }),
        "the tower never loosed"
    );
    let shots: Vec<_> = sim
        .projectiles
        .iter()
        .filter(|p| p.attacker_building == Some(t))
        .collect();
    assert_eq!(shots.len(), 5, "one volley of five");
    let with_bonus = shots
        .iter()
        .filter(|p| p.attacks.contains(&(classes::STONE_DEFENSE, 5)))
        .count();
    let without = shots.len() - with_bonus;
    assert_eq!(without, 1, "exactly the tower's own arrow flies the main table");
    assert_eq!(with_bonus, 4, "every garrison arrow carries +5 vs Stone Defense");
    // And the membership half: the wall's spec carries the
    // class, a palisade segment does not.
    let wall_spec = crate::combat::building_spec(BuildingKind::StoneWall, Age::Castle);
    assert!(wall_spec.classes & classes::bit(classes::STONE_DEFENSE) != 0);
    let pal_spec = crate::combat::building_spec(BuildingKind::PalisadeWall, Age::Castle);
    assert!(pal_spec.classes & classes::bit(classes::STONE_DEFENSE) == 0);
}

// ----------------------------------------------------------------
// Save round-trip over the three tiers.

#[test]
fn save_round_trip_keeps_the_three_tiers() {
    let mut sim = sim_in_imperial();
    let a = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
    let b = placed(&mut sim, BuildingKind::GuardTower, OWNER_PLAYER, (52, 40));
    let c = placed(&mut sim, BuildingKind::Keep, OWNER_PLAYER, (64, 40));
    grant(&mut sim, OWNER_PLAYER, TechId::GuardTower);
    grant(&mut sim, OWNER_PLAYER, TechId::Keep);
    grant(&mut sim, OWNER_PLAYER, TechId::Arrowslits);
    run(&mut sim, 120);
    let bytes = sim.save_game();
    let loaded = Sim::load_game(&bytes).expect("the save restores");
    assert_eq!(kind_of(&loaded, a), BuildingKind::WatchTower);
    assert_eq!(kind_of(&loaded, b), BuildingKind::GuardTower);
    assert_eq!(kind_of(&loaded, c), BuildingKind::Keep);
    assert!(loaded.has_tech(OWNER_PLAYER, TechId::Arrowslits));
    assert_eq!(loaded.state_hash(), sim.state_hash(), "the restored state is the same state");
}
