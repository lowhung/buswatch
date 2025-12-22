# buswatch-tui

[![Crates.io](https://img.shields.io/crates/v/buswatch-tui.svg)](https://crates.io/crates/buswatch-tui)
[![Documentation](https://docs.rs/buswatch-tui/badge.svg)](https://docs.rs/buswatch-tui)

Terminal UI for real-time message bus monitoring and diagnostics.

## Installation

```bash
cargo install buswatch
```

Or build from source:

```bash
cargo build -p buswatch-tui --release
```

## Usage

### Monitor a JSON file

```bash
buswatch -f monitor.json
```

The file should contain a [buswatch snapshot](/buswatch-types/schema/snapshot.schema.json).

### Connect to a TCP stream

```bash
buswatch --connect localhost:9090
```

Expects newline-delimited JSON snapshots.

### Subscribe to RabbitMQ

```bash
buswatch --subscribe rabbitmq.toml --topic caryatid.monitor.snapshot
```

Requires the `subscribe` feature and a config file:

```toml
# rabbitmq.toml
[rabbitmq]
url = "amqp://127.0.0.1:5672/%2f"
exchange = "caryatid"
```

## Views

### Summary (press `1`)

Overview of all modules with health status, message counts, rates, and sparklines.

```
┌─ Summary ────────────────────────────────────────────────────┐
│ Module            Reads    Rate   Writes   Pending   Status │
│ order-processor   15000   42.5/s   14997   -         OK     │
│ notification      14500   41.2/s       0   2.3s      WARN   │
│ analytics          8000   22.1/s    8000   -         OK     │
└──────────────────────────────────────────────────────────────┘
```

### Bottlenecks (press `2`)

Filtered view showing only topics in warning or critical state.

### Flow (press `3`)

Matrix visualization of module communication patterns.

```
┌─ Data Flow ──────────────────────────────────────────────────┐
│                    orders.new  orders.done  notifications    │
│ order-processor        ◀────      ────▶                      │
│ notification                      ◀────         ────▶        │
│ analytics              ◀────      ◀────                      │
└──────────────────────────────────────────────────────────────┘
```

## Controls

| Key | Action |
|-----|--------|
| `1` `2` `3` | Switch view |
| `j` / `k` or `↑` / `↓` | Navigate |
| `Enter` | Show detail overlay |
| `/` | Search |
| `s` | Sort by column |
| `S` | Reverse sort |
| `e` | Export snapshot to JSON |
| `?` | Show help |
| `q` | Quit |

## CLI Options

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

## Health Thresholds

Modules are color-coded based on their health:

- **Green (OK)**: All metrics within normal range
- **Yellow (WARN)**: Pending > 1s or backlog > 1000
- **Red (CRIT)**: Pending > 10s or backlog > 5000

Thresholds are configurable via CLI options.

## Library Usage

The TUI can also be used as a library for building custom monitoring solutions.

```toml
[dependencies]
buswatch-tui = "0.2"
```

### Custom Data Source

```rust
use buswatch_tui::{App, FileSource, Thresholds};

let source = Box::new(FileSource::new("monitor.json"));
let app = App::new(source, Thresholds::default());
```

### Channel Source (for embedding)

```rust
use buswatch_tui::{App, ChannelSource, Thresholds, Snapshot};
use tokio::sync::watch;

// Create a channel for pushing snapshots
let (tx, source) = ChannelSource::create("my-app");

// Push snapshots from your application
let snapshot = Snapshot::builder()
    .module("my-module", |m| {
        m.read("input", |r| r.count(100).backlog(5))
    })
    .build();
tx.send(snapshot).unwrap();

// Create the app
let app = App::new(Box::new(source), Thresholds::default());
```

### Custom Thresholds

```rust
use buswatch_tui::Thresholds;
use std::time::Duration;

let thresholds = Thresholds {
    pending_warning: Duration::from_secs(2),
    pending_critical: Duration::from_secs(30),
    unread_warning: 500,
    unread_critical: 2000,
};
```

## Examples

See the [examples](examples/) directory:

- [`file_source.rs`](examples/file_source.rs) - Monitor from a JSON file
- [`channel_source.rs`](examples/channel_source.rs) - Receive snapshots via channel
- [`stream_source.rs`](examples/stream_source.rs) - Connect to a TCP stream

Run an example:

```bash
cargo run -p buswatch-tui --example file_source -- path/to/monitor.json
```

## Features

| Feature | Description |
|---------|-------------|
| `subscribe` | RabbitMQ subscription via [lapin](https://crates.io/crates/lapin) |

Build with RabbitMQ support:

```bash
cargo build -p buswatch-tui --features subscribe
```
