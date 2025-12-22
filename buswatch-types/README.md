# buswatch-types

Core types for message bus observability. This crate defines the universal schema that any message bus system can use to emit metrics consumable by [buswatch](https://crates.io/crates/buswatch) and other monitoring tools.

## Features

- **Zero required dependencies**: Core types work without any serialization framework
- **Optional serialization**: Enable `serde` and/or `minicbor` features as needed
- **Protocol agnostic**: Works with RabbitMQ, Kafka, NATS, Redis Streams, or custom buses
- **Versioned schema**: Snapshots include version info for forward compatibility
- **Ergonomic builders**: Fluent API for constructing snapshots
- **`no_std` compatible**: Use in embedded or constrained environments

## Installation

```toml
[dependencies]
buswatch-types = "0.1"

# With serialization support
buswatch-types = { version = "0.1", features = ["serde"] }
buswatch-types = { version = "0.1", features = ["minicbor"] }
buswatch-types = { version = "0.1", features = ["all"] }
```

## Usage

### Building Snapshots

```rust
use buswatch_types::Snapshot;
use std::time::Duration;

let snapshot = Snapshot::builder()
    .module("order-processor", |m| {
        m.read("orders.new", |r| {
            r.count(1500)
             .backlog(23)
             .pending(Duration::from_millis(150))
        })
        .write("orders.processed", |w| {
            w.count(1497)
        })
    })
    .module("notification-sender", |m| {
        m.read("orders.processed", |r| r.count(1450).backlog(47))
    })
    .build();
```

### Serialization

With the `serde` feature:

```rust
use buswatch_types::Snapshot;

let snapshot = Snapshot::builder()
    .module("my-service", |m| m.read("events", |r| r.count(100)))
    .build();

// JSON
let json = serde_json::to_string_pretty(&snapshot)?;

// Or any other serde-compatible format
```

With the `minicbor` feature for compact binary encoding:

```rust
use buswatch_types::Snapshot;

let snapshot = Snapshot::builder()
    .module("my-service", |m| m.read("events", |r| r.count(100)))
    .build();

let bytes = minicbor::to_vec(&snapshot)?;
let decoded: Snapshot = minicbor::decode(&bytes)?;
```

## Schema

The snapshot schema is versioned for forward compatibility:

```json
{
  "version": { "major": 1, "minor": 0 },
  "timestamp_ms": 1703160000000,
  "modules": {
    "order-processor": {
      "reads": {
        "orders.new": {
          "count": 1500,
          "backlog": 23,
          "pending": 150000
        }
      },
      "writes": {
        "orders.processed": {
          "count": 1497
        }
      }
    }
  }
}
```

### Types

- **`Snapshot`**: Top-level container with timestamp and all module metrics
- **`ModuleMetrics`**: Reads and writes for a single module
- **`ReadMetrics`**: Count, backlog, pending time, and rate for a subscription
- **`WriteMetrics`**: Count, pending time, and rate for a publication
- **`Microseconds`**: Duration wrapper for consistent serialization
- **`SchemaVersion`**: Version info for compatibility checking

## License

Apache-2.0
