//! Snapshot - a point-in-time view of message bus state.

use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::{ModuleMetrics, ModuleMetricsBuilder, SchemaVersion};

/// A point-in-time snapshot of message bus metrics.
///
/// This is the top-level type that captures the state of all modules
/// in a message bus system. Snapshots are typically emitted periodically
/// (e.g., every second) and consumed by monitoring tools like buswatch.
///
/// # Example
///
/// ```rust
/// use buswatch_types::Snapshot;
/// use std::time::Duration;
///
/// let snapshot = Snapshot::builder()
///     .module("order-service", |m| {
///         m.read("orders.new", |r| r.count(500).backlog(10))
///          .write("orders.validated", |w| w.count(495))
///     })
///     .build();
///
/// // Serialize with serde (requires "serde" feature)
/// // let json = serde_json::to_string(&snapshot)?;
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "minicbor", derive(minicbor::Encode, minicbor::Decode))]
pub struct Snapshot {
    /// Schema version for forward compatibility.
    #[cfg_attr(feature = "minicbor", n(0))]
    pub version: SchemaVersion,

    /// Unix timestamp in milliseconds when this snapshot was taken.
    #[cfg_attr(feature = "minicbor", n(1))]
    pub timestamp_ms: u64,

    /// Metrics for each module, keyed by module name.
    #[cfg_attr(feature = "minicbor", n(2))]
    pub modules: BTreeMap<String, ModuleMetrics>,
}

impl Snapshot {
    /// Create a new snapshot with the current timestamp.
    #[cfg(feature = "std")]
    pub fn new() -> Self {
        Self {
            version: SchemaVersion::current(),
            timestamp_ms: current_timestamp_ms(),
            modules: BTreeMap::new(),
        }
    }

    /// Create a new snapshot with a specific timestamp.
    pub fn with_timestamp(timestamp_ms: u64) -> Self {
        Self {
            version: SchemaVersion::current(),
            timestamp_ms,
            modules: BTreeMap::new(),
        }
    }

    /// Create a builder for constructing snapshots.
    pub fn builder() -> SnapshotBuilder {
        SnapshotBuilder::new()
    }

    /// Check if the snapshot is empty (no modules).
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Number of modules in the snapshot.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Get metrics for a specific module.
    pub fn get(&self, module: &str) -> Option<&ModuleMetrics> {
        self.modules.get(module)
    }

    /// Iterate over all modules.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ModuleMetrics)> {
        self.modules.iter()
    }

    /// Total messages read across all modules.
    pub fn total_reads(&self) -> u64 {
        self.modules.values().map(|m| m.total_reads()).sum()
    }

    /// Total messages written across all modules.
    pub fn total_writes(&self) -> u64 {
        self.modules.values().map(|m| m.total_writes()).sum()
    }
}

#[cfg(feature = "std")]
impl Default for Snapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing `Snapshot` instances.
#[derive(Debug)]
pub struct SnapshotBuilder {
    timestamp_ms: Option<u64>,
    modules: BTreeMap<String, ModuleMetrics>,
}

impl SnapshotBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            timestamp_ms: None,
            modules: BTreeMap::new(),
        }
    }

    /// Set a specific timestamp (milliseconds since Unix epoch).
    pub fn timestamp_ms(mut self, ts: u64) -> Self {
        self.timestamp_ms = Some(ts);
        self
    }

    /// Add a module with metrics built using a closure.
    pub fn module<F>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(ModuleMetricsBuilder) -> ModuleMetricsBuilder,
    {
        let metrics = f(ModuleMetricsBuilder::new()).build();
        self.modules.insert(name.into(), metrics);
        self
    }

    /// Add a module with pre-built metrics.
    pub fn module_metrics(mut self, name: impl Into<String>, metrics: ModuleMetrics) -> Self {
        self.modules.insert(name.into(), metrics);
        self
    }

    /// Build the snapshot.
    #[cfg(feature = "std")]
    pub fn build(self) -> Snapshot {
        Snapshot {
            version: SchemaVersion::current(),
            timestamp_ms: self.timestamp_ms.unwrap_or_else(current_timestamp_ms),
            modules: self.modules,
        }
    }

    /// Build the snapshot with a specific timestamp (for no_std).
    #[cfg(not(feature = "std"))]
    pub fn build(self) -> Snapshot {
        Snapshot {
            version: SchemaVersion::current(),
            timestamp_ms: self.timestamp_ms.unwrap_or(0),
            modules: self.modules,
        }
    }
}

impl Default for SnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current timestamp in milliseconds since Unix epoch.
#[cfg(feature = "std")]
fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_builder() {
        let snapshot = Snapshot::builder()
            .timestamp_ms(1703160000000)
            .module("producer", |m| {
                m.write("events", |w| w.count(1000).rate(100.0))
            })
            .module("consumer", |m| {
                m.read("events", |r| r.count(950).backlog(50))
            })
            .build();

        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.timestamp_ms, 1703160000000);
        assert_eq!(snapshot.total_writes(), 1000);
        assert_eq!(snapshot.total_reads(), 950);
    }

    #[test]
    fn test_snapshot_version() {
        let snapshot = Snapshot::builder().build();
        assert!(snapshot.version.is_compatible());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_roundtrip() {
        let snapshot = Snapshot::builder()
            .timestamp_ms(1703160000000)
            .module("test", |m| m.read("topic", |r| r.count(42).backlog(5)))
            .build();

        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: Snapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot, parsed);
    }

    #[cfg(feature = "minicbor")]
    #[test]
    fn test_minicbor_roundtrip() {
        let snapshot = Snapshot::builder()
            .timestamp_ms(1703160000000)
            .module("test", |m| m.read("topic", |r| r.count(42).backlog(5)))
            .build();

        let bytes = minicbor::to_vec(&snapshot).unwrap();
        let parsed: Snapshot = minicbor::decode(&bytes).unwrap();

        assert_eq!(snapshot, parsed);
    }
}
