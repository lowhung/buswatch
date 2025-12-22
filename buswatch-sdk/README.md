# buswatch-sdk

Instrumentation SDK for emitting message bus metrics to buswatch.

## Quick Start

```rust
use buswatch_sdk::{Instrumentor, Output};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Create an instrumentor that emits snapshots every second
    let instrumentor = Instrumentor::builder()
        .output(Output::file("metrics.json"))
        .interval(Duration::from_secs(1))
        .build();

    // Register a module and get a handle for recording metrics
    let handle = instrumentor.register("my-service");

    // Start background emission
    instrumentor.start();

    // Record metrics as your service processes messages
    loop {
        // ... receive a message ...
        handle.record_read("orders.new", 1);
        
        // ... process and publish result ...
        handle.record_write("orders.processed", 1);
    }
}
```

## Features

- **Simple API**: Just `record_read()` and `record_write()`
- **Multiple outputs**: File, TCP, or custom channel
- **Background emission**: Automatic periodic snapshots
- **Thread-safe**: Use from any thread or async task
- **Backlog tracking**: Automatically computes unread message counts
- **Pending tracking**: Track how long operations are blocked

## Tracking Pending Operations

For tracking how long read/write operations are blocked:

```rust
// Using guards (RAII style)
let guard = handle.start_read("events");
let message = queue.recv().await; // Blocking operation
drop(guard); // Clears pending state
handle.record_read("events", 1);

// Or set pending time directly
handle.set_read_pending("events", Some(Instant::now()));
// ... do work ...
handle.set_read_pending("events", None);
```

## Output Options

### File Output

```rust
let instrumentor = Instrumentor::builder()
    .output(Output::file("metrics.json"))
    .build();
```

### TCP Output

```rust
let instrumentor = Instrumentor::builder()
    .output(Output::tcp("localhost:9090"))
    .build();
```

### Channel Output

```rust
let (output, mut rx) = Output::channel(16);

let instrumentor = Instrumentor::builder()
    .output(output)
    .build();

// Handle snapshots yourself
tokio::spawn(async move {
    while let Some(snapshot) = rx.recv().await {
        // Custom handling
    }
});
```

### Multiple Outputs

```rust
let instrumentor = Instrumentor::builder()
    .output(Output::file("metrics.json"))
    .output(Output::tcp("localhost:9090"))
    .build();
```

## Manual Emission

If you prefer not to use background emission:

```rust
let instrumentor = Instrumentor::new();
let handle = instrumentor.register("my-service");

// Record metrics
handle.record_read("events", 10);

// Collect snapshot manually
let snapshot = instrumentor.collect();
println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
```

## License

Apache-2.0
