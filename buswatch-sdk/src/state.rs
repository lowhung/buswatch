//! Internal state management for metrics collection.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use buswatch_types::{Microseconds, ModuleMetrics, ReadMetrics, Snapshot, WriteMetrics};
use parking_lot::RwLock;

/// Thread-safe metrics for a single topic read stream.
#[derive(Debug)]
pub struct ReadState {
    pub count: AtomicU64,
    pub pending_since: RwLock<Option<Instant>>,
    /// Previous count and timestamp for rate computation
    pub prev_snapshot: RwLock<Option<(u64, Instant)>>,
}

impl Default for ReadState {
    fn default() -> Self {
        Self {
            count: AtomicU64::new(0),
            pending_since: RwLock::new(None),
            prev_snapshot: RwLock::new(None),
        }
    }
}

/// Thread-safe metrics for a single topic write stream.
#[derive(Debug)]
pub struct WriteState {
    pub count: AtomicU64,
    pub pending_since: RwLock<Option<Instant>>,
    /// Previous count and timestamp for rate computation
    pub prev_snapshot: RwLock<Option<(u64, Instant)>>,
}

impl Default for WriteState {
    fn default() -> Self {
        Self {
            count: AtomicU64::new(0),
            pending_since: RwLock::new(None),
            prev_snapshot: RwLock::new(None),
        }
    }
}

/// Compute rate (messages per second) from previous and current state.
fn compute_rate(prev: Option<(u64, Instant)>, current_count: u64, now: Instant) -> Option<f64> {
    let (prev_count, prev_time) = prev?;
    let elapsed = now.duration_since(prev_time);
    let elapsed_secs = elapsed.as_secs_f64();

    // Avoid division by zero and require at least 10ms between samples
    if elapsed_secs < 0.01 {
        return None;
    }

    let delta = current_count.saturating_sub(prev_count);
    Some(delta as f64 / elapsed_secs)
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
    ///
    /// This also updates the previous snapshot for rate computation.
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

                // Compute rate from previous snapshot
                let prev = *state.prev_snapshot.read();
                let rate = compute_rate(prev, count, now);

                // Update previous snapshot for next collection
                *state.prev_snapshot.write() = Some((count, now));

                (
                    topic.clone(),
                    ReadMetrics {
                        count,
                        backlog: None, // SDK computes backlog at GlobalState level
                        pending,
                        rate,
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

                // Compute rate from previous snapshot
                let prev = *state.prev_snapshot.read();
                let rate = compute_rate(prev, count, now);

                // Update previous snapshot for next collection
                *state.prev_snapshot.write() = Some((count, now));

                (
                    topic.clone(),
                    WriteMetrics {
                        count,
                        pending,
                        rate,
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

    /// Unregister a module and remove it from internal state.
    ///
    /// Returns `true` if the module was found and removed, `false` if it didn't exist.
    ///
    /// Note: This does not affect global topic write counters, as those are shared
    /// across all modules and used for backlog computation.
    pub fn unregister_module(&self, name: &str) -> bool {
        let mut modules = self.modules.write();
        modules.remove(name).is_some()
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

    #[test]
    fn rate_is_none_on_first_collection() {
        let state = ModuleState::default();

        state
            .get_or_create_read("topic")
            .count
            .fetch_add(100, Ordering::Relaxed);
        state
            .get_or_create_write("output")
            .count
            .fetch_add(50, Ordering::Relaxed);

        let metrics = state.collect();

        // First collection should have no rate (no previous snapshot)
        assert_eq!(metrics.reads.get("topic").unwrap().rate, None);
        assert_eq!(metrics.writes.get("output").unwrap().rate, None);
    }

    #[test]
    fn rate_computed_on_second_collection() {
        let state = ModuleState::default();

        let read = state.get_or_create_read("topic");
        let write = state.get_or_create_write("output");

        // Initial state
        read.count.store(0, Ordering::Relaxed);
        write.count.store(0, Ordering::Relaxed);

        // First collection to establish baseline
        let _ = state.collect();

        // Wait a bit and add some messages
        std::thread::sleep(std::time::Duration::from_millis(50));
        read.count.fetch_add(100, Ordering::Relaxed);
        write.count.fetch_add(50, Ordering::Relaxed);

        // Second collection should compute rate
        let metrics = state.collect();

        let read_rate = metrics.reads.get("topic").unwrap().rate;
        let write_rate = metrics.writes.get("output").unwrap().rate;

        assert!(read_rate.is_some(), "Read rate should be computed");
        assert!(write_rate.is_some(), "Write rate should be computed");

        // Rate should be approximately 100 messages / 0.05 seconds = 2000 msg/s
        // Allow for timing variations
        let read_rate = read_rate.unwrap();
        assert!(
            read_rate > 500.0 && read_rate < 10000.0,
            "Read rate {} should be reasonable",
            read_rate
        );

        let write_rate = write_rate.unwrap();
        assert!(
            write_rate > 250.0 && write_rate < 5000.0,
            "Write rate {} should be reasonable",
            write_rate
        );
    }

    #[test]
    fn rate_handles_zero_delta() {
        let state = ModuleState::default();

        let read = state.get_or_create_read("topic");
        read.count.store(100, Ordering::Relaxed);

        // First collection
        let _ = state.collect();

        // Wait but don't add any messages
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Second collection
        let metrics = state.collect();

        let rate = metrics.reads.get("topic").unwrap().rate;
        assert!(rate.is_some());
        assert_eq!(rate.unwrap(), 0.0, "Rate should be 0 when no new messages");
    }

    #[test]
    fn compute_rate_function_basic() {
        let now = Instant::now();
        let one_sec_ago = now - std::time::Duration::from_secs(1);

        // 100 messages in 1 second = 100 msg/s
        let rate = compute_rate(Some((0, one_sec_ago)), 100, now);
        assert!(rate.is_some());
        let rate = rate.unwrap();
        assert!(
            (rate - 100.0).abs() < 1.0,
            "Rate should be ~100, got {}",
            rate
        );
    }

    #[test]
    fn compute_rate_returns_none_when_elapsed_too_short() {
        let now = Instant::now();
        let just_now = now - std::time::Duration::from_millis(5); // Only 5ms

        let rate = compute_rate(Some((0, just_now)), 100, now);
        assert!(rate.is_none(), "Rate should be None when elapsed < 10ms");
    }

    #[test]
    fn compute_rate_returns_none_when_no_previous() {
        let now = Instant::now();
        let rate = compute_rate(None, 100, now);
        assert!(rate.is_none());
    }

    #[test]
    fn unregister_module_removes_module() {
        let global = GlobalState::default();

        let module1 = global.register_module("service-a");
        let module2 = global.register_module("service-b");

        module1
            .get_or_create_read("topic")
            .count
            .fetch_add(10, Ordering::Relaxed);
        module2
            .get_or_create_read("topic")
            .count
            .fetch_add(20, Ordering::Relaxed);

        // Verify both modules exist
        let snapshot = global.collect();
        assert_eq!(snapshot.modules.len(), 2);

        // Unregister service-a
        let removed = global.unregister_module("service-a");
        assert!(removed, "Should return true when module is removed");

        // Verify only service-b remains
        let snapshot = global.collect();
        assert_eq!(snapshot.modules.len(), 1);
        assert!(snapshot.modules.contains_key("service-b"));
        assert!(!snapshot.modules.contains_key("service-a"));
    }

    #[test]
    fn unregister_nonexistent_module_returns_false() {
        let global = GlobalState::default();

        let _module = global.register_module("service-a");

        // Try to unregister a module that doesn't exist
        let removed = global.unregister_module("service-b");
        assert!(!removed, "Should return false when module doesn't exist");

        // service-a should still be there
        let snapshot = global.collect();
        assert_eq!(snapshot.modules.len(), 1);
        assert!(snapshot.modules.contains_key("service-a"));
    }

    #[test]
    fn unregister_module_does_not_affect_global_write_counters() {
        let global = GlobalState::default();

        let producer = global.register_module("producer");
        producer
            .get_or_create_write("events")
            .count
            .fetch_add(100, Ordering::Relaxed);
        global
            .get_topic_write_counter("events")
            .fetch_add(100, Ordering::Relaxed);

        // Verify global counter is set
        let counter = global.get_topic_write_counter("events");
        assert_eq!(counter.load(Ordering::Relaxed), 100);

        // Unregister the producer
        global.unregister_module("producer");

        // Global counter should still be 100
        let counter = global.get_topic_write_counter("events");
        assert_eq!(counter.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn can_reregister_after_unregister() {
        let global = GlobalState::default();

        // Register and record some metrics
        let module1 = global.register_module("service");
        module1
            .get_or_create_read("topic")
            .count
            .fetch_add(50, Ordering::Relaxed);

        let snapshot = global.collect();
        assert_eq!(snapshot.modules.get("service").unwrap().reads.get("topic").unwrap().count, 50);

        // Unregister
        global.unregister_module("service");

        // Re-register and verify metrics start fresh
        let module2 = global.register_module("service");
        let snapshot = global.collect();

        // New module should have fresh state (count = 0)
        let metrics = snapshot.modules.get("service").unwrap();
        assert_eq!(metrics.reads.len(), 0, "Re-registered module should start with no topics");

        // Add some new metrics
        module2
            .get_or_create_read("topic")
            .count
            .fetch_add(10, Ordering::Relaxed);

        let snapshot = global.collect();
        assert_eq!(snapshot.modules.get("service").unwrap().reads.get("topic").unwrap().count, 10);
    }

    #[test]
    fn unregister_module_multiple_times_is_safe() {
        let global = GlobalState::default();

        global.register_module("service");

        // First unregister should succeed
        assert!(global.unregister_module("service"));

        // Second unregister should return false
        assert!(!global.unregister_module("service"));

        // Third time for good measure
        assert!(!global.unregister_module("service"));
    }

}
