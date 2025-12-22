//! Example: Monitoring via a channel
//!
//! This example demonstrates how to integrate buswatch into your
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

use std::thread;
use std::time::Duration;

use buswatch_tui::{ChannelSource, DataSource, Snapshot};

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

            // Build a synthetic snapshot using the builder pattern
            let snapshot = Snapshot::builder()
                .module("Producer", |m| {
                    m.write("events", |w| {
                        let mut builder = w.count(counter * 10);
                        if counter % 5 == 0 {
                            builder = builder.pending(Duration::from_millis(100));
                        }
                        builder
                    })
                })
                .module("Consumer", |m| {
                    m.read("events", |r| r.count(counter * 10 - 2).backlog(2))
                })
                .build();

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
            for (name, state) in snapshot.iter() {
                println!("  Module: {}", name);
                for (topic, read) in &state.reads {
                    println!(
                        "    Read from '{}': {} messages, {} unread",
                        topic,
                        read.count,
                        read.backlog.unwrap_or(0)
                    );
                }
                for (topic, write) in &state.writes {
                    println!(
                        "    Write to '{}': {} messages{}",
                        topic,
                        write.count,
                        write
                            .pending
                            .map(|d| format!(" (pending: {}us)", d.as_micros()))
                            .unwrap_or_default()
                    );
                }
            }
            println!();
        }

        thread::sleep(Duration::from_millis(100));
    }
}
