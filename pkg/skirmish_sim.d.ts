/* tslint:disable */
/* eslint-disable */

/**
 * A `Sim` owned by JavaScript. One instance per match.
 */
export class WasmSim {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The human player's current age (M6): 0 Dark Age, 1 Feudal.
     */
    age(): number;
    /**
     * Current age tag of a specific player (0 Dark, 1 Feudal).
     */
    age_for(owner: number): number;
    /**
     * INF-05: [attack, armor_melee, armor_pierce] for one placed
     * building, from its effective weapon/armor rows (empty when
     * the id is unknown). Attack is -1 for unarmed kinds.
     */
    building_display_stats(building: number): Int32Array;
    /**
     * A placed building's current total arrows per volley (GAR-05).
     */
    building_garrison_arrows(building: number): number;
    /**
     * Entities garrisoned in a placed building (GAR-01).
     */
    building_garrison_count(building: number): number;
    /**
     * A placed building's queue as flat entry codes (M6) — see
     * `tc_queue`.
     */
    building_queue(building: number): Uint32Array;
    /**
     * Why training `kind_tag` (3 militia, 4 archer, 2 scout) at
     * `building` cannot currently proceed: same tags as
     * `train_block_reason` (0 ok, 1 cannot afford, 2 queue full /
     * not trainable here, 3 housed). UI affordance only.
     */
    building_train_block(building: number, kind_tag: number): number;
    /**
     * Placed buildings as a flat array: thirteen values per
     * building — `[id, kind, x, y, progress, complete, cell_x,
     * cell_y, size, queue_len, train_progress, owner, hp]` — in
     * building id order. `kind` tags: 0 house, 1 farm, 2 barracks
     * (M3); 3 lumber camp, 4 mill, 5 mining camp, 6 archery range,
     * 7 stable, 8 blacksmith (M6); 9 palisade wall (BLD-13). `progress` is the construction fraction 0..1 (1 when
     * complete); `complete` is 0/1; `size` is the footprint edge in
     * cells; `queue_len` and `train_progress` (0..1 on the queue's
     * head) describe a production building's queue. M5 adds `owner`
     * (0 human, 2 opponent — the entity owner tags) and `hp`, the
     * remaining-HP fraction 0..1 (buildings take damage now; a
     * destroyed building leaves the list entirely). A completed
     * farm is listed here (its food shows up as a node in
     * `nodes()`); an under-construction farm appears only here.
     */
    buildings(): Float64Array;
    /**
     * Cell edge length in world units.
     */
    cell_size(): number;
    /**
     * Per-entity AutoExplore flags (0/1) in entity-id order — the
     * command panel's pressed state for the toggle button.
     */
    entity_auto_explore(): Uint8Array;
    /**
     * Could AutoExplore (SEL-17) be toggled on this entity right
     * now — a living Scout Cavalry owned by a player? The client
     * mirrors its button placement and the G key from this query
     * (it replaces the old starting-scout id lookup: eligibility
     * is a per-entity rule now, not a spawn-order identity).
     */
    entity_can_auto_explore(entity: number): boolean;
    /**
     * Per-entity carried-resource tags in entity-id order:
     * 255 = hands empty; otherwise the `ResourceKind` index
     * (0 Food, 1 Wood, 2 Gold, 3 Stone). `carry_kind` is `None`
     * iff the carry is zero, so a tag here means the villager is
     * hauling that resource right now. Read-only presentation
     * glue — the renderer hangs the matching visible load on the
     * hauler and shows empty hands again the moment the deposit
     * lands, on the sim's own word rather than a client guess.
     */
    entity_carry(): Uint8Array;
    /**
     * Number of entities (Town Center + villagers).
     */
    entity_count(): number;
    /**
     * INF-05: [attack, armor_melee, armor_pierce] for one entity,
     * from its effective stat row (empty when the id is unknown).
     * Attack is -1 for kinds with no attack.
     */
    entity_display_stats(entity: number): Int32Array;
    /**
     * Per-entity garrison flags (GAR-02): 1 while the entity is
     * garrisoned inside a building or Town Center (the renderer
     * hides those entities — they are out of the field).
     */
    entity_garrisoned(): Uint8Array;
    /**
     * Per-entity hit points as flat `[hp, max_hp, …]` pairs in id
     * order (M3). An entity with hp < 1 is dead (the CMB-02 rule) —
     * the renderer skips it. The Town Center reports its full table
     * HP; buildings take no damage in M3. M6: `max_hp` is the
     * owner's *effective* maximum (a Loomed villager reads 40).
     */
    entity_hp(): Float64Array;
    /**
     * Per-entity kind tags in id order: 0 = villager, 1 = town center,
     * 2 = scout cavalry, 3 = militia, 4 = archer, 5 = sheep, 6 = boar,
     * 7 = deer (M3), 8 = man-at-arms (M6).
     */
    entity_kinds(): Uint8Array;
    /**
     * One entity's order route as flat world-point pairs
     * `[x0, y0, x1, y1, ...]` (INF-09): the current order's
     * target (only while orders are queued behind it) followed
     * by each queued order's target. Behind the client's
     * waypoint-pennant render. Render-only, like `rally_points`.
     */
    entity_order_route(entity: number): Float64Array;
    /**
     * Per-entity owner tags in id order (M3): 0 = the human player,
     * 1 = Gaia, 2 = the AI opponent (M5). A sheep flips 1 → its
     * capturer's tag when captured.
     */
    entity_owners(): Uint8Array;
    /**
     * Remaining waypoints of an entity's active move order, flat
     * `[x0, y0, …]`. Empty when the entity has no order.
     */
    entity_path(entity: number): Float64Array;
    /**
     * Per-entity stance tags in entity-id order (CMB-09:
     * 0 Aggressive, 1 Defensive, 2 Stand Ground, 3 No Attack) —
     * the command panel's pressed state for the stance row.
     */
    entity_stance(): Uint8Array;
    /**
     * Per-entity trade state in entity-id order (Package D,
     * TRD-05): 0 = not trading, 1 = tasked and travelling empty
     * (or waiting loaded for a home Market to stand), 2 = loaded
     * and returning home. Read-only presentation glue — the
     * renderer draws the gold load on a loaded cart on the sim's
     * own word, exactly like `entity_carry`.
     */
    entity_trade(): Uint8Array;
    /**
     * The LOCAL player's fog-of-war grid (M7, VIS-01): flat
     * row-major bytes, one per cell — 0 unexplored, 1 explored,
     * 2 currently visible. The client renders strictly from this:
     * enemy/Gaia units and projectiles only where it reads 2,
     * enemy buildings as last-known where it reads 1.
     */
    fog(): Uint8Array;
    /**
     * The fog-of-war grid of a specific player — `fog` for an
     * explicit seat.
     */
    fog_for(owner: number): Uint8Array;
    /**
     * One stockpile resource by name: convenience getters for the HUD.
     */
    food(): number;
    gold(): number;
    grid_height(): number;
    /**
     * Navigation grid dimensions, in cells.
     */
    grid_width(): number;
    /**
     * The generated map's height layer (M7, MAP-05): flat
     * row-major bytes, one elevation level per cell (0..=4).
     */
    heights(): Uint8Array;
    /**
     * Ids of the idle military units (Scout, Militia, Archer), in
     * id order. The mirror of `idle_villagers`.
     */
    idle_military(): Uint32Array;
    /**
     * Ids of a specific player's idle military.
     */
    idle_military_for(owner: number): Uint32Array;
    /**
     * Ids of the idle villagers (no work task, no move order, no
     * attack target), in id order. Behind the client's idle-villager
     * key and the top-bar idle indicator.
     */
    idle_villagers(): Uint32Array;
    /**
     * Ids of a specific player's idle villagers.
     */
    idle_villagers_for(owner: number): Uint32Array;
    /**
     * The live trade prices as a flat array in canonical
     * commodity order, sell then buy per commodity:
     * [sell_food, buy_food, sell_wood, buy_wood, sell_stone,
     * buy_stone]. The client displays these verbatim — the fee
     * math lives only in the sim (TRD-02).
     */
    market_prices(): Int32Array;
    /**
     * The match's global exchange rates (gold per 100) in
     * canonical commodity order: [food, wood, stone] (TRD-02).
     */
    market_rates(): Int32Array;
    /**
     * The match outcome (M5, Conquest): 0 ongoing, 1 victory,
     * 2 defeat — from the human player's seat. Always 0 in
     * Sandbox. The client polls this; the sim keeps ticking
     * harmlessly after a decision.
     */
    match_state(): number;
    /**
     * The match mode this sim was created with (M5): 0 sandbox,
     * 1 AI Easy, 2 AI Normal.
     */
    mode(): number;
    /**
     * Create a simulation from a seed.
     */
    constructor(seed: number);
    /**
     * Create a match against the scripted AI opponent (M5):
     * `mode_tag` 1 = AI Easy, 2 = AI Normal, 3 = Two-player (M8)
     * (the `MatchMode` tags;
     * 0 or anything unknown falls back to the plain sandbox
     * constructor's game). The opponent spawns mirrored across the
     * map and plays through the ordinary command queue from the
     * first tick.
     */
    static new_match(seed: number, mode_tag: number): WasmSim;
    /**
     * Resource nodes as a flat array: five values per node —
     * `[id, x, y, kind, amount_left]` — in node id order. `kind` tags:
     * 0 tree, 1 berry bush, 2 gold mine, 3 stone mine, 4 farm (M2.5 —
     * a completed farm), 5 hunt carcass, 6 herd carcass (M3).
     * Depleted nodes are included with `amount_left` 0 (the preview
     * hides them).
     */
    nodes(): Float64Array;
    /**
     * Obstacle cells as row-major bytes (0 = open, 1 = blocked:
     * water and standing-tree cells — terrain ∪ node block — plus
     * placed-building cells). (Before M7 this was the arena's
     * fixed wall layout; the meaning for consumers — "a land unit
     * cannot stand here" — is unchanged.)
     */
    obstacles(): Uint8Array;
    /**
     * Population capacity (Town Center +5, each completed House +5).
     */
    population_cap(): number;
    /**
     * Population cap of a specific player.
     */
    population_cap_for(owner: number): number;
    /**
     * Current population: living villagers plus the training queue.
     */
    population(): number;
    /**
     * Population (units + queued) of a specific player.
     */
    population_for(owner: number): number;
    /**
     * Flat `[x0, y0, x1, y1, …]` entity positions as f64. Rendering only —
     * the sim itself never sees these floats.
     */
    positions(): Float64Array;
    /**
     * Projectiles in flight (M3), flat: six values per projectile —
     * `[spawn_x, spawn_y, impact_x, impact_y, spawn_tick,
     * impact_tick]` — in spawn order. The renderer interpolates the
     * current position from `tick_count()`; the sim owns the truth.
     */
    projectiles(): Float64Array;
    /**
     * Order a unit to attack a placed building (M5): chase its
     * footprint and strike until it is destroyed or a new order
     * lands. Only another player's building is a valid target.
     * Returns whether the order *would* be accepted.
     */
    push_attack_building(entity: number, building: number): boolean;
    /**
     * Order a unit to attack a target entity (M3): chase and strike
     * until the target dies or a new order lands. Returns whether
     * the order *would* be accepted against the current state (the
     * sim re-validates when the command is applied).
     */
    push_attack(entity: number, target: number): boolean;
    /**
     * Attack-move (CMB-10): the unit walks to the point,
     * engaging whatever enters its sight en route, then resumes
     * and holds at the destination. Returns whether the order
     * *would* be accepted (a living military unit owned by a
     * player, with an attack row); the sim re-validates when the
     * command is applied.
     */
    push_attack_move(entity: number, x: number, y: number): boolean;
    /**
     * Queue an attack-move behind the unit's current order
     * (CMB-10): it executes when the unit goes idle.
     */
    push_attack_move_queued(entity: number, x: number, y: number): boolean;
    /**
     * Queue an attack order behind the unit's current order.
     */
    push_attack_queued(entity: number, target: number): boolean;
    /**
     * Toggle AutoExplore (SEL-17) on a player's Scout Cavalry:
     * with `enabled` the scout explores its owner's fog frontier
     * on its own; disabling stops it where it stands. Returns
     * whether the order *would* be accepted (a living Scout
     * Cavalry owned by a player — every scout is eligible,
     * starting and trained alike); the sim re-validates when the
     * command is applied.
     */
    push_auto_explore(entity: number, enabled: boolean): boolean;
    /**
     * Order a villager to build an Archery Range (175 wood, 3×3 —
     * M6; Feudal Age, requires a Barracks) — see
     * `push_build_house`.
     */
    push_build_archery_range(entity: number, x: number, y: number): boolean;
    /**
     * Queue an Archery Range build (M6) — see `push_build_house_queued`.
     */
    push_build_archery_range_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Barracks (175 wood, 3×3-cell
     * footprint) — see `push_build_house`.
     */
    push_build_barracks(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Barracks build — see `push_build_house_queued`.
     */
    push_build_barracks_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Blacksmith (150 wood, 3×3 — M6;
     * Feudal Age; the technology host) — see `push_build_house`.
     */
    push_build_blacksmith(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Blacksmith build (M6) — see `push_build_house_queued`.
     */
    push_build_blacksmith_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Castle (650 stone, 4×4;
     * Castle Age, no building prerequisite — Castle P3a) — see
     * `push_build_house`.
     */
    push_build_castle(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Castle build (Castle Age — Castle P3a) — see
     * `push_build_house_queued`.
     */
    push_build_castle_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Farm (60 wood) — see
     * `push_build_house`.
     */
    push_build_farm(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Farm build — see `push_build_house_queued`.
     */
    push_build_farm_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a House with its 2×2-cell footprint
     * anchored at the cell containing the world point `(x, y)`. The
     * 25-wood cost is deducted at placement (next tick). Returns
     * whether the order *would* be accepted against the current state
     * (same pattern as `push_train_villager`); the sim re-validates
     * when the command is applied.
     */
    push_build_house(entity: number, x: number, y: number): boolean;
    /**
     * Queue a House build behind the villager's current order. The
     * wood is charged when the order *executes*, not when queued —
     * see `Sim::can_queue`.
     */
    push_build_house_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Lumber Camp (100 wood, 2×2 —
     * M6; the wood drop-off) — see `push_build_house`.
     */
    push_build_lumber_camp(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Lumber Camp build (M6) — see `push_build_house_queued`.
     */
    push_build_lumber_camp_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Market (175 wood, 4×4; Feudal
     * Age, requires a completed Mill — TRD-01) — see
     * `push_build_house`.
     */
    push_build_market(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Market build — see `push_build_house_queued`.
     */
    push_build_market_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Mill (100 wood, 2×2 — M6; the
     * food drop-off, and the Farm prerequisite) — see
     * `push_build_house`.
     */
    push_build_mill(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Mill build (M6) — see `push_build_house_queued`.
     */
    push_build_mill_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Mining Camp (100 wood, 2×2 —
     * M6; the gold + stone drop-off) — see `push_build_house`.
     */
    push_build_mining_camp(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Mining Camp build (M6) — see `push_build_house_queued`.
     */
    push_build_mining_camp_queued(entity: number, x: number, y: number): boolean;
    /**
     * BLD-18: the Outpost (Dark Age, line-of-sight only).
     */
    push_build_outpost(entity: number, x: number, y: number): boolean;
    /**
     * Queue an Outpost build (BLD-18) — see
     * `push_build_house_queued`.
     */
    push_build_outpost_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build one Palisade Wall segment
     * (3 wood, 1×1 — BLD-13) — see `push_build_house`. A dragged
     * line of segments goes through `push_build_wall_line`.
     */
    push_build_palisade_wall(entity: number, x: number, y: number): boolean;
    /**
     * Queue a single Palisade Wall segment (BLD-13) — see
     * `push_build_house_queued`.
     */
    push_build_palisade_wall_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build a Stable (175 wood, 3×3 — M6;
     * Feudal Age, requires a Barracks) — see `push_build_house`.
     */
    push_build_stable(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Stable build (M6) — see `push_build_house_queued`.
     */
    push_build_stable_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build one Stone Wall segment
     * (5 stone, 1×1, Feudal Age — BLD-20) — see
     * `push_build_house`. A dragged line of segments goes
     * through `push_build_stone_wall_line`.
     */
    push_build_stone_wall(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to raise a dragged line of Stone Wall
     * segments (BLD-20) from the cell containing `(x0, y0)` to
     * the cell containing `(x1, y1)`: the same dragged-line
     * contract as `push_build_wall_line`, with the Stone Wall's
     * own row — Feudal Age, 5 stone charged per segment,
     * obstructed cells and segments past the stone stockpile
     * skipped. Returns whether at least one segment would take
     * effect; the sim re-validates when the command is applied.
     */
    push_build_stone_wall_line(entity: number, x0: number, y0: number, x1: number, y1: number): boolean;
    /**
     * Queue a single Stone Wall segment (BLD-20) — see
     * `push_build_house_queued`.
     */
    push_build_stone_wall_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to build an additional Town Center
     * (275 wood + 100 stone, 4×4 — Castle Age; the foundation
     * converts into a Town Center entity on completion) — see
     * `push_build_house`.
     */
    push_build_town_center(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Town Center build (Castle Age) — see
     * `push_build_house_queued`.
     */
    push_build_town_center_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to raise a dragged line of Palisade Wall
     * segments (BLD-13) from the cell containing `(x0, y0)` to
     * the cell containing `(x1, y1)`: the sim expands the line
     * deterministically, validates and charges each segment
     * (3 wood) individually — obstructed cells and segments past
     * the stockpile are skipped — and walks the villager along
     * the line building segment after segment. Returns whether
     * at least one segment would take effect; the sim
     * re-validates when the command is applied.
     */
    push_build_wall_line(entity: number, x0: number, y0: number, x1: number, y1: number): boolean;
    /**
     * BLD-18: the Watch Tower (Feudal, stone-cost, armed).
     */
    push_build_watch_tower(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Watch Tower build (BLD-18) — see
     * `push_build_house_queued`.
     */
    push_build_watch_tower_queued(entity: number, x: number, y: number): boolean;
    /**
     * Cancel one entry of a placed building's production queue
     * (TEC-05), refunding its full cost to the building's owner.
     * Returns whether the order *would* be accepted; the sim
     * re-validates at apply time.
     */
    push_cancel_queue_building(building: number, index: number): boolean;
    /**
     * Cancel one entry of a Town Center's production queue
     * (TEC-05), refunding its full cost to the TC's owner.
     * Returns whether the order *would* be accepted; the sim
     * re-validates at apply time.
     */
    push_cancel_queue_tc(tc: number, index: number): boolean;
    /**
     * Delete a placed building deliberately (SEL-14): a
     * foundation is a cancellation and refunds its full wood
     * cost (BLD-02); a completed building refunds nothing.
     * Returns whether the order *would* be accepted; the sim
     * re-validates at apply time.
     */
    push_delete_building(building: number): boolean;
    /**
     * Delete a unit deliberately (SEL-14): any living entity
     * owned by a player, the Town Center included. It dies this
     * tick through the ordinary death sweep; an animal leaves
     * its carcass. Returns whether the order *would* be
     * accepted; the sim re-validates at apply time.
     */
    push_delete(entity: number): boolean;
    /**
     * Force drop-off (M6, ECO-07): order a loaded villager to haul
     * its carry to the nearest valid drop-off now, then return to
     * its previous work on its own. Returns whether the order
     * *would* be accepted (carrying something, somewhere to go).
     */
    push_force_deposit(entity: number): boolean;
    /**
     * Order a unit to garrison in an own placed defensive
     * building (Castle P3b, GAR-01/SEL-11): it walks there and
     * enters on arrival. Returns whether the order *would* be
     * accepted against the current state.
     */
    push_garrison_building(entity: number, building: number): boolean;
    /**
     * Order a unit to garrison in an own Town Center (Castle P3b,
     * GAR-01/SEL-11). See `push_garrison_building`.
     */
    push_garrison_tc(entity: number, tc: number): boolean;
    /**
     * Order a villager to work a resource node (gather → haul → deposit
     * → repeat). Ids only — no floats cross this boundary here.
     */
    push_gather(entity: number, node: number): void;
    /**
     * Queue a gather order behind the villager's current order.
     */
    push_gather_queued(entity: number, node: number): boolean;
    /**
     * Buy one lot (100) of a commodity at a completed Market
     * (TRD-01..03): `resource_tag` is the `ResourceKind` tag
     * (food 1, wood 2, stone 4 — gold is never a commodity).
     * Returns false (and queues nothing) when the sim would
     * reject the trade: wrong building, unfinished, or the
     * owner's gold short of the current buy price.
     */
    push_market_buy(building: number, resource_tag: number): boolean;
    /**
     * Sell one lot (100) of a commodity at a completed Market
     * (TRD-01..03) — the mirror of `push_market_buy`; false when
     * the owner holds less than a full lot.
     */
    push_market_sell(building: number, resource_tag: number): boolean;
    /**
     * Order one entity to move to a world point.
     *
     * Floats are accepted ONLY here at the JS boundary and converted to
     * fixed-point immediately — they never enter the sim.
     */
    push_move(entity: number, x: number, y: number): void;
    /**
     * Queue a move order behind the entity's current order.
     */
    push_move_queued(entity: number, x: number, y: number): boolean;
    /**
     * Placeholder command input.
     */
    push_noop(): void;
    /**
     * Patrol (CMB-11): the unit loops between where it stands and
     * the point, engaging en route like an attack-move; on a unit
     * that already patrols, the point is appended to its loop.
     * Returns whether the order *would* be accepted (same rule as
     * `push_attack_move`); the sim re-validates at apply time.
     */
    push_patrol(entity: number, x: number, y: number): boolean;
    /**
     * Queue a patrol order behind the unit's current order
     * (CMB-11). On a unit that already patrols, the point appends
     * to its loop immediately instead (see `Sim::issue_queue`).
     */
    push_patrol_queued(entity: number, x: number, y: number): boolean;
    /**
     * Order a villager to repair one of its own player's placed
     * buildings (BLD-07): travel to the building's ring and
     * restore its HP at the documented flat rate (750 HP/min for
     * the first repairer, +375 per additional), charged as a wood
     * drip totaling 50% of the build cost over a full repair.
     * Only complete, damaged buildings of the villager's own
     * player are valid targets. Returns whether the order *would*
     * be accepted against the current state (the sim re-validates
     * when the command is applied).
     */
    push_repair(entity: number, building: number): boolean;
    /**
     * Queue a repair order behind the villager's current order
     * (BLD-07): the repair starts when the villager goes idle,
     * and re-validates fully at execution — see `push_repair`.
     */
    push_repair_queued(entity: number, building: number): boolean;
    /**
     * Enqueue a technology at a placed building by tech tag
     * (2 fletching, 3 scale mail, 4 padded archer armor at the
     * Blacksmith; 5 man-at-arms at the Barracks). Returns whether
     * the order *would* be accepted.
     */
    push_research_at(building: number, tech_tag: number): boolean;
    /**
     * Enqueue the Castle Age advancement at the Town Center
     * (800 food, 200 gold, 160 s; needs 2 completed Feudal-Age
     * buildings — TEC-01 Castle rung).
     */
    push_research_castle_age(): boolean;
    /**
     * Enqueue the Feudal Age advancement at the Town Center
     * (500 food, 130 s; needs 2 completed Dark-Age buildings).
     */
    push_research_feudal(): boolean;
    /**
     * Enqueue Hand Cart at the Town Center (300 food, 200 wood,
     * 55 s; Castle Age, requires Wheelbarrow — ECO-03).
     */
    push_research_hand_cart(): boolean;
    /**
     * Enqueue the Imperial Age advancement at the Town Center
     * (1000 food, 800 gold, 190 s; needs a Castle or two
     * completed Castle-Age buildings — TEC-01 Imperial rung).
     */
    push_research_imperial_age(): boolean;
    /**
     * Enqueue Loom at the Town Center (50 gold, 25 s). Returns
     * whether the order *would* be accepted against current state.
     */
    push_research_loom(): boolean;
    /**
     * Enqueue a Town Center technology (Loom 0, Feudal Age 1) at a
     * SPECIFIC Town Center: `push_research_tc` for an explicit
     * Town Center.
     */
    push_research_tc_at(tc: number, tech_tag: number): boolean;
    /**
     * Enqueue Wheelbarrow at the Town Center (175 food, 50 wood,
     * 75 s; Feudal Age — ECO-03).
     */
    push_research_wheelbarrow(): boolean;
    /**
     * Set a production building's rally point (M4: the Barracks).
     * Returns whether the building accepts a rally point.
     */
    push_set_rally_building(building: number, x: number, y: number): boolean;
    /**
     * Set the Town Center's rally point: new villagers walk there on
     * spawn — or straight to work, when the point snaps to a live
     * resource node (within snap range). Returns whether the order
     * would be accepted (the Town Center always accepts).
     */
    push_set_rally_tc(x: number, y: number): boolean;
    /**
     * Set a SPECIFIC Town Center's rally point (any seat):
     * `push_set_rally_tc` for an explicit Town Center.
     */
    push_set_rally_tc_of(tc: number, x: number, y: number): boolean;
    /**
     * Set a military unit's stance (CMB-09) by tag (0 Aggressive,
     * 1 Defensive, 2 Stand Ground, 3 No Attack). A stance is a
     * setting, not an order — nothing else about the unit
     * changes. Returns whether the order *would* be accepted (a
     * living military unit owned by a player, and a valid tag);
     * the sim re-validates when the command is applied.
     */
    push_set_stance(entity: number, stance: number): boolean;
    /**
     * Stop a unit's current order immediately (SEL-14): it
     * stands idle where it is; a villager keeps its carry.
     * Returns whether the order *would* be accepted (any living,
     * player-owned, mobile entity); the sim re-validates when
     * the command is applied.
     */
    push_stop(entity: number): boolean;
    /**
     * Ring a Town Center's Town Bell (Castle P3b, GAR-07): the
     * first ring shelters nearby villagers; the second sends them
     * back to their snapshotted work. Returns whether the TC can
     * ring.
     */
    push_town_bell(tc: number): boolean;
    /**
     * Task a Trade Cart to a foreign Market (Package D, TRD-05):
     * the cart runs the empty/loaded round trip until re-tasked,
     * stopped or killed, crediting its owner the TRD-06 gold on
     * each arrival home. Returns whether the order *would* be
     * accepted (a living player Trade Cart; a completed Market of
     * another player — TRD-07); the sim re-validates when the
     * command applies next tick.
     */
    push_trade(entity: number, building: number): boolean;
    /**
     * Enqueue an Archer at a completed Archery Range (M6 — the
     * Range is the Archer's home since M6; in M3 the Barracks
     * trained them by brief). See `push_train_militia`.
     */
    push_train_archer(building: number): boolean;
    /**
     * Enqueue a Cavalry Archer at a completed Archery Range
     * (Castle P2). See `push_train_militia`.
     */
    push_train_cavalry_archer(building: number): boolean;
    /**
     * Enqueue a Knight at a completed Stable (Castle P2 —
     * `can_train_at` carries the Castle Age gate). See
     * `push_train_militia`.
     */
    push_train_knight(building: number): boolean;
    /**
     * Enqueue a Militia at a completed Barracks (M3). Returns
     * whether the order *would* be accepted (queue room + funds);
     * the cost is deducted when the command is applied next tick.
     */
    push_train_militia(building: number): boolean;
    /**
     * Enqueue a Petard at a completed Castle (Castle P3a — the
     * Castle is the Petard's only home; `can_train_at` carries
     * the Castle Age gate). See `push_train_militia`.
     */
    push_train_petard(building: number): boolean;
    /**
     * Enqueue a Scout Cavalry at a completed Stable (M6). See
     * `push_train_militia`.
     */
    push_train_scout(building: number): boolean;
    /**
     * Enqueue a Skirmisher at a completed Archery Range. See
     * `push_train_militia`.
     */
    push_train_skirmisher(building: number): boolean;
    /**
     * Enqueue a Spearman at a completed Barracks (Feudal Age —
     * `can_train_at` carries the unit-level age gate). See
     * `push_train_militia`.
     */
    push_train_spearman(building: number): boolean;
    /**
     * Enqueue a Trade Cart at a completed Market (Package D,
     * TRD-05 — the Market is the cart's only home; `can_train_at`
     * carries the Feudal Age gate). See `push_train_militia`.
     */
    push_train_trade_cart(building: number): boolean;
    /**
     * Enqueue a villager at a SPECIFIC Town Center (any seat):
     * `push_train_villager` for an explicit Town Center.
     */
    push_train_villager_at(tc: number): boolean;
    /**
     * Enqueue a villager at the Town Center (50 food, deducted when the
     * command is applied next tick). Returns whether the order *would*
     * be accepted against the current state — a UI affordance; the sim
     * re-validates when the command is applied.
     */
    push_train_villager(): boolean;
    /**
     * Voluntarily ungarrison a placed building (Castle P3b,
     * GAR-03): every unit inside is placed at the exit point,
     * idle. Returns whether anyone was inside.
     */
    push_ungarrison_building(building: number): boolean;
    /**
     * Voluntarily ungarrison a Town Center (Castle P3b, GAR-03).
     */
    push_ungarrison_tc(tc: number): boolean;
    /**
     * All rally points as a flat array, four values per entry —
     * `[x, y, node, has]` — entry 0 is the Town Center, entries
     * 1.. follow `buildings()` order. `node` is the snapped resource
     * node id, or -1; `has` is 0 when no rally is set there (the
     * x/y are then 0 and meaningless).
     */
    rally_points(): Float64Array;
    /**
     * Why research `tech_tag` at `building` cannot currently
     * proceed: same tags as `research_block_tc`. UI affordance.
     */
    research_block_at(building: number, tech_tag: number): number;
    /**
     * `research_block_tc` for a specific Town Center (same tags).
     */
    research_block_tc_at(tc: number, tech_tag: number): number;
    /**
     * Why research `tech_tag` at the Town Center cannot currently
     * proceed: 0 ok, 1 cannot afford, 2 queue full / not a Town
     * Center technology, 3 already researched or queued,
     * 4 requirements not met. UI affordance only.
     */
    research_block_tc(tech_tag: number): number;
    /**
     * Any player's score by owner tag (VIC-04):
     * `[military, economy, technology, society, total]` for owner
     * 0 (human) or 2 (opponent); Gaia and unknown tags read the
     * zero breakdown. Behind the end-of-match score table.
     */
    score_for(owner: number): Float64Array;
    /**
     * Current state hash as hex, without advancing the sim.
     */
    state_hash(): string;
    /**
     * SIM-06 subsystem hash parts as one comma-separated string
     * of 16-char hex values: rng, world, economy, targeting,
     * full. Pure read, without advancing the sim — NET-04's
     * per-tick desync diagnostics consume the first four.
     */
    state_hash_parts(): string;
    /**
     * Stockpile as `[food, wood, gold, stone]` (f64 at the boundary).
     * The human player's — unchanged since M2; use `stockpile_for`
     * for another seat (M5).
     */
    stockpile(): Float64Array;
    /**
     * Any player's stockpile by owner tag (M5): `[food, wood, gold,
     * stone]` for owner 0 (human) or 2 (opponent).
     */
    stockpile_for(owner: number): Float64Array;
    stone(): number;
    /**
     * Whether a Town Center's bell is currently rung (GAR-07).
     */
    tc_bell_active(tc: number): boolean;
    /**
     * A Town Center's current arrow total (GAR-05/06).
     */
    tc_garrison_arrows(tc: number): number;
    /**
     * Entities garrisoned in a Town Center (GAR-01).
     */
    tc_garrison_count(tc: number): number;
    /**
     * Progress on the Town Center queue's head entry, 0..1 (M6).
     */
    tc_progress(): number;
    /**
     * Progress on a specific Town Center queue's head entry, 0..1.
     */
    tc_progress_for(tc: number): number;
    /**
     * The Town Center's queue as flat entry codes, head first
     * (M6): unit kinds keep their entity tag; technologies are
     * `100 + tech tag`. Behind the panel's queue display.
     */
    tc_queue(): Uint32Array;
    /**
     * A specific Town Center's queue as flat entry codes — see
     * `tc_queue`.
     */
    tc_queue_for(tc: number): Uint32Array;
    /**
     * A specific Town Center's rally point as `[x, y, node, has]` —
     * the single-entry form of one `rally_points` entry.
     */
    tc_rally_of(tc: number): Float64Array;
    /**
     * The human player's researched-technology bitmask (M6): bit
     * per tech tag (`TechId::tag`); the Feudal bit reads set while
     * the player stands in the Feudal Age.
     */
    tech_mask(): number;
    /**
     * Researched-technology bitmask of a specific player.
     */
    tech_mask_for(owner: number): number;
    /**
     * The generated map's terrain layer (M7, MAP-03): flat
     * row-major bytes, one per cell — 0 land, 1 water.
     */
    terrain(): Uint8Array;
    /**
     * Ticks elapsed. Returned as u32 (wraps after ~2.26 years at 60 Hz —
     * fine for the prototype; widen if a match ever needs more).
     */
    tick_count(): number;
    /**
     * Advance exactly one 60 Hz tick. Returns the per-tick state hash
     * as a 16-character hex string.
     */
    tick(): string;
    /**
     * `train_block_reason` for a specific Town Center (same tags).
     */
    train_block_reason_at(tc: number): number;
    /**
     * Why Town Center training cannot currently proceed:
     * 0 ok, 1 no food, 2 queue full, 3 housed (at population cap).
     */
    train_block_reason(): number;
    /**
     * Entries waiting in the Town Center queue (villagers and,
     * since M6, research — the queue is shared).
     */
    train_queue_len(): number;
    /**
     * Number of villagers (Town Center not counted).
     */
    villager_count(): number;
    wood(): number;
    /**
     * World half-extent in world units: the world spans [-half, half).
     */
    world_half(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmsim_free: (a: number, b: number) => void;
    readonly wasmsim_age: (a: number) => number;
    readonly wasmsim_age_for: (a: number, b: number) => number;
    readonly wasmsim_building_display_stats: (a: number, b: number) => [number, number];
    readonly wasmsim_building_garrison_arrows: (a: number, b: number) => number;
    readonly wasmsim_building_garrison_count: (a: number, b: number) => number;
    readonly wasmsim_building_queue: (a: number, b: number) => [number, number];
    readonly wasmsim_building_train_block: (a: number, b: number, c: number) => number;
    readonly wasmsim_buildings: (a: number) => [number, number];
    readonly wasmsim_cell_size: (a: number) => number;
    readonly wasmsim_entity_auto_explore: (a: number) => [number, number];
    readonly wasmsim_entity_can_auto_explore: (a: number, b: number) => number;
    readonly wasmsim_entity_carry: (a: number) => [number, number];
    readonly wasmsim_entity_count: (a: number) => number;
    readonly wasmsim_entity_display_stats: (a: number, b: number) => [number, number];
    readonly wasmsim_entity_garrisoned: (a: number) => [number, number];
    readonly wasmsim_entity_hp: (a: number) => [number, number];
    readonly wasmsim_entity_kinds: (a: number) => [number, number];
    readonly wasmsim_entity_order_route: (a: number, b: number) => [number, number];
    readonly wasmsim_entity_owners: (a: number) => [number, number];
    readonly wasmsim_entity_path: (a: number, b: number) => [number, number];
    readonly wasmsim_entity_stance: (a: number) => [number, number];
    readonly wasmsim_entity_trade: (a: number) => [number, number];
    readonly wasmsim_fog: (a: number) => [number, number];
    readonly wasmsim_fog_for: (a: number, b: number) => [number, number];
    readonly wasmsim_food: (a: number) => number;
    readonly wasmsim_gold: (a: number) => number;
    readonly wasmsim_grid_height: (a: number) => number;
    readonly wasmsim_grid_width: (a: number) => number;
    readonly wasmsim_heights: (a: number) => [number, number];
    readonly wasmsim_idle_military: (a: number) => [number, number];
    readonly wasmsim_idle_military_for: (a: number, b: number) => [number, number];
    readonly wasmsim_idle_villagers: (a: number) => [number, number];
    readonly wasmsim_idle_villagers_for: (a: number, b: number) => [number, number];
    readonly wasmsim_market_prices: (a: number) => [number, number];
    readonly wasmsim_market_rates: (a: number) => [number, number];
    readonly wasmsim_match_state: (a: number) => number;
    readonly wasmsim_mode: (a: number) => number;
    readonly wasmsim_new: (a: number) => number;
    readonly wasmsim_new_match: (a: number, b: number) => number;
    readonly wasmsim_nodes: (a: number) => [number, number];
    readonly wasmsim_obstacles: (a: number) => [number, number];
    readonly wasmsim_population: (a: number) => number;
    readonly wasmsim_population_cap: (a: number) => number;
    readonly wasmsim_population_cap_for: (a: number, b: number) => number;
    readonly wasmsim_population_for: (a: number, b: number) => number;
    readonly wasmsim_positions: (a: number) => [number, number];
    readonly wasmsim_projectiles: (a: number) => [number, number];
    readonly wasmsim_push_attack: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_attack_building: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_attack_move: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_attack_move_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_attack_queued: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_auto_explore: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_build_archery_range: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_archery_range_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_barracks: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_barracks_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_blacksmith: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_blacksmith_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_castle: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_castle_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_farm: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_farm_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_house: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_house_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_lumber_camp: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_lumber_camp_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_market: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_market_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mill: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mill_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mining_camp: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mining_camp_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_outpost: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_outpost_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_palisade_wall: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_palisade_wall_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_stable: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_stable_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_stone_wall: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_stone_wall_line: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmsim_push_build_stone_wall_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_town_center: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_town_center_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_wall_line: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmsim_push_build_watch_tower: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_watch_tower_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_cancel_queue_building: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_cancel_queue_tc: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_delete: (a: number, b: number) => number;
    readonly wasmsim_push_delete_building: (a: number, b: number) => number;
    readonly wasmsim_push_force_deposit: (a: number, b: number) => number;
    readonly wasmsim_push_garrison_building: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_garrison_tc: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_gather: (a: number, b: number, c: number) => void;
    readonly wasmsim_push_gather_queued: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_market_buy: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_market_sell: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_move: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsim_push_move_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_noop: (a: number) => void;
    readonly wasmsim_push_patrol: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_patrol_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_repair: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_repair_queued: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_research_at: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_research_castle_age: (a: number) => number;
    readonly wasmsim_push_research_feudal: (a: number) => number;
    readonly wasmsim_push_research_hand_cart: (a: number) => number;
    readonly wasmsim_push_research_imperial_age: (a: number) => number;
    readonly wasmsim_push_research_loom: (a: number) => number;
    readonly wasmsim_push_research_tc_at: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_research_wheelbarrow: (a: number) => number;
    readonly wasmsim_push_set_rally_building: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_set_rally_tc: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_set_rally_tc_of: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_set_stance: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_stop: (a: number, b: number) => number;
    readonly wasmsim_push_town_bell: (a: number, b: number) => number;
    readonly wasmsim_push_trade: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_train_archer: (a: number, b: number) => number;
    readonly wasmsim_push_train_cavalry_archer: (a: number, b: number) => number;
    readonly wasmsim_push_train_knight: (a: number, b: number) => number;
    readonly wasmsim_push_train_militia: (a: number, b: number) => number;
    readonly wasmsim_push_train_petard: (a: number, b: number) => number;
    readonly wasmsim_push_train_scout: (a: number, b: number) => number;
    readonly wasmsim_push_train_skirmisher: (a: number, b: number) => number;
    readonly wasmsim_push_train_spearman: (a: number, b: number) => number;
    readonly wasmsim_push_train_trade_cart: (a: number, b: number) => number;
    readonly wasmsim_push_train_villager: (a: number) => number;
    readonly wasmsim_push_train_villager_at: (a: number, b: number) => number;
    readonly wasmsim_push_ungarrison_building: (a: number, b: number) => number;
    readonly wasmsim_push_ungarrison_tc: (a: number, b: number) => number;
    readonly wasmsim_rally_points: (a: number) => [number, number];
    readonly wasmsim_research_block_at: (a: number, b: number, c: number) => number;
    readonly wasmsim_research_block_tc: (a: number, b: number) => number;
    readonly wasmsim_research_block_tc_at: (a: number, b: number, c: number) => number;
    readonly wasmsim_score_for: (a: number, b: number) => [number, number];
    readonly wasmsim_state_hash: (a: number) => [number, number];
    readonly wasmsim_state_hash_parts: (a: number) => [number, number];
    readonly wasmsim_stockpile: (a: number) => [number, number];
    readonly wasmsim_stockpile_for: (a: number, b: number) => [number, number];
    readonly wasmsim_stone: (a: number) => number;
    readonly wasmsim_tc_bell_active: (a: number, b: number) => number;
    readonly wasmsim_tc_garrison_arrows: (a: number, b: number) => number;
    readonly wasmsim_tc_garrison_count: (a: number, b: number) => number;
    readonly wasmsim_tc_progress: (a: number) => number;
    readonly wasmsim_tc_progress_for: (a: number, b: number) => number;
    readonly wasmsim_tc_queue: (a: number) => [number, number];
    readonly wasmsim_tc_queue_for: (a: number, b: number) => [number, number];
    readonly wasmsim_tc_rally_of: (a: number, b: number) => [number, number];
    readonly wasmsim_tech_mask: (a: number) => number;
    readonly wasmsim_tech_mask_for: (a: number, b: number) => number;
    readonly wasmsim_terrain: (a: number) => [number, number];
    readonly wasmsim_tick: (a: number) => [number, number];
    readonly wasmsim_tick_count: (a: number) => number;
    readonly wasmsim_train_block_reason: (a: number) => number;
    readonly wasmsim_train_block_reason_at: (a: number, b: number) => number;
    readonly wasmsim_train_queue_len: (a: number) => number;
    readonly wasmsim_villager_count: (a: number) => number;
    readonly wasmsim_wood: (a: number) => number;
    readonly wasmsim_world_half: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
