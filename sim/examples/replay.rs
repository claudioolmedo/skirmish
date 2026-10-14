//! Headless replay runner (M0).
//!
//! Runs the sim for N ticks from a seed with no rendering, no window, and
//! no wall clock, printing the per-tick state hash. Used to:
//!   - sanity-check determinism by hand (`replay 42 600` twice → same output),
//!   - produce hash streams for cross-target comparison (native vs WASM).
//!
//! Usage: `cargo run --example replay -- [seed] [ticks]`

use skirmish_sim::Sim;

fn parse_arg(args: &[String], index: usize, default: u64, name: &str) -> u64 {
    args.get(index)
        .map(|s| {
            s.parse::<u64>()
                .unwrap_or_else(|_| panic!("{} must be a non-negative integer", name))
        })
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = parse_arg(&args, 1, 12345, "seed");
    let ticks = parse_arg(&args, 2, 600, "ticks");

    let mut sim = Sim::new(seed);
    println!("# skirmish headless replay — seed={} ticks={}", seed, ticks);
    for _ in 0..ticks {
        let hash = sim.tick();
        println!("{:>8} {:016x}", sim.tick_count(), hash);
    }
}
