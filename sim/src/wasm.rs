//! Thin WebAssembly ABI stub (M0).
//!
//! Compiled only for `wasm32-unknown-unknown`. This is the narrow boundary
//! between the headless sim and the outside world: commands in, state out.
//! The real renderer and input shell arrive in M4 and will talk *only*
//! through this surface — the renderer must never compute game rules.
//!
//! M0 exposes the minimum needed to prove the sim compiles to WASM and runs:
//! create, tick, read the state hash. Real command/state types land in M1–M4.

use wasm_bindgen::prelude::*;

use crate::{Command, Sim};

/// A `Sim` owned by JavaScript. One instance per match.
#[wasm_bindgen]
pub struct WasmSim {
    inner: Sim,
}

#[wasm_bindgen]
impl WasmSim {
    /// Create a simulation from a seed.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> WasmSim {
        WasmSim {
            inner: Sim::new(seed as u64),
        }
    }

    /// Advance exactly one 60 Hz tick. Returns the per-tick state hash
    /// as a 16-character hex string.
    pub fn tick(&mut self) -> String {
        let hash = self.inner.tick();
        format!("{:016x}", hash)
    }

    /// Ticks elapsed. Returned as u32 (wraps after ~2.26 years at 60 Hz —
    /// fine for the M0 stub; M4 will use a wider type if needed).
    pub fn tick_count(&self) -> u32 {
        self.inner.tick_count() as u32
    }

    /// Placeholder command input. M1 defines the real command set.
    pub fn push_noop(&mut self) {
        self.inner.push_command(Command::Noop);
    }

    /// Current state hash as hex, without advancing the sim.
    pub fn state_hash(&self) -> String {
        self.inner.state_hash_hex()
    }
}
