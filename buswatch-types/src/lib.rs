//! # buswatch-types
//!
//! Core types for message bus observability. This crate defines the universal
//! schema that any message bus system can use to emit metrics consumable by
//! buswatch and other monitoring tools.
//!
//! ## Design Goals
//!
//! - **Zero required dependencies**: Core types work without any serialization framework
//! - **Optional serialization**: Enable `serde` and/or `minicbor` features as needed
//! - **Protocol agnostic**: Works with RabbitMQ, Kafka, NATS, Redis Streams, or custom buses
//! - **Versioned schema**: Snapshots include version info for forward compatibility
//! - **Ergonomic builders**: Fluent API for constructing snapshots
//!
//! ## Features
//!
//! - `std` (default): Standard library support
//! - `serde`: JSON/MessagePack/etc. serialization via serde
//! - `minicbor`: Compact binary serialization via CBOR
//! - `all`: Enable all serialization formats
//!
//! ## Example
//!
//! ```rust
//! use buswatch_types::{Snapshot, ModuleMetrics, ReadMetrics, WriteMetrics};
//! use std::time::Duration;
//!
//! // Build a snapshot using the builder pattern
//! let snapshot = Snapshot::builder()
//!     .module("order-processor", |m| {
//!         m.read("orders.new", |r| {
//!             r.count(1500)
//!              .backlog(23)
//!              .pending(Duration::from_millis(150))
//!         })
//!         .write("orders.processed", |w| {
//!             w.count(1497)
//!         })
//!     })
//!     .module("notification-sender", |m| {
//!         m.read("orders.processed", |r| r.count(1450).backlog(47))
//!     })
//!     .build();
//!
//! assert_eq!(snapshot.modules.len(), 2);
//! ```
//!
//! ## Schema Version
//!
//! The current schema version is **1**. The version is included in serialized
//! snapshots to allow consumers to handle format evolution gracefully.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod duration;
mod metrics;
mod snapshot;
mod version;

pub use duration::*;
pub use metrics::*;
pub use snapshot::*;
pub use version::*;

/// Current schema version.
///
/// Increment this when making breaking changes to the snapshot format.
/// Consumers should check this version and handle older formats appropriately.
pub const SCHEMA_VERSION: u32 = 1;
