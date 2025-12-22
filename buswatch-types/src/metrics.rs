//! Core metric types for message bus observability.

use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::Microseconds;

/// Metrics for a single module/consumer/producer in the message bus.
///
/// A module is any component that reads from or writes to topics.
/// This could be a microservice, a worker process, or any logical unit.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "minicbor", derive(minicbor::Encode, minicbor::Decode))]
pub struct ModuleMetrics {
    /// Metrics for topics this module reads from (subscriptions).
    #[cfg_attr(feature = "minicbor", n(0))]
    pub reads: BTreeMap<String, ReadMetrics>,

    /// Metrics for topics this module writes to (publications).
    #[cfg_attr(feature = "minicbor", n(1))]
    pub writes: BTreeMap<String, WriteMetrics>,
}

impl ModuleMetrics {
    /// Create empty module metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for module metrics.
    pub fn builder() -> ModuleMetricsBuilder {
        ModuleMetricsBuilder::new()
    }

    /// Check if the module has any activity.
    pub fn is_empty(&self) -> bool {
        self.reads.is_empty() && self.writes.is_empty()
    }

    /// Total messages read across all topics.
    pub fn total_reads(&self) -> u64 {
        self.reads.values().map(|r| r.count).sum()
    }

    /// Total messages written across all topics.
    pub fn total_writes(&self) -> u64 {
        self.writes.values().map(|w| w.count).sum()
    }
}

/// Metrics for reading from a topic (subscription/consumer).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "minicbor", derive(minicbor::Encode, minicbor::Decode))]
pub struct ReadMetrics {
    /// Number of messages successfully read.
    #[cfg_attr(feature = "minicbor", n(0))]
    pub count: u64,

    /// Number of messages waiting to be read (backlog/lag).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "minicbor", n(1))]
    pub backlog: Option<u64>,

    /// How long the consumer has been waiting for a message.
    ///
    /// If set, indicates the consumer is blocked waiting for messages.
    /// This helps identify slow producers or idle consumers.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "minicbor", n(2))]
    pub pending: Option<Microseconds>,

    /// Messages read per second (computed over a window).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "minicbor", n(3))]
    pub rate: Option<f64>,
}

impl ReadMetrics {
    /// Create new read metrics with a count.
    pub fn new(count: u64) -> Self {
        Self {
            count,
            ..Default::default()
        }
    }

    /// Create a builder for read metrics.
    pub fn builder() -> ReadMetricsBuilder {
        ReadMetricsBuilder::new()
    }

    /// Check if this read stream appears healthy.
    ///
    /// Returns false if there's a significant backlog or long pending time.
    pub fn is_healthy(&self, max_backlog: u64, max_pending: Microseconds) -> bool {
        let backlog_ok = self.backlog.map_or(true, |b| b <= max_backlog);
        let pending_ok = self.pending.map_or(true, |p| p <= max_pending);
        backlog_ok && pending_ok
    }
}

/// Metrics for writing to a topic (publication/producer).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "minicbor", derive(minicbor::Encode, minicbor::Decode))]
pub struct WriteMetrics {
    /// Number of messages successfully written.
    #[cfg_attr(feature = "minicbor", n(0))]
    pub count: u64,

    /// How long the producer has been waiting to write.
    ///
    /// If set, indicates backpressure - the topic or downstream consumers
    /// are not keeping up.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "minicbor", n(1))]
    pub pending: Option<Microseconds>,

    /// Messages written per second (computed over a window).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "minicbor", n(2))]
    pub rate: Option<f64>,
}

impl WriteMetrics {
    /// Create new write metrics with a count.
    pub fn new(count: u64) -> Self {
        Self {
            count,
            ..Default::default()
        }
    }

    /// Create a builder for write metrics.
    pub fn builder() -> WriteMetricsBuilder {
        WriteMetricsBuilder::new()
    }

    /// Check if this write stream appears healthy.
    ///
    /// Returns false if there's a long pending time (backpressure).
    pub fn is_healthy(&self, max_pending: Microseconds) -> bool {
        self.pending.map_or(true, |p| p <= max_pending)
    }
}

// ============================================================================
// Builders
// ============================================================================

/// Builder for `ModuleMetrics`.
#[derive(Debug, Default)]
pub struct ModuleMetricsBuilder {
    reads: BTreeMap<String, ReadMetrics>,
    writes: BTreeMap<String, WriteMetrics>,
}

impl ModuleMetricsBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add read metrics for a topic.
    pub fn read<F>(mut self, topic: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(ReadMetricsBuilder) -> ReadMetricsBuilder,
    {
        let metrics = f(ReadMetricsBuilder::new()).build();
        self.reads.insert(topic.into(), metrics);
        self
    }

    /// Add write metrics for a topic.
    pub fn write<F>(mut self, topic: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(WriteMetricsBuilder) -> WriteMetricsBuilder,
    {
        let metrics = f(WriteMetricsBuilder::new()).build();
        self.writes.insert(topic.into(), metrics);
        self
    }

    /// Build the module metrics.
    pub fn build(self) -> ModuleMetrics {
        ModuleMetrics {
            reads: self.reads,
            writes: self.writes,
        }
    }
}

/// Builder for `ReadMetrics`.
#[derive(Debug, Default)]
pub struct ReadMetricsBuilder {
    count: u64,
    backlog: Option<u64>,
    pending: Option<Microseconds>,
    rate: Option<f64>,
}

impl ReadMetricsBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the message count.
    pub fn count(mut self, count: u64) -> Self {
        self.count = count;
        self
    }

    /// Set the backlog (unread messages).
    pub fn backlog(mut self, backlog: u64) -> Self {
        self.backlog = Some(backlog);
        self
    }

    /// Set the pending duration.
    pub fn pending(mut self, duration: impl Into<Microseconds>) -> Self {
        self.pending = Some(duration.into());
        self
    }

    /// Set the rate (messages per second).
    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Build the read metrics.
    pub fn build(self) -> ReadMetrics {
        ReadMetrics {
            count: self.count,
            backlog: self.backlog,
            pending: self.pending,
            rate: self.rate,
        }
    }
}

/// Builder for `WriteMetrics`.
#[derive(Debug, Default)]
pub struct WriteMetricsBuilder {
    count: u64,
    pending: Option<Microseconds>,
    rate: Option<f64>,
}

impl WriteMetricsBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the message count.
    pub fn count(mut self, count: u64) -> Self {
        self.count = count;
        self
    }

    /// Set the pending duration.
    pub fn pending(mut self, duration: impl Into<Microseconds>) -> Self {
        self.pending = Some(duration.into());
        self
    }

    /// Set the rate (messages per second).
    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Build the write metrics.
    pub fn build(self) -> WriteMetrics {
        WriteMetrics {
            count: self.count,
            pending: self.pending,
            rate: self.rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    #[test]
    fn test_module_metrics_builder() {
        let metrics = ModuleMetrics::builder()
            .read("input", |r| r.count(100).backlog(5))
            .write("output", |w| w.count(95))
            .build();

        assert_eq!(metrics.total_reads(), 100);
        assert_eq!(metrics.total_writes(), 95);
        assert_eq!(metrics.reads.get("input").unwrap().backlog, Some(5));
    }

    #[test]
    fn test_read_metrics_health() {
        let healthy = ReadMetrics::new(100);
        assert!(healthy.is_healthy(10, Microseconds::from_secs(5)));

        let with_backlog = ReadMetrics::builder().count(100).backlog(20).build();
        assert!(!with_backlog.is_healthy(10, Microseconds::from_secs(5)));

        let with_pending = ReadMetrics::builder()
            .count(100)
            .pending(Duration::from_secs(10))
            .build();
        assert!(!with_pending.is_healthy(10, Microseconds::from_secs(5)));
    }
}
