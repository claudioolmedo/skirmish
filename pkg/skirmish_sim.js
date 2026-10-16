/* @ts-self-types="./skirmish_sim.d.ts" */

/**
 * A `Sim` owned by JavaScript. One instance per match.
 */
export class WasmSim {
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
     * Per-entity kind tags in id order: 0 = villager, 1 = town center.
     * @returns {Uint8Array}
     */
    entity_kinds() {
        const ret = wasm.wasmsim_entity_kinds(this.__wbg_ptr);
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
     * Resource nodes as a flat array: five values per node —
     * `[id, x, y, kind, amount_left]` — in node id order. `kind` tags:
     * 0 tree, 1 berry bush, 2 gold mine, 3 stone mine. Depleted nodes
     * are included with `amount_left` 0 (the preview hides them).
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
     * Order a villager to work a resource node (gather → haul → deposit
     * → repeat). Ids only — no floats cross this boundary here.
     * @param {number} entity
     * @param {number} node
     */
    push_gather(entity, node) {
        wasm.wasmsim_push_gather(this.__wbg_ptr, entity, node);
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
     * Placeholder command input.
     */
    push_noop() {
        wasm.wasmsim_push_noop(this.__wbg_ptr);
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
     * @returns {Float64Array}
     */
    stockpile() {
        const ret = wasm.wasmsim_stockpile(this.__wbg_ptr);
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
