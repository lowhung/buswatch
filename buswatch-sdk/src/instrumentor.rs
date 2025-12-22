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
    /// Returns a handle that can be used to stop the emission.
    #[cfg(feature = "tokio")]
    pub fn start(&self) -> EmissionHandle {
        use tokio::sync::watch;

        let (stop_tx, stop_rx) = watch::channel(false);
        let state = self.state.clone();
        let outputs = self.outputs.clone();
        let interval = self.interval;

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
}
