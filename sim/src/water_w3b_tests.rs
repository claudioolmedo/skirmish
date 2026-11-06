//! Water W3b tests (CLS-07 → MATCHES): the Castle Age ship
//! set — Medium Warships (TechId 47) converts the four Feudal
//! lines in place to War Galley (28), Fire Ship (29),
//! Demolition Ship (30) and War Hulk (31), later production
//! spawns at the Castle tier (the CLS-03 pattern), the tier
//! patch rows ride the same Blacksmith swaps, and the
//! Demolition Ship's blast is the second profile on the shared
//! helper: radius 3.0 tiles, **untapered** — 100% of its
//! 80-melee table across the whole radius, enemies only.

use crate::build::{Building, BuildingKind};
use crate::classes as cls;
use crate::econ::{NodeKind, ResourceKind};
use crate::entity::{Entity, EntityKind, Stance, OWNER_OPPONENT, OWNER_PLAYER};
use crate::grid::Domain;
use crate::tech::{Age, TechId};
use crate::{Command, Sim};
use fixed::types::I16F16;

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn dock_anchor(sim: &Sim) -> Option<(u32, u32)> {
    let grid = sim.grid();
    for ay in 0..=117u32 {
        for ax in 0..=117u32 {
            if grid.dock_placement_ok(ax, ay, 3) {
                return Some((ax, ay));
            }
        }
    }
    None
}

/// A completed Dock placed directly on the pond shore (the
/// W3a tests' `placed_dock` pattern, duplicated so this module
/// stands alone).
fn placed_dock(sim: &mut Sim, id: u32) -> u32 {
    let (ax, ay) = dock_anchor(sim).expect("seed pond offers a Dock site");
    let kind = BuildingKind::Dock;
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(ax, ay);
    let half = I16F16::from_num(size as i32 * 2);
    for dy in 0..size {
        for dx in 0..size {
            sim.grid.set_dynamic(ax + dx, ay + dy, true);
        }
    }
    sim.buildings.push(Building {
        id,
        kind,
        owner: OWNER_PLAYER,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: ax,
        cell_y: ay,
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

/// A point on the seed-42 pond: the first deep-fish node's
/// position (deep water by construction).
fn pond_point(sim: &Sim) -> (I16F16, I16F16) {
    let n = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::DeepFish)
        .expect("seed 42 stocks deep fish");
    (n.pos_x, n.pos_y)
}

/// A sim whose pond is wide enough for the blast test's
/// geometry: seed 42's pond is only ~3 cells across — a ship
/// spawned at the "outside" offset lands on the shore and the
/// engine relocates it into the pond, inside the radius (this
/// test's first stagings). Scan seeds for a deep-fish point
/// whose whole staging cross (the detonation point, the
/// killer at +6, the edge victim at +11, the outside victim
/// at (13, 4) and the owner's ship at −8) stands on open
/// water, and stage there.
fn wide_pond_sim() -> (Sim, (I16F16, I16F16)) {
    for seed in 1..=400u64 {
        let sim = Sim::new(seed);
        for n in sim.nodes().iter().filter(|n| n.kind == NodeKind::DeepFish) {
            let (x, y) = (n.pos_x, n.pos_y);
            let ok = [(0, 0), (6, 0), (11, 0), (13, 4), (-8, 0)].iter().all(|&(dx, dy)| {
                let (cx, cy) = sim
                    .grid()
                    .world_to_cell(x + I16F16::from_num(dx), y + I16F16::from_num(dy));
                sim.grid().is_water_side(cx, cy)
                    && !sim.grid().is_blocked_for(Domain::Water, cx, cy)
            });
            if ok {
                return (sim, (x, y));
            }
        }
    }
    panic!("no seed in 1..=400 offers a wide pond");
}

fn spawn_at(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) -> u32 {
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

fn hp_of(sim: &Sim, id: u32) -> I16F16 {
    sim.entities().iter().find(|e| e.id == id).expect("entity").hp
}

fn dead(sim: &Sim, id: u32) -> bool {
    sim.entities().iter().find(|e| e.id == id).expect("entity").dead
}

fn kind_of(sim: &Sim, id: u32) -> EntityKind {
    sim.entities().iter().find(|e| e.id == id).expect("entity").kind
}

fn set_age(sim: &mut Sim, owner: u8, age: Age) {
    let slot = crate::player_slot(owner).expect("player slot");
    sim.players[slot].age = age;
}

fn grant(sim: &mut Sim, owner: u8, tech: TechId) {
    let slot = crate::player_slot(owner).expect("player slot");
    sim.players[slot].techs |= tech.bit();
}

#[test]
fn castle_ship_rows_are_the_spec_rows() {
    // Tags append after the Feudal set (plan §2 allocation).
    assert_eq!(EntityKind::WarGalley.tag(), 28);
    assert_eq!(EntityKind::FireShip.tag(), 29);
    assert_eq!(EntityKind::DemolitionShip.tag(), 30);
    assert_eq!(EntityKind::WarHulk.tag(), 31);
    for (tag, kind) in [
        (28, EntityKind::WarGalley),
        (29, EntityKind::FireShip),
        (30, EntityKind::DemolitionShip),
        (31, EntityKind::WarHulk),
    ] {
        assert_eq!(EntityKind::from_tag(tag), Some(kind));
        assert_eq!(kind.domain(), Domain::Water);
        assert_eq!(kind.train_age(), Age::Castle);
        assert!(kind.counts_population());
        assert!(kind.is_military());
        // Upgrade-only kinds (the Man-at-Arms pattern): never
        // trained directly, at the Dock or anywhere else.
        assert!(!kind.is_trainable());
        assert!(!BuildingKind::Dock.trains(kind));
    }
    // The tech row (W3b Passo 0 pin).
    assert_eq!(TechId::MediumWarships.tag(), 47);
    assert_eq!(TechId::ALL.len(), 48);
    assert_eq!(TechId::MediumWarships.age_required(), Age::Castle);
    assert_eq!(TechId::MediumWarships.ticks(), 3000); // 50 s
    assert_eq!(TechId::MediumWarships.cost(), (0, 150, 100, 0));

    let g = crate::units::spec(EntityKind::WarGalley);
    assert_eq!(g.max_hp, 125);
    assert_eq!((g.armor_melee, g.armor_pierce), (0, 4));
    assert_eq!(
        g.attacks,
        &[
            (cls::PIERCE, 7),
            (cls::LONG_RANGE_WARSHIP, 5),
            (cls::STANDARD_BUILDINGS, 6),
            (cls::RAM, 3),
        ]
    );
    assert_eq!(g.classes, cls::mask(&[cls::SHIP, cls::LONG_RANGE_WARSHIP]));
    assert_eq!(g.range_max, I16F16::from_num(24)); // 6 tiles
    assert_eq!(g.reload_ticks, 180); // 3.0 s
    assert_eq!((g.wood_cost, g.gold_cost), (90, 30));
    assert_eq!(g.train_ticks, 1620); // 27 s
    assert_eq!(g.los, I16F16::from_num(32)); // 8 tiles

    let f = crate::units::spec(EntityKind::FireShip);
    assert_eq!(f.max_hp, 120);
    assert_eq!((f.armor_melee, f.armor_pierce), (2, 4));
    assert_eq!(f.attacks, &[(cls::MELEE, 3), (cls::LONG_RANGE_WARSHIP, 2)]);
    assert_eq!(f.classes, cls::mask(&[cls::SHIP, cls::FIRE_SHIP]));
    assert_eq!(f.range_max, I16F16::from_num(10)); // 2.5 tiles
    assert_eq!(f.reload_ticks, 15); // 0.25 s
    assert_eq!((f.wood_cost, f.gold_cost), (75, 45));
    assert_eq!(f.train_ticks, 1620); // 27 s

    let d = crate::units::spec(EntityKind::DemolitionShip);
    assert_eq!(d.max_hp, 50);
    assert_eq!((d.armor_melee, d.armor_pierce), (1, 0));
    assert_eq!(d.attacks, &[(cls::MELEE, 80)]);
    assert_eq!(d.classes, cls::mask(&[cls::SHIP]));
    assert_eq!(
        crate::units::DEMOLITION_SHIP_BLAST_RADIUS,
        I16F16::from_num(12) // 3.0 tiles
    );
    assert_eq!((d.wood_cost, d.gold_cost), (70, 80));
    assert_eq!(d.train_ticks, 1860); // 31 s

    let h = crate::units::spec(EntityKind::WarHulk);
    assert_eq!(h.max_hp, 110);
    assert_eq!((h.armor_melee, h.armor_pierce), (4, 1));
    assert_eq!(h.attacks, &[(cls::MELEE, 4), (cls::FIRE_SHIP, 1)]);
    assert_eq!(h.classes, cls::mask(&[cls::SHIP]));
    assert_eq!(h.reload_ticks, 90); // 1.5 s
    assert_eq!((h.wood_cost, h.gold_cost), (75, 35));
    assert_eq!(h.train_ticks, 1620); // 27 s
}

#[test]
fn medium_warships_researches_at_the_dock_and_converts_standing_ships() {
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    // The Castle gate: not researchable in Feudal.
    set_age(&mut sim, OWNER_PLAYER, Age::Feudal);
    assert!(!sim.can_research_at(dock, TechId::MediumWarships));
    set_age(&mut sim, OWNER_PLAYER, Age::Castle);
    assert!(sim.can_research_at(dock, TechId::MediumWarships));

    // One standing ship per Feudal line for the player, plus an
    // enemy Galley that must NOT convert.
    let (x, y) = pond_point(&sim);
    let galley = spawn_at(&mut sim, 950, EntityKind::Galley, OWNER_PLAYER, x, y);
    let fire = spawn_at(
        &mut sim,
        951,
        EntityKind::FireGalley,
        OWNER_PLAYER,
        x + I16F16::from_num(6),
        y,
    );
    let raft = spawn_at(
        &mut sim,
        952,
        EntityKind::DemolitionRaft,
        OWNER_PLAYER,
        x,
        y + I16F16::from_num(6),
    );
    let hulk = spawn_at(
        &mut sim,
        953,
        EntityKind::Hulk,
        OWNER_PLAYER,
        x + I16F16::from_num(6),
        y + I16F16::from_num(6),
    );
    let foe = spawn_at(
        &mut sim,
        954,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(12),
        y,
    );
    // A battle-worn Galley keeps its current HP across the
    // conversion (the CLS-03 precedent: the sweep rewrites the
    // kind, never the HP).
    let e = sim
        .entities
        .iter_mut()
        .find(|e| e.id == galley)
        .expect("the galley");
    e.hp = I16F16::from_num(55);

    // Research for real: 150 wood / 100 gold leave on enqueue,
    // completion lands 50 s later.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(1000));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(1000));
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    let gold_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold);
    sim.push_command(Command::ResearchAt {
        building: dock,
        tech: TechId::MediumWarships,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(150)
    );
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold),
        gold_before - I16F16::from_num(100)
    );
    let done = run_until(&mut sim, 10_000, |s| {
        s.has_tech(OWNER_PLAYER, TechId::MediumWarships)
    });
    assert!(done, "Medium Warships never completed at the Dock");
    assert_eq!(kind_of(&sim, galley), EntityKind::WarGalley);
    assert_eq!(kind_of(&sim, fire), EntityKind::FireShip);
    assert_eq!(kind_of(&sim, raft), EntityKind::DemolitionShip);
    assert_eq!(kind_of(&sim, hulk), EntityKind::WarHulk);
    assert_eq!(kind_of(&sim, foe), EntityKind::Galley, "the enemy's Galley converted");
    assert_eq!(hp_of(&sim, galley), I16F16::from_num(55), "conversion rewrote the HP");
    // The converted War Galley's effective maximum is the new
    // tier's 125.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::WarGalley).max_hp,
        125
    );
}

#[test]
fn upgraded_production_spawns_castle_tiers() {
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    set_age(&mut sim, OWNER_PLAYER, Age::Feudal);
    grant(&mut sim, OWNER_PLAYER, TechId::MediumWarships);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(2000));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(1000));
    // The queue entry still says Galley (the CLS-03 pattern);
    // the hull that leaves the Dock is a War Galley, at the
    // Galley's price.
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    sim.push_command(Command::TrainAt {
        building: dock,
        kind: EntityKind::Galley,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(90)
    );
    let spawned = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::WarGalley)
    });
    assert!(spawned, "no War Galley ever left the Dock queue");
    assert!(
        !sim.entities().iter().any(|e| e.kind == EntityKind::Galley),
        "a Feudal Galley hull spawned after the upgrade"
    );
    // Free the population slot the War Galley holds (the
    // sandbox start plus one ship already fills the Town
    // Center's support) before queueing the next hull.
    let wg = sim
        .entities
        .iter_mut()
        .find(|e| e.kind == EntityKind::WarGalley)
        .expect("the war galley");
    wg.hp = I16F16::ZERO;
    let cleared = run_until(&mut sim, 1_000, |s| {
        s.entities().iter().all(|e| e.kind != EntityKind::WarGalley || e.dead)
    });
    assert!(cleared, "the War Galley never cleared the population slot");
    // And a queued Hulk leaves as a War Hulk.
    sim.push_command(Command::TrainAt {
        building: dock,
        kind: EntityKind::Hulk,
    });
    let spawned = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::WarHulk)
    });
    assert!(spawned, "no War Hulk ever left the Dock queue");
}

#[test]
fn castle_tier_patch_rows_ride_the_blacksmith_swaps() {
    let mut sim = Sim::new(42);
    // Fletching on the War Galley: +1 pierce, bonus pairs
    // untouched, +1 tile of range — the Galley's own swap, one
    // tier up.
    let base = sim.stats_for(OWNER_PLAYER, EntityKind::WarGalley);
    grant(&mut sim, OWNER_PLAYER, TechId::Fletching);
    let fletched = sim.stats_for(OWNER_PLAYER, EntityKind::WarGalley);
    assert_eq!(fletched.attacks, crate::units::WAR_GALLEY_FLETCHING_ATTACKS);
    assert_eq!(fletched.attacks[0], (cls::PIERCE, 8));
    assert_eq!(fletched.range_max, base.range_max + I16F16::from_num(4));
    // The Fire Ship is melee spray: Fletching does not touch it.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::FireShip).attacks,
        crate::units::spec(EntityKind::FireShip).attacks
    );
    // Forging on the War Hulk: +1 melee, the anti-fire pair
    // untouched.
    grant(&mut sim, OWNER_PLAYER, TechId::Forging);
    let forged = sim.stats_for(OWNER_PLAYER, EntityKind::WarHulk);
    assert_eq!(forged.attacks, crate::units::WAR_HULK_FORGING_ATTACKS);
    assert_eq!(forged.attacks, &[(cls::MELEE, 5), (cls::FIRE_SHIP, 1)]);
}

#[test]
fn war_hulk_strikes_carry_the_anti_fire_bonus() {
    // Per-strike arithmetic on the Castle tier: the War Hulk
    // lands (4 − 2 melee armor) + 1 vs the Fire Ship class = 3
    // per strike on a Fire Ship, every 1.5 s.
    let mut sim = Sim::new(42);
    let (x, y) = pond_point(&sim);
    let hulk = spawn_at(&mut sim, 950, EntityKind::WarHulk, OWNER_PLAYER, x, y);
    let fire = spawn_at(
        &mut sim,
        951,
        EntityKind::FireShip,
        OWNER_OPPONENT,
        x + I16F16::from_num(2),
        y,
    );
    sim.push_command(Command::Attack {
        entity: hulk,
        target: fire,
    });
    sim.push_command(Command::SetStance {
        entity: fire,
        stance: Stance::NoAttack,
    });
    for _ in 0..1000 {
        sim.tick();
    }
    let fire_hp = hp_of(&sim, fire);
    assert!(
        fire_hp <= I16F16::from_num(120 - 24),
        "the War Hulk's anti-fire strikes under-delivered: fire ship at {fire_hp}"
    );
    assert_eq!(
        hp_of(&sim, hulk),
        I16F16::from_num(110),
        "the NoAttack Fire Ship fought back"
    );
}

#[test]
fn demolition_ship_blast_is_full_strength_at_the_radius_edge() {
    // The untapered profile, pinned against the Raft's taper:
    // an enemy Galley moored 11 world units (2.75 tiles) from
    // the wreck — near the edge of the 3.0-tile radius — and
    // battle-worn to 70 HP takes the FULL 80 melee and sinks.
    // On the Raft's tapered profile the same geometry would
    // land a single-digit hit and the ship would live; that
    // contrast is the pin. A second enemy ship just outside
    // the radius (13.6 units) is untouched, and the owner's
    // own ship inside the radius is untouched (enemies only,
    // as the Raft's).
    let (mut sim, (x, y)) = wide_pond_sim();
    let ship = spawn_at(&mut sim, 950, EntityKind::DemolitionShip, OWNER_PLAYER, x, y);
    // The geometry is the pin, so the ship must die where it
    // floats: on NoAttack it holds station instead of sailing
    // at its killer (which would drag the blast center 6 units
    // east and pull the outside ship into the radius).
    sim.push_command(Command::SetStance {
        entity: ship,
        stance: Stance::NoAttack,
    });
    let galley = spawn_at(
        &mut sim,
        951,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(6),
        y,
    );
    // The two victims are NoAttack Galleys, not Fishing Ships:
    // an idle Fishing Ship sails itself onto the fish node at
    // the pond point (auto-gather) and drags itself into the
    // radius before the wreck goes up — the W3a blast test's
    // staging lesson. A military hull on NoAttack holds
    // station, so the geometry pinned is the blast's.
    let edge_fish = spawn_at(
        &mut sim,
        952,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(11),
        y,
    );
    let outside_fish = spawn_at(
        &mut sim,
        953,
        EntityKind::Galley,
        OWNER_OPPONENT,
        x + I16F16::from_num(13),
        y + I16F16::from_num(4),
    );
    for id in [edge_fish, outside_fish] {
        sim.push_command(Command::SetStance {
            entity: id,
            stance: Stance::NoAttack,
        });
    }
    // The edge victim is battle-worn to 70: the full 80 kills
    // it, a tapered edge hit (~7) would leave it near 63.
    let e = sim
        .entities
        .iter_mut()
        .find(|e| e.id == edge_fish)
        .expect("the edge victim");
    e.hp = I16F16::from_num(70);
    // The owner's ship stands 8 units west: inside the radius
    // (the enemies-only pin) but clear of the separation shove's
    // reach — moored at 2 units it overlaps the wreck's radius
    // sum and the shove drags the blast center east with it.
    let own_fish = spawn_at(
        &mut sim,
        954,
        EntityKind::FishingShip,
        OWNER_PLAYER,
        x - I16F16::from_num(8),
        y,
    );
    sim.push_command(Command::Attack {
        entity: galley,
        target: ship,
    });
    let sunk = run_until(&mut sim, 120_000, |s| dead(s, ship));
    assert!(sunk, "the Galley never sank the Demolition Ship");
    assert!(
        dead(&sim, edge_fish),
        "the edge ship survived — the blast tapered (or never fired)"
    );
    assert_eq!(
        hp_of(&sim, outside_fish),
        I16F16::from_num(110),
        "a ship outside the 3.0 radius took blast damage"
    );
    assert_eq!(
        hp_of(&sim, own_fish),
        I16F16::from_num(50),
        "the blast hurt the owner's own ship (friendly fire)"
    );
    // The killer Galley, 6 units from the center, is inside the
    // radius and wounded but afloat (110 − 80 = 30).
    assert!(!dead(&sim, galley), "the blast sank the 110-HP Galley");
    assert!(
        hp_of(&sim, galley) <= I16F16::from_num(30),
        "the Galley inside the radius was not blasted: {}",
        hp_of(&sim, galley)
    );
}

