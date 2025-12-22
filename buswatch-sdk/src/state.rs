//! Internal state management for metrics collection.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use buswatch_types::{Microseconds, ModuleMetrics, ReadMetrics, Snapshot, WriteMetrics};
use parking_lot::RwLock;

/// Thread-safe metrics for a single topic read stream.
#[derive(Debug, Default)]
pub struct ReadState {
    pub count: AtomicU64,
    pub pending_since: RwLock<Option<Instant>>,
}

/// Thread-safe metrics for a single topic write stream.
#[derive(Debug, Default)]
pub struct WriteState {
    pub count: AtomicU64,
    pub pending_since: RwLock<Option<Instant>>,
}

/// Thread-safe metrics for a single module.
#[derive(Debug, Default)]
pub struct ModuleState {
    pub reads: RwLock<BTreeMap<String, Arc<ReadState>>>,
    pub writes: RwLock<BTreeMap<String, Arc<WriteState>>>,
}

impl ModuleState {
    /// Get or create a read state for a topic.
    pub fn get_or_create_read(&self, topic: &str) -> Arc<ReadState> {
        // Fast path: check if it exists
        {
            let reads = self.reads.read();
            if let Some(state) = reads.get(topic) {
                return state.clone();
            }
        }

        // Slow path: create it
        // Double-check after acquiring write lock
        let mut reads = self.reads.write();
        reads
            .entry(topic.to_string())
            .or_insert_with(|| Arc::new(ReadState::default()))
            .clone()
    }

    /// Get or create a write state for a topic.
    pub fn get_or_create_write(&self, topic: &str) -> Arc<WriteState> {
        // Fast path: check if it exists
        {
            let writes = self.writes.read();
            if let Some(state) = writes.get(topic) {
                return state.clone();
            }
        }

        // Slow path: create it
        let mut writes = self.writes.write();
        writes
            .entry(topic.to_string())
            .or_insert_with(|| Arc::new(WriteState::default()))
            .clone()
    }

    /// Collect current metrics into a ModuleMetrics snapshot.
    pub fn collect(&self) -> ModuleMetrics {
        let now = Instant::now();

        let reads = self
            .reads
            .read()
            .iter()
            .map(|(topic, state)| {
                let count = state.count.load(Ordering::Relaxed);
                let pending = state.pending_since.read().map(|since| {
                    let duration = now.duration_since(since);
                    Microseconds::from(duration)
                });

                (
                    topic.clone(),
                    ReadMetrics {
                        count,
                        backlog: None, // SDK doesn't track backlog directly
                        pending,
                        rate: None, // Could compute if we track history
                    },
                )
            })
            .collect();

        let writes = self
            .writes
            .read()
            .iter()
            .map(|(topic, state)| {
                let count = state.count.load(Ordering::Relaxed);
                let pending = state.pending_since.read().map(|since| {
                    let duration = now.duration_since(since);
                    Microseconds::from(duration)
                });

                (
                    topic.clone(),
                    WriteMetrics {
                        count,
                        pending,
                        rate: None,
                    },
                )
            })
            .collect();

        ModuleMetrics { reads, writes }
    }
}

/// Global state for all modules.
#[derive(Debug, Default)]
pub struct GlobalState {
    pub modules: RwLock<BTreeMap<String, Arc<ModuleState>>>,
    /// Track total writes per topic across all modules (for computing backlog)
    pub topic_write_counts: RwLock<BTreeMap<String, Arc<AtomicU64>>>,
}

impl GlobalState {
    /// Register a new module or get existing one.
    pub fn register_module(&self, name: &str) -> Arc<ModuleState> {
        // Fast path
        {
            let modules = self.modules.read();
            if let Some(state) = modules.get(name) {
                return state.clone();
            }
        }

        // Slow path
        let mut modules = self.modules.write();
        modules
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(ModuleState::default()))
            .clone()
    }

    /// Get or create a global write counter for a topic.
    pub fn get_topic_write_counter(&self, topic: &str) -> Arc<AtomicU64> {
        // Fast path
        {
            let counts = self.topic_write_counts.read();
            if let Some(counter) = counts.get(topic) {
                return counter.clone();
            }
        }

        // Slow path
        let mut counts = self.topic_write_counts.write();
        counts
            .entry(topic.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone()
    }

    /// Collect all modules into a Snapshot.
    pub fn collect(&self) -> Snapshot {
        let modules = self.modules.read();
        let topic_writes = self.topic_write_counts.read();

        let mut snapshot = Snapshot::builder();

        for (name, state) in modules.iter() {
            let mut metrics = state.collect();

            // Compute backlog for each read topic
            for (topic, read_metrics) in metrics.reads.iter_mut() {
                if let Some(total_writes) = topic_writes.get(topic) {
                    let total = total_writes.load(Ordering::Relaxed);
                    if total > read_metrics.count {
                        read_metrics.backlog = Some(total - read_metrics.count);
                    }
                }
            }

            snapshot = snapshot.module_metrics(name.clone(), metrics);
        }

        snapshot.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_state_read_write() {
        let state = ModuleState::default();

        let read = state.get_or_create_read("topic1");
        read.count.fetch_add(10, Ordering::Relaxed);

        let write = state.get_or_create_write("topic2");
        write.count.fetch_add(5, Ordering::Relaxed);

        let metrics = state.collect();
        assert_eq!(metrics.reads.get("topic1").unwrap().count, 10);
        assert_eq!(metrics.writes.get("topic2").unwrap().count, 5);
    }

    #[test]
    fn test_global_state_collect() {
        let global = GlobalState::default();

        let module1 = global.register_module("service-a");
        let module2 = global.register_module("service-b");

        module1
            .get_or_create_read("events")
            .count
            .fetch_add(100, Ordering::Relaxed);
        module2
            .get_or_create_write("events")
            .count
            .fetch_add(100, Ordering::Relaxed);

        // Track global writes for backlog computation
        global
            .get_topic_write_counter("events")
            .fetch_add(100, Ordering::Relaxed);

        let snapshot = global.collect();
        assert_eq!(snapshot.modules.len(), 2);
    }
}
