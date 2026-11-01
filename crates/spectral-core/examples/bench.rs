//! Rough throughput benchmark: sequential vs multi-threaded normalized difference.
//!
//! Run in release mode, optionally with a pixel count:
//!
//!     cargo run --release -p spectral-core --example bench -- 30000000
//!
//! A full Sentinel-2 tile band is about 120 million pixels (10980 x 10980).

use spectral_core::index::{
    default_threads, normalized_difference, normalized_difference_parallel, Calibration,
};
use std::hint::black_box;
use std::time::Instant;

fn noisy(len: usize, seed: u64) -> Vec<u16> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // Plausible Sentinel-2 values with a few zeros (no data).
            ((state >> 48) as u16) % 6000
        })
        .collect()
}

fn best_of<F: FnMut()>(runs: usize, mut work: F) -> f64 {
    (0..runs)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed().as_secs_f64()
        })
        .fold(f64::INFINITY, f64::min)
}

fn main() {
    let pixels: usize = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(30_000_000);
    let cal = Calibration {
        offset: -1000.0,
        nodata: Some(0),
    };
    let threads = default_threads();

    let first = noisy(pixels, 1);
    let second = noisy(pixels, 2);
    println!("{pixels} pixels, {threads} threads available (best of 5 runs)\n");

    let sequential = best_of(5, || {
        black_box(normalized_difference(black_box(&first), black_box(&second), cal).unwrap());
    });
    println!(
        "sequential:    {:8.1} ms  {:8.1} Mpixel/s",
        sequential * 1e3,
        pixels as f64 / sequential / 1e6
    );

    let mut counts = vec![2, 4, threads];
    counts.sort_unstable();
    counts.dedup();
    for t in counts.into_iter().filter(|&t| t >= 2) {
        let elapsed = best_of(5, || {
            black_box(
                normalized_difference_parallel(black_box(&first), black_box(&second), cal, t)
                    .unwrap(),
            );
        });
        println!(
            "{t:>2} threads:    {:8.1} ms  {:8.1} Mpixel/s  ({:.2}x)",
            elapsed * 1e3,
            pixels as f64 / elapsed / 1e6,
            sequential / elapsed
        );
    }
}
