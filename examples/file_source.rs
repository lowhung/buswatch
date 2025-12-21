//! Example: Monitoring from a JSON file
//!
//! This example demonstrates how to use caryatid-doctor to monitor
//! a Caryatid message bus by reading snapshots from a JSON file.
//!
//! The file should contain a JSON object mapping module names to their state,
//! as produced by Caryatid's Monitor.
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

use monitor_tui::{DataSource, FileSource};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --example file_source -- <path-to-monitor.json>");
        eprintln!();
        eprintln!("The file should contain a JSON snapshot in the format:");
        eprintln!(r#"  {{"ModuleName": {{"reads": {{}}, "writes": {{}}}}}}"#);
        std::process::exit(1);
    });

    println!("Monitoring file: {}", path);
    println!("Press Ctrl+C to stop\n");

    let mut source = FileSource::new(&path);

    loop {
        match source.poll() {
            Some(snapshot) => {
                println!("Snapshot received with {} modules:", snapshot.len());
                for (name, state) in &snapshot {
                    let reads: u64 = state.reads.values().map(|r| r.read).sum();
                    let writes: u64 = state.writes.values().map(|w| w.written).sum();
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
