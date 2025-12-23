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

    #[test]
    fn get_or_create_read_returns_same_arc_on_second_call() {
        let state = ModuleState::default();

        let read1 = state.get_or_create_read("topic");
        let read2 = state.get_or_create_read("topic");

        // Should be the same Arc (pointer equality)
        assert!(Arc::ptr_eq(&read1, &read2));

        // Mutations should be visible through both handles
        read1.count.fetch_add(10, Ordering::Relaxed);
        assert_eq!(read2.count.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn get_or_create_write_returns_same_arc_on_second_call() {
        let state = ModuleState::default();

        let write1 = state.get_or_create_write("topic");
        let write2 = state.get_or_create_write("topic");

        assert!(Arc::ptr_eq(&write1, &write2));

        write1.count.fetch_add(5, Ordering::Relaxed);
        assert_eq!(write2.count.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn register_module_returns_same_arc_on_second_call() {
        let global = GlobalState::default();

        let m1 = global.register_module("service");
        let m2 = global.register_module("service");

        assert!(Arc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn backlog_computed_correctly_when_writes_exceed_reads() {
        let global = GlobalState::default();

        let producer = global.register_module("producer");
        let consumer = global.register_module("consumer");

        // Producer writes 100 messages
        producer
            .get_or_create_write("events")
            .count
            .fetch_add(100, Ordering::Relaxed);
        global
            .get_topic_write_counter("events")
            .fetch_add(100, Ordering::Relaxed);

        // Consumer reads 70 messages
        consumer
            .get_or_create_read("events")
            .count
            .fetch_add(70, Ordering::Relaxed);

        let snapshot = global.collect();
        let consumer_metrics = snapshot.modules.get("consumer").unwrap();
        let events_read = consumer_metrics.reads.get("events").unwrap();

        assert_eq!(events_read.count, 70);
        assert_eq!(events_read.backlog, Some(30)); // 100 - 70 = 30 unread
    }

    #[test]
    fn backlog_is_none_when_no_global_writes_tracked() {
        let global = GlobalState::default();

        let consumer = global.register_module("consumer");
        consumer
            .get_or_create_read("events")
            .count
            .fetch_add(50, Ordering::Relaxed);

        // No writes tracked globally
        let snapshot = global.collect();
        let consumer_metrics = snapshot.modules.get("consumer").unwrap();
        let events_read = consumer_metrics.reads.get("events").unwrap();

        assert_eq!(events_read.backlog, None);
    }

    #[test]
    fn backlog_is_none_when_reads_equal_writes() {
        let global = GlobalState::default();

        let producer = global.register_module("producer");
        let consumer = global.register_module("consumer");

        producer
            .get_or_create_write("events")
            .count
            .fetch_add(100, Ordering::Relaxed);
        global
            .get_topic_write_counter("events")
            .fetch_add(100, Ordering::Relaxed);

        consumer
            .get_or_create_read("events")
            .count
            .fetch_add(100, Ordering::Relaxed);

        let snapshot = global.collect();
        let consumer_metrics = snapshot.modules.get("consumer").unwrap();
        let events_read = consumer_metrics.reads.get("events").unwrap();

        // No backlog when fully caught up
        assert_eq!(events_read.backlog, None);
    }

    #[test]
    fn pending_time_captured_in_collect() {
        let state = ModuleState::default();

        let read = state.get_or_create_read("topic");
        *read.pending_since.write() = Some(Instant::now());

        // Small sleep to ensure measurable pending time
        std::thread::sleep(std::time::Duration::from_millis(5));

        let metrics = state.collect();
        let pending = metrics.reads.get("topic").unwrap().pending;

        assert!(pending.is_some());
        assert!(pending.unwrap().as_micros() >= 5000); // At least 5ms
    }

    #[test]
    fn concurrent_increments_are_thread_safe() {
        use std::thread;

        let global = Arc::new(GlobalState::default());
        let module = global.register_module("test");

        let mut handles = vec![];
        for _ in 0..10 {
            let m = module.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    m.get_or_create_read("topic")
                        .count
                        .fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let metrics = module.collect();
        assert_eq!(metrics.reads.get("topic").unwrap().count, 1000);
    }

    #[test]
    fn multiple_topics_tracked_independently() {
        let state = ModuleState::default();

        state
            .get_or_create_read("topic-a")
            .count
            .fetch_add(10, Ordering::Relaxed);
        state
            .get_or_create_read("topic-b")
            .count
            .fetch_add(20, Ordering::Relaxed);
        state
            .get_or_create_write("topic-c")
            .count
            .fetch_add(30, Ordering::Relaxed);

        let metrics = state.collect();
        assert_eq!(metrics.reads.get("topic-a").unwrap().count, 10);
        assert_eq!(metrics.reads.get("topic-b").unwrap().count, 20);
        assert_eq!(metrics.writes.get("topic-c").unwrap().count, 30);
    }
}
