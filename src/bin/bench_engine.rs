use kish::Board;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

#[path = "../engine.rs"]
#[allow(dead_code)]
mod engine;
use engine::{Engine, EngineConfig};

fn parse_arg<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    args.windows(2)
        .find(|window| window[0] == key)
        .and_then(|window| window[1].parse::<T>().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let depth = parse_arg(&args, "--depth", 16_u32);
    let seconds = parse_arg(&args, "--seconds", 5_u64);
    let max_nodes_million = parse_arg(&args, "--nodes-m", 0_u64);
    let runs = parse_arg(&args, "--runs", 3_u32);

    let max_nodes = if max_nodes_million > 0 {
        Some(max_nodes_million * 1_000_000)
    } else {
        None
    };

    println!(
        "bench_engine: depth={depth}, seconds={seconds}, nodes-m={max_nodes_million}, runs={runs}"
    );
    let board = Board::default();
    for run in 1..=runs {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut engine = Engine::new(
            EngineConfig::with_limits(Some(depth), Some(Duration::from_secs(seconds)), max_nodes),
            cancel.clone(),
            board,
        );
        let report = engine.search(|_| {});
        cancel.store(true, Ordering::Relaxed);

        match report {
            Some(r) => {
                println!(
                    "run={run} depth={} nodes={} qnodes={} nps={} tt_hits={} cutoffs={} elapsed_ms={} pv_len={}",
                    r.completed_depth,
                    r.nodes,
                    r.qnodes,
                    r.nps,
                    r.tt_hits,
                    r.cutoffs,
                    r.elapsed.as_millis(),
                    r.principal_variation.len()
                );
            }
            None => {
                println!("run={run} no-report");
            }
        }
    }
}
