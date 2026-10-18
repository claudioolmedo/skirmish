//! Match tests (M5): the mirrored opponent start, the scripted AI
//! playing a real game through the command queue alone, Conquest
//! victory/defeat end-to-end, and bit-identical AI replays.
//!
//! The AI tests give the human *no commands at all* (or a fixed
//! script): everything the opponent does — its economy, its army,
//! its waves — is the AI script acting through `Command`s, exactly
//! like a second player would.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, EntityKind, MatchMode, MatchState, ResourceKind, Sim, TechId,
    OWNER_OPPONENT, OWNER_PLAYER,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn living(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind && !e.dead)
        .count() as u32
}

fn military_count(sim: &Sim, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && !e.dead && e.kind.is_military())
        .count() as u32
}

fn stockpile_total(sim: &Sim, owner: u8) -> f64 {
    sim.stockpile_of(owner)
        .amounts()
        .iter()
        .map(|a| f(*a))
        .sum()
}

#[test]
fn opponent_start_is_mirrored_and_complete() {
    let sim = Sim::new_match(3, MatchMode::AiNormal);
    // Two Town Centers. M7: the cells come from the seeded
    // generator — the fairness law is rotational symmetry: the
    // opponent's start is the human's rotated 180° through the
    // map center (the arena's fixed (-6,-6)/(6,6) cells are gone).
    let tcs: Vec<_> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::TownCenter)
        .collect();
    assert_eq!(tcs.len(), 2);
    let human_tc = tcs.iter().find(|e| e.owner == OWNER_PLAYER).unwrap();
    let enemy_tc = tcs.iter().find(|e| e.owner == OWNER_OPPONENT).unwrap();
    let dim = sim.grid().width();
    let (hx, hy) = sim.grid().world_to_cell(human_tc.pos_x, human_tc.pos_y);
    let (ex, ey) = sim.grid().world_to_cell(enemy_tc.pos_x, enemy_tc.pos_y);
    assert_eq!(
        (ex, ey),
        (dim - 1 - hx, dim - 1 - hy),
        "the opponent's start is not the human's rotated 180°"
    );
    // Mirrored start package: 3 villagers + 1 Scout, own stockpile,
    // own population room.
    assert_eq!(living(&sim, OWNER_OPPONENT, EntityKind::Villager), 3);
    assert_eq!(living(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry), 1);
    assert_eq!(sim.population_of(OWNER_OPPONENT), 4);
    assert_eq!(sim.population_cap_of(OWNER_OPPONENT), 5);
    assert_eq!(stockpile_total(&sim, OWNER_OPPONENT), 700.0);
    // The resource layout is doubled: per kind, each side's
    // package (nearer its own Town Center) holds equal counts.
    assert!(sim.nodes().len() >= 150, "nodes: {}", sim.nodes().len());
    for kind in [
        skirmish_sim::NodeKind::BerryBush,
        skirmish_sim::NodeKind::Tree,
        skirmish_sim::NodeKind::GoldMine,
        skirmish_sim::NodeKind::StoneMine,
    ] {
        let (mut a, mut b) = (0usize, 0usize);
        for n in sim.nodes().iter().filter(|n| n.kind == kind) {
            let dh = dist(n.pos_x - human_tc.pos_x, n.pos_y - human_tc.pos_y);
            let de = dist(n.pos_x - enemy_tc.pos_x, n.pos_y - enemy_tc.pos_y);
            if dh < de {
                a += 1;
            } else if de < dh {
                b += 1;
            }
        }
        assert_eq!(a, b, "unfair {kind:?} split: {a} vs {b}");
    }
    // The opponent has its own sheep: the Gaia herd doubles too.
    assert_eq!(living(&sim, 1, EntityKind::Sheep), 16);
    // Nobody starts inside a blocked cell (the generator seats
    // every unit of the package on open ground).
    for e in sim.entities() {
        let (cx, cy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
        assert!(
            !sim.grid().is_blocked(cx, cy),
            "entity {} starts inside a blocked cell",
            e.id
        );
    }
    // And the whole start is deterministic.
    let again = Sim::new_match(3, MatchMode::AiNormal);
    assert_eq!(sim.entities(), again.entities());
    assert_eq!(sim.nodes(), again.nodes());
}

#[test]
fn ai_economy_grows_without_human_commands() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    // Eleven game-minutes, zero human commands — long enough for
    // the AI to finish its buildup (it invests everything early,
    // so the stockpile only turns net-positive once its villager
    // and army targets are met and the spending stops).
    for _ in 0..39_600 {
        sim.tick();
    }
    // It trained villagers well past the starting three…
    assert!(
        living(&sim, OWNER_OPPONENT, EntityKind::Villager) >= 14,
        "AI villagers: {}",
        living(&sim, OWNER_OPPONENT, EntityKind::Villager)
    );
    // …grew its population far past the starting four…
    assert!(
        sim.population_of(OWNER_OPPONENT) >= 20,
        "AI population: {}",
        sim.population_of(OWNER_OPPONENT)
    );
    // …built houses and its Barracks…
    assert!(
        sim.buildings()
            .iter()
            .filter(|b| b.owner == OWNER_OPPONENT)
            .count()
            >= 3,
        "AI buildings: {}",
        sim.buildings()
            .iter()
            .filter(|b| b.owner == OWNER_OPPONENT)
            .count()
    );
    // …and its stockpile ends net-positive against the 700 it
    // started with, *after* paying for all those villagers,
    // buildings and soldiers: the gather loop really deposits.
    assert!(
        stockpile_total(&sim, OWNER_OPPONENT) > 800.0,
        "AI stockpile total: {}",
        stockpile_total(&sim, OWNER_OPPONENT)
    );
    // The human side never issued a command: its stockpile is the
    // untouched 700 it started with (the AI plays only its own
    // seat — no cross-player leakage), and its Town Center still
    // stands, so the match is Ongoing. Its villagers are *not*
    // asserted on: by minute eight the first waves have landed,
    // and a passive player's workers die — that is the wave test's
    // subject, not this one's.
    assert_eq!(stockpile_total(&sim, OWNER_PLAYER), 700.0);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn ai_raises_army_and_wave_engages_the_human() {
    let mut sim = Sim::new_match(9, MatchMode::AiNormal);
    let mut saw_barracks = false;
    let mut saw_army = false;
    let mut engaged = false;
    // Up to twenty game-minutes; the human does nothing at all.
    // (M6: the window doubled from ten — the script now builds
    // its economy layer, researches Loom and saves for the Feudal
    // Age before the first full wave, so the wave lands later
    // than the M5 script's did. It is a Feudal timing attack now,
    // which is the pacing the milestone asked the AI to learn.)
    for _ in 0..72_000 {
        sim.tick();
        saw_barracks |= sim.buildings().iter().any(|b| {
            b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Barracks && b.complete
        });
        saw_army |= military_count(&sim, OWNER_OPPONENT) >= 5;
        // Engagement: an AI soldier has a human target, or a human
        // unit has taken damage.
        engaged |= sim.entities().iter().any(|e| {
            (e.owner == OWNER_OPPONENT
                && e.kind.is_military()
                && e.attack_target
                    .map(|t| {
                        sim.entities()
                            .iter()
                            .find(|x| x.id == t)
                            .map(|x| x.owner == OWNER_PLAYER)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false))
                || (e.owner == OWNER_PLAYER
                    && e.kind.counts_population()
                    && (e.dead || e.hp < I16F16::from_num(max_hp(e.kind))))
        });
        if saw_barracks && saw_army && engaged {
            break;
        }
    }
    assert!(saw_barracks, "AI never completed a Barracks");
    assert!(saw_army, "AI never raised an army of 5");
    assert!(engaged, "no AI wave ever engaged the human side");
}

fn max_hp(kind: EntityKind) -> i32 {
    skirmish_sim::unit_spec(kind).max_hp
}

#[test]
fn ai_match_replays_bit_identically() {
    let run = |seed: u64| -> Vec<u64> {
        let mut sim = Sim::new_match(seed, MatchMode::AiNormal);
        let mut hashes = Vec::new();
        for _ in 0..5_400 {
            hashes.push(sim.tick());
        }
        hashes
    };
    assert_eq!(run(13), run(13), "identical AI matches diverged");
}

#[test]
fn scripted_siege_wins_the_match() {
    let mut sim = Sim::new_match(21, MatchMode::AiEasy);
    let mut driver = HumanScript::default();
    let mut won_at = None;
    for _ in 0..120_000 {
        driver.drive(&mut sim);
        sim.tick();
        if sim.match_state() == MatchState::Victory {
            won_at = Some(sim.tick_count());
            break;
        }
        assert_eq!(
            sim.match_state(),
            MatchState::Ongoing,
            "the scripted human must not lose its own siege"
        );
    }
    assert!(won_at.is_some(), "scripted siege never won the match");
}

#[test]
fn passive_human_loses_the_match() {
    let mut sim = Sim::new_match(33, MatchMode::AiNormal);
    let mut lost_at = None;
    for _ in 0..200_000 {
        sim.tick();
        if sim.match_state() == MatchState::Defeat {
            lost_at = Some(sim.tick_count());
            break;
        }
    }
    assert!(
        lost_at.is_some(),
        "a passive human was never defeated by the AI"
    );
}

/// A fixed scripted build order for the human side of
/// `scripted_siege_wins_the_match` — deliberately the same shape a
/// player would follow (and the same shape the AI script follows):
/// grow to ~12 villagers, two Barracks, a mixed Militia/Archer
/// army, then a committed push — rallies set to the enemy Town
/// Center, reinforcements streaming — until the enemy Town Center
/// falls and the mop-up triggers its resignation. Deterministic:
/// every decision reads live sim state in id order, like the AI's.
///
/// M6 adaptation (the game changed under the script, so the script
/// plays the new layer, like the AI does): Loom once the village
/// is grown, a Mill + Lumber Camp for the Feudal requirement,
/// Feudal Age, and Archers from an Archery Range — the Barracks
/// no longer trains them, and an all-Militia army bankrupts the
/// script's food (Archers are paid in wood/gold). Free soldiers
/// also defend the home Town Center before pushing: the AI's
/// waves genuinely arrive now, and an undefended economy bleeds
/// out before the push ever forms.
#[derive(Default)]
struct HumanScript {
    acted: Vec<u32>,
    pushing: bool,
}

impl HumanScript {
    fn drive(&mut self, sim: &mut Sim) {
        self.acted.clear();
        let me = OWNER_PLAYER;
        let foe = OWNER_OPPONENT;
        // Town Center research (M6): Loom once the village is
        // grown; Feudal Age when a food spare exists (its two
        // building requirements are the Mill + Lumber Camp from
        // the construction chain below). Research is tried before
        // training so a topped-up villager queue cannot starve it.
        let villagers = living(sim, me, EntityKind::Villager);
        let food = f(sim.stockpile_of(me).amount(ResourceKind::Food));
        if sim.can_research_tc(TechId::Loom) && villagers >= 8 {
            sim.push_command(Command::ResearchTc {
                tech: TechId::Loom,
            });
        } else if sim.can_research_tc(TechId::FeudalAge) && food >= 550.0 {
            sim.push_command(Command::ResearchTc {
                tech: TechId::FeudalAge,
            });
        } else if villagers + sim.train_queue_len() < 12 && sim.can_train_villager() {
            // Town Center: train toward 12 villagers.
            sim.push_command(Command::TrainVillager);
        }
        // Military production at the M6 homes: Militia from the
        // Barracks, Archers from the Archery Range — a mixed army
        // toward 16, kept up during the push (reinforcements).
        //
        // M7 adaptation — the age-up saving discipline the AI
        // script itself plays by (its plan_train_military pause):
        // while Dark, Loomed and nearly grown, hold production
        // until the Feudal 500 is in hand. On the arena's short
        // trips the script out-earned its own army sink and the
        // 550-food moment arrived anyway; at generated-map scale
        // the sink wins the race and the script sits Dark forever,
        // trading its army into a Feudal defense (seed 21 caught
        // it: food pinned at ~50 for 100,000 ticks).
        let saving_for_feudal = sim.age_of(me) == Age::Dark
            && sim.has_tech(me, TechId::Loom)
            && sim
                .buildings()
                .iter()
                .filter(|b| b.owner == me && b.complete && b.kind.counts_toward_feudal())
                .count()
                >= 2
            && villagers >= 10
            && food < 550.0;
        let army = living(sim, me, EntityKind::Militia) + living(sim, me, EntityKind::Archer);
        if army < 16 && !saving_for_feudal {
            let producers: Vec<(u32, EntityKind)> = sim
                .buildings()
                .iter()
                .filter(|b| b.owner == me && b.complete)
                .filter_map(|b| match b.kind {
                    BuildingKind::Barracks => Some((b.id, EntityKind::Militia)),
                    BuildingKind::ArcheryRange => Some((b.id, EntityKind::Archer)),
                    _ => None,
                })
                .collect();
            for (bid, kind) in producers {
                if sim.can_train_at(bid, kind) {
                    sim.push_command(Command::TrainAt {
                        building: bid,
                        kind,
                    });
                }
            }
        }
        // Construction: the M6 opening chain — Mill, Lumber Camp
        // (the Feudal requirement pair), Barracks, Archery Range
        // once Feudal, a second Barracks — then houses ahead of
        // the cap. One placement per pass, by the first idle
        // villager.
        let count_kind = |kind: BuildingKind| {
            sim.buildings()
                .iter()
                .filter(|b| b.owner == me && b.kind == kind)
                .count()
        };
        let barracks = count_kind(BuildingKind::Barracks);
        let wood = f(sim.stockpile_of(me).amount(ResourceKind::Wood));
        let want = if count_kind(BuildingKind::Mill) == 0 && villagers >= 5 && wood >= 100.0 {
            Some(BuildingKind::Mill)
        } else if count_kind(BuildingKind::LumberCamp) == 0 && villagers >= 6 && wood >= 100.0 {
            Some(BuildingKind::LumberCamp)
        } else if barracks < 1 && villagers >= 8 && wood >= 175.0 {
            Some(BuildingKind::Barracks)
        } else if sim.age_of(me) == Age::Feudal
            && count_kind(BuildingKind::ArcheryRange) == 0
            && wood >= 175.0
        {
            Some(BuildingKind::ArcheryRange)
        } else if barracks < 2 && villagers >= 8 && wood >= 175.0 {
            Some(BuildingKind::Barracks)
        } else if count_kind(BuildingKind::Farm) < 6
            && count_kind(BuildingKind::Mill) > 0
            && villagers >= 10
            && wood >= 120.0
        {
            // Farms (M6): the Mill-gated fields are the script's
            // renewable food — without them its food ceiling
            // (~1,300 from berries + sheep) cannot pay for twelve
            // villagers, the Feudal Age and a twelve-soldier army.
            Some(BuildingKind::Farm)
        } else if sim.population_of(me) + 1 >= sim.population_cap_of(me) && wood >= 25.0 {
            Some(BuildingKind::House)
        } else {
            None
        };
        if let Some(kind) = want {
            if let Some(builder) = self.first_idle_villager(sim) {
                if let Some((x, y)) = find_anchor(sim, builder, kind) {
                    sim.push_command(Command::Build {
                        entity: builder,
                        kind,
                        x,
                        y,
                    });
                    self.acted.push(builder);
                }
            }
        }
        // Idle villagers to work: food 6 / wood 4 / gold 3, surplus
        // to food; hunt own sheep when no food source is near home.
        self.assign_workers(sim);
        // Home defense (M6): free soldiers engage raiders near the
        // home Town Center — the same shape as the AI's own
        // defense step — instead of idling until the push forms.
        {
            let tc = sim.town_center();
            let (tc_x, tc_y) = (tc.pos_x, tc.pos_y);
            let raiders: Vec<u32> = sim
                .entities()
                .iter()
                .filter(|e| {
                    e.owner == foe
                        && !e.dead
                        && e.kind.is_military()
                        && dist(e.pos_x - tc_x, e.pos_y - tc_y) <= 18.0
                })
                .map(|e| e.id)
                .collect();
            if let Some(&first_raider) = raiders.first() {
                let free: Vec<u32> = sim
                    .entities()
                    .iter()
                    .filter(|e| {
                        e.owner == me
                            && !e.dead
                            && e.kind.is_military()
                            && e.attack_target.is_none()
                            && e.attack_building.is_none()
                    })
                    .map(|e| e.id)
                    .collect();
                for id in free {
                    sim.push_command(Command::Attack {
                        entity: id,
                        target: first_raider,
                    });
                }
            }
        }
        // The push: at a dozen soldiers the script commits. Both
        // Barracks rally to the enemy Town Center (fresh troops
        // walk to the front), and every free soldier is ordered in
        // — Town Center first, then the nearest enemy unit, then
        // the nearest enemy building. The latch stays on: this is
        // a siege, not a raid.
        if army >= 12 {
            self.pushing = true;
        }
        if self.pushing {
            let foe_tc = sim
                .entities()
                .iter()
                .find(|e| e.owner == foe && e.kind == EntityKind::TownCenter && !e.dead)
                .map(|e| (e.id, e.pos_x, e.pos_y));
            if let Some((_, tx, ty)) = foe_tc {
                let rally_bids: Vec<u32> = sim
                    .buildings()
                    .iter()
                    .filter(|b| {
                        b.owner == me
                            && (b.kind == BuildingKind::Barracks
                                || b.kind == BuildingKind::ArcheryRange)
                            && b.complete
                            && b.rally != Some((tx, ty))
                    })
                    .map(|b| b.id)
                    .collect();
                for bid in rally_bids {
                    sim.push_command(Command::SetRally {
                        building: Some(bid),
                        x: tx,
                        y: ty,
                    });
                }
            }
            // Snapshot the free soldiers first (commands mutate).
            let soldiers: Vec<(u32, I16F16, I16F16)> = sim
                .entities()
                .iter()
                .filter(|e| {
                    e.owner == me
                        && !e.dead
                        && e.kind.is_military()
                        && e.attack_target.is_none()
                        && e.attack_building.is_none()
                })
                .map(|e| (e.id, e.pos_x, e.pos_y))
                .collect();
            for (id, mx, my) in soldiers {
                if let Some((tc, _, _)) = foe_tc {
                    sim.push_command(Command::Attack {
                        entity: id,
                        target: tc,
                    });
                    continue;
                }
                if let Some(t) = nearest_living_unit(sim, foe, mx, my) {
                    sim.push_command(Command::Attack {
                        entity: id,
                        target: t,
                    });
                    continue;
                }
                if let Some(b) = nearest_building(sim, foe, mx, my) {
                    sim.push_command(Command::AttackBuilding {
                        entity: id,
                        building: b,
                    });
                }
            }
        }
    }

    fn first_idle_villager(&self, sim: &Sim) -> Option<u32> {
        sim.entities()
            .iter()
            .find(|e| {
                e.owner == OWNER_PLAYER
                    && e.kind == EntityKind::Villager
                    && !e.dead
                    && e.task == skirmish_sim::Task::Idle
                    && e.order.is_none()
                    && e.attack_target.is_none()
                    && !self.acted.contains(&e.id)
            })
            .map(|e| e.id)
    }

    fn assign_workers(&mut self, sim: &mut Sim) {
        let me = OWNER_PLAYER;
        let tc = sim.town_center();
        let (tc_x, tc_y) = (tc.pos_x, tc.pos_y);
        let targets = [6u32, 4, 3]; // food, wood, gold
        let mut counts = [0u32; 3];
        for e in sim.entities() {
            if e.owner != me || e.kind != EntityKind::Villager || e.dead {
                continue;
            }
            if e.task != skirmish_sim::Task::Idle {
                if let Some(nid) = e.task_node {
                    if let Some(n) = sim.nodes().iter().find(|n| n.id == nid) {
                        if !n.is_depleted() {
                            counts[match n.resource() {
                                ResourceKind::Food => 0,
                                ResourceKind::Wood => 1,
                                ResourceKind::Gold => 2,
                                ResourceKind::Stone => 0,
                            }] += 1;
                        }
                    }
                }
            }
        }
        let idle: Vec<u32> = sim
            .entities()
            .iter()
            .filter(|e| {
                e.owner == me
                    && e.kind == EntityKind::Villager
                    && !e.dead
                    && e.task == skirmish_sim::Task::Idle
                    && e.order.is_none()
                    && e.attack_target.is_none()
                    && !self.acted.contains(&e.id)
            })
            .map(|e| e.id)
            .collect();
        // M7: "near" is 11 tiles (44 units) — the generated
        // start's berries stand 5–8 tiles out; the arena's 20-unit
        // yardstick predates real map scale.
        let food_near = nearest_node(sim, ResourceKind::Food, tc_x, tc_y)
            .and_then(|nid| sim.nodes().iter().find(|n| n.id == nid))
            .map(|n| dist(n.pos_x - tc_x, n.pos_y - tc_y) <= 44.0)
            .unwrap_or(false);
        for vid in idle {
            let mut slot = 0usize;
            let mut best = i64::MIN;
            for s in 0..3 {
                let d = targets[s] as i64 - counts[s] as i64;
                if d > best {
                    best = d;
                    slot = s;
                }
            }
            let resource = [
                ResourceKind::Food,
                ResourceKind::Wood,
                ResourceKind::Gold,
            ][slot];
            if resource == ResourceKind::Food && !food_near {
                if let Some(sheep) = sim
                    .entities()
                    .iter()
                    .find(|e| e.kind == EntityKind::Sheep && e.owner == me && !e.dead)
                    .map(|e| e.id)
                {
                    sim.push_command(Command::Attack {
                        entity: vid,
                        target: sheep,
                    });
                    self.acted.push(vid);
                    counts[0] += 1;
                    continue;
                }
            }
            if let Some(nid) = nearest_node(sim, resource, tc_x, tc_y) {
                sim.push_command(Command::Gather {
                    entity: vid,
                    node: nid,
                });
                self.acted.push(vid);
                counts[slot] += 1;
            }
        }
    }
}

fn dist(dx: I16F16, dy: I16F16) -> f64 {
    let (x, y) = (f(dx), f(dy));
    (x * x + y * y).sqrt()
}

fn nearest_node(sim: &Sim, resource: ResourceKind, x: I16F16, y: I16F16) -> Option<u32> {
    // Own side of the map only — the same rule the AI plays by
    // (M7): a node belongs to the Town Center it stands nearer
    // (ties to the asker — here the human, whose (x, y) this is).
    // (Plain distance alone would send miners to the enemy's
    // gold when the map's rotation puts it a hair nearer.)
    let foe_tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::TownCenter && !e.dead);
    let mut best: Option<(f64, u32)> = None;
    for n in sim.nodes() {
        if n.is_depleted() || n.resource() != resource {
            continue;
        }
        let mine = match foe_tc {
            Some(ftc) => {
                dist(n.pos_x - x, n.pos_y - y)
                    <= dist(n.pos_x - ftc.pos_x, n.pos_y - ftc.pos_y)
            }
            None => true,
        };
        if !mine {
            continue;
        }
        let d = dist(n.pos_x - x, n.pos_y - y);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, n.id));
        }
    }
    best.map(|(_, id)| id)
}

fn nearest_living_unit(sim: &Sim, owner: u8, x: I16F16, y: I16F16) -> Option<u32> {
    let mut best: Option<(f64, u32)> = None;
    for e in sim.entities() {
        if e.owner != owner || e.dead || !e.kind.counts_population() {
            continue;
        }
        let d = dist(e.pos_x - x, e.pos_y - y);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, e.id));
        }
    }
    best.map(|(_, id)| id)
}

fn nearest_building(sim: &Sim, owner: u8, x: I16F16, y: I16F16) -> Option<u32> {
    let mut best: Option<(f64, u32)> = None;
    for b in sim.buildings() {
        if b.owner != owner {
            continue;
        }
        let d = dist(b.pos_x - x, b.pos_y - y);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, b.id));
        }
    }
    best.map(|(_, id)| id)
}

/// First legal anchor (footprint top-left cell) on expanding rings
/// around the human Town Center, validated by `Sim::can_build` —
/// the same search shape the AI uses.
fn find_anchor(sim: &Sim, builder: u32, kind: BuildingKind) -> Option<(I16F16, I16F16)> {
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let size = kind.footprint_size();
    for r in 3..=9i32 {
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
                // Footprint center from the anchor cell (build.rs's
                // formula, restated): the anchor center plus half
                // the remaining footprint extent.
                let (ccx, ccy) = sim.grid().cell_center(ax, ay);
                let off = I16F16::from_num((size as i32 - 1) * 2);
                let (x, y) = (ccx + off, ccy + off);
                if sim.can_build(builder, kind, x, y) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}
