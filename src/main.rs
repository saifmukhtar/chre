use chre::hypergraph::Hypergraph;
use chre::persistence::Persistence;
use chre::rewrite_engine::{ConservationMode, EmergenceMode, RewriteEngine};
use rayon::prelude::*;
use std::env;
use std::io::Write;
use std::sync::{Arc, Mutex};

fn main() {
    println!("=== HCSN HYPER-FLOW DUAL-CORE ENTRY POINT (v5.9.1) ===");

    // Default Study Parameters
    let total_steps = env::var("CHRE_STEPS")
        .unwrap_or("250000".to_string())
        .parse::<usize>()
        .unwrap_or(250000);
    let p_create = env::var("CHRE_P_CREATE")
        .unwrap_or("0.58".to_string())
        .parse::<f64>()
        .unwrap_or(0.58);
    let num_threads: usize = 2; // Hard-locked to Dual-Core as requested

    let steps_per_thread = total_steps / num_threads;
    let out_file = Persistence::generate_filename("main");

    println!("Configuration:");
    println!("  Threads:      {}", num_threads);
    println!("  Steps/Th:     {}", steps_per_thread);
    println!("  p_create:     {}", p_create);
    println!("  Output File:  {}", out_file);
    println!("  Persistence:  Unified SSD Streaming (Every 2,000 steps)");

    // Initialize Multi-Threaded Streaming I/O
    let writer = Persistence::open_writer(&out_file);
    let mut header_writer = writer;
    Persistence::write_header(&mut header_writer).unwrap();
    header_writer.flush().unwrap();

    let shared_writer = Arc::new(Mutex::new(header_writer));

    // Parallel Study Execution
    (0..num_threads).into_par_iter().for_each(|tid| {
        // Initialize Hypergraph for this thread
        let mut h = Hypergraph::new();
        let v1 = h.add_vertex();
        let v2 = h.add_vertex();
        h.add_causal_relation(v1.id, v2.id);
        h.add_hyperedge(vec![v1.id, v2.id]);

        // Initialize Engine
        let mut engine = RewriteEngine::new(h, p_create, None);
        engine.mode = EmergenceMode::Assisted;
        engine.conservation_mode = ConservationMode::Hybrid;
        engine.thread_id = Some(tid);
        engine.max_steps = steps_per_thread;
        engine.verbose = false; // Keep console clean for multi-thread

        println!(
            "  Thread {}: Initiating {} step Hyper-Flow simulation...",
            tid, steps_per_thread
        );

        for s in 1..=steps_per_thread {
            engine.step();

            // SSD Streaming Loop (Every 2,000 steps)
            if s % 2000 == 0 || s == steps_per_thread {
                let mut writer_lock = shared_writer.lock().unwrap();
                let events = std::mem::take(&mut engine.interaction_events);

                for event in events {
                    if let Some(csv_row) = Persistence::format_event(&event, tid) {
                        writeln!(writer_lock, "{}", csv_row).unwrap();
                    }
                }

                if s % 10000 == 0 {
                    writer_lock.flush().unwrap();
                }
            }
        }
    });

    println!("\n=== Dual-Core Scaling Study Completed Successfully ===");
    println!("High-Fidelity Dataset generated at: {}", out_file);
}
