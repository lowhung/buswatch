//! Example: Monitoring via a channel
//!
//! This example demonstrates how to integrate caryatid-doctor into your
//! own application by sending snapshots through a channel.
//!
//! This is useful when you want to:
//! - Receive snapshots from a message queue (RabbitMQ, etc.)
//! - Generate synthetic data for testing
//! - Bridge from any async data source
//!
//! # Usage
//!
//! ```bash
//! cargo run --example channel_source
//! ```

use std::collections::BTreeMap;
use std::thread;
use std::time::Duration;

use buswatch::{
    ChannelSource, DataSource, SerializedModuleState, SerializedReadStreamState,
    SerializedWriteStreamState,
};

fn main() {
    println!("Channel source example");
    println!("Generating synthetic monitor data...\n");

    // Create a channel source - this returns both a sender and the source
    let (tx, mut source) = ChannelSource::create("synthetic-data");

    // Spawn a thread to generate synthetic snapshots
    thread::spawn(move || {
        let mut counter = 0u64;

        loop {
            counter += 1;

            // Build a synthetic snapshot
            let mut snapshot = BTreeMap::new();

            // Simulate a "Producer" module
            let mut producer_writes = BTreeMap::new();
            producer_writes.insert(
                "events".to_string(),
                SerializedWriteStreamState {
                    written: counter * 10,
                    pending_for: if counter % 5 == 0 {
                        Some("100ms".to_string())
                    } else {
                        None
                    },
                },
            );
            snapshot.insert(
                "Producer".to_string(),
                SerializedModuleState {
                    reads: BTreeMap::new(),
                    writes: producer_writes,
                },
            );

            // Simulate a "Consumer" module
            let mut consumer_reads = BTreeMap::new();
            consumer_reads.insert(
                "events".to_string(),
                SerializedReadStreamState {
                    read: counter * 10 - 2,
                    unread: Some(2),
                    pending_for: None,
                },
            );
            snapshot.insert(
                "Consumer".to_string(),
                SerializedModuleState {
                    reads: consumer_reads,
                    writes: BTreeMap::new(),
                },
            );

            // Send the snapshot
            if tx.send(snapshot).is_err() {
                break; // Receiver dropped
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    // Poll the source in the main thread
    println!("Receiving snapshots (press Ctrl+C to stop):\n");

    loop {
        if let Some(snapshot) = source.poll() {
            println!("Received snapshot:");
            for (name, state) in &snapshot {
                println!("  Module: {}", name);
                for (topic, read) in &state.reads {
                    println!(
                        "    Read from '{}': {} messages, {} unread",
                        topic,
                        read.read,
                        read.unread.unwrap_or(0)
                    );
                }
                for (topic, write) in &state.writes {
                    println!(
                        "    Write to '{}': {} messages{}",
                        topic,
                        write.written,
                        write
                            .pending_for
                            .as_ref()
                            .map(|d| format!(" (pending: {})", d))
                            .unwrap_or_default()
                    );
                }
            }
            println!();
        }

        thread::sleep(Duration::from_millis(100));
    }
}
