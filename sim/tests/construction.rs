//! Construction tests (M2.5): Houses and Farms built by villagers,
//! sim-validated placement, population capacity and the "housed"
//! training stall, farm lifecycle (raise → gather → deplete → ground
//! freed), and determinism of scripted build runs.
//!
//! Numbers under test come from the public mechanics study
//! (MECHANICS-STUDY.md §5, §6.3, §9.4): House 25 wood / 1500 ticks /
//! +5 room; Farm 60 wood / 900 ticks / 175 food; Town Center +5 room.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, NodeKind, ResourceKind, Sim, Task, TrainBlock,
};

fn villager(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

/// A world point inside anchor cell `(cx, cy)` (its center), the way a
/// client would report a placement click.
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

// Open, verified-free anchors used across the tests (checked against
// the static layout: no walls, no nodes, not the Town Center cell):
//   house plot (18, 27) — west field, south of the berries
//   farm plot  (20, 27) — two cells east, no anchor overlap with (18, 27)

#[test]
fn house_completes_raises_cap_and_blocks_its_footprint() {
    let mut sim = Sim::new(31);
    // M3 standard start: 3 villagers + the Scout Cavalry = 4 pop.
    assert_eq!(sim.population(), 4);
    assert_eq!(sim.population_cap(), 5, "Town Center alone gives +5");

    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    sim.tick(); // placement applied here
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood - I16F16::from_num(25),
        "25 wood must be deducted at placement"
    );
    assert_eq!(sim.buildings().len(), 1);
    let house = &sim.buildings()[0];
    assert_eq!(house.kind, BuildingKind::House);
    assert!(!house.complete);
    assert_eq!(
        sim.population_cap(),
        5,
        "an unfinished house gives no room yet"
    );
    // The footprint cells are blocked for pathing immediately.
    for (cx, cy) in [(18, 27), (19, 27), (18, 28), (19, 28)] {
        assert!(
            sim.grid().is_dynamic(cx, cy),
            "house cell ({cx},{cy}) not blocked"
        );
    }
    // A path across the plot must detour around the footprint.
    let from = sim.grid().cell_center(16, 27);
    let to = sim.grid().cell_center(22, 27);
    let path = sim
        .grid()
        .find_path(from, to)
        .expect("a detour around the house must exist");
    for &(wx, wy) in &path {
        let cell = sim.grid().world_to_cell(wx, wy);
        assert!(
            ![(18, 27), (19, 27), (18, 28), (19, 28)].contains(&cell),
            "path crosses the house footprint at {cell:?}"
        );
    }

    // The builder travels, builds (1500 ticks of active work), finishes.
    let done = run_until(&mut sim, 20_000, |s| s.buildings()[0].complete);
    assert!(done, "house never completed");
    assert_eq!(sim.population_cap(), 10, "completed house must add +5 room");
    // The builder's job is done: it stands down.
    let v = villager(&sim, 1);
    assert_eq!(v.task, Task::Idle);
    assert_eq!(v.task_building, None);
}

#[test]
fn training_stalls_at_cap_and_resumes_after_a_house() {
    let mut sim = Sim::new(37);
    // Fill the starting room (M3 start: 3 villagers + 1 Scout = 4;
    // one trained villager fills it to the cap of 5).
    sim.push_command(Command::TrainVillager);
    let filled = run_until(&mut sim, 20_000, |s| s.villager_count() == 4);
    assert!(filled, "the first villager never trained");
    assert_eq!(sim.population(), 5);
    assert_eq!(sim.train_queue_len(), 0);
    assert_eq!(sim.train_block_reason(), TrainBlock::Housed);

    // One more order enqueues (food is fine) but cannot *finish*:
    // training progress is stalled while housed.
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.train_queue_len(), 1);
    for _ in 0..2500 {
        sim.tick();
        assert_eq!(
            sim.villager_count(),
            4,
            "a villager completed training while housed"
        );
        assert_eq!(sim.train_queue_len(), 1, "stalled queue must keep its place");
    }
    assert_eq!(sim.train_block_reason(), TrainBlock::Housed);

    // Build a house: capacity rises to 10 and the stalled training
    // resumes on its own, no new order needed.
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    let housed_no_more = run_until(&mut sim, 20_000, |s| {
        s.buildings().iter().any(|b| b.complete)
    });
    assert!(housed_no_more, "house never completed");
    assert_eq!(sim.population_cap(), 10);
    let spawned = run_until(&mut sim, 5_000, |s| s.villager_count() == 5);
    assert!(spawned, "stalled training did not resume after housing");
    assert_eq!(sim.train_queue_len(), 0);
}

#[test]
fn farm_completes_is_gatherable_depletes_and_frees_the_ground() {
    let mut sim = Sim::new(41);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    // M4 note: the plot sits east of the wall, far from every other
    // food node — when the field dries, ECO-09 retargeting must find
    // nothing within its radius and the farmer goes idle, which is
    // the lifecycle this test pins. (At the old spot by the berries
    // the farmer would — correctly, per ECO-09 — walk to the berry
    // patch and keep working.)
    build(&mut sim, 1, BuildingKind::Farm, 28, 20);
    sim.tick();
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood - I16F16::from_num(60),
        "60 wood must be deducted at placement"
    );
    // A farm never blocks cells — the farmer works standing on it.
    assert!(!sim.grid().is_dynamic(28, 20));

    let raised = run_until(&mut sim, 20_000, |s| {
        s.nodes().iter().any(|n| n.kind == NodeKind::Farm)
    });
    assert!(raised, "farm never became a node");
    let farm = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Farm)
        .unwrap();
    assert_eq!(
        farm.amount_left,
        I16F16::from_num(175),
        "a fresh farm holds exactly 175 food (study §6.3)"
    );
    let farm_id = farm.id;
    // The builder starts farming its own field unprompted.
    let v = villager(&sim, 1);
    assert_eq!(v.task_node, Some(farm_id), "builder did not start farming");

    // Work it dry: every bit of the 175 must land in the stockpile.
    let dry = run_until(&mut sim, 400_000, |s| {
        let n = s.nodes().iter().find(|n| n.id == farm_id).unwrap();
        let v = s.entities().iter().find(|e| e.id == 1).unwrap();
        n.amount_left == I16F16::ZERO && v.carry == I16F16::ZERO && v.task == Task::Idle
    });
    assert!(dry, "farm never depleted");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        start_food + I16F16::from_num(175),
        "total farmed food must equal the farm's contents exactly"
    );
    // The building record retires with the node: the ground is free.
    assert!(
        sim.buildings().iter().all(|b| b.node_id != Some(farm_id)),
        "depleted farm's building record did not retire"
    );
    let (fx, fy) = anchor_point(&sim, 28, 20);
    assert!(
        sim.can_build(2, BuildingKind::Farm, fx, fy),
        "a new farm cannot be placed on the freed ground"
    );
}

#[test]
fn invalid_placements_are_rejected_whole() {
    let mut sim = Sim::new(43);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);

    // (a) on a wall cell, (b) on a berry node, (c) on the Town Center,
    // (d) hanging off the map edge.
    for (cx, cy) in [(25, 10), (21, 24), (23, 23), (49, 49)] {
        build(&mut sim, 1, BuildingKind::House, cx, cy);
        sim.tick();
        assert_eq!(
            sim.buildings().len(),
            0,
            "placement at ({cx},{cy}) should have been rejected"
        );
    }
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood,
        "rejected placements must not deduct"
    );

    // (e) overlapping a standing building.
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1);
    build(&mut sim, 2, BuildingKind::House, 19, 28);
    sim.tick();
    assert_eq!(
        sim.buildings().len(),
        1,
        "overlapping placement was accepted"
    );
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood - I16F16::from_num(25),
        "only the valid placement may deduct"
    );

    // (f) the Town Center cannot build, and neither can a ghost.
    let (x, y) = anchor_point(&sim, 30, 40);
    sim.push_command(Command::Build {
        entity: 0,
        kind: BuildingKind::House,
        x,
        y,
    });
    sim.push_command(Command::Build {
        entity: 999,
        kind: BuildingKind::House,
        x,
        y,
    });
    sim.tick();
    assert_eq!(sim.buildings().len(), 1);
}

#[test]
fn unaffordable_placement_is_rejected() {
    let mut sim = Sim::new(47);
    // Three farms = 180 of the 200 starting wood; the fourth cannot
    // be paid for and must be rejected without deduction.
    for (i, (cx, cy)) in [(18, 20), (27, 20), (33, 27)].iter().enumerate() {
        build(&mut sim, 1 + (i as u32) % 3, BuildingKind::Farm, *cx, *cy);
    }
    sim.tick();
    assert_eq!(sim.buildings().len(), 3);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        I16F16::from_num(20)
    );
    build(&mut sim, 1, BuildingKind::Farm, 38, 33);
    sim.tick();
    assert_eq!(sim.buildings().len(), 3, "unpayable farm was placed");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        I16F16::from_num(20),
        "rejected placement deducted wood"
    );
    let (fx, fy) = anchor_point(&sim, 38, 33);
    assert!(!sim.can_build(1, BuildingKind::Farm, fx, fy));
}

#[test]
fn a_second_builder_resumes_an_unfinished_building() {
    let mut sim = Sim::new(53);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    sim.tick();
    // Villager 2 is ordered onto the same anchor: it must resume the
    // standing construction, not place (or pay for) a second house.
    build(&mut sim, 2, BuildingKind::House, 18, 27);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "resume placed a duplicate");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood - I16F16::from_num(25),
        "resume must not charge again"
    );
    let done = run_until(&mut sim, 20_000, |s| s.buildings()[0].complete);
    assert!(done, "house with two builders never completed");
    assert_eq!(sim.population_cap(), 10);
}

#[test]
fn build_script_replays_bit_identically() {
    fn run(seed: u64, extra_build: bool) -> Vec<u64> {
        let mut sim = Sim::new(seed);
        let mut hashes = Vec::new();
        for tick in 0..9000u64 {
            match tick {
                5 => {
                    let (x, y) = anchor_point(&sim, 18, 27);
                    sim.push_command(Command::Build {
                        entity: 1,
                        kind: BuildingKind::House,
                        x,
                        y,
                    });
                }
                10 => {
                    let (x, y) = anchor_point(&sim, 20, 27);
                    sim.push_command(Command::Build {
                        entity: 2,
                        kind: BuildingKind::Farm,
                        x,
                        y,
                    });
                }
                20 => sim.push_command(Command::TrainVillager),
                5000 if extra_build => {
                    let (x, y) = anchor_point(&sim, 27, 33);
                    sim.push_command(Command::Build {
                        entity: 3,
                        kind: BuildingKind::House,
                        x,
                        y,
                    });
                }
                _ => {}
            }
            hashes.push(sim.tick());
        }
        hashes
    }

    let a = run(42, false);
    let b = run(42, false);
    assert_eq!(a, b, "identical build scripts diverged");
    let c = run(42, true);
    assert_ne!(a, c, "an extra build order did not change the hash stream");
    // Sanity: the script really exercised the systems (house done, farm
    // raised, training completed).
    let mut probe = Sim::new(42);
    let (hx, hy) = anchor_point(&probe, 18, 27);
    probe.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    let done = run_until(&mut probe, 20_000, |s| {
        s.buildings().iter().any(|b| b.complete)
    });
    assert!(done);
    assert_eq!(probe.population_cap(), 10);
}

/// Tick until the first building completes, asserting the exact
/// accrual invariant on every tick: progress grows by precisely
/// (n + 2) thirds, where n is the number of the given builders
/// observed in the `Building` phase on that building after the
/// tick — the AoE2 rate (n+2)/3 applied to whoever is actually
/// building, tick by tick. (A builder can briefly drop back to
/// `ToBuild` when a colleague's arrival shoves it out of build
/// range; the invariant follows the count, so such ticks accrue
/// at the lower rate.) Returns (ticks used, ticks with the full
/// listed crew observed building — the completing tick is not
/// counted, since completion itself resets the builders' tasks
/// before they can be observed).
fn tick_building(sim: &mut Sim, builders: &[u32]) -> (u64, u64) {
    let bid = sim.buildings()[0].id;
    let mut ticks = 0;
    let mut full_crew = 0;
    loop {
        let before = sim.buildings()[0].progress_thirds;
        sim.tick();
        ticks += 1;
        assert!(ticks < 20_000, "building never completed");
        if sim.buildings()[0].complete {
            return (ticks, full_crew);
        }
        let n = builders
            .iter()
            .filter(|&&id| {
                let v = villager(sim, id);
                v.task == Task::Building && v.task_building == Some(bid)
            })
            .count() as u64;
        if n == builders.len() as u64 {
            full_crew += 1;
        }
        let expected = if n == 0 { 0 } else { n + 2 };
        assert_eq!(
            sim.buildings()[0].progress_thirds - before,
            expected,
            "progress did not accrue at the AoE2 rate for {n} builder(s)"
        );
    }
}

#[test]
fn solo_house_takes_exactly_1500_ticks_of_work() {
    let mut sim = Sim::new(59);
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    sim.tick(); // placement applied here
    let started = run_until(&mut sim, 20_000, |s| villager(s, 1).task == Task::Building);
    assert!(started, "the builder never started building");
    // The arrival tick itself already accrued one solo tick of work:
    // 3 thirds. The remaining 4497 thirds take exactly 1499 ticks —
    // 1500 ticks of active building in total, unchanged by the
    // multi-builder formula (rate (1+2)/3 = 1).
    assert_eq!(sim.buildings()[0].progress_thirds, 3);
    let (ticks, full_crew) = tick_building(&mut sim, &[1]);
    assert_eq!(ticks, 1499, "solo house must take exactly 1500 ticks of work");
    // Every observable tick had the builder on site (the completing
    // tick itself is unobservable — see tick_building).
    assert_eq!(full_crew, ticks - 1, "the solo builder was interrupted");
    assert_eq!(sim.population_cap(), 10);
}

#[test]
fn two_builders_finish_a_house_in_about_1125_ticks() {
    let mut sim = Sim::new(67);
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    sim.tick();
    build(&mut sim, 2, BuildingKind::House, 18, 27); // resume: second builder
    sim.tick();
    assert_eq!(sim.buildings().len(), 1);

    let both = run_until(&mut sim, 20_000, |s| {
        villager(s, 1).task == Task::Building && villager(s, 2).task == Task::Building
    });
    assert!(both, "the two builders never worked side by side");
    assert!(!sim.buildings()[0].complete);
    // tick_building pins the per-tick rate exactly; here we assert
    // the aggregate: the two builders share essentially the whole
    // build (a couple of jostle ticks aside), so the house stands
    // after ≈1125 ticks of work plus the walk to the plot — the
    // ≈19 s two-builder house the opening depends on, not the
    // 750 ticks linear stacking (T/2) would give.
    let (ticks, full_crew) = tick_building(&mut sim, &[1, 2]);
    assert!(
        full_crew >= 1000,
        "the builders shared only {full_crew} ticks of the build"
    );
    assert!(
        ticks - full_crew <= 10,
        "a builder was shoved out for {} ticks",
        ticks - full_crew
    );
    let total = sim.tick_count();
    assert!(
        (1125..=1125 + 150).contains(&total),
        "house completed at tick {total}, expected ≈1125 + travel"
    );
    assert_eq!(sim.population_cap(), 10);
}

#[test]
fn a_builder_joining_mid_build_changes_the_pace() {
    let mut sim = Sim::new(61);
    build(&mut sim, 1, BuildingKind::House, 18, 27);
    sim.tick();
    let started = run_until(&mut sim, 20_000, |s| villager(s, 1).task == Task::Building);
    assert!(started, "the builder never started building");

    // Solo pace: exactly 3 thirds per tick.
    let p0 = sim.buildings()[0].progress_thirds;
    for _ in 0..100 {
        sim.tick();
    }
    assert_eq!(
        sim.buildings()[0].progress_thirds,
        p0 + 300,
        "solo accrual is not exactly 1 tick of work per tick"
    );
    // Work past halfway solo (2250 of 4500 thirds).
    let half = run_until(&mut sim, 20_000, |s| s.buildings()[0].progress_thirds >= 2250);
    assert!(half);
    assert!(!sim.buildings()[0].complete);

    // The second builder joins the standing foundation mid-build.
    build(&mut sim, 2, BuildingKind::House, 18, 27);
    let joined = run_until(&mut sim, 20_000, |s| villager(s, 2).task == Task::Building);
    assert!(joined, "the second builder never joined the build");
    assert!(!sim.buildings()[0].complete);

    // From the join, the per-tick invariant runs at the two-builder
    // rate (pinned inside tick_building), and the remaining work
    // finishes well ahead of the solo projection for the same
    // remainder (remaining thirds / 3 ticks).
    let remaining = BuildingKind::House.build_thirds() - sim.buildings()[0].progress_thirds;
    let (ticks, full_crew) = tick_building(&mut sim, &[1, 2]);
    assert!(
        ticks < remaining / 3,
        "joining mid-build did not speed up the remaining work: {ticks} ticks vs a {}-tick solo projection",
        remaining / 3
    );
    assert!(
        ticks - full_crew <= 10,
        "a builder was shoved out for {} ticks",
        ticks - full_crew
    );
    assert_eq!(sim.population_cap(), 10);
}
