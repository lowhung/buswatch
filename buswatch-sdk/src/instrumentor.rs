//! The main Instrumentor type for collecting and emitting metrics.

use std::sync::Arc;
use std::time::Duration;

use crate::handle::ModuleHandle;
use crate::output::Output;
use crate::state::GlobalState;

/// The main entry point for instrumenting a message bus.
///
/// An Instrumentor collects metrics from registered modules and periodically
/// emits snapshots to configured outputs.
///
/// # Example
///
/// ```rust,no_run
/// use buswatch_sdk::{Instrumentor, Output};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() {
///     let instrumentor = Instrumentor::builder()
///         .output(Output::file("metrics.json"))
///         .interval(Duration::from_secs(1))
///         .build();
///
///     let handle = instrumentor.register("my-service");
///
///     // Start background emission
///     instrumentor.start();
///
///     // Record some metrics
///     handle.record_read("events", 10);
///     handle.record_write("results", 10);
///
///     // Keep the application running
///     tokio::time::sleep(Duration::from_secs(5)).await;
/// }
/// ```
#[derive(Debug)]
pub struct Instrumentor {
    state: Arc<GlobalState>,
    outputs: Arc<Vec<Output>>,
    interval: Duration,
}

impl Instrumentor {
    /// Create a new instrumentor with default settings.
    ///
    /// By default, no outputs are configured and the interval is 1 second.
    pub fn new() -> Self {
        Self {
            state: Arc::new(GlobalState::default()),
            outputs: Arc::new(Vec::new()),
            interval: Duration::from_secs(1),
        }
    }

    /// Create a builder for configuring the instrumentor.
    pub fn builder() -> InstrumentorBuilder {
        InstrumentorBuilder::new()
    }

    /// Register a module and get a handle for recording metrics.
    ///
    /// If a module with this name already exists, returns a handle to
    /// the existing module.
    ///
    /// # Arguments
    ///
    /// * `name` - The module name (e.g., "order-processor", "notification-sender")
    pub fn register(&self, name: &str) -> ModuleHandle {
        let module_state = self.state.register_module(name);
        ModuleHandle {
            state: module_state,
            global: self.state.clone(),
            name: name.to_string(),
        }
    }

    /// Unregister a module and remove it from internal state.
    ///
    /// This removes the module and all its associated metrics from future snapshots.
    /// Returns `true` if the module was found and removed, `false` if it didn't exist.
    ///
    /// Note: Any existing `ModuleHandle` instances for this module will continue to work
    /// and accumulate metrics, but those metrics will not appear in snapshots unless
    /// the module is re-registered.
    ///
    /// # Arguments
    ///
    /// * `name` - The module name to unregister
    ///
    /// # Example
    ///
    /// ```rust
    /// use buswatch_sdk::Instrumentor;
    ///
    /// let instrumentor = Instrumentor::new();
    /// let handle = instrumentor.register("temp-worker");
    ///
    /// // ... use the handle ...
    ///
    /// // When done, unregister the module
    /// let removed = instrumentor.unregister("temp-worker");
    /// assert!(removed);
    /// ```
    pub fn unregister(&self, name: &str) -> bool {
        self.state.unregister_module(name)
    }

    /// Collect a snapshot of all current metrics.
    ///
    /// This is useful if you want to emit snapshots manually rather than
    /// using the background emission.
    pub fn collect(&self) -> buswatch_types::Snapshot {
        self.state.collect()
    }

    /// Start background emission of snapshots.
    ///
    /// This spawns a tokio task that periodically collects and emits
    /// snapshots to all configured outputs.
    ///
    /// For Prometheus outputs, this also starts the HTTP server to serve metrics.
    ///
    /// Returns a handle that can be used to stop the emission.
    #[cfg(feature = "tokio")]
    pub fn start(&self) -> EmissionHandle {
        use tokio::sync::watch;

        let (stop_tx, stop_rx) = watch::channel(false);
        let state = self.state.clone();
        let outputs = self.outputs.clone();
        let interval = self.interval;

        // Start Prometheus HTTP servers for any Prometheus outputs
        #[cfg(feature = "prometheus")]
        for output in outputs.iter() {
            if let Output::Prometheus(exporter) = output {
                exporter.start_server();
            }
        }

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            let mut stop_rx = stop_rx;

            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {
                        let snapshot = state.collect();
                        for output in outputs.iter() {
                            let _ = output.emit(&snapshot).await;
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        EmissionHandle { stop_tx }
    }

    /// Emit a snapshot to all outputs immediately.
    #[cfg(feature = "tokio")]
    pub async fn emit_now(&self) {
        let snapshot = self.state.collect();
        for output in self.outputs.iter() {
            let _ = output.emit(&snapshot).await;
        }
    }
}

impl Default for Instrumentor {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for configuring an Instrumentor.
#[derive(Debug, Default)]
pub struct InstrumentorBuilder {
    outputs: Vec<Output>,
    interval: Option<Duration>,
}

impl InstrumentorBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an output destination.
    ///
    /// Multiple outputs can be added; snapshots will be emitted to all of them.
    pub fn output(mut self, output: Output) -> Self {
        self.outputs.push(output);
        self
    }

    /// Set the emission interval.
    ///
    /// Defaults to 1 second if not specified.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Build the instrumentor.
    pub fn build(self) -> Instrumentor {
        Instrumentor {
            state: Arc::new(GlobalState::default()),
            outputs: Arc::new(self.outputs),
            interval: self.interval.unwrap_or(Duration::from_secs(1)),
        }
    }
}

/// Handle for controlling background emission.
///
/// Drop this handle to stop emission, or call `stop()` explicitly.
#[cfg(feature = "tokio")]
pub struct EmissionHandle {
    stop_tx: tokio::sync::watch::Sender<bool>,
}

#[cfg(feature = "tokio")]
impl EmissionHandle {
    /// Stop background emission.
    pub fn stop(self) {
        let _ = self.stop_tx.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrumentor_new() {
        let instrumentor = Instrumentor::new();
        let handle = instrumentor.register("test-module");
        assert_eq!(handle.name(), "test-module");
    }

    #[test]
    fn test_instrumentor_collect() {
        let instrumentor = Instrumentor::new();
        let handle = instrumentor.register("producer");

        handle.record_write("events", 100);
        handle.record_read("commands", 50);

        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 1);

        let metrics = snapshot.modules.get("producer").unwrap();
        assert_eq!(metrics.writes.get("events").unwrap().count, 100);
        assert_eq!(metrics.reads.get("commands").unwrap().count, 50);
    }

    #[test]
    fn test_multiple_modules() {
        let instrumentor = Instrumentor::new();

        let producer = instrumentor.register("producer");
        let consumer = instrumentor.register("consumer");

        producer.record_write("events", 100);
        consumer.record_read("events", 95);

        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 2);

        // Check backlog computation
        let consumer_metrics = snapshot.modules.get("consumer").unwrap();
        let events_read = consumer_metrics.reads.get("events").unwrap();
        assert_eq!(events_read.count, 95);
        assert_eq!(events_read.backlog, Some(5)); // 100 written - 95 read = 5 backlog
    }

    #[test]
    fn test_builder() {
        let instrumentor = Instrumentor::builder()
            .output(Output::file("test.json"))
            .interval(Duration::from_millis(500))
            .build();

        assert_eq!(instrumentor.interval, Duration::from_millis(500));
        assert_eq!(instrumentor.outputs.len(), 1);
    }

    #[test]
    fn default_interval_is_one_second() {
        let instrumentor = Instrumentor::new();
        assert_eq!(instrumentor.interval, Duration::from_secs(1));
    }

    #[test]
    fn default_has_no_outputs() {
        let instrumentor = Instrumentor::new();
        assert!(instrumentor.outputs.is_empty());
    }

    #[test]
    fn builder_can_add_multiple_outputs() {
        let instrumentor = Instrumentor::builder()
            .output(Output::file("metrics1.json"))
            .output(Output::file("metrics2.json"))
            .output(Output::tcp("localhost:9090"))
            .build();

        assert_eq!(instrumentor.outputs.len(), 3);
    }

    #[test]
    fn register_same_module_twice_returns_same_state() {
        let instrumentor = Instrumentor::new();

        let handle1 = instrumentor.register("service");
        let handle2 = instrumentor.register("service");

        handle1.record_read("topic", 10);
        handle2.record_read("topic", 5);

        let snapshot = instrumentor.collect();
        let metrics = snapshot.modules.get("service").unwrap();
        assert_eq!(metrics.reads.get("topic").unwrap().count, 15);
    }

    #[test]
    fn collect_returns_snapshot_with_timestamp() {
        let instrumentor = Instrumentor::new();
        let _ = instrumentor.register("test");

        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let snapshot = instrumentor.collect();

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        assert!(snapshot.timestamp_ms >= before);
        assert!(snapshot.timestamp_ms <= after);
    }

    #[test]
    fn collect_includes_schema_version() {
        let instrumentor = Instrumentor::new();
        let snapshot = instrumentor.collect();
        assert!(snapshot.version.is_compatible());
    }

    #[test]
    fn instrumentor_default_same_as_new() {
        let i1 = Instrumentor::new();
        let i2 = Instrumentor::default();

        assert_eq!(i1.interval, i2.interval);
        assert_eq!(i1.outputs.len(), i2.outputs.len());
    }

    #[test]
    fn builder_default_interval_when_not_specified() {
        let instrumentor = Instrumentor::builder()
            .output(Output::file("test.json"))
            .build();

        assert_eq!(instrumentor.interval, Duration::from_secs(1));
    }

    #[test]
    fn complex_multi_module_scenario() {
        let instrumentor = Instrumentor::new();

        // Simulate a pipeline: API -> Processor -> Notifier
        let api = instrumentor.register("api");
        let processor = instrumentor.register("processor");
        let notifier = instrumentor.register("notifier");

        // API receives requests and writes to orders topic
        api.record_write("orders", 1000);

        // Processor reads orders, writes to notifications
        processor.record_read("orders", 950);
        processor.record_write("notifications", 950);

        // Notifier reads notifications
        notifier.record_read("notifications", 900);

        let snapshot = instrumentor.collect();

        // Verify backlogs
        let proc_metrics = snapshot.modules.get("processor").unwrap();
        assert_eq!(proc_metrics.reads.get("orders").unwrap().backlog, Some(50)); // 1000 - 950

        let notif_metrics = snapshot.modules.get("notifier").unwrap();
        assert_eq!(
            notif_metrics.reads.get("notifications").unwrap().backlog,
            Some(50)
        ); // 950 - 900
    }

    #[test]
    fn unregister_existing_module() {
        let instrumentor = Instrumentor::new();

        let handle = instrumentor.register("service-a");
        handle.record_read("topic", 100);

        // Verify module exists
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 1);
        assert!(snapshot.modules.contains_key("service-a"));

        // Unregister the module
        let removed = instrumentor.unregister("service-a");
        assert!(removed, "Should return true when module exists");

        // Verify module is gone
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 0);
        assert!(!snapshot.modules.contains_key("service-a"));
    }

    #[test]
    fn unregister_nonexistent_module() {
        let instrumentor = Instrumentor::new();

        let _handle = instrumentor.register("service-a");

        // Try to unregister a module that doesn't exist
        let removed = instrumentor.unregister("service-b");
        assert!(!removed, "Should return false when module doesn't exist");

        // Original module should still be there
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 1);
        assert!(snapshot.modules.contains_key("service-a"));
    }

    #[test]
    fn unregister_removes_metrics_from_snapshot() {
        let instrumentor = Instrumentor::new();

        let producer = instrumentor.register("producer");
        let consumer = instrumentor.register("consumer");

        producer.record_write("events", 100);
        consumer.record_read("events", 50);

        // Both modules present
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 2);
        assert_eq!(
            snapshot.modules.get("producer").unwrap().writes.get("events").unwrap().count,
            100
        );

        // Unregister producer
        instrumentor.unregister("producer");

        // Only consumer remains
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 1);
        assert!(snapshot.modules.contains_key("consumer"));
        assert!(!snapshot.modules.contains_key("producer"));
    }

    #[test]
    fn can_reregister_after_unregister() {
        let instrumentor = Instrumentor::new();

        // Register and record metrics
        let handle1 = instrumentor.register("worker");
        handle1.record_read("jobs", 50);

        let snapshot = instrumentor.collect();
        assert_eq!(
            snapshot.modules.get("worker").unwrap().reads.get("jobs").unwrap().count,
            50
        );

        // Unregister
        instrumentor.unregister("worker");

        // Re-register - should get fresh state
        let handle2 = instrumentor.register("worker");
        let snapshot = instrumentor.collect();

        // New module should start with empty metrics
        let metrics = snapshot.modules.get("worker").unwrap();
        assert_eq!(metrics.reads.len(), 0, "Re-registered module should have no topics");

        // Add new metrics
        handle2.record_read("jobs", 25);

        let snapshot = instrumentor.collect();
        assert_eq!(
            snapshot.modules.get("worker").unwrap().reads.get("jobs").unwrap().count,
            25
        );
    }

    #[test]
    fn old_handle_still_works_after_unregister() {
        let instrumentor = Instrumentor::new();

        let handle = instrumentor.register("service");
        handle.record_write("events", 100);

        // Unregister the module
        instrumentor.unregister("service");

        // Handle can still record metrics (to its internal state)
        handle.record_write("events", 50);

        // But they won't appear in snapshots
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 0);

        // Re-registering creates a new module with fresh state
        let new_handle = instrumentor.register("service");
        let snapshot = instrumentor.collect();

        // The new module starts fresh (old handle's metrics are not visible)
        let metrics = snapshot.modules.get("service").unwrap();
        assert_eq!(metrics.writes.len(), 0);

        // But the old handle's internal state is still at 150
        // (this is implementation detail - old handle keeps its Arc to old state)
        new_handle.record_write("events", 10);
        let snapshot = instrumentor.collect();
        assert_eq!(
            snapshot.modules.get("service").unwrap().writes.get("events").unwrap().count,
            10
        );
    }

    #[test]
    fn unregister_one_module_does_not_affect_others() {
        let instrumentor = Instrumentor::new();

        let service_a = instrumentor.register("service-a");
        let service_b = instrumentor.register("service-b");
        let service_c = instrumentor.register("service-c");

        service_a.record_read("topic", 10);
        service_b.record_read("topic", 20);
        service_c.record_read("topic", 30);

        // Unregister service-b
        instrumentor.unregister("service-b");

        // service-a and service-c should remain
        let snapshot = instrumentor.collect();
        assert_eq!(snapshot.modules.len(), 2);
        assert!(snapshot.modules.contains_key("service-a"));
        assert!(!snapshot.modules.contains_key("service-b"));
        assert!(snapshot.modules.contains_key("service-c"));

        assert_eq!(
            snapshot.modules.get("service-a").unwrap().reads.get("topic").unwrap().count,
            10
        );
        assert_eq!(
            snapshot.modules.get("service-c").unwrap().reads.get("topic").unwrap().count,
            30
        );
    }

    #[test]
    fn unregister_multiple_times_is_safe() {
        let instrumentor = Instrumentor::new();

        instrumentor.register("service");

        // First unregister succeeds
        assert!(instrumentor.unregister("service"));

        // Subsequent unregisters return false
        assert!(!instrumentor.unregister("service"));
        assert!(!instrumentor.unregister("service"));
    }

}
