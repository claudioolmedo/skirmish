/* tslint:disable */
/* eslint-disable */

/**
 * A `Sim` owned by JavaScript. One instance per match.
 */
export class WasmSim {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Cell edge length in world units.
     */
    cell_size(): number;
    /**
     * Number of entities.
     */
    entity_count(): number;
    /**
     * Remaining waypoints of an entity's active move order, flat
     * `[x0, y0, …]`. Empty when the entity has no order.
     */
    entity_path(entity: number): Float64Array;
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
     * Obstacle cells as row-major bytes (0 = open, 1 = blocked).
     */
    obstacles(): Uint8Array;
    /**
     * Flat `[x0, y0, x1, y1, …]` entity positions as f64. Rendering only —
     * the sim itself never sees these floats.
     */
    positions(): Float64Array;
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
     * Current state hash as hex, without advancing the sim.
     */
    state_hash(): string;
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
     * World half-extent in world units: the world spans [-half, half).
     */
    world_half(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmsim_free: (a: number, b: number) => void;
    readonly wasmsim_cell_size: (a: number) => number;
    readonly wasmsim_entity_count: (a: number) => number;
    readonly wasmsim_entity_path: (a: number, b: number) => [number, number];
    readonly wasmsim_grid_height: (a: number) => number;
    readonly wasmsim_grid_width: (a: number) => number;
    readonly wasmsim_new: (a: number) => number;
    readonly wasmsim_obstacles: (a: number) => [number, number];
    readonly wasmsim_positions: (a: number) => [number, number];
    readonly wasmsim_push_move: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsim_push_noop: (a: number) => void;
    readonly wasmsim_state_hash: (a: number) => [number, number];
    readonly wasmsim_tick: (a: number) => [number, number];
    readonly wasmsim_tick_count: (a: number) => number;
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
