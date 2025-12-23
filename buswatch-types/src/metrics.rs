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

    // ========================================================================
    // ReadMetrics Tests
    // ========================================================================

    #[test]
    fn read_metrics_new_with_count() {
        let r = ReadMetrics::new(100);
        assert_eq!(r.count, 100);
        assert!(r.backlog.is_none());
        assert!(r.pending.is_none());
        assert!(r.rate.is_none());
    }

    #[test]
    fn read_metrics_builder_all_fields() {
        let r = ReadMetrics::builder()
            .count(500)
            .backlog(50)
            .pending(Duration::from_secs(2))
            .rate(25.5)
            .build();

        assert_eq!(r.count, 500);
        assert_eq!(r.backlog, Some(50));
        assert_eq!(r.pending, Some(Microseconds::from_secs(2)));
        assert_eq!(r.rate, Some(25.5));
    }

    #[test]
    fn read_metrics_builder_partial_fields() {
        let r = ReadMetrics::builder().count(100).backlog(10).build();

        assert_eq!(r.count, 100);
        assert_eq!(r.backlog, Some(10));
        assert!(r.pending.is_none());
        assert!(r.rate.is_none());
    }

    #[test]
    fn read_metrics_builder_default_count() {
        let r = ReadMetrics::builder().build();
        assert_eq!(r.count, 0);
    }

    #[test]
    fn read_metrics_is_healthy_with_no_issues() {
        let r = ReadMetrics::new(100);
        assert!(r.is_healthy(10, Microseconds::from_secs(5)));
    }

    #[test]
    fn read_metrics_is_healthy_backlog_at_threshold() {
        let r = ReadMetrics::builder().count(100).backlog(10).build();
        assert!(r.is_healthy(10, Microseconds::from_secs(5))); // equal is healthy
    }

    #[test]
    fn read_metrics_is_unhealthy_backlog_exceeds_threshold() {
        let r = ReadMetrics::builder().count(100).backlog(11).build();
        assert!(!r.is_healthy(10, Microseconds::from_secs(5)));
    }

    #[test]
    fn read_metrics_is_healthy_pending_at_threshold() {
        let r = ReadMetrics::builder()
            .count(100)
            .pending(Duration::from_secs(5))
            .build();
        assert!(r.is_healthy(10, Microseconds::from_secs(5))); // equal is healthy
    }

    #[test]
    fn read_metrics_is_unhealthy_pending_exceeds_threshold() {
        let r = ReadMetrics::builder()
            .count(100)
            .pending(Duration::from_secs(6))
            .build();
        assert!(!r.is_healthy(10, Microseconds::from_secs(5)));
    }

    #[test]
    fn read_metrics_is_unhealthy_both_exceed() {
        let r = ReadMetrics::builder()
            .count(100)
            .backlog(20)
            .pending(Duration::from_secs(10))
            .build();
        assert!(!r.is_healthy(10, Microseconds::from_secs(5)));
    }

    #[test]
    fn read_metrics_default_is_empty() {
        let r = ReadMetrics::default();
        assert_eq!(r.count, 0);
        assert!(r.backlog.is_none());
        assert!(r.pending.is_none());
        assert!(r.rate.is_none());
    }

    #[test]
    fn read_metrics_clone_and_equality() {
        let r1 = ReadMetrics::builder().count(100).backlog(5).build();
        let r2 = r1.clone();
        assert_eq!(r1, r2);
    }

    #[test]
    fn read_metrics_max_values() {
        let r = ReadMetrics::builder()
            .count(u64::MAX)
            .backlog(u64::MAX)
            .build();
        assert_eq!(r.count, u64::MAX);
        assert_eq!(r.backlog, Some(u64::MAX));
    }

    #[test]
    fn read_metrics_special_rate_values() {
        // Negative rate (counts could theoretically decrease)
        let r = ReadMetrics::builder().rate(-10.0).build();
        assert_eq!(r.rate, Some(-10.0));

        // NaN rate
        let r = ReadMetrics::builder().rate(f64::NAN).build();
        assert!(r.rate.unwrap().is_nan());

        // Infinity rate
        let r = ReadMetrics::builder().rate(f64::INFINITY).build();
        assert!(r.rate.unwrap().is_infinite());
    }

    // ========================================================================
    // WriteMetrics Tests
    // ========================================================================

    #[test]
    fn write_metrics_new_with_count() {
        let w = WriteMetrics::new(200);
        assert_eq!(w.count, 200);
        assert!(w.pending.is_none());
        assert!(w.rate.is_none());
    }

    #[test]
    fn write_metrics_builder_all_fields() {
        let w = WriteMetrics::builder()
            .count(1000)
            .pending(Duration::from_millis(500))
            .rate(100.0)
            .build();

        assert_eq!(w.count, 1000);
        assert_eq!(w.pending, Some(Microseconds::from_millis(500)));
        assert_eq!(w.rate, Some(100.0));
    }

    #[test]
    fn write_metrics_builder_partial_fields() {
        let w = WriteMetrics::builder().count(50).build();

        assert_eq!(w.count, 50);
        assert!(w.pending.is_none());
        assert!(w.rate.is_none());
    }

    #[test]
    fn write_metrics_is_healthy_with_no_pending() {
        let w = WriteMetrics::new(100);
        assert!(w.is_healthy(Microseconds::from_secs(5)));
    }

    #[test]
    fn write_metrics_is_healthy_pending_at_threshold() {
        let w = WriteMetrics::builder()
            .count(100)
            .pending(Duration::from_secs(5))
            .build();
        assert!(w.is_healthy(Microseconds::from_secs(5)));
    }

    #[test]
    fn write_metrics_is_unhealthy_pending_exceeds_threshold() {
        let w = WriteMetrics::builder()
            .count(100)
            .pending(Duration::from_secs(6))
            .build();
        assert!(!w.is_healthy(Microseconds::from_secs(5)));
    }

    #[test]
    fn write_metrics_default_is_empty() {
        let w = WriteMetrics::default();
        assert_eq!(w.count, 0);
        assert!(w.pending.is_none());
        assert!(w.rate.is_none());
    }

    // ========================================================================
    // ModuleMetrics Tests
    // ========================================================================

    #[test]
    fn module_metrics_new_is_empty() {
        let m = ModuleMetrics::new();
        assert!(m.is_empty());
        assert_eq!(m.total_reads(), 0);
        assert_eq!(m.total_writes(), 0);
    }

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
    fn module_metrics_builder_with_multiple_topics() {
        let m = ModuleMetrics::builder()
            .read("input-a", |r| r.count(100).backlog(5))
            .read("input-b", |r| r.count(200))
            .write("output-a", |w| w.count(150))
            .write("output-b", |w| w.count(140))
            .build();

        assert!(!m.is_empty());
        assert_eq!(m.total_reads(), 300);
        assert_eq!(m.total_writes(), 290);
        assert_eq!(m.reads.len(), 2);
        assert_eq!(m.writes.len(), 2);
    }

    #[test]
    fn module_metrics_builder_only_reads() {
        let m = ModuleMetrics::builder()
            .read("events", |r| r.count(500))
            .build();

        assert!(!m.is_empty());
        assert_eq!(m.total_reads(), 500);
        assert_eq!(m.total_writes(), 0);
    }

    #[test]
    fn module_metrics_builder_only_writes() {
        let m = ModuleMetrics::builder()
            .write("events", |w| w.count(1000))
            .build();

        assert!(!m.is_empty());
        assert_eq!(m.total_reads(), 0);
        assert_eq!(m.total_writes(), 1000);
    }

    #[test]
    fn module_metrics_is_empty_with_empty_collections() {
        let m = ModuleMetrics::builder().build();
        assert!(m.is_empty());
    }

    #[test]
    fn module_metrics_access_individual_topics() {
        let m = ModuleMetrics::builder()
            .read("orders", |r| r.count(42).backlog(3))
            .write("notifications", |w| w.count(40).rate(5.0))
            .build();

        let orders = m.reads.get("orders").unwrap();
        assert_eq!(orders.count, 42);
        assert_eq!(orders.backlog, Some(3));

        let notifications = m.writes.get("notifications").unwrap();
        assert_eq!(notifications.count, 40);
        assert_eq!(notifications.rate, Some(5.0));
    }

    #[test]
    fn module_metrics_default_is_empty() {
        let m = ModuleMetrics::default();
        assert!(m.is_empty());
    }

    #[test]
    fn module_metrics_duplicate_topic_overwrites() {
        let m = ModuleMetrics::builder()
            .read("topic", |r| r.count(100))
            .read("topic", |r| r.count(200)) // Same topic, should overwrite
            .build();

        assert_eq!(m.reads.len(), 1);
        assert_eq!(m.reads.get("topic").unwrap().count, 200);
    }

    #[test]
    fn module_metrics_unicode_topic_names() {
        let m = ModuleMetrics::builder()
            .read("订单", |r| r.count(100))
            .read("événements", |r| r.count(200))
            .read("イベント", |r| r.count(300))
            .build();

        assert_eq!(m.reads.get("订单").unwrap().count, 100);
        assert_eq!(m.reads.get("événements").unwrap().count, 200);
        assert_eq!(m.reads.get("イベント").unwrap().count, 300);
        assert_eq!(m.total_reads(), 600);
    }

    #[test]
    fn module_metrics_special_characters_in_names() {
        let m = ModuleMetrics::builder()
            .read("topic/with/slashes", |r| r.count(1))
            .read("topic.with.dots", |r| r.count(2))
            .read("topic:with:colons", |r| r.count(3))
            .read("topic-with-dashes", |r| r.count(4))
            .read("topic_with_underscores", |r| r.count(5))
            .build();

        assert_eq!(m.total_reads(), 15);
    }

    #[test]
    fn module_metrics_empty_topic_name() {
        let m = ModuleMetrics::builder()
            .read("", |r| r.count(100))
            .write("", |w| w.count(50))
            .build();

        assert_eq!(m.reads.get("").unwrap().count, 100);
        assert_eq!(m.writes.get("").unwrap().count, 50);
    }

    #[test]
    fn module_metrics_many_topics() {
        let mut m_builder = ModuleMetrics::builder();
        for i in 0..100 {
            m_builder = m_builder.read(alloc::format!("read-topic-{}", i), |r| r.count(i as u64));
            m_builder = m_builder.write(alloc::format!("write-topic-{}", i), |w| w.count(i as u64));
        }
        let m = m_builder.build();

        assert_eq!(m.reads.len(), 100);
        assert_eq!(m.writes.len(), 100);
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

    // ========================================================================
    // Builder Default Tests
    // ========================================================================

    #[test]
    fn read_metrics_builder_default() {
        let b = ReadMetricsBuilder::default();
        let r = b.build();
        assert_eq!(r.count, 0);
    }

    #[test]
    fn write_metrics_builder_default() {
        let b = WriteMetricsBuilder::default();
        let w = b.build();
        assert_eq!(w.count, 0);
    }

    #[test]
    fn module_metrics_builder_default() {
        let b = ModuleMetricsBuilder::default();
        let m = b.build();
        assert!(m.is_empty());
    }
}
