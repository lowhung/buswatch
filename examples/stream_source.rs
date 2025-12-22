//! Example: Monitoring via a TCP stream
//!
//! This example demonstrates how to use StreamSource to receive
//! monitor snapshots over a TCP connection.
//!
//! # Usage
//!
//! First, start a server that sends JSON snapshots (one per line):
//!
//! ```bash
//! # Example using netcat to send a test snapshot:
//! echo '{"version":{"major":1,"minor":0},"timestamp_ms":0,"modules":{"TestModule":{"reads":{},"writes":{}}}}' | nc -l 9090
//! ```
//!
//! Then run this example:
//!
//! ```bash
//! cargo run --example stream_source -- localhost:9090
//! ```

use std::env;
use std::thread;
use std::time::Duration;

use tokio::net::TcpStream;

use buswatch::{DataSource, StreamSource};

#[tokio::main]
async fn main() {
    let addr = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --example stream_source -- <host:port>");
        eprintln!();
        eprintln!("Example: cargo run --example stream_source -- localhost:9090");
        std::process::exit(1);
    });

    println!("Connecting to {}...", addr);

    let stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    println!("Connected! Waiting for snapshots...\n");

    // Create a StreamSource from the TCP stream
    // The source will spawn a background task to read from the stream
    let mut source = StreamSource::spawn(stream, &addr);

    // Poll for snapshots
    loop {
        match source.poll() {
            Some(snapshot) => {
                println!("Received snapshot with {} modules:", snapshot.len());
                for (name, state) in snapshot.iter() {
                    println!(
                        "  - {}: {} read topics, {} write topics",
                        name,
                        state.reads.len(),
                        state.writes.len()
                    );
                }
                println!();
            }
            None => {
                if let Some(err) = source.error() {
                    eprintln!("Error: {}", err);
                    break;
                }
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}
