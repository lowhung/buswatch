# buswatch-types

[![Crates.io](https://img.shields.io/crates/v/buswatch-types.svg)](https://crates.io/crates/buswatch-types)
[![Documentation](https://docs.rs/buswatch-types/badge.svg)](https://docs.rs/buswatch-types)

Core type definitions for the buswatch ecosystem.

This crate defines the canonical wire format for message bus metrics. All buswatch components use these types for serialization and deserialization.

## Types

| Type | Description |
|------|-------------|
| `Snapshot` | Point-in-time view of all modules and their metrics |
| `ModuleMetrics` | Read and write metrics for a single module |
| `ReadMetrics` | Consumption metrics: count, backlog, pending duration, rate |
| `WriteMetrics` | Production metrics: count, pending duration, rate |
| `Microseconds` | Duration wrapper for consistent serialization |
| `SchemaVersion` | Version info for forward compatibility |

## Usage

```toml
[dependencies]
buswatch-types = { version = "0.1", features = ["serde"] }
```

### Building Snapshots

```rust
use buswatch_types::Snapshot;
use std::time::Duration;

let snapshot = Snapshot::builder()
    .module("order-processor", |m| {
        m.read("orders.new", |r| r.count(1500).backlog(23))
         .write("orders.processed", |w| w.count(1497))
    })
    .module("notification-sender", |m| {
        m.read("orders.processed", |r| r.count(1450).backlog(47))
    })
    .build();

println!("Modules: {}", snapshot.len());
```

### Serialization

```rust
use buswatch_types::Snapshot;

let snapshot = Snapshot::default();

// To JSON
let json = serde_json::to_string(&snapshot)?;

// From JSON
let parsed: Snapshot = serde_json::from_str(&json)?;
```

## Features

| Feature | Description |
|---------|-------------|
| `serde` | JSON/MessagePack serialization via serde |
| `minicbor` | Compact CBOR binary format |
| `std` | Standard library support (enabled by default) |

### no_std Support

This crate supports `no_std` environments with `alloc`:

```toml
[dependencies]
buswatch-types = { version = "0.1", default-features = false, features = ["serde"] }
```

## JSON Schema

A formal JSON Schema is available at [`schema/snapshot.schema.json`](schema/snapshot.schema.json).

This enables:
- Validation of snapshots from any language
- Auto-generation of types for non-Rust systems
- Documentation of the wire format

### Example Snapshot

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
          "pending": 150000,
          "rate": 42.5
        }
      },
      "writes": {
        "orders.processed": {
          "count": 1497,
          "rate": 42.3
        }
      }
    }
  }
}
```

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version.major` | u32 | Yes | Breaking changes increment this |
| `version.minor` | u32 | Yes | Backwards-compatible additions |
| `timestamp_ms` | u64 | Yes | Unix timestamp in milliseconds |
| `modules` | object | Yes | Map of module name to metrics |
| `reads.*.count` | u64 | Yes | Total messages read |
| `reads.*.backlog` | u64 | No | Unread messages waiting |
| `reads.*.pending` | u64 | No | Wait time in microseconds |
| `reads.*.rate` | f64 | No | Messages per second |
| `writes.*.count` | u64 | Yes | Total messages written |
| `writes.*.pending` | u64 | No | Backpressure time in microseconds |
| `writes.*.rate` | f64 | No | Messages per second |

## Version Compatibility

The `SchemaVersion` type enables forward compatibility:

```rust
use buswatch_types::SchemaVersion;

let version = SchemaVersion::current();
assert!(version.is_compatible()); // true for current major version
```

- **Major version change**: Breaking format change, old parsers may fail
- **Minor version change**: New optional fields added, old parsers still work
