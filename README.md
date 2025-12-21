# caryatid-doctor

[![Crates.io](https://img.shields.io/crates/v/caryatid-doctor.svg)](https://crates.io/crates/caryatid-doctor)
[![Documentation](https://docs.rs/caryatid-doctor/badge.svg)](https://docs.rs/caryatid-doctor)
[![License](https://img.shields.io/crates/l/caryatid-doctor.svg)](LICENSE)

A diagnostic TUI for monitoring Caryatid message bus activity.

## Installation

### From crates.io

```bash
cargo install caryatid-doctor
```

### From source

```bash
cargo build -p caryatid-doctor --release
```

## Usage

```bash
# Monitor a JSON file (default mode)
caryatid-doctor -f monitor.json

# Connect to a TCP stream
caryatid-doctor --connect localhost:9090

# Subscribe to RabbitMQ (requires --features subscribe)
caryatid-doctor --subscribe rabbitmq.toml --topic caryatid.monitor.snapshot
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `-f, --file` | `monitor.json` | Monitor JSON file path |
| `-c, --connect` | - | TCP endpoint (host:port) |
| `-s, --subscribe` | - | RabbitMQ config file |
| `-t, --topic` | `caryatid.monitor.snapshot` | Subscription topic |
| `-r, --refresh` | `1` | Refresh interval (seconds) |
| `--pending-warn` | `1s` | Pending warning threshold |
| `--pending-crit` | `10s` | Pending critical threshold |
| `--unread-warn` | `1000` | Unread warning threshold |
| `--unread-crit` | `5000` | Unread critical threshold |
| `-e, --export` | - | Export to JSON and exit |

## Views

| View | Key | Description |
|------|-----|-------------|
| Summary | `1` | Module overview with health status, rates, and sparklines |
| Bottlenecks | `2` | Topics in warning/critical state |
| Flow | `3` | Module communication patterns |

## Controls

| Key | Action |
|-----|--------|
| `1` `2` `3` | Switch view |
| `j`/`k` or arrows | Navigate |
| `Enter` | Detail overlay |
| `/` | Search |
| `s` / `S` | Sort / reverse |
| `e` | Export |
| `?` | Help |
| `q` | Quit |

## RabbitMQ Subscription

Build with the subscribe feature:

```bash
cargo build -p caryatid-doctor --features subscribe --release
```

Create a config file:

```toml
# rabbitmq.toml
[rabbitmq]
url = "amqp://127.0.0.1:5672/%2f"
exchange = "caryatid"
```

Enable publishing in your Caryatid process:

```toml
[monitor]
topic = "caryatid.monitor.snapshot"
frequency_secs = 5.0

[[message-router.route]]
pattern = "caryatid.monitor.*"
bus = "external"
```

## Health Thresholds

Modules are marked unhealthy based on:

- **Pending duration**: Time waiting on reads/writes
- **Unread count**: Backlog of unread messages

A module's status is the worst across all its topics.

## Library Usage

caryatid-doctor can also be used as a library for building custom monitoring solutions.

Add to your `Cargo.toml`:

```toml
[dependencies]
caryatid-doctor = "0.1"
```

### Examples

See the [examples](examples/) directory for runnable examples:

- [`file_source.rs`](examples/file_source.rs) - Monitor from a JSON file
- [`channel_source.rs`](examples/channel_source.rs) - Receive snapshots via channel
- [`stream_source.rs`](examples/stream_source.rs) - Connect to a TCP stream

```rust
use caryatid_doctor::{App, FileSource, Thresholds};

let source = Box::new(FileSource::new("monitor.json"));
let app = App::new(source, Thresholds::default());
```

## Features

| Feature | Description |
|---------|-------------|
| `subscribe` | RabbitMQ integration via [lapin](https://crates.io/crates/lapin) |

Enable features:

```bash
cargo build --features subscribe
```

Or in `Cargo.toml`:

```toml
[dependencies]
caryatid-doctor = { version = "0.1", features = ["subscribe"] }
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
