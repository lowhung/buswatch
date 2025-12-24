//! # buswatch-sdk
//!
//! Instrumentation SDK for emitting message bus metrics to buswatch.
//!
//! This crate provides a simple API for instrumenting any message bus system
//! to emit metrics that can be consumed by buswatch or other monitoring tools.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use buswatch_sdk::{Instrumentor, Output};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create an instrumentor that emits snapshots every second
//!     let instrumentor = Instrumentor::builder()
//!         .output(Output::file("metrics.json"))
//!         .interval(Duration::from_secs(1))
//!         .build();
//!
//!     // Register a module and get a handle for recording metrics
//!     let handle = instrumentor.register("my-service");
//!
//!     // Record metrics as your service processes messages
//!     handle.record_read("orders.new", 1);
//!     handle.record_write("orders.processed", 1);
//!
//!     // Start background emission (non-blocking)
//!     instrumentor.start();
//!
//!     // ... your application runs ...
//! }
//! ```
//!
//! ## Features
//!
//! - **Simple API**: Just `record_read()` and `record_write()`
//! - **Multiple outputs**: File, TCP, or custom channel
//! - **Background emission**: Automatic periodic snapshots
//! - **Thread-safe**: Use from any thread or async task
//! - **Low overhead**: Lock-free counters where possible

mod handle;
mod instrumentor;
mod output;
mod state;

#[cfg(feature = "otel")]
pub mod otel;

#[cfg(feature = "prometheus")]
pub mod prometheus;

pub use handle::ModuleHandle;
pub use instrumentor::{Instrumentor, InstrumentorBuilder};
pub use output::Output;

#[cfg(feature = "otel")]
pub use otel::{OtelConfig, OtelExporter};

// Re-export types for convenience
pub use buswatch_types::{Microseconds, ModuleMetrics, ReadMetrics, Snapshot, WriteMetrics};
