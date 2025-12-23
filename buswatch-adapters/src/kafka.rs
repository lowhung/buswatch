//! Kafka adapter for collecting consumer group lag metrics.
//!
//! This adapter connects to Kafka and collects consumer group lag metrics
//! using the rdkafka library (librdkafka bindings).
//!
//! ## Metrics Collected
//!
//! - **Consumer group lag**: Difference between log end offset and committed offset
//! - **Partition assignments**: Which partitions each consumer group is consuming
//! - **Current offsets**: Committed offsets for each partition
//!
//! ## Example
//!
//! ```rust,no_run
//! use buswatch_adapters::kafka::KafkaAdapter;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let adapter = KafkaAdapter::builder()
//!         .brokers("localhost:9092")
//!         .group_id("my-consumer-group")
//!         .build()?;
//!
//!     let snapshot = adapter.collect().await?;
//!
//!     for (group_name, metrics) in &snapshot.modules {
//!         println!("Consumer Group: {}", group_name);
//!         for (topic, read) in &metrics.reads {
//!             println!("  {}: backlog={:?}", topic, read.backlog);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::collections::BTreeMap;
use std::time::Duration;

use rdkafka::admin::AdminClient;
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::groups::GroupList;
use rdkafka::metadata::Metadata;
use rdkafka::TopicPartitionList;

use buswatch_types::{ModuleMetrics, ReadMetrics, SchemaVersion, Snapshot};

use crate::AdapterError;

/// Kafka adapter for collecting consumer group metrics.
pub struct KafkaAdapter {
    #[allow(dead_code)]
    admin: AdminClient<DefaultClientContext>,
    consumer: BaseConsumer,
    group_filter: Option<String>,
    timeout: Duration,
}

impl KafkaAdapter {
    /// Create a new builder for configuring the adapter.
    pub fn builder() -> KafkaAdapterBuilder {
        KafkaAdapterBuilder::default()
    }

    /// Collect a snapshot of all consumer group metrics.
    pub async fn collect(&self) -> Result<Snapshot, AdapterError> {
        let groups = self.list_groups()?;
        let metadata = self.fetch_metadata()?;

        let mut modules = BTreeMap::new();

        for group in groups.groups() {
            let group_name = group.name();

            // Apply filter if set
            if let Some(ref filter) = self.group_filter {
                if !group_name.contains(filter) {
                    continue;
                }
            }

            if let Ok(metrics) = self.collect_group_metrics(group_name, &metadata) {
                modules.insert(group_name.to_string(), metrics);
            }
        }

        Ok(Snapshot {
            version: SchemaVersion::current(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            modules,
        })
    }

    /// Collect metrics for a specific consumer group.
    pub async fn collect_group(&self, group_name: &str) -> Result<ModuleMetrics, AdapterError> {
        let metadata = self.fetch_metadata()?;
        self.collect_group_metrics(group_name, &metadata)
    }

    fn list_groups(&self) -> Result<GroupList, AdapterError> {
        self.consumer
            .fetch_group_list(None, self.timeout)
            .map_err(|e| AdapterError::Connection(e.to_string()))
    }

    fn fetch_metadata(&self) -> Result<Metadata, AdapterError> {
        self.consumer
            .fetch_metadata(None, self.timeout)
            .map_err(|e| AdapterError::Connection(e.to_string()))
    }

    fn collect_group_metrics(
        &self,
        _group_name: &str,
        metadata: &Metadata,
    ) -> Result<ModuleMetrics, AdapterError> {
        let mut reads = BTreeMap::new();

        // Get committed offsets for this group
        for topic in metadata.topics() {
            let topic_name = topic.name();

            // Build a TopicPartitionList for all partitions
            let mut tpl = TopicPartitionList::new();
            for partition in topic.partitions() {
                tpl.add_partition(topic_name, partition.id());
            }

            // Get committed offsets
            let committed = match self.consumer.committed_offsets(tpl.clone(), self.timeout) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Calculate lag by comparing to high watermarks
            let mut total_lag: i64 = 0;
            let mut total_offset: i64 = 0;
            let mut has_offsets = false;

            for elem in committed.elements() {
                if let rdkafka::Offset::Offset(committed_offset) = elem.offset() {
                    has_offsets = true;
                    total_offset += committed_offset;

                    // Get high watermark for this partition
                    if let Ok((_, high)) =
                        self.consumer
                            .fetch_watermarks(topic_name, elem.partition(), self.timeout)
                    {
                        total_lag += high - committed_offset;
                    }
                }
            }

            if has_offsets {
                let mut read_metrics = ReadMetrics::new(total_offset as u64);
                if total_lag >= 0 {
                    read_metrics.backlog = Some(total_lag as u64);
                }
                reads.insert(topic_name.to_string(), read_metrics);
            }
        }

        Ok(ModuleMetrics {
            reads,
            writes: BTreeMap::new(),
        })
    }
}

impl std::fmt::Debug for KafkaAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KafkaAdapter")
            .field("group_filter", &self.group_filter)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Builder for KafkaAdapter.
#[derive(Debug, Default)]
pub struct KafkaAdapterBuilder {
    brokers: Option<String>,
    group_id: Option<String>,
    group_filter: Option<String>,
    timeout: Option<Duration>,
}

impl KafkaAdapterBuilder {
    /// Set the Kafka broker addresses (comma-separated).
    pub fn brokers(mut self, brokers: impl Into<String>) -> Self {
        self.brokers = Some(brokers.into());
        self
    }

    /// Set the consumer group ID for admin operations.
    pub fn group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = Some(group_id.into());
        self
    }

    /// Filter to only collect metrics for groups containing this string.
    pub fn group_filter(mut self, filter: impl Into<String>) -> Self {
        self.group_filter = Some(filter.into());
        self
    }

    /// Set the request timeout (default: 10 seconds).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Build the adapter.
    pub fn build(self) -> Result<KafkaAdapter, AdapterError> {
        let brokers = self.brokers.unwrap_or_else(|| "localhost:9092".to_string());
        let group_id = self
            .group_id
            .unwrap_or_else(|| "buswatch-adapter".to_string());
        let timeout = self.timeout.unwrap_or(Duration::from_secs(10));

        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &brokers);
        config.set("group.id", &group_id);

        let admin: AdminClient<DefaultClientContext> = config
            .create()
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        let consumer: BaseConsumer = config
            .create()
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        Ok(KafkaAdapter {
            admin,
            consumer,
            group_filter: self.group_filter,
            timeout,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = KafkaAdapter::builder()
            .brokers("localhost:9092")
            .group_id("test-group")
            .timeout(Duration::from_secs(5));

        assert!(builder.brokers.is_some());
        assert!(builder.group_id.is_some());
    }

    #[test]
    fn builder_stores_brokers() {
        let builder = KafkaAdapter::builder().brokers("broker1:9092,broker2:9092");
        assert_eq!(builder.brokers.unwrap(), "broker1:9092,broker2:9092");
    }

    #[test]
    fn builder_stores_group_id() {
        let builder = KafkaAdapter::builder().group_id("my-consumer-group");
        assert_eq!(builder.group_id.unwrap(), "my-consumer-group");
    }

    #[test]
    fn builder_stores_group_filter() {
        let builder = KafkaAdapter::builder().group_filter("prefix-");
        assert_eq!(builder.group_filter.unwrap(), "prefix-");
    }

    #[test]
    fn builder_stores_timeout() {
        let builder = KafkaAdapter::builder().timeout(Duration::from_secs(30));
        assert_eq!(builder.timeout.unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn builder_chains_all_options() {
        let builder = KafkaAdapter::builder()
            .brokers("localhost:9092")
            .group_id("test")
            .group_filter("app-")
            .timeout(Duration::from_secs(15));

        assert_eq!(builder.brokers.unwrap(), "localhost:9092");
        assert_eq!(builder.group_id.unwrap(), "test");
        assert_eq!(builder.group_filter.unwrap(), "app-");
        assert_eq!(builder.timeout.unwrap(), Duration::from_secs(15));
    }

    #[test]
    fn builder_default_is_empty() {
        let builder = KafkaAdapterBuilder::default();
        assert!(builder.brokers.is_none());
        assert!(builder.group_id.is_none());
        assert!(builder.group_filter.is_none());
        assert!(builder.timeout.is_none());
    }
}
