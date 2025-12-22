//! Module handle for recording metrics.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use crate::state::{GlobalState, ModuleState};

/// A handle for recording metrics for a specific module.
///
/// This is the primary interface for instrumenting your message bus.
/// Obtain a handle by calling `Instrumentor::register()`.
///
/// # Example
///
/// ```rust
/// use buswatch_sdk::Instrumentor;
///
/// let instrumentor = Instrumentor::new();
/// let handle = instrumentor.register("my-service");
///
/// // Record a read from a topic
/// handle.record_read("orders.new", 1);
///
/// // Record a write to a topic
/// handle.record_write("orders.processed", 1);
///
/// // Track pending operations
/// let guard = handle.start_read("orders.new");
/// // ... do the read ...
/// drop(guard); // Clears pending state
/// ```
#[derive(Clone)]
pub struct ModuleHandle {
    pub(crate) state: Arc<ModuleState>,
    pub(crate) global: Arc<GlobalState>,
    pub(crate) name: String,
}

impl ModuleHandle {
    /// Record that messages were read from a topic.
    ///
    /// # Arguments
    ///
    /// * `topic` - The topic name
    /// * `count` - Number of messages read
    pub fn record_read(&self, topic: &str, count: u64) {
        let read_state = self.state.get_or_create_read(topic);
        read_state.count.fetch_add(count, Ordering::Relaxed);
    }

    /// Record that messages were written to a topic.
    ///
    /// # Arguments
    ///
    /// * `topic` - The topic name
    /// * `count` - Number of messages written
    pub fn record_write(&self, topic: &str, count: u64) {
        let write_state = self.state.get_or_create_write(topic);
        write_state.count.fetch_add(count, Ordering::Relaxed);

        // Also update global write counter for backlog computation
        let global_counter = self.global.get_topic_write_counter(topic);
        global_counter.fetch_add(count, Ordering::Relaxed);
    }

    /// Start tracking a pending read operation.
    ///
    /// Returns a guard that clears the pending state when dropped.
    /// This is useful for tracking how long reads are blocked.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use buswatch_sdk::Instrumentor;
    /// # let instrumentor = Instrumentor::new();
    /// # let handle = instrumentor.register("test");
    /// let guard = handle.start_read("events");
    /// // ... blocking read operation ...
    /// drop(guard); // Clears pending state
    /// handle.record_read("events", 1);
    /// ```
    pub fn start_read(&self, topic: &str) -> PendingGuard {
        let read_state = self.state.get_or_create_read(topic);
        *read_state.pending_since.write() = Some(Instant::now());

        PendingGuard {
            state: PendingState::Read(read_state),
        }
    }

    /// Start tracking a pending write operation.
    ///
    /// Returns a guard that clears the pending state when dropped.
    /// This is useful for tracking backpressure (slow consumers).
    pub fn start_write(&self, topic: &str) -> PendingGuard {
        let write_state = self.state.get_or_create_write(topic);
        *write_state.pending_since.write() = Some(Instant::now());

        PendingGuard {
            state: PendingState::Write(write_state),
        }
    }

    /// Set the pending duration for a read directly.
    ///
    /// Use this if you're computing pending time yourself rather than
    /// using the guard-based API.
    pub fn set_read_pending(&self, topic: &str, since: Option<Instant>) {
        let read_state = self.state.get_or_create_read(topic);
        *read_state.pending_since.write() = since;
    }

    /// Set the pending duration for a write directly.
    pub fn set_write_pending(&self, topic: &str, since: Option<Instant>) {
        let write_state = self.state.get_or_create_write(topic);
        *write_state.pending_since.write() = since;
    }

    /// Get the module name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Debug for ModuleHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleHandle")
            .field("name", &self.name)
            .finish()
    }
}

/// Internal state for pending guard.
enum PendingState {
    Read(Arc<crate::state::ReadState>),
    Write(Arc<crate::state::WriteState>),
}

/// Guard that clears pending state when dropped.
///
/// This implements RAII-style tracking of pending operations.
pub struct PendingGuard {
    state: PendingState,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        match &self.state {
            PendingState::Read(s) => *s.pending_since.write() = None,
            PendingState::Write(s) => *s.pending_since.write() = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GlobalState;

    fn create_handle() -> ModuleHandle {
        let global = Arc::new(GlobalState::default());
        let state = global.register_module("test");
        ModuleHandle {
            state,
            global,
            name: "test".to_string(),
        }
    }

    #[test]
    fn test_record_read() {
        let handle = create_handle();
        handle.record_read("topic", 5);
        handle.record_read("topic", 3);

        let metrics = handle.state.collect();
        assert_eq!(metrics.reads.get("topic").unwrap().count, 8);
    }

    #[test]
    fn test_record_write() {
        let handle = create_handle();
        handle.record_write("topic", 10);

        let metrics = handle.state.collect();
        assert_eq!(metrics.writes.get("topic").unwrap().count, 10);
    }

    #[test]
    fn test_pending_guard() {
        let handle = create_handle();

        {
            let _guard = handle.start_read("topic");
            // While guard is held, pending should be set
            let state = handle.state.get_or_create_read("topic");
            assert!(state.pending_since.read().is_some());
        }

        // After guard is dropped, pending should be cleared
        let state = handle.state.get_or_create_read("topic");
        assert!(state.pending_since.read().is_none());
    }
}
