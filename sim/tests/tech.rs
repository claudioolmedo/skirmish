//! Technology tests (M6): the age system (Feudal requirements,
//! cost, timing, shared TC queue), Loom's stat deltas in the
//! damage math, nearest-valid-drop-off routing + TC fallback,
//! force drop-off, the builder-completion deposit, the Farm/Mill
//! gate, the Barracks→Range production split, Blacksmith deltas,
//! the Man-at-Arms upgrade, and an AI-match replay that reaches
//! Feudal bit-identically twice.
//!
//! Numbers come from the project docs: OPENING-SPEC §2.4–§2.7
//! (camps 100 wood / 35 s, Loom 50 gold / 25 s, Feudal 500 food /
//! 130 s / 2 buildings), MECHANICS-STUDY §5/§8 (Range/Stable
//! 175 wood, Blacksmith tech costs/times), RULEBOOK TEC-01..08.

use fixed::types::I16F16;
use skirmish_sim::{
    compute_damage, Age, BuildingKind, Command, EntityKind, MatchMode, NodeKind, ResearchBlock,
    ResourceKind, Sim, Task, TechId, TrainBlock, OWNER_OPPONENT, OWNER_PLAYER,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn villager(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
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

/// First node of `kind` that still holds resources. (A gather
/// order onto a depleted node is rejected by the sim — the
/// villager stays idle — so long tests must pick live nodes.)
fn first_live_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .find(|n| n.kind == kind && n.amount_left > I16F16::from_num(0))
        .unwrap()
        .id
}

fn building_id(sim: &Sim, kind: BuildingKind) -> u32 {
    sim.buildings()
        .iter()
        .find(|b| b.kind == kind && b.complete)
        .unwrap()
        .id
}

/// Raise `kind` at `(cx, cy)` with `entity` and wait for completion.
fn raise(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    build(sim, entity, kind, cx, cy);
    let up = run_until(sim, 30_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == kind && b.complete)
    });
    assert!(up, "{kind:?} never completed");
}

/// Play the standard start up to the Feudal Age: Mill + Lumber
/// Camp (the requirement pair), berries until 500 food, click up.
/// Shared setup for the Feudal-dependent tests. Note: the camps
/// consumed the whole starting wood — tests that build afterwards
/// must chop first (`wait_wood`).
fn reach_feudal(sim: &mut Sim) {
    raise(sim, 2, BuildingKind::Mill, 21, 20);
    // Lumber Camp anchor (16, 20): the natural (18, 21) spot has a
    // sheep standing on it, and placement refuses to cover a
    // standing unit (M6 validate_build rule).
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
    assert_eq!(sim.feudal_building_count(OWNER_PLAYER), 2);
    assert!(sim.can_research_tc(TechId::FeudalAge));
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    let up = run_until(sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");
}

/// Wait until the wood stockpile holds at least `want`.
fn wait_wood(sim: &mut Sim, want: i32) {
    let ok = run_until(sim, 240_000, |s| {
        s.stockpile().amount(ResourceKind::Wood) >= I16F16::from_num(want)
    });
    assert!(ok, "never stocked {want} wood");
}

/// Put every villager on the woodline.
fn all_chop(sim: &mut Sim) {
    let tree = first_node(sim, NodeKind::Tree);
    for id in 1..=3 {
        sim.push_command(Command::Gather {
            entity: id,
            node: tree,
        });
    }
}

#[test]
fn feudal_research_gates_cost_and_exact_timing() {
    let mut sim = Sim::new(61);
    // No buildings yet: the advancement is Locked (requirements).
    assert_eq!(
        sim.research_block_tc(TechId::FeudalAge),
        ResearchBlock::Locked
    );
    assert!(!sim.can_research_tc(TechId::FeudalAge));
    // The command is refused whole: nothing queued, nothing spent.
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 0);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        I16F16::from_num(200)
    );

    // The Dark-Age pair stands: now only the food is missing.
    raise(&mut sim, 2, BuildingKind::Mill, 21, 20);
    raise(&mut sim, 3, BuildingKind::LumberCamp, 16, 20); // (18, 21) has a sheep on it
    assert_eq!(sim.feudal_building_count(OWNER_PLAYER), 2);
    assert_eq!(
        sim.research_block_tc(TechId::FeudalAge),
        ResearchBlock::NoFunds
    );

    // Gather the 500, park the villagers so the stockpile is still,
    // then the click charges exactly 500 and takes exactly 7800
    // ticks (130 s, TEC-01).
    let berry = first_node(&sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: berry,
    });
    let food = run_until(&mut sim, 120_000, |s| {
        s.stockpile().amount(ResourceKind::Food) >= I16F16::from_num(500)
    });
    assert!(food, "never gathered 500 food");
    for id in 1..=3 {
        sim.push_command(Command::Move {
            entity: id,
            x: I16F16::from_num(30),
            y: I16F16::from_num(-30),
        });
    }
    run_until(&mut sim, 5_000, |s| {
        (1..=3).all(|id| villager(s, id).task == Task::Idle)
    });
    let food_before = sim.stockpile().amount(ResourceKind::Food);
    assert!(sim.can_research_tc(TechId::FeudalAge));
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    // The tick that applies the command also banks the first progress
    // tick (the same convention as villager training), so the full
    // 7800 progress ticks span 7800 sim ticks from here.
    let t0 = sim.tick_count();
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 1);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        food_before - I16F16::from_num(500),
        "Feudal must charge exactly 500 food at enqueue (TEC-05)"
    );
    let up = run_until(&mut sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "Feudal never landed");
    assert_eq!(
        sim.tick_count() - t0,
        7800,
        "Feudal research must take exactly 130 s (TEC-01)"
    );
    assert!(sim.has_tech(OWNER_PLAYER, TechId::FeudalAge));
    assert!(sim.tech_mask_of(OWNER_PLAYER) & TechId::FeudalAge.bit() != 0);
    // Once researched, it cannot be queued again.
    assert_eq!(
        sim.research_block_tc(TechId::FeudalAge),
        ResearchBlock::AlreadyDone
    );
}

#[test]
fn loom_charges_gold_and_patches_villager_stats() {
    let mut sim = Sim::new(67);
    // The boar kills an un-Loomed villager in 4 hits (7 damage vs
    // 25 HP), and the villager row is the base row.
    let base = sim.stats_for(OWNER_PLAYER, EntityKind::Villager);
    assert_eq!(base.max_hp, 25);
    assert_eq!(
        compute_damage(&[(0u8, 7)], &base),
        7,
        "boar vs un-Loomed villager"
    );
    assert!((base.max_hp as f64 / 7.0).ceil() as i32 == 4);

    assert!(sim.can_research_tc(TechId::Loom));
    let gold_before = sim.stockpile().amount(ResourceKind::Gold);
    sim.push_command(Command::ResearchTc {
        tech: TechId::Loom,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Gold),
        gold_before - I16F16::from_num(50),
        "Loom must charge exactly 50 gold at enqueue"
    );
    // Halfway through the 1500 ticks the head progress reads 50%.
    for _ in 0..749 {
        sim.tick();
    }
    assert_eq!(sim.tc_progress_bits(), 32768);
    let done = run_until(&mut sim, 5_000, |s| s.has_tech(OWNER_PLAYER, TechId::Loom));
    assert!(done, "Loom never completed");

    // TEC-04's numbers, empire-wide and at once: +15 HP (current HP
    // rises with the max), +1 melee / +2 pierce armor, and the
    // RULEBOOK's +0.1 tiles/s speed (TEC-06/CLS-05).
    let loomed = sim.stats_for(OWNER_PLAYER, EntityKind::Villager);
    assert_eq!(loomed.max_hp, 40);
    assert_eq!(loomed.armor_melee, 1);
    assert_eq!(loomed.armor_pierce, 2);
    assert_eq!(loomed.speed.to_bits(), 73_728);
    for id in 1..=3 {
        assert_eq!(
            villager(&sim, id).hp,
            I16F16::from_num(40),
            "existing villager did not gain Loom's +15 HP"
        );
    }
    // In the damage math: the boar now needs 7 hits (6 damage vs
    // 40 HP) — OPENING-SPEC §2.2's kill math.
    assert_eq!(compute_damage(&[(0u8, 7)], &loomed), 6);
    assert!((loomed.max_hp as f64 / 6.0).ceil() as i32 == 7);
    // And it is a one-time research.
    assert_eq!(
        sim.research_block_tc(TechId::Loom),
        ResearchBlock::AlreadyDone
    );
}

#[test]
fn research_shares_the_tc_queue_with_training() {
    let mut sim = Sim::new(71);
    sim.push_command(Command::TrainVillager);
    sim.push_command(Command::ResearchTc {
        tech: TechId::Loom,
    });
    sim.tick();
    // One FIFO: villager first (entity tag 0), Loom behind it
    // (research code 100 + tag 0). TEC-03.
    let codes: Vec<u32> = sim.tc_queue().iter().map(|e| e.code()).collect();
    assert_eq!(codes, vec![0, 100]);
    // The villager completes on its own 1500 ticks; Loom only then
    // starts — and completes 1500 ticks later.
    let spawned = run_until(&mut sim, 5_000, |s| s.villager_count() == 4);
    assert!(spawned, "queued villager never trained");
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::Loom));
    let done = run_until(&mut sim, 5_000, |s| s.has_tech(OWNER_PLAYER, TechId::Loom));
    assert!(done, "Loom never completed behind the villager");
    assert_eq!(sim.tc_queue().len(), 0);
}

#[test]
fn deposits_route_to_the_nearest_valid_camp() {
    let mut sim = Sim::new(73);
    // A Mill beside the berry patch — strictly nearer to the
    // gatherer than the Town Center.
    raise(&mut sim, 2, BuildingKind::Mill, 22, 27);
    let berry = first_node(&sim, NodeKind::BerryBush);
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    // Track every Returning tick: the hauler must never come
    // within deposit range of the Town Center's center — its
    // deposits land at the Mill instead (ECO-06).
    let tc = villager(&sim, 0);
    let (tcx, tcy) = (f(tc.pos_x), f(tc.pos_y));
    let mut min_tc_dist = f64::MAX;
    let mut ticks = 0u64;
    while sim.stockpile().amount(ResourceKind::Food) < start_food + I16F16::from_num(50) {
        sim.tick();
        ticks += 1;
        assert!(ticks < 60_000, "hauler never banked 50 food");
        let v = villager(&sim, 1);
        if v.task == Task::Returning {
            let d = ((f(v.pos_x) - tcx).powi(2) + (f(v.pos_y) - tcy).powi(2)).sqrt();
            min_tc_dist = min_tc_dist.min(d);
        }
    }
    assert!(
        min_tc_dist > 3.0,
        "hauler came within {min_tc_dist:.1} of the Town Center — it must use the Mill"
    );
}

#[test]
fn deposits_fall_back_to_the_tc_without_a_camp() {
    let mut sim = Sim::new(79);
    // No camps at all: the pre-M6 route — haul to the Town Center.
    let berry = first_node(&sim, NodeKind::BerryBush);
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    let tc = villager(&sim, 0);
    let (tcx, tcy) = (f(tc.pos_x), f(tc.pos_y));
    // Measure where the villager stands on the tick a deposit
    // lands: a post-tick *Returning* observation can never be
    // inside the deposit radius (arrival fires in the same tick),
    // so the deposit tick itself is the honest sample.
    let mut deposit_dist = f64::MAX;
    let mut prev_food = start_food;
    let mut ticks = 0u64;
    while sim.stockpile().amount(ResourceKind::Food) <= start_food {
        sim.tick();
        ticks += 1;
        assert!(ticks < 60_000, "no deposit ever landed");
        let now = sim.stockpile().amount(ResourceKind::Food);
        if now > prev_food {
            let v = villager(&sim, 1);
            deposit_dist = ((f(v.pos_x) - tcx).powi(2) + (f(v.pos_y) - tcy).powi(2)).sqrt();
        }
        prev_food = now;
    }
    assert!(
        deposit_dist <= 3.001,
        "the deposit landed {deposit_dist:.2} from the Town Center's center — not at it"
    );
}

#[test]
fn force_drop_off_deposits_and_returns_to_work() {
    let mut sim = Sim::new(83);
    raise(&mut sim, 2, BuildingKind::Mill, 22, 27);
    let berry = first_node(&sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    // Work until the basket holds a visible load, mid-shift.
    let loaded = run_until(&mut sim, 30_000, |s| {
        let v = villager(s, 1);
        v.task == Task::Gathering && v.carry >= I16F16::from_num(1)
    });
    assert!(loaded, "villager never picked up food");
    let food_before = sim.stockpile().amount(ResourceKind::Food);
    assert!(sim.can_force_deposit(1));
    sim.push_command(Command::ForceDeposit { entity: 1 });
    // The load lands (exactly the carried amount), and the
    // villager goes back to the same bush on its own (ECO-07).
    // (Wait for the order to visibly take first: the return-to-work
    // condition below is *already* true before the command applies
    // — the villager is still gathering with its assignment set.)
    let underway = run_until(&mut sim, 5_000, |s| {
        villager(s, 1).task == Task::Returning
    });
    assert!(underway, "force drop-off never took");
    let back = run_until(&mut sim, 30_000, |s| {
        let v = villager(s, 1);
        v.task_node == Some(berry) && matches!(v.task, Task::ToNode | Task::Gathering)
    });
    assert!(back, "villager did not return to its bush after force drop-off");
    assert!(
        sim.stockpile().amount(ResourceKind::Food) > food_before,
        "the forced load was never credited"
    );
}

#[test]
fn a_builder_finishing_a_camp_deposits_its_carry_there() {
    let mut sim = Sim::new(89);
    // Villager 1 fills its basket at the berries, then builds the
    // Mill itself — basket in hand the whole time (Bucket B setup).
    let berry = first_node(&sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    let loaded = run_until(&mut sim, 30_000, |s| {
        villager(s, 1).carry >= I16F16::from_num(5)
    });
    assert!(loaded, "villager never loaded up");
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    build(&mut sim, 1, BuildingKind::Mill, 22, 27);
    // Watch the basket until the Mill stands: it must not change
    // while building (no deposit happened anywhere else).
    let mut carry_before = villager(&sim, 1).carry;
    let mut ticks = 0u64;
    while !sim
        .buildings()
        .iter()
        .any(|b| b.kind == BuildingKind::Mill && b.complete)
    {
        carry_before = villager(&sim, 1).carry;
        sim.tick();
        ticks += 1;
        assert!(ticks < 30_000, "mill never completed");
    }
    assert!(carry_before > I16F16::ZERO);
    assert_eq!(
        villager(&sim, 1).carry,
        I16F16::ZERO,
        "builder's basket was not emptied at the new Mill"
    );
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        start_food + carry_before,
        "the builder's load was not credited at completion (Bucket B)"
    );
}

#[test]
fn farm_placement_requires_a_completed_mill() {
    let mut sim = Sim::new(97);
    let (fx, fy) = anchor_point(&sim, 28, 20);
    assert!(
        !sim.can_build(1, BuildingKind::Farm, fx, fy),
        "farm placeable with no Mill at all"
    );
    // An unfinished Mill does not open the gate either.
    build(&mut sim, 1, BuildingKind::Mill, 20, 20);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1);
    assert!(
        !sim.can_build(2, BuildingKind::Farm, fx, fy),
        "farm placeable with the Mill still rising"
    );
    let up = run_until(&mut sim, 30_000, |s| s.buildings()[0].complete);
    assert!(up, "mill never completed");
    assert!(
        sim.can_build(2, BuildingKind::Farm, fx, fy),
        "farm still blocked with a standing Mill"
    );
}

#[test]
fn archer_production_moves_to_the_range_and_scout_to_the_stable() {
    let mut sim = Sim::new(101);
    reach_feudal(&mut sim);
    all_chop(&mut sim);
    wait_wood(&mut sim, 175);
    raise(&mut sim, 1, BuildingKind::Barracks, 27, 20);
    let barracks = building_id(&sim, BuildingKind::Barracks);
    // The M6 split: the Barracks refuses Archers entirely.
    assert!(!sim.can_train_at(barracks, EntityKind::Archer));
    assert_eq!(
        sim.train_at_block(barracks, EntityKind::Archer),
        TrainBlock::QueueFull
    );
    assert!(sim.can_train_at(barracks, EntityKind::Militia));
    // The Range takes Archers; the Stable takes Scouts.
    wait_wood(&mut sim, 175);
    raise(&mut sim, 1, BuildingKind::ArcheryRange, 27, 24);
    let range = building_id(&sim, BuildingKind::ArcheryRange);
    // The Archer's line cost is 25 wood + 45 gold (M3 table) —
    // top the wood back up before judging trainability.
    wait_wood(&mut sim, 25);
    assert!(sim.can_train_at(range, EntityKind::Archer));
    assert_eq!(
        sim.train_at_block(range, EntityKind::Archer),
        TrainBlock::Ok
    );
    wait_wood(&mut sim, 175);
    raise(&mut sim, 1, BuildingKind::Stable, 27, 30);
    let stable = building_id(&sim, BuildingKind::Stable);
    assert!(sim.can_train_at(stable, EntityKind::ScoutCavalry));
    // And the Feudal buildings were age-gated: a Dark-Age sim
    // cannot even place them.
    let dark = Sim::new(103);
    let (x, y) = anchor_point(&dark, 27, 20);
    assert!(!dark.can_build(1, BuildingKind::ArcheryRange, x, y));
    assert!(!dark.can_build(1, BuildingKind::Stable, x, y));
    assert!(!dark.can_build(1, BuildingKind::Blacksmith, x, y));
}

#[test]
fn blacksmith_technologies_patch_the_combat_rows() {
    let mut sim = Sim::new(107);
    reach_feudal(&mut sim);
    // Wood for the Blacksmith: villager 1 chops (the camps ate the
    // starting stockpile); 2 and 3 stay on food duty below.
    let tree = first_node(&sim, NodeKind::Tree);
    sim.push_command(Command::Gather {
        entity: 1,
        node: tree,
    });
    wait_wood(&mut sim, 150);
    // Anchor (30,30): clear of the horizontal wall row (y=32 spans
    // cols 6..=20) and the vertical wall column.
    raise(&mut sim, 1, BuildingKind::Blacksmith, 30, 30);
    let smith = building_id(&sim, BuildingKind::Blacksmith);
    // Food for the three researches (100 each): the berry patch is
    // spent by the age-up, so farm instead — the Mill from
    // reach_feudal opens the gate. Two farms, one farmer each.
    // (Wood first: the Blacksmith ate the whole stockpile, and
    // villager 1 goes back to the woodline to refill it. The tree
    // is re-resolved live: the first chop depleted the original
    // node, and a gather order onto a depleted node is refused.)
    let tree = first_live_node(&sim, NodeKind::Tree);
    sim.push_command(Command::Gather {
        entity: 1,
        node: tree,
    });
    wait_wood(&mut sim, 120);
    raise(&mut sim, 2, BuildingKind::Farm, 23, 24);
    raise(&mut sim, 3, BuildingKind::Farm, 23, 27);
    // (`raise` returns on the *first* Farm's completion; wait for
    // the second one's node explicitly.)
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
    assert_eq!(farms.len(), 2);
    sim.push_command(Command::Gather {
        entity: 2,
        node: farms[0],
    });
    sim.push_command(Command::Gather {
        entity: 3,
        node: farms[1],
    });
    let archer_before = sim.stats_for(OWNER_PLAYER, EntityKind::Archer);
    assert_eq!(archer_before.range_max.to_bits(), 1_048_576); // 16 (4 tiles)
    let militia_base = sim.stats_for(OWNER_PLAYER, EntityKind::Militia);
    // Archer pierce 4 vs the Militia's 1 pierce armor = 3.
    assert_eq!(compute_damage(archer_before.attacks, &militia_base), 3);

    let research = |sim: &mut Sim, tech: TechId| {
        let ready = run_until(sim, 120_000, |s| s.can_research_at(smith, tech));
        assert!(ready, "{tech:?} never became affordable/available");
        sim.push_command(Command::ResearchAt {
            building: smith,
            tech,
        });
        let done = run_until(sim, 10_000, |s| s.has_tech(OWNER_PLAYER, tech));
        assert!(done, "{tech:?} never completed");
    };
    research(&mut sim, TechId::Fletching);
    let archer = sim.stats_for(OWNER_PLAYER, EntityKind::Archer);
    // +1 attack (pierce 4 → 5: damage vs the Militia's 1 pierce
    // armor goes 3 → 4) and +1 tile of range (16 → 20 units).
    assert_eq!(compute_damage(archer.attacks, &militia_base), 4);
    assert_eq!(archer.range_max.to_bits(), 1_310_720);

    research(&mut sim, TechId::ScaleMail);
    let militia = sim.stats_for(OWNER_PLAYER, EntityKind::Militia);
    assert_eq!((militia.armor_melee, militia.armor_pierce), (1, 2));

    research(&mut sim, TechId::PaddedArcherArmor);
    let archer = sim.stats_for(OWNER_PLAYER, EntityKind::Archer);
    assert_eq!((archer.armor_melee, archer.armor_pierce), (1, 1));
}

#[test]
fn man_at_arms_upgrades_existing_and_future_militia() {
    let mut sim = Sim::new(109);
    reach_feudal(&mut sim);
    // Wood for the Barracks: villager 3 chops; 1 and 2 stay on the
    // berries for the Militia and upgrade food costs.
    let tree = first_node(&sim, NodeKind::Tree);
    sim.push_command(Command::Gather {
        entity: 3,
        node: tree,
    });
    wait_wood(&mut sim, 175);
    raise(&mut sim, 1, BuildingKind::Barracks, 27, 20);
    let barracks = building_id(&sim, BuildingKind::Barracks);
    // Food for a Militia (60) and the upgrade (100): two villagers
    // back on the berries.
    let berry = first_node(&sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: berry,
    });
    let ready = run_until(&mut sim, 60_000, |s| s.can_train_at(barracks, EntityKind::Militia));
    assert!(ready, "militia never affordable");
    sim.push_command(Command::TrainAt {
        building: barracks,
        kind: EntityKind::Militia,
    });
    let trained = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Militia)
    });
    assert!(trained, "militia never trained");
    let militia_id = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Militia)
        .unwrap()
        .id;
    // Research the upgrade at the Barracks.
    let ready = run_until(&mut sim, 120_000, |s| {
        s.can_research_at(barracks, TechId::ManAtArms)
    });
    assert!(ready, "man-at-arms never became available");
    sim.push_command(Command::ResearchAt {
        building: barracks,
        tech: TechId::ManAtArms,
    });
    let done = run_until(&mut sim, 10_000, |s| {
        s.has_tech(OWNER_PLAYER, TechId::ManAtArms)
    });
    assert!(done, "man-at-arms never completed");
    // The standing Militia became a Man-at-Arms in place (CLS-03):
    // same entity, full HP carried over (it was never hurt).
    let upgraded = villager(&sim, militia_id);
    assert_eq!(upgraded.kind, EntityKind::ManAtArms);
    assert_eq!(upgraded.hp, I16F16::from_num(40));
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::ManAtArms).max_hp,
        45
    );
    // Future production uses the new tier too — with housing for
    // it first (the first Man-at-Arms filled the starting cap;
    // villager 3 has been chopping, so the wood is in hand).
    wait_wood(&mut sim, 25);
    raise(&mut sim, 3, BuildingKind::House, 18, 25); // (20, 27) has a sheep on it
    assert_eq!(sim.population_cap(), 10);
    // And food: the berries are spent by now, so villager 3 farms
    // (the Mill from reach_feudal opens the gate). It has to refill
    // the wood the House ate first — nobody is on the woodline
    // while it builds, and the tree it used before is depleted
    // (a gather order onto a dead node is rejected), so pick a
    // live one.
    let live_tree = first_live_node(&sim, NodeKind::Tree);
    sim.push_command(Command::Gather {
        entity: 3,
        node: live_tree,
    });
    wait_wood(&mut sim, 60);
    raise(&mut sim, 3, BuildingKind::Farm, 23, 24);
    let farm = first_node(&sim, NodeKind::Farm);
    sim.push_command(Command::Gather {
        entity: 3,
        node: farm,
    });
    let ready = run_until(&mut sim, 60_000, |s| s.can_train_at(barracks, EntityKind::Militia));
    assert!(ready, "second militia never affordable");
    sim.push_command(Command::TrainAt {
        building: barracks,
        kind: EntityKind::Militia,
    });
    let spawned = run_until(&mut sim, 10_000, |s| {
        s.entities()
            .iter()
            .filter(|e| e.kind == EntityKind::ManAtArms)
            .count()
            >= 2
    });
    assert!(spawned, "new production did not come out as Man-at-Arms");
}

#[test]
fn ai_match_replays_bit_identically_and_ages_up() {
    fn run(seed: u64) -> (Vec<u64>, Sim) {
        let mut sim = Sim::new_match(seed, MatchMode::AiNormal);
        let hashes = (0..60_000).map(|_| sim.tick()).collect();
        (hashes, sim)
    }
    let (a, sim_a) = run(1234);
    let (b, _) = run(1234);
    assert_eq!(a, b, "the M6 AI match diverged between identical runs");
    // The stream contains the whole new layer: the AI researched
    // Loom, stands in Feudal, and fields a completed Archery Range.
    assert!(sim_a.has_tech(OWNER_OPPONENT, TechId::Loom), "AI never researched Loom");
    assert_eq!(sim_a.age_of(OWNER_OPPONENT), Age::Feudal, "AI never aged up");
    assert_eq!(
        sim_a.completed_buildings(OWNER_OPPONENT, BuildingKind::ArcheryRange),
        1,
        "AI never raised its Archery Range"
    );
    assert_eq!(
        sim_a.completed_buildings(OWNER_OPPONENT, BuildingKind::Mill),
        1,
        "AI never raised its Mill"
    );
}
