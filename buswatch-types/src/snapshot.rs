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

    #[test]
    fn with_timestamp() {
        let s = Snapshot::with_timestamp(1703160000000);
        assert_eq!(s.timestamp_ms, 1703160000000);
        assert!(s.is_empty());
        assert!(s.version.is_compatible());
    }

    #[test]
    fn builder_with_prebuilt_metrics() {
        let metrics = ModuleMetrics::builder()
            .read("input", |r| r.count(100))
            .build();

        let s = Snapshot::builder()
            .timestamp_ms(1000)
            .module_metrics("my-service", metrics)
            .build();

        assert_eq!(s.len(), 1);
        assert!(s.get("my-service").is_some());
    }

    #[test]
    fn get_module() {
        let s = Snapshot::builder()
            .module("test", |m| m.read("topic", |r| r.count(42)))
            .build();

        assert!(s.get("test").is_some());
        assert!(s.get("nonexistent").is_none());

        let m = s.get("test").unwrap();
        assert_eq!(m.total_reads(), 42);
    }

    #[test]
    fn iterate_modules() {
        let s = Snapshot::builder()
            .module("a", |m| m.read("t", |r| r.count(1)))
            .module("b", |m| m.read("t", |r| r.count(2)))
            .module("c", |m| m.read("t", |r| r.count(3)))
            .build();

        let names: Vec<_> = s.iter().map(|(name, _)| name.as_str()).collect();
        // BTreeMap iterates in sorted order
        assert_eq!(names, vec!["a", "b", "c"]);

        let total: u64 = s.iter().map(|(_, m)| m.total_reads()).sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn empty_snapshot() {
        let s = Snapshot::builder().timestamp_ms(0).build();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.total_reads(), 0);
        assert_eq!(s.total_writes(), 0);
    }

    #[test]
    fn version_is_set() {
        let s = Snapshot::builder().build();
        assert_eq!(s.version.major, crate::SCHEMA_VERSION);
    }

    #[cfg(feature = "std")]
    #[test]
    fn new_has_current_timestamp() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let s = Snapshot::new();

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        assert!(s.timestamp_ms >= before);
        assert!(s.timestamp_ms <= after);
    }

    #[cfg(feature = "std")]
    #[test]
    fn default_uses_new() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let s = Snapshot::default();

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        assert!(s.timestamp_ms >= before);
        assert!(s.timestamp_ms <= after);
    }

    #[test]
    fn zero_timestamp() {
        let s = Snapshot::with_timestamp(0);
        assert_eq!(s.timestamp_ms, 0);
    }

    #[test]
    fn max_timestamp() {
        let s = Snapshot::with_timestamp(u64::MAX);
        assert_eq!(s.timestamp_ms, u64::MAX);
    }

    #[test]
    fn many_modules() {
        let mut builder = Snapshot::builder().timestamp_ms(0);
        for i in 0..100 {
            builder = builder.module(alloc::format!("module-{}", i), |m| {
                m.read("topic", |r| r.count(i as u64))
            });
        }
        let s = builder.build();

        assert_eq!(s.len(), 100);
        assert_eq!(s.total_reads(), (0..100u64).sum::<u64>());
    }

    #[test]
    fn snapshot_builder_default() {
        let b = SnapshotBuilder::default();
        let s = b.build();
        assert!(s.is_empty());
    }

    #[test]
    fn clone_and_equality() {
        let s1 = Snapshot::builder()
            .timestamp_ms(1000)
            .module("test", |m| m.read("t", |r| r.count(42)))
            .build();
        let s2 = s1.clone();
        assert_eq!(s1, s2);
    }

    #[test]
    fn debug_format() {
        let s = Snapshot::builder()
            .module("test", |m| m.read("t", |r| r.count(1)))
            .build();
        let debug = alloc::format!("{:?}", s);
        assert!(debug.contains("Snapshot"));
        assert!(debug.contains("test"));
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

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

    #[cfg(feature = "serde")]
    #[test]
    fn serde_json_structure() {
        let s = Snapshot::builder()
            .timestamp_ms(1703160000000)
            .module("test", |m| m.read("topic", |r| r.count(42)))
            .build();

        let json: serde_json::Value = serde_json::to_value(&s).unwrap();

        assert!(json.get("version").is_some());
        assert!(json.get("timestamp_ms").is_some());
        assert!(json.get("modules").is_some());

        let version = json.get("version").unwrap();
        assert_eq!(version.get("major").unwrap(), crate::SCHEMA_VERSION);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_deserialize_from_external_json() {
        let json = r#"{
            "version": { "major": 1, "minor": 0 },
            "timestamp_ms": 1703160000000,
            "modules": {
                "my-service": {
                    "reads": {
                        "input": { "count": 100, "backlog": 5 }
                    },
                    "writes": {
                        "output": { "count": 95 }
                    }
                }
            }
        }"#;

        let s: Snapshot = serde_json::from_str(json).unwrap();
        assert_eq!(s.timestamp_ms, 1703160000000);
        assert_eq!(s.len(), 1);

        let service = s.get("my-service").unwrap();
        assert_eq!(service.reads.get("input").unwrap().count, 100);
        assert_eq!(service.reads.get("input").unwrap().backlog, Some(5));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_pretty_print() {
        let s = Snapshot::builder()
            .timestamp_ms(1000)
            .module("test", |m| m.read("t", |r| r.count(1)))
            .build();

        let pretty = serde_json::to_string_pretty(&s).unwrap();
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  ")); // indentation
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
