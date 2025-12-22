//! OpenTelemetry integration for buswatch metrics.
//!
//! This module provides OTLP export functionality, converting buswatch
//! snapshots to OpenTelemetry metrics format.
//!
//! # Example
//!
//! ```rust,no_run
//! use buswatch_sdk::{Instrumentor, Output};
//! use buswatch_sdk::otel::OtelConfig;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let otel_config = OtelConfig::builder()
//!         .endpoint("http://localhost:4318")
//!         .service_name("my-service")
//!         .build();
//!
//!     let instrumentor = Instrumentor::builder()
//!         .output(Output::otel(otel_config).unwrap())
//!         .interval(Duration::from_secs(1))
//!         .build();
//!
//!     let handle = instrumentor.register("my-module");
//!     handle.record_read("events", 10);
//!
//!     instrumentor.start();
//! }
//! ```

use std::sync::Arc;

use opentelemetry::metrics::{Gauge, Meter, MeterProvider};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::metrics::SdkMeterProvider;

use buswatch_types::Snapshot;

/// Configuration for OpenTelemetry export.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// OTLP endpoint (e.g., "http://localhost:4318")
    pub endpoint: String,
    /// Service name for metrics attribution
    pub service_name: String,
}

impl OtelConfig {
    /// Create a new builder for OtelConfig.
    pub fn builder() -> OtelConfigBuilder {
        OtelConfigBuilder::default()
    }
}

/// Builder for OtelConfig.
#[derive(Debug, Default)]
pub struct OtelConfigBuilder {
    endpoint: Option<String>,
    service_name: Option<String>,
}

impl OtelConfigBuilder {
    /// Set the OTLP endpoint.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the service name.
    pub fn service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = Some(name.into());
        self
    }

    /// Build the OtelConfig.
    pub fn build(self) -> OtelConfig {
        OtelConfig {
            endpoint: self
                .endpoint
                .unwrap_or_else(|| "http://localhost:4318".to_string()),
            service_name: self.service_name.unwrap_or_else(|| "buswatch".to_string()),
        }
    }
}

/// OpenTelemetry exporter for buswatch metrics.
///
/// This exporter converts buswatch Snapshots into OpenTelemetry metrics
/// and exports them via OTLP.
pub struct OtelExporter {
    meter: Meter,
    _provider: Arc<SdkMeterProvider>,
    // Gauges for metrics
    read_count: Gauge<u64>,
    read_backlog: Gauge<u64>,
    read_pending: Gauge<u64>,
    read_rate: Gauge<f64>,
    write_count: Gauge<u64>,
    write_pending: Gauge<u64>,
    write_rate: Gauge<f64>,
}

impl OtelExporter {
    /// Create a new OtelExporter with the given configuration.
    pub fn new(config: &OtelConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use opentelemetry_otlp::MetricExporter;
        use opentelemetry_sdk::metrics::PeriodicReader;
        use opentelemetry_sdk::Resource;

        // Build the OTLP exporter
        let exporter = MetricExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/metrics", config.endpoint))
            .build()?;

        // Create a periodic reader with the exporter
        let reader = PeriodicReader::builder(exporter).build();

        // Create the meter provider with service name resource
        let resource = Resource::builder()
            .with_service_name(config.service_name.clone())
            .build();

        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(resource)
            .build();

        let meter = provider.meter("buswatch");
        let provider = Arc::new(provider);

        // Create gauges for all metrics
        let read_count = meter
            .u64_gauge("buswatch.read.count")
            .with_description("Total messages read from a topic")
            .build();

        let read_backlog = meter
            .u64_gauge("buswatch.read.backlog")
            .with_description("Estimated message backlog (unread messages)")
            .build();

        let read_pending = meter
            .u64_gauge("buswatch.read.pending")
            .with_description("Number of pending read operations")
            .build();

        let read_rate = meter
            .f64_gauge("buswatch.read.rate")
            .with_description("Read rate in messages per second")
            .build();

        let write_count = meter
            .u64_gauge("buswatch.write.count")
            .with_description("Total messages written to a topic")
            .build();

        let write_pending = meter
            .u64_gauge("buswatch.write.pending")
            .with_description("Number of pending write operations")
            .build();

        let write_rate = meter
            .f64_gauge("buswatch.write.rate")
            .with_description("Write rate in messages per second")
            .build();

        Ok(Self {
            meter,
            _provider: provider,
            read_count,
            read_backlog,
            read_pending,
            read_rate,
            write_count,
            write_pending,
            write_rate,
        })
    }

    /// Record a snapshot as OpenTelemetry metrics.
    pub fn record(&self, snapshot: &Snapshot) {
        for (module_name, module_metrics) in &snapshot.modules {
            // Record read metrics
            for (topic, read_metrics) in &module_metrics.reads {
                let attributes = [
                    KeyValue::new("module", module_name.clone()),
                    KeyValue::new("topic", topic.clone()),
                ];

                self.read_count.record(read_metrics.count, &attributes);

                if let Some(backlog) = read_metrics.backlog {
                    self.read_backlog.record(backlog, &attributes);
                }

                if let Some(pending) = read_metrics.pending {
                    self.read_pending.record(pending.as_micros(), &attributes);
                }

                if let Some(rate) = read_metrics.rate {
                    self.read_rate.record(rate, &attributes);
                }
            }

            // Record write metrics
            for (topic, write_metrics) in &module_metrics.writes {
                let attributes = [
                    KeyValue::new("module", module_name.clone()),
                    KeyValue::new("topic", topic.clone()),
                ];

                self.write_count.record(write_metrics.count, &attributes);

                if let Some(pending) = write_metrics.pending {
                    self.write_pending.record(pending.as_micros(), &attributes);
                }

                if let Some(rate) = write_metrics.rate {
                    self.write_rate.record(rate, &attributes);
                }
            }
        }
    }

    /// Get a reference to the meter for custom metrics.
    pub fn meter(&self) -> &Meter {
        &self.meter
    }
}

impl std::fmt::Debug for OtelExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OtelExporter")
            .field("meter", &"Meter { ... }")
            .finish()
    }
}
