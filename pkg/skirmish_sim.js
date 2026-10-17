/* @ts-self-types="./skirmish_sim.d.ts" */

/**
 * A `Sim` owned by JavaScript. One instance per match.
 */
export class WasmSim {
    static __wrap(ptr) {
        const obj = Object.create(WasmSim.prototype);
        obj.__wbg_ptr = ptr;
        WasmSimFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmSimFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmsim_free(ptr, 0);
    }
    /**
     * Why training `kind_tag` (3 militia, 4 archer) at `building`
     * cannot currently proceed: same tags as `train_block_reason`
     * (0 ok, 1 cannot afford, 2 queue full / not trainable here,
     * 3 housed). UI affordance only.
     * @param {number} building
     * @param {number} kind_tag
     * @returns {number}
     */
    building_train_block(building, kind_tag) {
        const ret = wasm.wasmsim_building_train_block(this.__wbg_ptr, building, kind_tag);
        return ret >>> 0;
    }
    /**
     * Placed buildings as a flat array: thirteen values per
     * building — `[id, kind, x, y, progress, complete, cell_x,
     * cell_y, size, queue_len, train_progress, owner, hp]` — in
     * building id order. `kind` tags: 0 house, 1 farm, 2 barracks
     * (M3). `progress` is the construction fraction 0..1 (1 when
     * complete); `complete` is 0/1; `size` is the footprint edge in
     * cells; `queue_len` and `train_progress` (0..1 on the queue's
     * head) describe a production building's queue. M5 adds `owner`
     * (0 human, 2 opponent — the entity owner tags) and `hp`, the
     * remaining-HP fraction 0..1 (buildings take damage now; a
     * destroyed building leaves the list entirely). A completed
     * farm is listed here (its food shows up as a node in
     * `nodes()`); an under-construction farm appears only here.
     * @returns {Float64Array}
     */
    buildings() {
        const ret = wasm.wasmsim_buildings(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Cell edge length in world units.
     * @returns {number}
     */
    cell_size() {
        const ret = wasm.wasmsim_cell_size(this.__wbg_ptr);
        return ret;
    }
    /**
     * Number of entities (Town Center + villagers).
     * @returns {number}
     */
    entity_count() {
        const ret = wasm.wasmsim_entity_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Per-entity hit points as flat `[hp, max_hp, …]` pairs in id
     * order (M3). An entity with hp < 1 is dead (the CMB-02 rule) —
     * the renderer skips it. The Town Center reports its full table
     * HP; buildings take no damage in M3.
     * @returns {Float64Array}
     */
    entity_hp() {
        const ret = wasm.wasmsim_entity_hp(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Per-entity kind tags in id order: 0 = villager, 1 = town center,
     * 2 = scout cavalry, 3 = militia, 4 = archer, 5 = sheep, 6 = boar,
     * 7 = deer (M3).
     * @returns {Uint8Array}
     */
    entity_kinds() {
        const ret = wasm.wasmsim_entity_kinds(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Per-entity owner tags in id order (M3): 0 = the human player,
     * 1 = Gaia, 2 = the AI opponent (M5). A sheep flips 1 → its
     * capturer's tag when captured.
     * @returns {Uint8Array}
     */
    entity_owners() {
        const ret = wasm.wasmsim_entity_owners(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Remaining waypoints of an entity's active move order, flat
     * `[x0, y0, …]`. Empty when the entity has no order.
     * @param {number} entity
     * @returns {Float64Array}
     */
    entity_path(entity) {
        const ret = wasm.wasmsim_entity_path(this.__wbg_ptr, entity);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * One stockpile resource by name: convenience getters for the HUD.
     * @returns {number}
     */
    food() {
        const ret = wasm.wasmsim_food(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    gold() {
        const ret = wasm.wasmsim_gold(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    grid_height() {
        const ret = wasm.wasmsim_grid_height(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Navigation grid dimensions, in cells.
     * @returns {number}
     */
    grid_width() {
        const ret = wasm.wasmsim_grid_width(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Ids of the idle military units (Scout, Militia, Archer), in
     * id order. The mirror of `idle_villagers`.
     * @returns {Uint32Array}
     */
    idle_military() {
        const ret = wasm.wasmsim_idle_military(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Ids of the idle villagers (no work task, no move order, no
     * attack target), in id order. Behind the client's idle-villager
     * key and the top-bar idle indicator.
     * @returns {Uint32Array}
     */
    idle_villagers() {
        const ret = wasm.wasmsim_idle_villagers(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * The match outcome (M5, Conquest): 0 ongoing, 1 victory,
     * 2 defeat — from the human player's seat. Always 0 in
     * Sandbox. The client polls this; the sim keeps ticking
     * harmlessly after a decision.
     * @returns {number}
     */
    match_state() {
        const ret = wasm.wasmsim_match_state(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * The match mode this sim was created with (M5): 0 sandbox,
     * 1 AI Easy, 2 AI Normal.
     * @returns {number}
     */
    mode() {
        const ret = wasm.wasmsim_mode(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Create a simulation from a seed.
     * @param {number} seed
     */
    constructor(seed) {
        const ret = wasm.wasmsim_new(seed);
        this.__wbg_ptr = ret;
        WasmSimFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Create a match against the scripted AI opponent (M5):
     * `mode_tag` 1 = AI Easy, 2 = AI Normal (the `MatchMode` tags;
     * 0 or anything unknown falls back to the plain sandbox
     * constructor's game). The opponent spawns mirrored across the
     * map and plays through the ordinary command queue from the
     * first tick.
     * @param {number} seed
     * @param {number} mode_tag
     * @returns {WasmSim}
     */
    static new_match(seed, mode_tag) {
        const ret = wasm.wasmsim_new_match(seed, mode_tag);
        return WasmSim.__wrap(ret);
    }
    /**
     * Resource nodes as a flat array: five values per node —
     * `[id, x, y, kind, amount_left]` — in node id order. `kind` tags:
     * 0 tree, 1 berry bush, 2 gold mine, 3 stone mine, 4 farm (M2.5 —
     * a completed farm), 5 hunt carcass, 6 herd carcass (M3).
     * Depleted nodes are included with `amount_left` 0 (the preview
     * hides them).
     * @returns {Float64Array}
     */
    nodes() {
        const ret = wasm.wasmsim_nodes(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Obstacle cells as row-major bytes (0 = open, 1 = blocked).
     * @returns {Uint8Array}
     */
    obstacles() {
        const ret = wasm.wasmsim_obstacles(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Population capacity (Town Center +5, each completed House +5).
     * @returns {number}
     */
    population_cap() {
        const ret = wasm.wasmsim_population_cap(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Current population: living villagers plus the training queue.
     * @returns {number}
     */
    population() {
        const ret = wasm.wasmsim_population(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Flat `[x0, y0, x1, y1, …]` entity positions as f64. Rendering only —
     * the sim itself never sees these floats.
     * @returns {Float64Array}
     */
    positions() {
        const ret = wasm.wasmsim_positions(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Projectiles in flight (M3), flat: six values per projectile —
     * `[spawn_x, spawn_y, impact_x, impact_y, spawn_tick,
     * impact_tick]` — in spawn order. The renderer interpolates the
     * current position from `tick_count()`; the sim owns the truth.
     * @returns {Float64Array}
     */
    projectiles() {
        const ret = wasm.wasmsim_projectiles(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Order a unit to attack a placed building (M5): chase its
     * footprint and strike until it is destroyed or a new order
     * lands. Only another player's building is a valid target.
     * Returns whether the order *would* be accepted.
     * @param {number} entity
     * @param {number} building
     * @returns {boolean}
     */
    push_attack_building(entity, building) {
        const ret = wasm.wasmsim_push_attack_building(this.__wbg_ptr, entity, building);
        return ret !== 0;
    }
    /**
     * Order a unit to attack a target entity (M3): chase and strike
     * until the target dies or a new order lands. Returns whether
     * the order *would* be accepted against the current state (the
     * sim re-validates when the command is applied).
     * @param {number} entity
     * @param {number} target
     * @returns {boolean}
     */
    push_attack(entity, target) {
        const ret = wasm.wasmsim_push_attack(this.__wbg_ptr, entity, target);
        return ret !== 0;
    }
    /**
     * Queue an attack order behind the unit's current order.
     * @param {number} entity
     * @param {number} target
     * @returns {boolean}
     */
    push_attack_queued(entity, target) {
        const ret = wasm.wasmsim_push_attack_queued(this.__wbg_ptr, entity, target);
        return ret !== 0;
    }
    /**
     * Order a villager to build a Barracks (175 wood, 3×3-cell
     * footprint) — see `push_build_house`.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_build_barracks(entity, x, y) {
        const ret = wasm.wasmsim_push_build_barracks(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Queue a Barracks build — see `push_build_house_queued`.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_build_barracks_queued(entity, x, y) {
        const ret = wasm.wasmsim_push_build_barracks_queued(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Order a villager to build a Farm (60 wood) — see
     * `push_build_house`.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_build_farm(entity, x, y) {
        const ret = wasm.wasmsim_push_build_farm(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Queue a Farm build — see `push_build_house_queued`.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_build_farm_queued(entity, x, y) {
        const ret = wasm.wasmsim_push_build_farm_queued(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Order a villager to build a House with its 2×2-cell footprint
     * anchored at the cell containing the world point `(x, y)`. The
     * 25-wood cost is deducted at placement (next tick). Returns
     * whether the order *would* be accepted against the current state
     * (same pattern as `push_train_villager`); the sim re-validates
     * when the command is applied.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_build_house(entity, x, y) {
        const ret = wasm.wasmsim_push_build_house(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Queue a House build behind the villager's current order. The
     * wood is charged when the order *executes*, not when queued —
     * see `Sim::can_queue`.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_build_house_queued(entity, x, y) {
        const ret = wasm.wasmsim_push_build_house_queued(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Order a villager to work a resource node (gather → haul → deposit
     * → repeat). Ids only — no floats cross this boundary here.
     * @param {number} entity
     * @param {number} node
     */
    push_gather(entity, node) {
        wasm.wasmsim_push_gather(this.__wbg_ptr, entity, node);
    }
    /**
     * Queue a gather order behind the villager's current order.
     * @param {number} entity
     * @param {number} node
     * @returns {boolean}
     */
    push_gather_queued(entity, node) {
        const ret = wasm.wasmsim_push_gather_queued(this.__wbg_ptr, entity, node);
        return ret !== 0;
    }
    /**
     * Order one entity to move to a world point.
     *
     * Floats are accepted ONLY here at the JS boundary and converted to
     * fixed-point immediately — they never enter the sim.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     */
    push_move(entity, x, y) {
        wasm.wasmsim_push_move(this.__wbg_ptr, entity, x, y);
    }
    /**
     * Queue a move order behind the entity's current order.
     * @param {number} entity
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_move_queued(entity, x, y) {
        const ret = wasm.wasmsim_push_move_queued(this.__wbg_ptr, entity, x, y);
        return ret !== 0;
    }
    /**
     * Placeholder command input.
     */
    push_noop() {
        wasm.wasmsim_push_noop(this.__wbg_ptr);
    }
    /**
     * Set a production building's rally point (M4: the Barracks).
     * Returns whether the building accepts a rally point.
     * @param {number} building
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_set_rally_building(building, x, y) {
        const ret = wasm.wasmsim_push_set_rally_building(this.__wbg_ptr, building, x, y);
        return ret !== 0;
    }
    /**
     * Set the Town Center's rally point: new villagers walk there on
     * spawn — or straight to work, when the point snaps to a live
     * resource node (within snap range). Returns whether the order
     * would be accepted (the Town Center always accepts).
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    push_set_rally_tc(x, y) {
        const ret = wasm.wasmsim_push_set_rally_tc(this.__wbg_ptr, x, y);
        return ret !== 0;
    }
    /**
     * Enqueue an Archer at a completed Barracks (M3) — see
     * `push_train_militia`.
     * @param {number} building
     * @returns {boolean}
     */
    push_train_archer(building) {
        const ret = wasm.wasmsim_push_train_archer(this.__wbg_ptr, building);
        return ret !== 0;
    }
    /**
     * Enqueue a Militia at a completed Barracks (M3). Returns
     * whether the order *would* be accepted (queue room + funds);
     * the cost is deducted when the command is applied next tick.
     * @param {number} building
     * @returns {boolean}
     */
    push_train_militia(building) {
        const ret = wasm.wasmsim_push_train_militia(this.__wbg_ptr, building);
        return ret !== 0;
    }
    /**
     * Enqueue a villager at the Town Center (50 food, deducted when the
     * command is applied next tick). Returns whether the order *would*
     * be accepted against the current state — a UI affordance; the sim
     * re-validates when the command is applied.
     * @returns {boolean}
     */
    push_train_villager() {
        const ret = wasm.wasmsim_push_train_villager(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * All rally points as a flat array, four values per entry —
     * `[x, y, node, has]` — entry 0 is the Town Center, entries
     * 1.. follow `buildings()` order. `node` is the snapped resource
     * node id, or -1; `has` is 0 when no rally is set there (the
     * x/y are then 0 and meaningless).
     * @returns {Float64Array}
     */
    rally_points() {
        const ret = wasm.wasmsim_rally_points(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Current state hash as hex, without advancing the sim.
     * @returns {string}
     */
    state_hash() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmsim_state_hash(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Stockpile as `[food, wood, gold, stone]` (f64 at the boundary).
     * The human player's — unchanged since M2; use `stockpile_for`
     * for another seat (M5).
     * @returns {Float64Array}
     */
    stockpile() {
        const ret = wasm.wasmsim_stockpile(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Any player's stockpile by owner tag (M5): `[food, wood, gold,
     * stone]` for owner 0 (human) or 2 (opponent).
     * @param {number} owner
     * @returns {Float64Array}
     */
    stockpile_for(owner) {
        const ret = wasm.wasmsim_stockpile_for(this.__wbg_ptr, owner);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {number}
     */
    stone() {
        const ret = wasm.wasmsim_stone(this.__wbg_ptr);
        return ret;
    }
    /**
     * Ticks elapsed. Returned as u32 (wraps after ~2.26 years at 60 Hz —
     * fine for the prototype; widen if a match ever needs more).
     * @returns {number}
     */
    tick_count() {
        const ret = wasm.wasmsim_tick_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Advance exactly one 60 Hz tick. Returns the per-tick state hash
     * as a 16-character hex string.
     * @returns {string}
     */
    tick() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmsim_tick(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Why Town Center training cannot currently proceed:
     * 0 ok, 1 no food, 2 queue full, 3 housed (at population cap).
     * @returns {number}
     */
    train_block_reason() {
        const ret = wasm.wasmsim_train_block_reason(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Villagers waiting in the Town Center training queue.
     * @returns {number}
     */
    train_queue_len() {
        const ret = wasm.wasmsim_train_queue_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Number of villagers (Town Center not counted).
     * @returns {number}
     */
    villager_count() {
        const ret = wasm.wasmsim_villager_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    wood() {
        const ret = wasm.wasmsim_wood(this.__wbg_ptr);
        return ret;
    }
    /**
     * World half-extent in world units: the world spans [-half, half).
     * @returns {number}
     */
    world_half() {
        const ret = wasm.wasmsim_world_half(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) WasmSim.prototype[Symbol.dispose] = WasmSim.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_41e9ee4f547fc59a: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./skirmish_sim_bg.js": import0,
    };
}

const WasmSimFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmsim_free(ptr, 1));

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedFloat64ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('skirmish_sim_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
