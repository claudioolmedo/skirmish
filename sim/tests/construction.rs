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
    assert_eq!(sim.population(), 3);
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
    // Fill the starting room: 3 villagers + 2 trained = 5 = cap.
    sim.push_command(Command::TrainVillager);
    sim.push_command(Command::TrainVillager);
    let filled = run_until(&mut sim, 20_000, |s| s.villager_count() == 5);
    assert!(filled, "the first two villagers never trained");
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
            5,
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
    let spawned = run_until(&mut sim, 5_000, |s| s.villager_count() == 6);
    assert!(spawned, "stalled training did not resume after housing");
    assert_eq!(sim.train_queue_len(), 0);
}

#[test]
fn farm_completes_is_gatherable_depletes_and_frees_the_ground() {
    let mut sim = Sim::new(41);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    build(&mut sim, 1, BuildingKind::Farm, 20, 27);
    sim.tick();
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood - I16F16::from_num(60),
        "60 wood must be deducted at placement"
    );
    // A farm never blocks cells — the farmer works standing on it.
    assert!(!sim.grid().is_dynamic(20, 27));

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
    let (fx, fy) = anchor_point(&sim, 20, 27);
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
