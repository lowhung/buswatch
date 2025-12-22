//! Example: Monitoring from a JSON file
//!
//! This example demonstrates how to use buswatch to monitor
//! a Caryatid message bus by reading snapshots from a JSON file.
//!
//! The file should contain a JSON object in the buswatch-types format.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example file_source -- path/to/monitor.json
//! ```

use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use buswatch_tui::{DataSource, FileSource};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --example file_source -- <path-to-monitor.json>");
        eprintln!();
        eprintln!("The file should contain a JSON snapshot in the format:");
        eprintln!(
            r#"  {{"version": {{"major": 1, "minor": 0}}, "timestamp_ms": 0, "modules": {{}}}}"#
        );
        std::process::exit(1);
    });

    println!("Monitoring file: {}", path);
    println!("Press Ctrl+C to stop\n");

    let mut source = FileSource::new(&path);

    loop {
        match source.poll() {
            Some(snapshot) => {
                println!("Snapshot received with {} modules:", snapshot.len());
                for (name, state) in snapshot.iter() {
                    let reads: u64 = state.reads.values().map(|r| r.count).sum();
                    let writes: u64 = state.writes.values().map(|w| w.count).sum();
                    println!(
                        "  - {}: {} reads across {} topics, {} writes across {} topics",
                        name,
                        reads,
                        state.reads.len(),
                        writes,
                        state.writes.len()
                    );
                }
                println!();
            }
            None => {
                if let Some(err) = source.error() {
                    eprint!("\rError: {}  ", err);
                } else {
                    print!("\rWaiting for changes...  ");
                }
                io::stdout().flush().unwrap();
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}
