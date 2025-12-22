# buswatch-adapters

[![Crates.io](https://img.shields.io/crates/v/buswatch-adapters.svg)](https://crates.io/crates/buswatch-adapters)
[![Documentation](https://docs.rs/buswatch-adapters/badge.svg)](https://docs.rs/buswatch-adapters)

Pre-built collectors for popular message bus systems.

Instead of instrumenting your code, use adapters to pull metrics directly from your message bus infrastructure.

## Supported Systems

| Adapter | Feature | Metrics Collected |
|---------|---------|-------------------|
| RabbitMQ | `rabbitmq` | Queue depths, consumer counts, message rates |
| Kafka | `kafka` | Consumer group lag, partition offsets |
| NATS | `nats` | JetStream consumer and stream metrics |

## Quick Start

### RabbitMQ

```toml
[dependencies]
buswatch-adapters = { version = "0.1", features = ["rabbitmq"] }
```

```rust
use buswatch_adapters::rabbitmq::RabbitMqAdapter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = RabbitMqAdapter::builder()
        .endpoint("http://localhost:15672")
        .credentials("guest", "guest")
        .build();

    // Collect a snapshot
    let snapshot = adapter.collect().await?;

    for (queue, metrics) in &snapshot.modules {
        println!("Queue: {} - {} messages", queue, 
            metrics.reads.get("messages")
                .map(|r| r.backlog.unwrap_or(0))
                .unwrap_or(0));
    }

    Ok(())
}
```

### Kafka

```toml
[dependencies]
buswatch-adapters = { version = "0.1", features = ["kafka"] }
```

```rust
use buswatch_adapters::kafka::KafkaAdapter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = KafkaAdapter::builder()
        .brokers("localhost:9092")
        .consumer_group("my-consumer-group")
        .build();

    let snapshot = adapter.collect().await?;

    for (topic, metrics) in &snapshot.modules {
        println!("Topic: {} - lag: {:?}", topic,
            metrics.reads.get("partition-0")
                .and_then(|r| r.backlog));
    }

    Ok(())
}
```

### NATS JetStream

```toml
[dependencies]
buswatch-adapters = { version = "0.1", features = ["nats"] }
```

```rust
use buswatch_adapters::nats::NatsAdapter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = NatsAdapter::builder()
        .url("nats://localhost:4222")
        .build()
        .await?;

    let snapshot = adapter.collect().await?;

    for (consumer, metrics) in &snapshot.modules {
        println!("Consumer: {}", consumer);
    }

    Ok(())
}
```

## Continuous Collection

Adapters can be run in a loop to continuously collect metrics:

```rust
use buswatch_adapters::rabbitmq::RabbitMqAdapter;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = RabbitMqAdapter::builder()
        .endpoint("http://localhost:15672")
        .credentials("guest", "guest")
        .build();

    let mut interval = time::interval(Duration::from_secs(5));

    loop {
        interval.tick().await;
        
        match adapter.collect().await {
            Ok(snapshot) => {
                // Write to file for buswatch TUI
                let json = serde_json::to_string_pretty(&snapshot)?;
                tokio::fs::write("metrics.json", json).await?;
            }
            Err(e) => eprintln!("Collection failed: {}", e),
        }
    }
}
```

## Feeding the TUI

Adapters produce `Snapshot` objects that can be:

1. **Written to a file** for `buswatch -f metrics.json`
2. **Streamed over TCP** for `buswatch --connect host:port`
3. **Sent to a channel** for in-process TUI embedding

### Example: TCP Server

```rust
use buswatch_adapters::rabbitmq::RabbitMqAdapter;
use tokio::net::TcpListener;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = RabbitMqAdapter::builder()
        .endpoint("http://localhost:15672")
        .credentials("guest", "guest")
        .build();

    let listener = TcpListener::bind("0.0.0.0:9090").await?;
    println!("Listening on :9090");

    loop {
        let (mut socket, _) = listener.accept().await?;
        let adapter = adapter.clone();

        tokio::spawn(async move {
            loop {
                match adapter.collect().await {
                    Ok(snapshot) => {
                        let json = serde_json::to_string(&snapshot).unwrap();
                        if socket.write_all(json.as_bytes()).await.is_err() {
                            break;
                        }
                        if socket.write_all(b"\n").await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }
}
```

Then connect with: `buswatch --connect localhost:9090`

## Features

| Feature | Dependencies | Description |
|---------|--------------|-------------|
| `rabbitmq` | reqwest | RabbitMQ Management API collector |
| `kafka` | rdkafka | Kafka consumer lag collector |
| `nats` | async-nats | NATS JetStream collector |

Enable multiple adapters:

```toml
[dependencies]
buswatch-adapters = { version = "0.1", features = ["rabbitmq", "kafka"] }
```

## Error Handling

All adapters return `Result<Snapshot, AdapterError>`:

```rust
use buswatch_adapters::{rabbitmq::RabbitMqAdapter, AdapterError};

match adapter.collect().await {
    Ok(snapshot) => { /* process */ }
    Err(AdapterError::Connection(msg)) => {
        eprintln!("Connection failed: {}", msg);
    }
    Err(AdapterError::Parse(msg)) => {
        eprintln!("Failed to parse response: {}", msg);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```
