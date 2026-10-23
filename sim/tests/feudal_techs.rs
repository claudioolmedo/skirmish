//! The Feudal completion set (2026-10-22): Forging, Scale
//! Barding Armor (Blacksmith), Bloodlines (Stable), Double-Bit
//! Axe (Lumber Camp), Gold Mining / Stone Mining (Mining Camp)
//! and Horse Collar (Mill) — every cost, time and effect fetched
//! from the Age of Empires wiki this run and cited in
//! `sim/src/tech.rs`. These tests pin the *effects* end to end:
//! the patched stat rows in the damage/armor lookups, Bloodlines'
//! current-HP grant, the gather-rate patches measured as exact
//! node drain, Horse Collar's creation stamp + proportional
//! top-up (update 169123), and a camp research flowing through
//! TEC-05's cancel-with-refund.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, EntityKind, NodeKind, ResourceKind, Sim, Task, TechId,
    OWNER_PLAYER,
};

fn villager(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn entity_of(sim: &Sim, kind: EntityKind, owner: u8) -> &skirmish_sim::Entity {
    sim.entities()
        .iter()
        .find(|e| e.kind == kind && e.owner == owner && !e.dead)
        .unwrap()
}

fn anchor_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn build(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    let (x, y) = anchor_point(sim, cx, cy);
    sim.push_command(Command::Build {
        entity,
        kind,
        x,
        y,
    });
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

fn first_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes().iter().find(|n| n.kind == kind).unwrap().id
}

fn first_live_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .find(|n| n.kind == kind && n.amount_left > I16F16::from_num(0))
        .unwrap()
        .id
}

/// A woodline tree for sustained chopping (same selection rule
/// as tests/tech.rs): a reachable forest-edge tree with several
/// tree neighbors in retarget reach.
fn woodline_tree(sim: &Sim) -> u32 {
    let tc = sim.town_center();
    let trees: Vec<_> = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Tree && !n.is_depleted())
        .collect();
    let neighbors = |t: &skirmish_sim::ResourceNode| {
        trees
            .iter()
            .filter(|o| {
                let dx = o.pos_x.to_num::<f64>() - t.pos_x.to_num::<f64>();
                let dy = o.pos_y.to_num::<f64>() - t.pos_y.to_num::<f64>();
                dx * dx + dy * dy <= 144.0
            })
            .count()
    };
    let workable = |t: &skirmish_sim::ResourceNode| {
        let (cx, cy) = sim.grid().world_to_cell(t.pos_x, t.pos_y);
        (-1i32..=1).any(|dy| {
            (-1i32..=1).any(|dx| {
                let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                if nx < 0 || ny < 0 || sim.grid().is_blocked(nx as u32, ny as u32) {
                    return false;
                }
                let center = sim.grid().cell_center(nx as u32, ny as u32);
                sim.grid().find_path((tc.pos_x, tc.pos_y), center).is_some()
            })
        })
    };
    trees
        .iter()
        .filter(|t| workable(t) && neighbors(t) >= 3)
        .max_by_key(|t| neighbors(t))
        .expect("no workable woodline tree")
        .id
}

fn building_id(sim: &Sim, kind: BuildingKind) -> u32 {
    sim.buildings()
        .iter()
        .find(|b| b.kind == kind && b.complete)
        .unwrap()
        .id
}

fn raise(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    build(sim, entity, kind, cx, cy);
    let up = run_until(sim, 30_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == kind && b.complete)
    });
    assert!(up, "{kind:?} never completed");
}

/// Mill + Lumber Camp, berries to 500 food, click up — the
/// standard Feudal opening (mirrors tests/tech.rs).
fn reach_feudal(sim: &mut Sim) {
    raise(sim, 2, BuildingKind::Mill, 21, 20);
    raise(sim, 3, BuildingKind::LumberCamp, 16, 20);
    let berry = first_node(sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: berry,
    });
    let food = run_until(sim, 120_000, |s| {
        s.stockpile().amount(ResourceKind::Food) >= I16F16::from_num(500)
    });
    assert!(food, "never gathered the Feudal food");
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    let up = run_until(sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");
}

fn wait_wood(sim: &mut Sim, want: i32) {
    let ok = run_until(sim, 240_000, |s| {
        s.stockpile().amount(ResourceKind::Wood) >= I16F16::from_num(want)
    });
    assert!(ok, "never stocked {want} wood");
}

/// Research `tech` at `building`, waiting for affordability.
fn research_at(sim: &mut Sim, building: u32, tech: TechId) {
    let ready = run_until(sim, 300_000, |s| s.can_research_at(building, tech));
    assert!(ready, "{tech:?} never became researchable");
    sim.push_command(Command::ResearchAt { building, tech });
    let done = run_until(sim, 20_000, |s| s.has_tech(OWNER_PLAYER, tech));
    assert!(done, "{tech:?} never completed");
}

fn node_amount(sim: &Sim, node: u32) -> I16F16 {
    sim.nodes()
        .iter()
        .find(|n| n.id == node)
        .unwrap()
        .amount_left
}

/// Send villager `v` to gather `node`, wait until the drain has
/// actually started, then measure the exact node drop (in bits)
/// over `ticks` further ticks of gathering.
fn measure_drain(sim: &mut Sim, v: u32, node: u32, ticks: u64) -> i32 {
    sim.push_command(Command::Gather { entity: v, node });
    let before = node_amount(sim, node);
    let started = run_until(sim, 90_000, |s| node_amount(s, node) < before);
    assert!(started, "gather never started on node {node}");
    let a0 = node_amount(sim, node);
    for _ in 0..ticks {
        sim.tick();
    }
    a0.to_bits() - node_amount(sim, node).to_bits()
}

fn park(sim: &mut Sim, v: u32) {
    sim.push_command(Command::Move {
        entity: v,
        x: I16F16::from_num(30),
        y: I16F16::from_num(-30),
    });
    let idle = run_until(sim, 30_000, |s| villager(s, v).task == Task::Idle);
    assert!(idle, "villager {v} never parked");
}

#[test]
fn forging_and_scale_barding_patch_the_rows() {
    let mut sim = Sim::new(107);
    reach_feudal(&mut sim);
    // Wood for the Blacksmith (150): villager 3 chops; 1 and 2
    // stay on food (farms below — the berry patch is spent).
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 150);
    raise(&mut sim, 3, BuildingKind::Blacksmith, 30, 30);
    let smith = building_id(&sim, BuildingKind::Blacksmith);
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 120);
    raise(&mut sim, 1, BuildingKind::Farm, 23, 24);
    raise(&mut sim, 2, BuildingKind::Farm, 23, 27);
    let two = run_until(&mut sim, 60_000, |s| {
        s.nodes().iter().filter(|n| n.kind == NodeKind::Farm).count() == 2
    });
    assert!(two, "the second Farm never completed");
    let farms: Vec<u32> = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Farm)
        .map(|n| n.id)
        .collect();
    sim.push_command(Command::Gather {
        entity: 1,
        node: farms[0],
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: farms[1],
    });

    // Baselines: Militia melee 4, Man-at-Arms melee 6 (+2/+2
    // bonuses), Feudal Scout melee 5 / armor (0, 2), Archer
    // pierce 4.
    let militia = sim.stats_for(OWNER_PLAYER, EntityKind::Militia);
    assert_eq!(militia.attacks.len(), 1);
    assert_eq!(militia.attacks[0].1, 4);
    let maa = sim.stats_for(OWNER_PLAYER, EntityKind::ManAtArms);
    assert_eq!(maa.attacks[0].1, 6);
    let scout = sim.stats_for(OWNER_PLAYER, EntityKind::ScoutCavalry);
    assert_eq!(scout.attacks[0].1, 5);
    assert_eq!((scout.armor_melee, scout.armor_pierce), (0, 2));
    // Spearman (the units package's row): melee 3 with its bonus
    // ladder behind it.
    let spear = sim.stats_for(OWNER_PLAYER, EntityKind::Spearman);
    assert_eq!(spear.attacks.len(), 7);
    assert_eq!(spear.attacks[0].1, 3);

    // Forging: exact charge (150 F) and exact duration (50 s =
    // 3000 progress ticks, TEC-05's enqueue convention).
    let ready = run_until(&mut sim, 300_000, |s| {
        s.can_research_at(smith, TechId::Forging)
    });
    assert!(ready, "Forging never became researchable");
    let food_before = sim.stockpile().amount(ResourceKind::Food);
    let t0 = sim.tick_count();
    sim.push_command(Command::ResearchAt {
        building: smith,
        tech: TechId::Forging,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        food_before - I16F16::from_num(150),
        "Forging must charge exactly 150 food at enqueue"
    );
    let done = run_until(&mut sim, 10_000, |s| s.has_tech(OWNER_PLAYER, TechId::Forging));
    assert!(done, "Forging never completed");
    assert_eq!(sim.tick_count() - t0, 3000, "Forging must take exactly 50 s");

    // +1 melee on the infantry and cavalry rows; bonuses and the
    // Archer's pierce untouched.
    let militia = sim.stats_for(OWNER_PLAYER, EntityKind::Militia);
    assert_eq!(militia.attacks[0].1, 5);
    let maa = sim.stats_for(OWNER_PLAYER, EntityKind::ManAtArms);
    assert_eq!(maa.attacks.len(), 3);
    assert_eq!(maa.attacks[0].1, 7);
    assert_eq!(maa.attacks[1].1, 2);
    assert_eq!(maa.attacks[2].1, 2);
    let scout = sim.stats_for(OWNER_PLAYER, EntityKind::ScoutCavalry);
    assert_eq!(scout.attacks[0].1, 6);
    let archer = sim.stats_for(OWNER_PLAYER, EntityKind::Archer);
    assert_eq!(archer.attacks[0].1, 4);
    // The Spearman is infantry: its base melee rises 3 → 4 and
    // every bonus pair (cavalry 15 first among them) is unmoved.
    // The Skirmisher deals pierce — Forging's melee bonus does
    // not touch its table.
    let spear = sim.stats_for(OWNER_PLAYER, EntityKind::Spearman);
    assert_eq!(spear.attacks[0].1, 4);
    assert_eq!(spear.attacks[1].1, 15);
    let skirm = sim.stats_for(OWNER_PLAYER, EntityKind::Skirmisher);
    assert_eq!(skirm.attacks[0].1, 2);

    // Scale Barding Armor: cavalry +1/+1, infantry unchanged.
    research_at(&mut sim, smith, TechId::ScaleBarding);
    let scout = sim.stats_for(OWNER_PLAYER, EntityKind::ScoutCavalry);
    assert_eq!((scout.armor_melee, scout.armor_pierce), (1, 3));
    let militia = sim.stats_for(OWNER_PLAYER, EntityKind::Militia);
    assert_eq!((militia.armor_melee, militia.armor_pierce), (0, 1));
    let archer = sim.stats_for(OWNER_PLAYER, EntityKind::Archer);
    assert_eq!((archer.armor_melee, archer.armor_pierce), (0, 0));
}

#[test]
fn bloodlines_raises_max_and_current_hp_of_mounted_units() {
    let mut sim = Sim::new(101);
    reach_feudal(&mut sim);
    // The Stable requires a completed Barracks first (the
    // Feudal military chain): wood for both (175 + 175),
    // villager 3 chops. Anchors mirror the proven seed-101
    // sequence in tests/tech.rs.
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 350);
    raise(&mut sim, 3, BuildingKind::Barracks, 27, 20);
    raise(&mut sim, 3, BuildingKind::Stable, 27, 30);
    let stable = building_id(&sim, BuildingKind::Stable);

    // The starting Scout: 45 HP, full.
    let scout_id = entity_of(&sim, EntityKind::ScoutCavalry, OWNER_PLAYER).id;
    assert_eq!(villager(&sim, scout_id).hp, I16F16::from_num(45));
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::ScoutCavalry).max_hp,
        45
    );

    // Bloodlines costs exactly the starting gold (150 F / 100 G).
    research_at(&mut sim, stable, TechId::Bloodlines);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Gold),
        I16F16::from_num(0),
        "Bloodlines must have charged the full 100 gold"
    );
    // Max HP 45 → 65 in the lookup, and the standing Scout's
    // current HP rises with it (Loom's rule).
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::ScoutCavalry).max_hp,
        65
    );
    assert_eq!(villager(&sim, scout_id).hp, I16F16::from_num(65));
    // Nothing on foot changes.
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Militia).max_hp, 40);
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Villager).max_hp, 25);
}

#[test]
fn gather_technologies_scale_the_node_drain_exactly() {
    let mut sim = Sim::new(107);
    reach_feudal(&mut sim);
    let lc = building_id(&sim, BuildingKind::LumberCamp);
    // Mining Camp (100 W): villager 3 chops, then builds.
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 100);
    raise(&mut sim, 3, BuildingKind::MiningCamp, 30, 30);
    let camp = building_id(&sim, BuildingKind::MiningCamp);
    // Villager 3 stocks ALL the research wood up front (50 + 75
    // + 75), then parks: the drain measurements below must have
    // exactly ONE gatherer on the measured node — a second
    // chopper on the same tree silently adds its own takes.
    // Villager 1 is the measurer; 2 keeps the food coming.
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 220);
    park(&mut sim, 3);

    // Wood, before Double-Bit Axe: the base rate, 426 bits/tick.
    let tree = woodline_tree(&sim);
    assert_eq!(measure_drain(&mut sim, 1, tree, 600), 600 * 426);
    park(&mut sim, 1);
    research_at(&mut sim, lc, TechId::DoubleBitAxe);
    // Wood, after: ×120/100 → 511 bits/tick.
    assert_eq!(measure_drain(&mut sim, 1, tree, 600), 600 * 511);
    park(&mut sim, 1);

    // Gold, before Gold Mining: 415 bits/tick.
    let gold = first_live_node(&sim, NodeKind::GoldMine);
    assert_eq!(measure_drain(&mut sim, 1, gold, 600), 600 * 415);
    park(&mut sim, 1);
    research_at(&mut sim, camp, TechId::GoldMining);
    // Gold, after: ×115/100 → 477 bits/tick.
    assert_eq!(measure_drain(&mut sim, 1, gold, 600), 600 * 477);
    park(&mut sim, 1);

    // Stone, before Stone Mining: 393 bits/tick.
    let stone = first_live_node(&sim, NodeKind::StoneMine);
    assert_eq!(measure_drain(&mut sim, 1, stone, 600), 600 * 393);
    park(&mut sim, 1);
    research_at(&mut sim, camp, TechId::StoneMining);
    // Stone, after: ×115/100 → 451 bits/tick.
    assert_eq!(measure_drain(&mut sim, 1, stone, 600), 600 * 451);
    park(&mut sim, 1);
}

#[test]
fn horse_collar_stamps_new_farms_and_tops_up_standing_ones() {
    let mut sim = Sim::new(111);
    reach_feudal(&mut sim);
    let mill = building_id(&sim, BuildingKind::Mill);
    // Wood for a farm (60) and Horse Collar (75): villager 3 chops.
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 135);
    // Farm A, raised BEFORE the tech: 175 food, then partly eaten.
    raise(&mut sim, 2, BuildingKind::Farm, 23, 24);
    let farm_a = first_live_node(&sim, NodeKind::Farm);
    assert_eq!(node_amount(&sim, farm_a), I16F16::from_num(175));
    let drained = run_until(&mut sim, 60_000, |s| {
        node_amount(s, farm_a) < I16F16::from_num(170)
    });
    assert!(drained, "the farmer never ate into Farm A");
    // Park the farmer so the amount is frozen across the research.
    park(&mut sim, 2);
    let left = node_amount(&sim, farm_a).to_num::<f64>();

    research_at(&mut sim, mill, TechId::HorseCollar);
    // Update 169123: the standing farm gains 75 × (left/175)²,
    // rounded UP to whole food — ECO-14: every worked example in
    // the articles rounds the result up (Horse Collar +19 at
    // half, from the raw 18.75), though no rounding rule is
    // stated; the inference is followed. The raw share at this
    // remainder is fractional (~70.6), so a non-rounding sweep
    // lands visibly below the ceiled expectation.
    let gained = node_amount(&sim, farm_a).to_num::<f64>() - left;
    let expected = (75.0 * (left / 175.0) * (left / 175.0)).ceil();
    assert!(
        (gained - expected).abs() < 0.01,
        "Horse Collar top-up: gained {gained}, expected {expected} (left was {left})"
    );
    assert_eq!(
        gained.fract(),
        0.0,
        "the top-up lands on whole food: gained {gained}"
    );

    // A farm raised AFTER the tech is stamped at 250. Two
    // scenario details: villager 3 funds it first (the Horse
    // Collar research at 75 wood plus Farm A at 60 leave the
    // stockpile short of the 60 a second farm costs, and a Build
    // order placed without the wood is refused); and the wait
    // targets the farm NODE count — `raise`'s completion
    // predicate is already satisfied by Farm A, so it would
    // return before this farm exists.
    wait_wood(&mut sim, 60);
    build(&mut sim, 3, BuildingKind::Farm, 23, 27);
    let up_b = run_until(&mut sim, 30_000, |s| {
        s.nodes().iter().filter(|n| n.kind == NodeKind::Farm).count() == 2
    });
    assert!(up_b, "the post-tech farm never completed");
    let farms: Vec<u32> = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Farm && n.id != farm_a)
        .map(|n| n.id)
        .collect();
    assert_eq!(farms.len(), 1);
    assert_eq!(node_amount(&sim, farms[0]), I16F16::from_num(250));
}

#[test]
fn camp_research_cancel_refunds_exactly() {
    let mut sim = Sim::new(113);
    reach_feudal(&mut sim);
    let lc = building_id(&sim, BuildingKind::LumberCamp);
    // Double-Bit Axe needs 100 F / 50 W on hand.
    let tree = woodline_tree(&sim);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    let ready = run_until(&mut sim, 300_000, |s| {
        s.can_research_at(lc, TechId::DoubleBitAxe)
    });
    assert!(ready, "Double-Bit Axe never became researchable");
    let food_before = sim.stockpile().amount(ResourceKind::Food);
    let wood_before = sim.stockpile().amount(ResourceKind::Wood);
    sim.push_command(Command::ResearchAt {
        building: lc,
        tech: TechId::DoubleBitAxe,
    });
    sim.tick();
    assert_eq!(sim.building_queue(lc).len(), 1);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        food_before - I16F16::from_num(100)
    );
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(50)
    );
    // TEC-05: cancelling the head entry refunds in full.
    sim.push_command(Command::CancelQueueBuilding {
        building: lc,
        index: 0,
    });
    sim.tick();
    assert_eq!(sim.building_queue(lc).len(), 0);
    assert_eq!(sim.stockpile().amount(ResourceKind::Food), food_before);
    assert_eq!(sim.stockpile().amount(ResourceKind::Wood), wood_before);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::DoubleBitAxe));
}
