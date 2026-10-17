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
     * 7 stable, 8 blacksmith (M6). `progress` is the construction fraction 0..1 (1 when
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
     * Number of entities (Town Center + villagers).
     */
    entity_count(): number;
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
     * Ids of the idle military units (Scout, Militia, Archer), in
     * id order. The mirror of `idle_villagers`.
     */
    idle_military(): Uint32Array;
    /**
     * Ids of the idle villagers (no work task, no move order, no
     * attack target), in id order. Behind the client's idle-villager
     * key and the top-bar idle indicator.
     */
    idle_villagers(): Uint32Array;
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
     * `mode_tag` 1 = AI Easy, 2 = AI Normal (the `MatchMode` tags;
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
     * Obstacle cells as row-major bytes (0 = open, 1 = blocked).
     */
    obstacles(): Uint8Array;
    /**
     * Population capacity (Town Center +5, each completed House +5).
     */
    population_cap(): number;
    /**
     * Current population: living villagers plus the training queue.
     */
    population(): number;
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
     * Queue an attack order behind the unit's current order.
     */
    push_attack_queued(entity: number, target: number): boolean;
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
     * Order a villager to build a Stable (175 wood, 3×3 — M6;
     * Feudal Age, requires a Barracks) — see `push_build_house`.
     */
    push_build_stable(entity: number, x: number, y: number): boolean;
    /**
     * Queue a Stable build (M6) — see `push_build_house_queued`.
     */
    push_build_stable_queued(entity: number, x: number, y: number): boolean;
    /**
     * Force drop-off (M6, ECO-07): order a loaded villager to haul
     * its carry to the nearest valid drop-off now, then return to
     * its previous work on its own. Returns whether the order
     * *would* be accepted (carrying something, somewhere to go).
     */
    push_force_deposit(entity: number): boolean;
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
     * Enqueue a technology at a placed building by tech tag
     * (2 fletching, 3 scale mail, 4 padded archer armor at the
     * Blacksmith; 5 man-at-arms at the Barracks). Returns whether
     * the order *would* be accepted.
     */
    push_research_at(building: number, tech_tag: number): boolean;
    /**
     * Enqueue the Feudal Age advancement at the Town Center
     * (500 food, 130 s; needs 2 completed Dark-Age buildings).
     */
    push_research_feudal(): boolean;
    /**
     * Enqueue Loom at the Town Center (50 gold, 25 s). Returns
     * whether the order *would* be accepted against current state.
     */
    push_research_loom(): boolean;
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
     * Enqueue an Archer at a completed Archery Range (M6 — the
     * Range is the Archer's home since M6; in M3 the Barracks
     * trained them by brief). See `push_train_militia`.
     */
    push_train_archer(building: number): boolean;
    /**
     * Enqueue a Militia at a completed Barracks (M3). Returns
     * whether the order *would* be accepted (queue room + funds);
     * the cost is deducted when the command is applied next tick.
     */
    push_train_militia(building: number): boolean;
    /**
     * Enqueue a Scout Cavalry at a completed Stable (M6). See
     * `push_train_militia`.
     */
    push_train_scout(building: number): boolean;
    /**
     * Enqueue a villager at the Town Center (50 food, deducted when the
     * command is applied next tick). Returns whether the order *would*
     * be accepted against the current state — a UI affordance; the sim
     * re-validates when the command is applied.
     */
    push_train_villager(): boolean;
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
     * Why research `tech_tag` at the Town Center cannot currently
     * proceed: 0 ok, 1 cannot afford, 2 queue full / not a Town
     * Center technology, 3 already researched or queued,
     * 4 requirements not met. UI affordance only.
     */
    research_block_tc(tech_tag: number): number;
    /**
     * Current state hash as hex, without advancing the sim.
     */
    state_hash(): string;
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
     * Progress on the Town Center queue's head entry, 0..1 (M6).
     */
    tc_progress(): number;
    /**
     * The Town Center's queue as flat entry codes, head first
     * (M6): unit kinds keep their entity tag; technologies are
     * `100 + tech tag`. Behind the panel's queue display.
     */
    tc_queue(): Uint32Array;
    /**
     * The human player's researched-technology bitmask (M6): bit
     * per tech tag (`TechId::tag`); the Feudal bit reads set while
     * the player stands in the Feudal Age.
     */
    tech_mask(): number;
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
    readonly wasmsim_building_queue: (a: number, b: number) => [number, number];
    readonly wasmsim_building_train_block: (a: number, b: number, c: number) => number;
    readonly wasmsim_buildings: (a: number) => [number, number];
    readonly wasmsim_cell_size: (a: number) => number;
    readonly wasmsim_entity_count: (a: number) => number;
    readonly wasmsim_entity_hp: (a: number) => [number, number];
    readonly wasmsim_entity_kinds: (a: number) => [number, number];
    readonly wasmsim_entity_owners: (a: number) => [number, number];
    readonly wasmsim_entity_path: (a: number, b: number) => [number, number];
    readonly wasmsim_food: (a: number) => number;
    readonly wasmsim_gold: (a: number) => number;
    readonly wasmsim_grid_height: (a: number) => number;
    readonly wasmsim_grid_width: (a: number) => number;
    readonly wasmsim_idle_military: (a: number) => [number, number];
    readonly wasmsim_idle_villagers: (a: number) => [number, number];
    readonly wasmsim_match_state: (a: number) => number;
    readonly wasmsim_mode: (a: number) => number;
    readonly wasmsim_new: (a: number) => number;
    readonly wasmsim_new_match: (a: number, b: number) => number;
    readonly wasmsim_nodes: (a: number) => [number, number];
    readonly wasmsim_obstacles: (a: number) => [number, number];
    readonly wasmsim_population: (a: number) => number;
    readonly wasmsim_population_cap: (a: number) => number;
    readonly wasmsim_positions: (a: number) => [number, number];
    readonly wasmsim_projectiles: (a: number) => [number, number];
    readonly wasmsim_push_attack: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_attack_building: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_attack_queued: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_build_archery_range: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_archery_range_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_barracks: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_barracks_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_blacksmith: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_blacksmith_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_farm: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_farm_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_house: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_house_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_lumber_camp: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_lumber_camp_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mill: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mill_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mining_camp: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_mining_camp_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_stable: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_stable_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_force_deposit: (a: number, b: number) => number;
    readonly wasmsim_push_gather: (a: number, b: number, c: number) => void;
    readonly wasmsim_push_gather_queued: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_move: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsim_push_move_queued: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_noop: (a: number) => void;
    readonly wasmsim_push_research_at: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_research_feudal: (a: number) => number;
    readonly wasmsim_push_research_loom: (a: number) => number;
    readonly wasmsim_push_set_rally_building: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_set_rally_tc: (a: number, b: number, c: number) => number;
    readonly wasmsim_push_train_archer: (a: number, b: number) => number;
    readonly wasmsim_push_train_militia: (a: number, b: number) => number;
    readonly wasmsim_push_train_scout: (a: number, b: number) => number;
    readonly wasmsim_push_train_villager: (a: number) => number;
    readonly wasmsim_rally_points: (a: number) => [number, number];
    readonly wasmsim_research_block_at: (a: number, b: number, c: number) => number;
    readonly wasmsim_research_block_tc: (a: number, b: number) => number;
    readonly wasmsim_state_hash: (a: number) => [number, number];
    readonly wasmsim_stockpile: (a: number) => [number, number];
    readonly wasmsim_stockpile_for: (a: number, b: number) => [number, number];
    readonly wasmsim_stone: (a: number) => number;
    readonly wasmsim_tc_progress: (a: number) => number;
    readonly wasmsim_tc_queue: (a: number) => [number, number];
    readonly wasmsim_tech_mask: (a: number) => number;
    readonly wasmsim_tick: (a: number) => [number, number];
    readonly wasmsim_tick_count: (a: number) => number;
    readonly wasmsim_train_block_reason: (a: number) => number;
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
