//! # buswatch-adapters
//!
//! Pre-built adapters for collecting metrics from popular message bus systems.
//!
//! This crate provides ready-to-use collectors that automatically gather
//! metrics from message buses and convert them to buswatch format.
//!
//! ## Supported Systems
//!
//! - **RabbitMQ** (`rabbitmq` feature) - Collects queue depths, consumer counts,
//!   and message rates via the RabbitMQ Management API
//! - **Kafka** (`kafka` feature) - Collects consumer group lag and partition metrics
//! - **NATS** (`nats` feature) - Collects JetStream consumer and stream metrics
//!
//! ## Quick Start (RabbitMQ)
//!
//! ```rust,no_run
//! use buswatch_adapters::rabbitmq::RabbitMqAdapter;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let adapter = RabbitMqAdapter::builder()
//!         .endpoint("http://localhost:15672")
//!         .credentials("guest", "guest")
//!         .build();
//!
//!     // Collect a snapshot
//!     let snapshot = adapter.collect().await?;
//!
//!     println!("Collected {} modules", snapshot.modules.len());
//!     Ok(())
//! }
//! ```

pub mod error;

#[cfg(feature = "rabbitmq")]
pub mod rabbitmq;

#[cfg(feature = "kafka")]
pub mod kafka;

#[cfg(feature = "nats")]
pub mod nats;

pub use error::AdapterError;

// Re-export types for convenience
pub use buswatch_types::{ModuleMetrics, ReadMetrics, Snapshot, WriteMetrics};
