/*
Test functions taken from https://github.com/alexheretic/spin-sleep/tree/main/experiments.
This file is under Apache 2.0 license (https://www.apache.org/licenses/LICENSE-2.0).
*/

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
enum SpinStrategy {
    None,
    YieldThread,
    SpinLoopHint,
}

// Call OS native sleep for **1ns, 1µs & 1ms** and see how long it actually takes.
pub fn measure_sleep_accuracy(simulate_thread_load: Option<usize>) {
    if cfg!(debug_assertions) {
        eprintln!("Should run with `--release`");
        std::process::exit(1);
    }

    if let Some(num_threads) = simulate_thread_load {
        if num_threads > 0 {
            eprintln!("Simulating {num_threads} thread load");
            for _ in 0..num_threads {
                std::thread::spawn(|| {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    while rng.gen::<u64>() > 0 {}
                });
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    eprintln!("==> sleep 1ns");

    const ITS: u32 = 1000;

    let mut best = Duration::MAX;
    let mut sum = Duration::ZERO;
    let mut worst = Duration::ZERO;

    for _ in 0..ITS {
        let before = Instant::now();
        std::thread::sleep(Duration::from_nanos(1));
        let elapsed = before.elapsed();
        sum += elapsed;
        if elapsed < best {
            best = elapsed;
        }
        if elapsed > worst {
            worst = elapsed;
        }
    }

    println!(
        "average: {:.1?}, best: {best:.1?}, worst: {worst:.1?}",
        sum / ITS
    );

    eprintln!("==> sleep 1µs");

    let mut best = Duration::MAX;
    let mut sum = Duration::ZERO;
    let mut worst = Duration::ZERO;

    for _ in 0..ITS {
        let before = Instant::now();
        std::thread::sleep(Duration::from_micros(1));
        let elapsed = before.elapsed();
        sum += elapsed;
        if elapsed < best {
            best = elapsed;
        }
        if elapsed > worst {
            worst = elapsed;
        }
    }

    println!(
        "average: {:.1?}, best: {best:.1?}, worst: {worst:.1?}",
        sum / ITS
    );

    eprintln!("==> sleep 1ms");

    let mut best = Duration::MAX;
    let mut sum = Duration::ZERO;
    let mut worst = Duration::ZERO;

    for _ in 0..50 {
        let before = Instant::now();
        std::thread::sleep(Duration::from_millis(1));
        let elapsed = before.elapsed();
        sum += elapsed;
        if elapsed < best {
            best = elapsed;
        }
        if elapsed > worst {
            worst = elapsed;
        }
    }

    println!(
        "average: {:.3?}, best: {best:.3?}, worst: {worst:.3?}",
        sum / 50
    );
}

// Measure `SpinStrategy` latencies and spin counts across various wait durations _5ms, 900µs, 5µs, 100ns_.
pub fn measure_spin_strategy_latency(simulate_thread_load: Option<usize>) {
    if cfg!(debug_assertions) {
        eprintln!("Should run with `--release`");
        std::process::exit(1);
    }

    if let Some(num_threads) = simulate_thread_load {
        if num_threads > 0 {
            eprintln!("Simulating {num_threads} thread load");
            for _ in 0..num_threads {
                std::thread::spawn(|| {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    while rng.gen::<u64>() > 0 {}
                });
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    // warmup
    eprintln!("warming up...");
    for _ in 0..200 {
        let before = Instant::now();
        while before.elapsed() < Duration::from_millis(5) {}
    }

    for duration in [
        Duration::from_millis(5),
        Duration::from_micros(900),
        Duration::from_micros(5),
        Duration::from_nanos(100),
    ] {
        for strategy in [
            SpinStrategy::None,
            SpinStrategy::SpinLoopHint,
            SpinStrategy::YieldThread,
        ] {
            let mut sum = Duration::from_secs(0);
            let mut spins = 0_u32;

            for _ in 0..100 {
                let before = Instant::now();
                while before.elapsed() < duration {
                    match strategy {
                        SpinStrategy::YieldThread => std::thread::yield_now(),
                        SpinStrategy::SpinLoopHint => std::hint::spin_loop(),
                        SpinStrategy::None => {}
                    }
                    spins += 1;
                }
                sum += before.elapsed();
            }
            println!(
                "{duration: <6?} {: <13} avg-spins: {:<8} avg-actual: {:?}",
                format!("{strategy:?}"),
                spins / 100,
                sum / 100,
            );
        }
    }
}
