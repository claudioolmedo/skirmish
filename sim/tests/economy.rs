//! Economy tests (M2): the gather → carry → deposit loop, node
//! depletion, villager training, and determinism of the whole system.
//!
//! These prove the M2 contract: a villager fills its carry at a node,
//! hauls it home, and the stockpile grows by exactly the carried amount;
//! nodes deplete bit-exactly; training costs 50 food and takes 1500
//! ticks; and scripted economy runs replay bit-identically.

use fixed::types::I16F16;
use skirmish_sim::{
    Command, NodeKind, ResourceKind, Sim, Task, CARRY_CAPACITY, VILLAGER_TRAIN_TICKS,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn villager(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn first_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes().iter().find(|n| n.kind == kind).unwrap().id
}

fn node_amount(sim: &Sim, id: u32) -> I16F16 {
    sim.nodes().iter().find(|n| n.id == id).unwrap().amount_left
}

#[test]
fn nodes_are_reachable_from_town_center() {
    let sim = Sim::new(3);
    let tc = sim.town_center();
    for n in sim.nodes() {
        assert!(
            sim.grid()
                .find_path((tc.pos_x, tc.pos_y), (n.pos_x, n.pos_y))
                .is_some(),
            "node {} ({:?}) is unreachable from the Town Center",
            n.id,
            n.kind
        );
        assert_eq!(
            n.amount_left,
            n.kind.full_amount(),
            "node {} does not start full",
            n.id
        );
    }
}

#[test]
fn full_gather_cycle_deposits_exact_carry() {
    let mut sim = Sim::new(7);
    let berry = first_node(&sim, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });

    // First deposit must be exactly one full carry (10 food): the berry
    // bush holds 125, so nothing partial can happen on the first cycle.
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    let mut first_deposit_at = None;
    let mut second_deposit_at = None;
    for tick in 0..20_000 {
        sim.tick();
        let food = sim.stockpile().amount(ResourceKind::Food);
        if first_deposit_at.is_none() && food > start_food {
            assert_eq!(
                food,
                start_food + CARRY_CAPACITY,
                "first deposit was not exactly one full carry"
            );
            first_deposit_at = Some(tick);
        }
        if first_deposit_at.is_some() && food >= start_food + CARRY_CAPACITY * I16F16::from_num(2)
        {
            second_deposit_at = Some(tick);
            break;
        }
    }
    assert!(first_deposit_at.is_some(), "villager never deposited");
    assert!(
        second_deposit_at.is_some(),
        "gather order did not persist into a second cycle"
    );
    // The assignment is persistent: still working the same node.
    assert_eq!(villager(&sim, 1).task_node, Some(berry));
}

#[test]
fn node_depletion_is_bit_exact_and_node_goes_inert() {
    let mut sim = Sim::new(9);
    let tree = first_node(&sim, NodeKind::Tree);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    sim.push_command(Command::Gather {
        entity: 1,
        node: tree,
    });

    // One villager works the 100-wood tree dry. Every bit taken from
    // the node must land in the stockpile: totals are conserved
    // exactly. The "finished" moment is the deposit of the tree's
    // last load: the node is dry, the carry is in the bank, and the
    // assignment has moved on.
    //
    // M4 deliberate update (ECO-09): "moved on" is no longer "idle"
    // — the villager retargets to the tree's same-resource neighbor
    // (id 1, the tree 4 units east; the tie with the tree 4 units
    // south breaks to the lower id). The conservation assertion is
    // unchanged: it is taken at the deposit tick, before any wood
    // from the *next* tree can land.
    let mut finished = false;
    for _ in 0..60_000 {
        sim.tick();
        let v = villager(&sim, 1);
        if node_amount(&sim, tree) == I16F16::ZERO
            && v.carry == I16F16::ZERO
            && v.task_node != Some(tree)
            && sim.stockpile().amount(ResourceKind::Wood) > start_wood
        {
            finished = true;
            break;
        }
    }
    assert!(finished, "villager never finished the depleted tree");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood + I16F16::from_num(100),
        "total gathered must equal the node's full amount exactly"
    );
    // ECO-09: the villager continued at the neighboring tree instead
    // of going idle (the west forest chains; see tests/input.rs for
    // the dedicated retarget scenarios).
    assert_eq!(villager(&sim, 1).task_node, Some(1));

    // A depleted node rejects new gather orders: the villager stays idle.
    sim.push_command(Command::Gather {
        entity: 2,
        node: tree,
    });
    for _ in 0..60 {
        sim.tick();
    }
    let v2 = villager(&sim, 2);
    assert_eq!(v2.task, Task::Idle);
    assert!(v2.order.is_none());
}

#[test]
fn train_villager_costs_food_and_spawns_after_1500_ticks() {
    let mut sim = Sim::new(13);
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    assert_eq!(sim.villager_count(), 3);

    sim.push_command(Command::TrainVillager);
    sim.tick(); // command applied here: cost deducted on enqueue
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        start_food - I16F16::from_num(50),
        "50 food must be deducted on enqueue"
    );
    assert_eq!(sim.train_queue_len(), 1);

    // The applying tick above already counted as training tick 1, so
    // after `VILLAGER_TRAIN_TICKS - 2` more ticks the progress stands at
    // 1499 of 1500: still no new villager…
    for _ in 0..(VILLAGER_TRAIN_TICKS - 2) {
        sim.tick();
        assert_eq!(sim.villager_count(), 3, "villager spawned early");
    }
    // …and exactly at tick 1500 from the order, it appears.
    sim.tick();
    assert_eq!(sim.villager_count(), 4, "villager did not spawn on time");
    assert_eq!(sim.train_queue_len(), 0);

    // The new villager spawns adjacent to the Town Center.
    let tc = sim.town_center();
    let (tx, ty) = (f(tc.pos_x), f(tc.pos_y));
    let new = sim.entities().last().unwrap();
    let d = ((f(new.pos_x) - tx).powi(2) + (f(new.pos_y) - ty).powi(2)).sqrt();
    assert!(d <= 6.0, "new villager spawned {d:.1} units from the TC");
}

#[test]
fn train_villager_rejected_without_food() {
    let mut sim = Sim::new(17);
    // Four villagers cost the entire 200-food start stockpile.
    for _ in 0..4 {
        sim.push_command(Command::TrainVillager);
    }
    sim.tick();
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        I16F16::ZERO,
        "four trainings must drain the 200 food"
    );
    assert_eq!(sim.train_queue_len(), 4);

    // The fifth order cannot be paid for: rejected, nothing changes.
    sim.push_command(Command::TrainVillager);
    sim.tick();
    assert_eq!(sim.train_queue_len(), 4, "unpayable order was enqueued");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Food),
        I16F16::ZERO,
        "rejected order must not deduct"
    );
}

#[test]
fn gather_and_train_replay_bit_identically() {
    // A scripted economy run: gather orders and training interleaved
    // with the ticks. Same seed + same script = identical hash stream.
    fn run(seed: u64, extra_order: bool) -> Vec<u64> {
        let (berry, tree, gold, stone) = {
            let probe = Sim::new(seed);
            (
                first_node(&probe, NodeKind::BerryBush),
                first_node(&probe, NodeKind::Tree),
                first_node(&probe, NodeKind::GoldMine),
                first_node(&probe, NodeKind::StoneMine),
            )
        };

        let mut sim = Sim::new(seed);
        let mut hashes = Vec::new();
        for tick in 0..3600 {
            match tick {
                5 => {
                    sim.push_command(Command::Gather {
                        entity: 1,
                        node: berry,
                    });
                    sim.push_command(Command::Gather {
                        entity: 2,
                        node: tree,
                    });
                }
                10 => sim.push_command(Command::TrainVillager),
                100 => sim.push_command(Command::Gather {
                    entity: 3,
                    node: gold,
                }),
                2000 => sim.push_command(Command::TrainVillager),
                3000 if extra_order => sim.push_command(Command::Gather {
                    entity: 1,
                    node: stone,
                }),
                _ => {}
            }
            hashes.push(sim.tick());
        }
        hashes
    }

    let a = run(42, false);
    let b = run(42, false);
    assert_eq!(a, b, "identical economy scripts diverged");
    // The economy actually ran during the script (sanity: training
    // completed, so entity count — part of the hash — changed midway).
    let c = run(42, true);
    assert_ne!(
        a, c,
        "an extra gather order at tick 3000 did not change the hash stream"
    );
}

#[test]
fn gather_to_wrong_kind_first_deposits_carry() {
    // Carrying wood and ordered onto gold: the villager hauls the wood
    // home first, then heads to the gold — wood lands in the stockpile,
    // never in the gold count.
    let mut sim = Sim::new(21);
    let tree = first_node(&sim, NodeKind::Tree);
    let gold = first_node(&sim, NodeKind::GoldMine);
    sim.push_command(Command::Gather {
        entity: 1,
        node: tree,
    });
    // Wait until it carries some wood, then re-task it to gold.
    let mut carrying = false;
    for _ in 0..2400 {
        sim.tick();
        let v = villager(&sim, 1);
        if v.task == Task::Gathering && v.carry > I16F16::ZERO {
            carrying = true;
            break;
        }
    }
    assert!(carrying, "villager never gathered wood");
    let wood_at_retask = sim.stockpile().amount(ResourceKind::Wood);
    sim.push_command(Command::Gather {
        entity: 1,
        node: gold,
    });
    // It must pass through Returning (deposit) before gathering gold.
    let mut saw_returning = false;
    let mut reached_gold = false;
    for _ in 0..12_000 {
        sim.tick();
        let v = villager(&sim, 1);
        if v.task == Task::Returning {
            saw_returning = true;
        }
        if saw_returning && v.task_node == Some(gold) && v.task == Task::Gathering {
            reached_gold = true;
            break;
        }
    }
    assert!(saw_returning, "villager did not haul its wood home first");
    assert!(reached_gold, "villager never started on the gold mine");
    assert!(
        sim.stockpile().amount(ResourceKind::Wood) > wood_at_retask,
        "the carried wood was never deposited"
    );
    // Whatever is in the carry now (if anything) is gold, not wood.
    let v = villager(&sim, 1);
    assert!(v.carry == I16F16::ZERO || v.carry_kind == Some(ResourceKind::Gold));
}
