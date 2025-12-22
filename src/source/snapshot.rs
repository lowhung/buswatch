//! Shared types for monitor snapshots.
//!
//! Re-exports the canonical types from `buswatch-types`.

pub use buswatch_types::{
    Microseconds, ModuleMetrics, ReadMetrics, SchemaVersion, Snapshot, WriteMetrics,
};

/// Legacy type alias for compatibility with existing code.
///
/// This is a simple alias - use `Snapshot` directly for new code.
pub type MonitorSnapshot = Snapshot;
