# buswatch-sdk

[![Crates.io](https://img.shields.io/crates/v/buswatch-sdk.svg)](https://crates.io/crates/buswatch-sdk)
[![Documentation](https://docs.rs/buswatch-sdk/badge.svg)](https://docs.rs/buswatch-sdk)

Lightweight instrumentation SDK for emitting message bus metrics.

Add buswatch-sdk to your Rust application to emit metrics that can be consumed by the buswatch TUI or any compatible consumer.

## Quick Start

```toml
[dependencies]
buswatch-sdk = "0.1"
```

```rust
use buswatch_sdk::{Instrumentor, Output};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Create an instrumentor that writes to a file
    let instrumentor = Instrumentor::builder()
        .output(Output::file("metrics.json"))
        .interval(Duration::from_secs(1))
        .build();

    // Register a module and get a handle
    let handle = instrumentor.register_module("my-service");

    // Record metrics as your app processes messages
    handle.record_read("orders.new", 1);
    handle.record_write("orders.processed", 1);

    // The instrumentor emits snapshots automatically
}
```

## Output Destinations

The SDK supports multiple output destinations:

### File Output

Writes JSON snapshots to a file (useful for local development):

```rust
use buswatch_sdk::Output;

let output = Output::file("metrics.json");
```

### TCP Output

Streams newline-delimited JSON to a TCP endpoint:

```rust
use buswatch_sdk::Output;

let output = Output::tcp("127.0.0.1:9090");
```

### Channel Output

Sends snapshots to a tokio channel (for in-process consumers):

```rust
use buswatch_sdk::Output;
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel(16);
let output = Output::channel(tx);
```

### OpenTelemetry OTLP

Exports metrics via OpenTelemetry Protocol (requires `otel` feature):

```rust
use buswatch_sdk::Output;

let output = Output::otlp("http://localhost:4317");
```

## Recording Metrics

### Basic Counting

```rust
// Record message reads/writes
handle.record_read("topic-name", 1);
handle.record_write("topic-name", 1);

// Record batches
handle.record_read("topic-name", 100);
```

### Tracking Pending Duration

Use guards to automatically track how long operations take:

```rust
// Track how long a read operation is pending
let _guard = handle.start_read("orders.new");
let message = consumer.receive().await; // blocking call
drop(_guard); // automatically records the pending duration
```

### Setting Backlog

```rust
// Report the current backlog for a topic
handle.set_backlog("orders.new", 42);
```

## Configuration

### Emission Interval

Control how often snapshots are emitted:

```rust
use std::time::Duration;

let instrumentor = Instrumentor::builder()
    .interval(Duration::from_secs(5)) // emit every 5 seconds
    .build();
```

### Multiple Outputs

Send metrics to multiple destinations:

```rust
let instrumentor = Instrumentor::builder()
    .output(Output::file("metrics.json"))
    .output(Output::tcp("monitoring-server:9090"))
    .build();
```

## Features

| Feature | Description |
|---------|-------------|
| `tokio` | Async runtime support (enabled by default) |
| `otel` | OpenTelemetry OTLP export |

### OpenTelemetry Integration

Enable the `otel` feature for OTLP export:

```toml
[dependencies]
buswatch-sdk = { version = "0.1", features = ["otel"] }
```

```rust
use buswatch_sdk::{Instrumentor, Output};

let instrumentor = Instrumentor::builder()
    .output(Output::otlp("http://localhost:4317"))
    .build();
```

This allows buswatch metrics to flow into Grafana, Datadog, or any OTLP-compatible backend.

## Thread Safety

The SDK is designed for concurrent use:

- `Instrumentor` is `Send + Sync`
- `ModuleHandle` is `Clone + Send + Sync`
- Metrics are collected using lock-free atomics where possible

```rust
let handle = instrumentor.register_module("my-service");

// Clone handles for use across threads
let handle2 = handle.clone();
tokio::spawn(async move {
    handle2.record_read("topic", 1);
});
```

## Performance

The SDK is designed to have minimal overhead:

- Lock-free atomic counters for counts
- Lazy snapshot collection (only when emitting)
- Configurable emission interval to control I/O frequency
- No allocations on the hot path (record_read/write)
