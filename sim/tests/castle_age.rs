//! Castle Age P1 tests (2026-10-22): the Castle Age advancement
//! (two Feudal-Age building requirement, 800 food / 200 gold,
//! exactly 160 s), the BLD-11 Castle step for building HP/armor,
//! Hand Cart (ECO-03's second link: Wheelbarrow prerequisite,
//! 300 food / 200 wood, 55 s, carry 13 → 20 / hunt 44 → 66 per
//! the wiki Wheelbarrow and Hand Cart comparison tables, pinned
//! 2026-10-22 — no 13 × 1.5), and buildable Town Centers
//! (275 wood + 100 stone, Castle Age only, BLD-15/BLD-16).
//!
//! Values: MECHANICS-STUDY §5/§7, the Castle Age dossier
//! (~/workspace/skirmish-study/castle-age/), RULEBOOK TEC-01/02/
//! 04/07, ECO-03, BLD-11/15/16.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, EntityKind, NodeKind, ResearchBlock, ResourceKind, Sim, Task,
    TechId, OWNER_PLAYER,
};

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

fn first_live_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .find(|n| n.kind == kind && n.amount_left > I16F16::from_num(0))
        .unwrap()
        .id
}

fn stock(sim: &Sim, kind: ResourceKind) -> I16F16 {
    sim.stockpile().amount(kind)
}

fn wait_stock(sim: &mut Sim, kind: ResourceKind, want: i32) {
    let ok = run_until(sim, 400_000, |s| stock(s, kind) >= I16F16::from_num(want));
    assert!(ok, "never stocked {want} {kind:?}");
}

/// A woodline tree for sustained chopping (same picker as the
/// tech suite: a forest-edge tree with reachable standing room
/// and chained neighbors).
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

fn raise(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    build(sim, entity, kind, cx, cy);
    let up = run_until(sim, 30_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == kind && b.complete)
    });
    assert!(up, "{kind:?} never completed");
}

fn complete_count(sim: &Sim, kind: BuildingKind) -> usize {
    sim.buildings()
        .iter()
        .filter(|b| b.kind == kind && b.complete)
        .count()
}

/// Raise `kind` at the first valid anchor on expanding rings
/// around the Town Center. Returns the new building's id.
fn raise_scan(sim: &mut Sim, entity: u32, kind: BuildingKind) -> u32 {
    let before_ids: Vec<u32> = sim
        .buildings()
        .iter()
        .filter(|b| b.kind == kind)
        .map(|b| b.id)
        .collect();
    let before = complete_count(sim, kind);
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    for r in 2i32..=20 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (cx, cy) = (tcx as i32 + dx, tcy as i32 + dy);
                if cx < 0 || cy < 0 {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                let (x, y) = anchor_point(sim, cx, cy);
                if !sim.can_build(entity, kind, x, y) {
                    continue;
                }
                build(sim, entity, kind, cx, cy);
                let up = run_until(sim, 90_000, |s| complete_count(s, kind) > before);
                assert!(up, "{kind:?} placed at ({cx}, {cy}) never completed");
                return sim
                    .buildings()
                    .iter()
                    .find(|b| b.kind == kind && b.complete && !before_ids.contains(&b.id))
                    .unwrap()
                    .id;
            }
        }
    }
    panic!("no valid anchor for {kind:?}");
}

/// The standard Feudal play-up (the tech suite's helper): Mill +
/// Lumber Camp, berries to 500, click up.
fn reach_feudal(sim: &mut Sim) {
    raise(sim, 2, BuildingKind::Mill, 21, 20);
    raise(sim, 3, BuildingKind::LumberCamp, 16, 20);
    let berry = first_node(sim, NodeKind::BerryBush);
    for id in [1, 2] {
        sim.push_command(Command::Gather {
            entity: id,
            node: berry,
        });
    }
    let food = run_until(sim, 120_000, |s| stock(s, ResourceKind::Food) >= I16F16::from_num(500));
    assert!(food, "never gathered the Feudal food");
    sim.push_command(Command::ResearchTc {
        tech: TechId::FeudalAge,
    });
    let up = run_until(sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");
}

fn my_villagers(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER && !e.dead)
        .map(|e| e.id)
        .collect()
}

fn gather(sim: &mut Sim, entity: u32, node: u32) {
    sim.push_command(Command::Gather { entity, node });
}

/// From a fresh sim: Feudal, six villagers, the three Feudal-born
/// buildings standing (Castle count 2), farms laid, and a stock
/// ready for the Castle click (food and gold banked past it).
fn reach_castle_ready(sim: &mut Sim) {
    reach_feudal(sim);
    // Wood first: the camps consumed the whole starting stock.
    let tree = woodline_tree(sim);
    gather(sim, 3, tree);
    wait_stock(sim, ResourceKind::Wood, 30);
    raise_scan(sim, 3, BuildingKind::House);
    // Grow to six villagers while the berries last.
    for _ in 0..3 {
        sim.push_command(Command::TrainVillager);
    }
    let six = run_until(sim, 120_000, |s| my_villagers(s).len() >= 6);
    assert!(six, "never trained up to six villagers");
    let vils = my_villagers(sim);
    // vils[0..2] are on berries (reach_feudal); the rest chop.
    for &id in &vils[2..] {
        gather(sim, id, tree);
    }
    // The Feudal-born trio, one at a time as the wood lands.
    wait_stock(sim, ResourceKind::Wood, 200);
    raise_scan(sim, vils[2], BuildingKind::Barracks);
    wait_stock(sim, ResourceKind::Wood, 175);
    raise_scan(sim, vils[2], BuildingKind::Blacksmith);
    wait_stock(sim, ResourceKind::Wood, 200);
    raise_scan(sim, vils[2], BuildingKind::ArcheryRange);
    assert_eq!(sim.castle_building_count(OWNER_PLAYER), 2);
    // Farms: the builders start farming their own fields.
    //
    // Scenario repair (2026-10-22, the P1 landing diagnosis):
    // this loop originally laid FOUR farms and the food bank
    // below starved at 1300 — measured on the state dump, the
    // berry bushes are fully eaten by the trio phase (the berry
    // crew is never pulled off; it simply runs out, berry_left
    // = 0 with farm_left = 0 and the stock frozen ~1090), so
    // the bank had to come from farm food alone and 4 × 175 =
    // 700 could not carry it. Six farms is the DE boom answer:
    // 6 × 175 = 1050 of farm food behind the same waits, with
    // the berry deposits made during the build phases (~400)
    // as headroom. The waits themselves are unchanged.
    for i in 0..6 {
        wait_stock(sim, ResourceKind::Wood, 85);
        raise_scan(sim, vils[i % vils.len()], BuildingKind::Farm);
    }
    // Two hands to the gold mine; the Castle click needs 200.
    let gold = first_live_node(sim, NodeKind::GoldMine);
    gather(sim, vils[4], gold);
    gather(sim, vils[5], gold);
    wait_stock(sim, ResourceKind::Gold, 210);
    // Food past the click: the farms alone (the berries are
    // gone — see above). The miners go back to THEIR farms —
    // the 5th and 6th laid, in creation order — for the food
    // bank, then to the woodline for the wood bank.
    let farm_nodes: Vec<u32> = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Farm)
        .map(|n| n.id)
        .collect();
    gather(sim, vils[4], farm_nodes[4]);
    gather(sim, vils[5], farm_nodes[5]);
    wait_stock(sim, ResourceKind::Food, 1300);
    let tree = woodline_tree(sim);
    gather(sim, vils[4], tree);
    gather(sim, vils[5], tree);
    wait_stock(sim, ResourceKind::Wood, 500);
}

fn park_all(sim: &mut Sim) {
    for id in my_villagers(sim) {
        sim.push_command(Command::Move {
            entity: id,
            x: I16F16::from_num(30),
            y: I16F16::from_num(-30),
        });
    }
    let idle = run_until(sim, 10_000, |s| {
        my_villagers(s)
            .iter()
            .all(|&id| villager(s, id).task == Task::Idle)
    });
    assert!(idle, "villagers never parked");
}

#[test]
fn castle_age_gates_cost_timing_and_the_bld11_step() {
    let mut sim = Sim::new(61);
    reach_feudal(&mut sim);

    // Mill + Lumber Camp are Dark-born: the Castle count is 0
    // and the advancement is Locked behind its requirement.
    assert_eq!(sim.castle_building_count(OWNER_PLAYER), 0);
    assert_eq!(
        sim.research_block_tc(TechId::CastleAge),
        ResearchBlock::Locked
    );
    assert!(!sim.can_research_tc(TechId::CastleAge));

    // The Town Center's armor at Feudal (study §5): 4 / 6.
    let tc_stats = sim.stats_for(OWNER_PLAYER, EntityKind::TownCenter);
    assert_eq!((tc_stats.armor_melee, tc_stats.armor_pierce), (4, 6));

    // A House raised in Feudal stands at the Feudal ceiling.
    let tree = woodline_tree(&sim);
    gather(&mut sim, 3, tree);
    wait_stock(&mut sim, ResourceKind::Wood, 30);
    let house_id = raise_scan(&mut sim, 3, BuildingKind::House);
    let house = sim
        .buildings()
        .iter()
        .find(|b| b.id == house_id)
        .unwrap()
        .clone();
    assert_eq!(house.hp_frac_bits(Age::Feudal), 65536, "full at its Age");
    assert_eq!(
        house.hp_frac_bits(Age::Castle),
        54613,
        "the same 750 HP read against the Castle ceiling of 900"
    );
    // The Castle columns themselves (BLD-11, study §5).
    assert_eq!(BuildingKind::House.max_hp_at(Age::Castle), 900);
    assert_eq!(BuildingKind::Barracks.max_hp_at(Age::Castle), 1800);
    assert_eq!(BuildingKind::ArcheryRange.max_hp_at(Age::Castle), 1800);
    assert_eq!(BuildingKind::Stable.max_hp_at(Age::Castle), 1800);
    assert_eq!(BuildingKind::House.armor_melee_at(Age::Castle), 1);
    assert_eq!(BuildingKind::House.armor_pierce_at(Age::Castle), 9);
    assert_eq!(BuildingKind::Barracks.armor_melee_at(Age::Castle), 2);

    // The Barracks is Dark-born too: still no Castle progress.
    for id in [1, 2, 3] {
        gather(&mut sim, id, tree);
    }
    wait_stock(&mut sim, ResourceKind::Wood, 200);
    raise_scan(&mut sim, 3, BuildingKind::Barracks);
    assert_eq!(sim.castle_building_count(OWNER_PLAYER), 0);
    assert_eq!(
        sim.research_block_tc(TechId::CastleAge),
        ResearchBlock::Locked
    );

    // One Feudal-born building: count 1, still Locked.
    wait_stock(&mut sim, ResourceKind::Wood, 175);
    raise_scan(&mut sim, 3, BuildingKind::Blacksmith);
    assert_eq!(sim.castle_building_count(OWNER_PLAYER), 1);
    assert_eq!(
        sim.research_block_tc(TechId::CastleAge),
        ResearchBlock::Locked
    );

    // The second Feudal-born building opens the gate.
    wait_stock(&mut sim, ResourceKind::Wood, 200);
    raise_scan(&mut sim, 3, BuildingKind::ArcheryRange);
    assert_eq!(sim.castle_building_count(OWNER_PLAYER), 2);
    assert_eq!(
        sim.research_block_tc(TechId::CastleAge),
        ResearchBlock::NoFunds
    );

    // Stock the click: farms for food, two hands on gold, then
    // everyone back to food (one on the berry tail, two farming).
    //
    // Scenario repair (2026-10-22, the P1 landing diagnosis): the
    // serial builder (vils[2]) has been Idle since the Archery
    // Range — a Build order clears the builder's gather
    // assignment, and nothing re-tasked him. The farm loop then
    // pulls BOTH woodcutters onto farms 1 and 2, so the third
    // wait_stock(85) has zero wood income and starves forever
    // (proven by state dump: wood frozen at 25, all three
    // villagers Idle, farms 1–2 depleted at exactly 2 × 175
    // food). A DE player never lets the woodline hit zero while
    // banking farm wood: the builder goes back to the trees
    // first, and chops through every wait below. The woodline
    // is re-picked, not reused: the original `tree` handle is
    // depleted by now, and a Gather order on a depleted node
    // is rejected whole (econ.rs) — the player clicks the
    // standing forest the crew has chained to, not the stump.
    let vils = my_villagers(&sim);
    let live_tree = woodline_tree(&sim);
    gather(&mut sim, vils[2], live_tree);
    for i in 0..3 {
        wait_stock(&mut sim, ResourceKind::Wood, 85);
        raise_scan(&mut sim, vils[i], BuildingKind::Farm);
    }
    let gold = first_live_node(&sim, NodeKind::GoldMine);
    gather(&mut sim, vils[0], gold);
    gather(&mut sim, vils[1], gold);
    wait_stock(&mut sim, ResourceKind::Gold, 210);
    let berry = first_live_node(&sim, NodeKind::BerryBush);
    let farm_node = first_live_node(&sim, NodeKind::Farm);
    gather(&mut sim, vils[0], berry);
    gather(&mut sim, vils[1], farm_node);
    wait_stock(&mut sim, ResourceKind::Food, 820);

    // Exact charge and exact timing (800 F / 200 G, 9600 ticks).
    park_all(&mut sim);
    let (food0, gold0) = (stock(&sim, ResourceKind::Food), stock(&sim, ResourceKind::Gold));
    assert!(sim.can_research_tc(TechId::CastleAge));
    let t0 = sim.tick_count();
    sim.push_command(Command::ResearchTc {
        tech: TechId::CastleAge,
    });
    sim.tick();
    assert_eq!(sim.tc_queue().len(), 1);
    assert_eq!(stock(&sim, ResourceKind::Food), food0 - I16F16::from_num(800));
    assert_eq!(stock(&sim, ResourceKind::Gold), gold0 - I16F16::from_num(200));
    for _ in 0..9598 {
        sim.tick();
    }
    assert_eq!(sim.age_of(OWNER_PLAYER), Age::Feudal, "not yet Castle");
    sim.tick();
    assert_eq!(sim.tick_count() - t0, 9600);
    assert_eq!(sim.age_of(OWNER_PLAYER), Age::Castle);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::CastleAge));
    assert!(sim.tech_mask_of(OWNER_PLAYER) & TechId::CastleAge.bit() != 0);
    assert_eq!(
        sim.research_block_tc(TechId::CastleAge),
        ResearchBlock::AlreadyDone
    );

    // The BLD-11 Castle step lands on the Town Center itself.
    let tc_stats = sim.stats_for(OWNER_PLAYER, EntityKind::TownCenter);
    assert_eq!((tc_stats.armor_melee, tc_stats.armor_pierce), (5, 7));
}

#[test]
fn hand_cart_chain_carry_and_speed() {
    let mut sim = Sim::new(61);
    reach_castle_ready(&mut sim);
    sim.push_command(Command::ResearchTc {
        tech: TechId::CastleAge,
    });
    let up = run_until(&mut sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Castle);
    assert!(up, "never reached Castle");

    // Hand Cart is Locked until Wheelbarrow stands — the game's
    // first tech-to-tech prerequisite (TEC-07).
    assert_eq!(
        sim.research_block_tc(TechId::HandCart),
        ResearchBlock::Locked
    );
    assert!(!sim.can_research_tc(TechId::HandCart));

    sim.push_command(Command::ResearchTc {
        tech: TechId::Wheelbarrow,
    });
    let wb = run_until(&mut sim, 30_000, |s| s.has_tech(OWNER_PLAYER, TechId::Wheelbarrow));
    assert!(wb, "Wheelbarrow never landed");
    let speed_wb = sim
        .stats_for(OWNER_PLAYER, EntityKind::Villager)
        .speed
        .to_bits();

    // Exact charge (300 F / 200 W) and exact timing (3300 ticks).
    park_all(&mut sim);
    let (food0, wood0) = (stock(&sim, ResourceKind::Food), stock(&sim, ResourceKind::Wood));
    assert!(food0 >= I16F16::from_num(300) && wood0 >= I16F16::from_num(200));
    let t0 = sim.tick_count();
    sim.push_command(Command::ResearchTc {
        tech: TechId::HandCart,
    });
    sim.tick();
    assert_eq!(stock(&sim, ResourceKind::Food), food0 - I16F16::from_num(300));
    assert_eq!(stock(&sim, ResourceKind::Wood), wood0 - I16F16::from_num(200));
    for _ in 0..3298 {
        sim.tick();
    }
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::HandCart), "not yet");
    sim.tick();
    assert_eq!(sim.tick_count() - t0, 3300);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::HandCart));

    // The pinned carry integers (wiki comparison tables): the
    // generic capacity goes 10 → 13 → 20 — NOT 13 × 1.5 — and
    // hunted meat 35 → 44 → 66.
    assert_eq!(
        NodeKind::Tree.carry_cap_tiered(true, true),
        I16F16::from_num(20)
    );
    assert_eq!(
        NodeKind::CarcassHunt.carry_cap_tiered(true, true),
        I16F16::from_num(66)
    );
    assert_eq!(
        NodeKind::Tree.carry_cap_tiered(true, false),
        I16F16::from_num(13)
    );
    assert_eq!(
        NodeKind::CarcassHunt.carry_cap_tiered(true, false),
        I16F16::from_num(44)
    );

    // Speed: the second +10% layer over the Wheelbarrow layer,
    // in exact bits.
    let expect = ((speed_wb as i64 * 11) / 10) as i32;
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Villager)
            .speed
            .to_bits(),
        expect
    );

    // Functional proof: a chopping villager now fills past the
    // old Wheelbarrow cap of 13, up to the new 20.
    let tree = woodline_tree(&sim);
    gather(&mut sim, 1, tree);
    let full = run_until(&mut sim, 120_000, |s| {
        villager(s, 1).carry > I16F16::from_num(13)
    });
    assert!(full, "villager never carried past 13 with Hand Cart");
    assert!(villager(&sim, 1).carry <= I16F16::from_num(20));
}

#[test]
fn town_center_buildable_in_castle_age() {
    let mut sim = Sim::new(61);
    reach_castle_ready(&mut sim);

    // Feudal: the build order is refused whole — no foundation,
    // no spend — even with wood and stone banked.
    let (wood0, stone0) = (stock(&sim, ResourceKind::Wood), stock(&sim, ResourceKind::Stone));
    assert!(stone0 >= I16F16::from_num(100));
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    build(&mut sim, 1, BuildingKind::TownCenter, tcx + 6, tcy);
    for _ in 0..5 {
        sim.tick();
    }
    assert_eq!(complete_count(&sim, BuildingKind::TownCenter), 0);
    assert!(sim
        .buildings()
        .iter()
        .all(|b| b.kind != BuildingKind::TownCenter));
    assert_eq!(stock(&sim, ResourceKind::Wood), wood0);
    assert_eq!(stock(&sim, ResourceKind::Stone), stone0);

    // Click up.
    sim.push_command(Command::ResearchTc {
        tech: TechId::CastleAge,
    });
    let up = run_until(&mut sim, 30_000, |s| s.age_of(OWNER_PLAYER) == Age::Castle);
    assert!(up, "never reached Castle");

    // Find a free 4×4 anchor and place: the spend is exactly
    // 275 wood + 100 stone on the applying tick.
    let mut anchor = None;
    'scan: for r in 2i32..=20 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (cx, cy) = (tcx as i32 + dx, tcy as i32 + dy);
                if cx < 0 || cy < 0 {
                    continue;
                }
                let (x, y) = anchor_point(&sim, cx as u32, cy as u32);
                if sim.can_build(1, BuildingKind::TownCenter, x, y) {
                    anchor = Some((cx as u32, cy as u32));
                    break 'scan;
                }
            }
        }
    }
    let (ax, ay) = anchor.expect("no free 4x4 anchor for the second TC");
    let (wood0, stone0) = (stock(&sim, ResourceKind::Wood), stock(&sim, ResourceKind::Stone));
    build(&mut sim, 1, BuildingKind::TownCenter, ax, ay);
    sim.tick();
    assert_eq!(stock(&sim, ResourceKind::Wood), wood0 - I16F16::from_num(275));
    assert_eq!(stock(&sim, ResourceKind::Stone), stone0 - I16F16::from_num(100));
    assert!(sim
        .buildings()
        .iter()
        .any(|b| b.kind == BuildingKind::TownCenter && !b.complete));

    // BLD-16: the standing foundation's ground is spoken for —
    // a second Town Center cannot anchor on the same cells.
    //
    // Assertion repair (2026-10-22, the P1 landing diagnosis —
    // this line was unreachable while the scenario starved
    // upstream, and as written it misreads the API): an order
    // by the SAME owner at the foundation's own anchor is not
    // a placement but a resume (SEL-11) — `can_build` rightly
    // accepts it, the villager is sent to work the standing
    // foundation. Pin that first so the refusal below cannot
    // be a resume in disguise.
    let (x, y) = anchor_point(&sim, ax, ay);
    assert!(
        sim.can_build(2, BuildingKind::TownCenter, x, y),
        "the same-owner order at the anchor is a resume (SEL-11)"
    );
    // The refusal itself: a FRESH placement whose 4x4 overlaps
    // the foundation, anchored one cell off the footprint (so
    // no resume applies), is rejected whole — a TC foundation
    // blocks its cells like any building under construction
    // (blocks_footprint), even though the converted TC entity
    // never will (BLD-16 pass-through).
    let (ox, oy) = if ax >= 1 { (ax - 1, ay) } else { (ax, ay - 1) };
    let (x, y) = anchor_point(&sim, ox, oy);
    assert!(
        !sim.can_build(2, BuildingKind::TownCenter, x, y),
        "a fresh Town Center cannot overlap the standing foundation"
    );

    // Completion converts the record into a second TC entity.
    let two = run_until(&mut sim, 180_000, |s| {
        s.entities()
            .iter()
            .filter(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_PLAYER && !e.dead)
            .count()
            == 2
    });
    assert!(two, "the second Town Center never stood");
    assert!(sim
        .buildings()
        .iter()
        .all(|b| b.kind != BuildingKind::TownCenter));
    // Population room follows living Town Centers: 5 (the
    // starting TC) + 5 (the House raised on the way up) + 5
    // (the new TC) = 15. (Expectation repair, 2026-10-22: as
    // committed this read 10 — the Town Centers alone — and
    // never ran; the scenario's own House counts too.)
    assert_eq!(sim.population_cap_of(OWNER_PLAYER), 15);

    // The new Town Center works: it trains and researches.
    let first_tc = sim.town_center().id;
    let new_tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_PLAYER && e.id != first_tc)
        .unwrap()
        .id;
    assert!(sim.can_train_villager_at(new_tc));
    assert!(matches!(
        sim.research_block_at_tc(new_tc, TechId::Loom),
        ResearchBlock::Ok | ResearchBlock::NoFunds
    ));
    let before = my_villagers(&sim).len();
    sim.push_command(Command::TrainVillagerAt { tc: new_tc });
    let trained = run_until(&mut sim, 10_000, |s| my_villagers(s).len() == before + 1);
    assert!(trained, "the new Town Center never trained a villager");
}
