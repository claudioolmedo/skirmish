/* tslint:disable */
/* eslint-disable */

/**
 * A `Sim` owned by JavaScript. One instance per match.
 */
export class WasmSim {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Placed buildings as a flat array: eight values per building —
     * `[id, kind, x, y, progress, complete, cell_x, cell_y]` — in
     * building id order. `kind` tags: 0 house, 1 farm. `progress` is
     * 0..1 (1 when complete); `complete` is 0/1. A completed farm is
     * listed here (its food shows up as a node in `nodes()`); an
     * under-construction farm appears only here.
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
     * Per-entity kind tags in id order: 0 = villager, 1 = town center.
     */
    entity_kinds(): Uint8Array;
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
     * Create a simulation from a seed.
     */
    constructor(seed: number);
    /**
     * Resource nodes as a flat array: five values per node —
     * `[id, x, y, kind, amount_left]` — in node id order. `kind` tags:
     * 0 tree, 1 berry bush, 2 gold mine, 3 stone mine, 4 farm (M2.5 —
     * a completed farm). Depleted nodes are included with
     * `amount_left` 0 (the preview hides them).
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
     * Order a villager to build a Farm (60 wood) — see
     * `push_build_house`.
     */
    push_build_farm(entity: number, x: number, y: number): boolean;
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
     * Order a villager to work a resource node (gather → haul → deposit
     * → repeat). Ids only — no floats cross this boundary here.
     */
    push_gather(entity: number, node: number): void;
    /**
     * Order one entity to move to a world point.
     *
     * Floats are accepted ONLY here at the JS boundary and converted to
     * fixed-point immediately — they never enter the sim.
     */
    push_move(entity: number, x: number, y: number): void;
    /**
     * Placeholder command input.
     */
    push_noop(): void;
    /**
     * Enqueue a villager at the Town Center (50 food, deducted when the
     * command is applied next tick). Returns whether the order *would*
     * be accepted against the current state — a UI affordance; the sim
     * re-validates when the command is applied.
     */
    push_train_villager(): boolean;
    /**
     * Current state hash as hex, without advancing the sim.
     */
    state_hash(): string;
    /**
     * Stockpile as `[food, wood, gold, stone]` (f64 at the boundary).
     */
    stockpile(): Float64Array;
    stone(): number;
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
     * Villagers waiting in the Town Center training queue.
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
    readonly wasmsim_buildings: (a: number) => [number, number];
    readonly wasmsim_cell_size: (a: number) => number;
    readonly wasmsim_entity_count: (a: number) => number;
    readonly wasmsim_entity_kinds: (a: number) => [number, number];
    readonly wasmsim_entity_path: (a: number, b: number) => [number, number];
    readonly wasmsim_food: (a: number) => number;
    readonly wasmsim_gold: (a: number) => number;
    readonly wasmsim_grid_height: (a: number) => number;
    readonly wasmsim_grid_width: (a: number) => number;
    readonly wasmsim_new: (a: number) => number;
    readonly wasmsim_nodes: (a: number) => [number, number];
    readonly wasmsim_obstacles: (a: number) => [number, number];
    readonly wasmsim_population: (a: number) => number;
    readonly wasmsim_population_cap: (a: number) => number;
    readonly wasmsim_positions: (a: number) => [number, number];
    readonly wasmsim_push_build_farm: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_build_house: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsim_push_gather: (a: number, b: number, c: number) => void;
    readonly wasmsim_push_move: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsim_push_noop: (a: number) => void;
    readonly wasmsim_push_train_villager: (a: number) => number;
    readonly wasmsim_state_hash: (a: number) => [number, number];
    readonly wasmsim_stockpile: (a: number) => [number, number];
    readonly wasmsim_stone: (a: number) => number;
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
