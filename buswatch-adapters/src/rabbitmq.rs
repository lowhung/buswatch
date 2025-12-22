//! RabbitMQ adapter using the Management HTTP API.
//!
//! This adapter collects metrics from RabbitMQ by querying the Management API,
//! which is typically available on port 15672.
//!
//! ## Metrics Collected
//!
//! - **Queue depth** (backlog): Number of messages ready in each queue
//! - **Consumer count**: Number of consumers attached to each queue
//! - **Message rates**: Publish and deliver rates per queue
//! - **Unacked messages**: Messages delivered but not yet acknowledged
//!
//! ## Example
//!
//! ```rust,no_run
//! use buswatch_adapters::rabbitmq::RabbitMqAdapter;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let adapter = RabbitMqAdapter::builder()
//!         .endpoint("http://localhost:15672")
//!         .credentials("guest", "guest")
//!         .vhost("/")
//!         .build();
//!
//!     let snapshot = adapter.collect().await?;
//!
//!     for (queue_name, metrics) in &snapshot.modules {
//!         println!("Queue: {}", queue_name);
//!         if let Some(read) = metrics.reads.get("messages") {
//!             println!("  Ready: {:?}", read.backlog);
//!             println!("  Rate: {:?} msg/s", read.rate);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use buswatch_types::{ModuleMetrics, ReadMetrics, SchemaVersion, Snapshot, WriteMetrics};

use crate::AdapterError;

/// RabbitMQ adapter for collecting queue metrics.
#[derive(Debug, Clone)]
pub struct RabbitMqAdapter {
    client: Client,
    endpoint: String,
    username: String,
    password: String,
    vhost: String,
}

impl RabbitMqAdapter {
    /// Create a new builder for configuring the adapter.
    pub fn builder() -> RabbitMqAdapterBuilder {
        RabbitMqAdapterBuilder::default()
    }

    /// Collect a snapshot of all queue metrics.
    pub async fn collect(&self) -> Result<Snapshot, AdapterError> {
        let queues = self.fetch_queues().await?;

        let mut modules = BTreeMap::new();

        for queue in queues {
            let module_metrics = self.queue_to_metrics(&queue);
            modules.insert(queue.name, module_metrics);
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

    /// Collect metrics for a specific queue.
    pub async fn collect_queue(&self, queue_name: &str) -> Result<ModuleMetrics, AdapterError> {
        let queue = self.fetch_queue(queue_name).await?;
        Ok(self.queue_to_metrics(&queue))
    }

    async fn fetch_queues(&self) -> Result<Vec<QueueInfo>, AdapterError> {
        let url = format!("{}/api/queues/{}", self.endpoint, urlencoded(&self.vhost));

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AdapterError::Auth("Invalid credentials".to_string()));
        }

        if !response.status().is_success() {
            return Err(AdapterError::Http(format!(
                "API returned status {}",
                response.status()
            )));
        }

        let queues: Vec<QueueInfo> = response
            .json()
            .await
            .map_err(|e| AdapterError::Parse(e.to_string()))?;

        Ok(queues)
    }

    async fn fetch_queue(&self, queue_name: &str) -> Result<QueueInfo, AdapterError> {
        let url = format!(
            "{}/api/queues/{}/{}",
            self.endpoint,
            urlencoded(&self.vhost),
            urlencoded(queue_name)
        );

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AdapterError::Auth("Invalid credentials".to_string()));
        }

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::Http(format!(
                "Queue '{}' not found",
                queue_name
            )));
        }

        if !response.status().is_success() {
            return Err(AdapterError::Http(format!(
                "API returned status {}",
                response.status()
            )));
        }

        let queue: QueueInfo = response
            .json()
            .await
            .map_err(|e| AdapterError::Parse(e.to_string()))?;

        Ok(queue)
    }

    fn queue_to_metrics(&self, queue: &QueueInfo) -> ModuleMetrics {
        let mut reads = BTreeMap::new();
        let mut writes = BTreeMap::new();

        // Consumer metrics (reading from the queue)
        if queue.consumers > 0 {
            let mut read_metrics = ReadMetrics::new(queue.messages_delivered.unwrap_or(0));
            read_metrics.backlog = Some(queue.messages_ready);
            if let Some(rate) = queue
                .message_stats
                .as_ref()
                .and_then(|s| s.deliver_get_rate())
            {
                read_metrics.rate = Some(rate);
            }
            reads.insert("messages".to_string(), read_metrics);
        } else {
            // No consumers, just show backlog
            let mut read_metrics = ReadMetrics::new(0);
            read_metrics.backlog = Some(queue.messages_ready);
            reads.insert("messages".to_string(), read_metrics);
        }

        // Publisher metrics (writing to the queue)
        let mut write_metrics = WriteMetrics::new(queue.messages_published.unwrap_or(0));
        if let Some(rate) = queue.message_stats.as_ref().and_then(|s| s.publish_rate()) {
            write_metrics.rate = Some(rate);
        }
        writes.insert("messages".to_string(), write_metrics);

        ModuleMetrics { reads, writes }
    }
}

/// Builder for RabbitMqAdapter.
#[derive(Debug, Default)]
pub struct RabbitMqAdapterBuilder {
    endpoint: Option<String>,
    username: Option<String>,
    password: Option<String>,
    vhost: Option<String>,
    timeout: Option<Duration>,
}

impl RabbitMqAdapterBuilder {
    /// Set the Management API endpoint (e.g., "http://localhost:15672").
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the username and password for authentication.
    pub fn credentials(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Set the vhost to query (default: "/").
    pub fn vhost(mut self, vhost: impl Into<String>) -> Self {
        self.vhost = Some(vhost.into());
        self
    }

    /// Set the request timeout (default: 10 seconds).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Build the adapter.
    pub fn build(self) -> RabbitMqAdapter {
        let timeout = self.timeout.unwrap_or(Duration::from_secs(10));

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build HTTP client");

        RabbitMqAdapter {
            client,
            endpoint: self
                .endpoint
                .unwrap_or_else(|| "http://localhost:15672".to_string()),
            username: self.username.unwrap_or_else(|| "guest".to_string()),
            password: self.password.unwrap_or_else(|| "guest".to_string()),
            vhost: self.vhost.unwrap_or_else(|| "/".to_string()),
        }
    }
}

// URL encode a string for use in paths
fn urlencoded(s: &str) -> String {
    s.replace('/', "%2F")
}

/// Queue information from the RabbitMQ Management API.
#[derive(Debug, Deserialize)]
struct QueueInfo {
    name: String,
    #[serde(default)]
    messages_ready: u64,
    #[serde(default)]
    #[allow(dead_code)]
    messages_unacknowledged: u64,
    #[serde(default)]
    consumers: u32,
    #[serde(default)]
    messages_delivered: Option<u64>,
    #[serde(default)]
    messages_published: Option<u64>,
    message_stats: Option<MessageStats>,
}

#[derive(Debug, Deserialize)]
struct MessageStats {
    #[serde(default, rename = "publish_details")]
    publish_details: Option<RateDetails>,
    #[serde(default, rename = "deliver_get_details")]
    deliver_get_details: Option<RateDetails>,
}

impl MessageStats {
    fn publish_rate(&self) -> Option<f64> {
        self.publish_details.as_ref().map(|d| d.rate)
    }

    fn deliver_get_rate(&self) -> Option<f64> {
        self.deliver_get_details.as_ref().map(|d| d.rate)
    }
}

#[derive(Debug, Deserialize)]
struct RateDetails {
    rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let adapter = RabbitMqAdapter::builder().build();
        assert_eq!(adapter.endpoint, "http://localhost:15672");
        assert_eq!(adapter.username, "guest");
        assert_eq!(adapter.password, "guest");
        assert_eq!(adapter.vhost, "/");
    }

    #[test]
    fn test_builder_custom() {
        let adapter = RabbitMqAdapter::builder()
            .endpoint("http://rabbit.local:15672")
            .credentials("admin", "secret")
            .vhost("myapp")
            .build();

        assert_eq!(adapter.endpoint, "http://rabbit.local:15672");
        assert_eq!(adapter.username, "admin");
        assert_eq!(adapter.password, "secret");
        assert_eq!(adapter.vhost, "myapp");
    }

    #[test]
    fn test_urlencoded() {
        assert_eq!(urlencoded("/"), "%2F");
        assert_eq!(urlencoded("my/vhost"), "my%2Fvhost");
        assert_eq!(urlencoded("simple"), "simple");
    }

    #[test]
    fn test_queue_to_metrics() {
        let adapter = RabbitMqAdapter::builder().build();

        let queue = QueueInfo {
            name: "test-queue".to_string(),
            messages_ready: 100,
            messages_unacknowledged: 5,
            consumers: 2,
            messages_delivered: Some(500),
            messages_published: Some(600),
            message_stats: Some(MessageStats {
                publish_details: Some(RateDetails { rate: 10.5 }),
                deliver_get_details: Some(RateDetails { rate: 9.2 }),
            }),
        };

        let metrics = adapter.queue_to_metrics(&queue);

        let read = metrics.reads.get("messages").unwrap();
        assert_eq!(read.count, 500);
        assert_eq!(read.backlog, Some(100));
        assert_eq!(read.rate, Some(9.2));

        let write = metrics.writes.get("messages").unwrap();
        assert_eq!(write.count, 600);
        assert_eq!(write.rate, Some(10.5));
    }
}
