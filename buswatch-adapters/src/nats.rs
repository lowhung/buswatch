//! NATS JetStream adapter for collecting stream and consumer metrics.
//!
//! This adapter connects to NATS and collects JetStream metrics
//! including stream sizes, consumer pending counts, and ack pending.
//!
//! ## Metrics Collected
//!
//! - **Stream message count**: Total messages in each stream
//! - **Consumer pending**: Number of messages pending delivery
//! - **Ack pending**: Messages delivered but not yet acknowledged
//!
//! ## Example
//!
//! ```rust,no_run
//! use buswatch_adapters::nats::NatsAdapter;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let adapter = NatsAdapter::builder()
//!         .url("nats://localhost:4222")
//!         .build()
//!         .await?;
//!
//!     let snapshot = adapter.collect().await?;
//!
//!     for (stream_name, metrics) in &snapshot.modules {
//!         println!("Stream: {}", stream_name);
//!         for (consumer, read) in &metrics.reads {
//!             println!("  Consumer {}: backlog={:?}", consumer, read.backlog);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::collections::BTreeMap;

use async_nats::jetstream;
use futures_util::StreamExt;

use buswatch_types::{ModuleMetrics, ReadMetrics, SchemaVersion, Snapshot, WriteMetrics};

use crate::AdapterError;

/// NATS JetStream adapter for collecting stream and consumer metrics.
pub struct NatsAdapter {
    jetstream: jetstream::Context,
}

impl NatsAdapter {
    /// Create a new builder for configuring the adapter.
    pub fn builder() -> NatsAdapterBuilder {
        NatsAdapterBuilder::default()
    }

    /// Collect a snapshot of all JetStream metrics.
    pub async fn collect(&self) -> Result<Snapshot, AdapterError> {
        let mut modules = BTreeMap::new();

        // List all stream names first
        let mut stream_names = self.jetstream.stream_names();
        let mut names = Vec::new();

        while let Some(name_result) = stream_names.next().await {
            let name = name_result.map_err(|e| AdapterError::Connection(e.to_string()))?;
            names.push(name);
        }

        // Now get each stream and collect metrics
        for stream_name in names {
            let mut stream = self
                .jetstream
                .get_stream(&stream_name)
                .await
                .map_err(|e| AdapterError::Connection(e.to_string()))?;

            let metrics = self.collect_stream_metrics(&mut stream).await?;
            modules.insert(stream_name, metrics);
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

    async fn collect_stream_metrics(
        &self,
        stream: &mut jetstream::stream::Stream,
    ) -> Result<ModuleMetrics, AdapterError> {
        let info = stream
            .info()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;

        // Extract values we need before further borrows
        let total_messages = info.state.messages;

        let mut reads = BTreeMap::new();
        let mut writes = BTreeMap::new();

        // Stream write metrics (messages published to the stream)
        let write_metrics = WriteMetrics::new(total_messages);
        writes.insert("stream".to_string(), write_metrics);

        // Collect consumer metrics by name
        let mut consumer_names_stream = stream.consumer_names();
        let mut consumer_names = Vec::new();

        while let Some(name_result) = consumer_names_stream.next().await {
            let name = name_result.map_err(|e| AdapterError::Connection(e.to_string()))?;
            consumer_names.push(name);
        }

        for consumer_name in consumer_names {
            let consumer_info = stream
                .consumer_info(&consumer_name)
                .await
                .map_err(|e| AdapterError::Connection(e.to_string()))?;

            // Calculate backlog: messages in stream - messages delivered
            let delivered = consumer_info.delivered.stream_sequence;
            let backlog = total_messages.saturating_sub(delivered);

            let mut read_metrics = ReadMetrics::new(delivered);
            read_metrics.backlog = Some(backlog);

            reads.insert(consumer_name, read_metrics);
        }

        Ok(ModuleMetrics { reads, writes })
    }
}

impl std::fmt::Debug for NatsAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsAdapter").finish()
    }
}

/// Builder for NatsAdapter.
#[derive(Debug, Default)]
pub struct NatsAdapterBuilder {
    url: Option<String>,
    credentials: Option<String>,
}

impl NatsAdapterBuilder {
    /// Set the NATS server URL (default: "nats://localhost:4222").
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Set the path to a credentials file for authentication.
    pub fn credentials_file(mut self, path: impl Into<String>) -> Self {
        self.credentials = Some(path.into());
        self
    }

    /// Build the adapter.
    pub async fn build(self) -> Result<NatsAdapter, AdapterError> {
        let url = self
            .url
            .unwrap_or_else(|| "nats://localhost:4222".to_string());

        let client = if let Some(creds) = self.credentials {
            async_nats::ConnectOptions::new()
                .credentials_file(&creds)
                .await
                .map_err(|e| AdapterError::Auth(e.to_string()))?
                .connect(&url)
                .await
                .map_err(|e| AdapterError::Connection(e.to_string()))?
        } else {
            async_nats::connect(&url)
                .await
                .map_err(|e| AdapterError::Connection(e.to_string()))?
        };

        let jetstream = jetstream::new(client);

        Ok(NatsAdapter { jetstream })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = NatsAdapter::builder().url("nats://localhost:4222");

        assert!(builder.url.is_some());
        assert_eq!(builder.url.unwrap(), "nats://localhost:4222");
    }
}
