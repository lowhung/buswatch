//! Prometheus exposition format support.
//!
//! This module provides functionality to export buswatch metrics in the
//! Prometheus text-based exposition format, which can be scraped by Prometheus
//! or compatible monitoring systems.
//!
//! ## Example
//!
//! ```rust,no_run
//! use buswatch_sdk::{Instrumentor, Output};
//! use buswatch_sdk::prometheus::PrometheusConfig;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = PrometheusConfig::builder()
//!         .listen_addr("0.0.0.0:9090")
//!         .metrics_path("/metrics")
//!         .build();
//!
//!     let instrumentor = Instrumentor::builder()
//!         .output(Output::prometheus(config))
//!         .build();
//!
//!     let handle = instrumentor.register("my-service");
//!     handle.record_read("events", 100);
//!
//!     instrumentor.start();
//!
//!     // Metrics available at http://localhost:9090/metrics
//! }
//! ```

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use buswatch_types::Snapshot;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use tokio::net::TcpListener;

/// Configuration for Prometheus metrics endpoint.
#[derive(Debug, Clone)]
pub struct PrometheusConfig {
    /// Address to listen on (e.g., "0.0.0.0:9090")
    pub listen_addr: String,
    /// Path for metrics endpoint (e.g., "/metrics")
    pub metrics_path: String,
    /// Optional namespace prefix for all metrics
    pub namespace: Option<String>,
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:9090".to_string(),
            metrics_path: "/metrics".to_string(),
            namespace: None,
        }
    }
}

impl PrometheusConfig {
    /// Create a new builder for PrometheusConfig.
    pub fn builder() -> PrometheusConfigBuilder {
        PrometheusConfigBuilder::default()
    }
}

/// Builder for PrometheusConfig.
#[derive(Debug, Default)]
pub struct PrometheusConfigBuilder {
    listen_addr: Option<String>,
    metrics_path: Option<String>,
    namespace: Option<String>,
}

impl PrometheusConfigBuilder {
    /// Set the listen address.
    pub fn listen_addr(mut self, addr: impl Into<String>) -> Self {
        self.listen_addr = Some(addr.into());
        self
    }

    /// Set the metrics path.
    pub fn metrics_path(mut self, path: impl Into<String>) -> Self {
        self.metrics_path = Some(path.into());
        self
    }

    /// Set the namespace prefix for all metrics.
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    /// Build the PrometheusConfig.
    pub fn build(self) -> PrometheusConfig {
        PrometheusConfig {
            listen_addr: self
                .listen_addr
                .unwrap_or_else(|| "0.0.0.0:9090".to_string()),
            metrics_path: self.metrics_path.unwrap_or_else(|| "/metrics".to_string()),
            namespace: self.namespace,
        }
    }
}

/// Prometheus exporter that serves metrics over HTTP.
#[derive(Debug)]
pub struct PrometheusExporter {
    config: PrometheusConfig,
    /// Latest snapshot for serving
    latest_snapshot: Arc<RwLock<Option<Snapshot>>>,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter.
    pub fn new(config: PrometheusConfig) -> Self {
        Self {
            config,
            latest_snapshot: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &PrometheusConfig {
        &self.config
    }

    /// Update the latest snapshot.
    pub fn record(&self, snapshot: &Snapshot) {
        *self.latest_snapshot.write() = Some(snapshot.clone());
    }

    /// Get the current metrics in Prometheus exposition format.
    pub fn render(&self) -> String {
        let snapshot = self.latest_snapshot.read();
        match snapshot.as_ref() {
            Some(s) => format_prometheus(s, self.config.namespace.as_deref()),
            None => String::new(),
        }
    }

    /// Get a clone of the snapshot storage for sharing with the HTTP server.
    pub fn snapshot_storage(&self) -> Arc<RwLock<Option<Snapshot>>> {
        self.latest_snapshot.clone()
    }

    /// Start the HTTP server to serve Prometheus metrics.
    ///
    /// This spawns a background task that listens for HTTP requests and serves
    /// metrics at the configured path. The server runs until the runtime shuts down.
    ///
    /// Returns a `JoinHandle` that can be used to await the server or abort it.
    pub fn start_server(&self) -> tokio::task::JoinHandle<()> {
        let listen_addr = self.config.listen_addr.clone();
        let metrics_path = self.config.metrics_path.clone();
        let namespace = self.config.namespace.clone();
        let snapshot_storage = self.latest_snapshot.clone();

        tokio::spawn(async move {
            if let Err(e) = run_server(listen_addr, metrics_path, namespace, snapshot_storage).await
            {
                eprintln!("Prometheus server error: {}", e);
            }
        })
    }
}

async fn run_server(
    listen_addr: String,
    metrics_path: String,
    namespace: Option<String>,
    snapshot_storage: Arc<RwLock<Option<Snapshot>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = listen_addr.parse()?;
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let metrics_path = metrics_path.clone();
        let namespace = namespace.clone();
        let snapshot_storage = snapshot_storage.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                let metrics_path = metrics_path.clone();
                let namespace = namespace.clone();
                let snapshot_storage = snapshot_storage.clone();

                async move {
                    handle_request(req, &metrics_path, namespace.as_deref(), &snapshot_storage)
                }
            });

            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Prometheus connection error: {}", e);
            }
        });
    }
}

fn handle_request(
    req: Request<hyper::body::Incoming>,
    metrics_path: &str,
    namespace: Option<&str>,
    snapshot_storage: &Arc<RwLock<Option<Snapshot>>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();

    if path == metrics_path {
        let snapshot = snapshot_storage.read();
        let body = match snapshot.as_ref() {
            Some(s) => format_prometheus(s, namespace),
            None => String::new(),
        };

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
            .body(Full::new(Bytes::from(body)))
            .unwrap())
    } else if path == "/health" || path == "/healthz" {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain")
            .body(Full::new(Bytes::from("OK")))
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "text/plain")
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap())
    }
}

/// Format a snapshot as Prometheus exposition format.
pub fn format_prometheus(snapshot: &Snapshot, namespace: Option<&str>) -> String {
    let mut output = String::new();
    let prefix = namespace.map(|n| format!("{}_", n)).unwrap_or_default();

    // Add HELP and TYPE comments for each metric family
    output.push_str(&format!(
        "# HELP {}buswatch_read_count Total number of messages read from a topic\n",
        prefix
    ));
    output.push_str(&format!("# TYPE {}buswatch_read_count counter\n", prefix));

    output.push_str(&format!(
        "# HELP {}buswatch_write_count Total number of messages written to a topic\n",
        prefix
    ));
    output.push_str(&format!("# TYPE {}buswatch_write_count counter\n", prefix));

    output.push_str(&format!(
        "# HELP {}buswatch_read_backlog Number of unread messages in topic backlog\n",
        prefix
    ));
    output.push_str(&format!("# TYPE {}buswatch_read_backlog gauge\n", prefix));

    output.push_str(&format!(
        "# HELP {}buswatch_read_pending_seconds Time spent waiting for a read operation\n",
        prefix
    ));
    output.push_str(&format!(
        "# TYPE {}buswatch_read_pending_seconds gauge\n",
        prefix
    ));

    output.push_str(&format!(
        "# HELP {}buswatch_write_pending_seconds Time spent waiting for a write operation\n",
        prefix
    ));
    output.push_str(&format!(
        "# TYPE {}buswatch_write_pending_seconds gauge\n",
        prefix
    ));

    output.push_str(&format!(
        "# HELP {}buswatch_read_rate_per_second Messages read per second\n",
        prefix
    ));
    output.push_str(&format!(
        "# TYPE {}buswatch_read_rate_per_second gauge\n",
        prefix
    ));

    output.push_str(&format!(
        "# HELP {}buswatch_write_rate_per_second Messages written per second\n",
        prefix
    ));
    output.push_str(&format!(
        "# TYPE {}buswatch_write_rate_per_second gauge\n",
        prefix
    ));

    // Emit metrics for each module
    for (module_name, metrics) in &snapshot.modules {
        let module_label = escape_label_value(module_name);

        // Read metrics
        for (topic, read) in &metrics.reads {
            let topic_label = escape_label_value(topic);
            let labels = format!("module=\"{}\",topic=\"{}\"", module_label, topic_label);

            // Count (counter)
            output.push_str(&format!(
                "{}buswatch_read_count{{{}}} {}\n",
                prefix, labels, read.count
            ));

            // Backlog (gauge)
            if let Some(backlog) = read.backlog {
                output.push_str(&format!(
                    "{}buswatch_read_backlog{{{}}} {}\n",
                    prefix, labels, backlog
                ));
            }

            // Pending time (gauge, in seconds)
            if let Some(pending) = &read.pending {
                let seconds = pending.as_micros() as f64 / 1_000_000.0;
                output.push_str(&format!(
                    "{}buswatch_read_pending_seconds{{{}}} {:.6}\n",
                    prefix, labels, seconds
                ));
            }

            // Rate (gauge)
            if let Some(rate) = read.rate {
                output.push_str(&format!(
                    "{}buswatch_read_rate_per_second{{{}}} {:.2}\n",
                    prefix, labels, rate
                ));
            }
        }

        // Write metrics
        for (topic, write) in &metrics.writes {
            let topic_label = escape_label_value(topic);
            let labels = format!("module=\"{}\",topic=\"{}\"", module_label, topic_label);

            // Count (counter)
            output.push_str(&format!(
                "{}buswatch_write_count{{{}}} {}\n",
                prefix, labels, write.count
            ));

            // Pending time (gauge, in seconds)
            if let Some(pending) = &write.pending {
                let seconds = pending.as_micros() as f64 / 1_000_000.0;
                output.push_str(&format!(
                    "{}buswatch_write_pending_seconds{{{}}} {:.6}\n",
                    prefix, labels, seconds
                ));
            }

            // Rate (gauge)
            if let Some(rate) = write.rate {
                output.push_str(&format!(
                    "{}buswatch_write_rate_per_second{{{}}} {:.2}\n",
                    prefix, labels, rate
                ));
            }
        }
    }

    // Add timestamp metric
    output.push_str(&format!(
        "# HELP {}buswatch_snapshot_timestamp_seconds Unix timestamp of the snapshot\n",
        prefix
    ));
    output.push_str(&format!(
        "# TYPE {}buswatch_snapshot_timestamp_seconds gauge\n",
        prefix
    ));
    output.push_str(&format!(
        "{}buswatch_snapshot_timestamp_seconds {:.3}\n",
        prefix,
        snapshot.timestamp_ms as f64 / 1000.0
    ));

    output
}

/// Escape a label value for Prometheus format.
/// Backslash, double-quote, and newline must be escaped.
fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use buswatch_types::{Microseconds, ModuleMetrics, ReadMetrics, WriteMetrics};

    fn create_test_snapshot() -> Snapshot {
        use std::collections::BTreeMap;

        let mut reads = BTreeMap::new();
        reads.insert(
            "events".to_string(),
            ReadMetrics {
                count: 1000,
                backlog: Some(50),
                pending: Some(Microseconds::from_millis(100)),
                rate: Some(50.5),
            },
        );

        let mut writes = BTreeMap::new();
        writes.insert(
            "output".to_string(),
            WriteMetrics {
                count: 500,
                pending: None,
                rate: Some(25.0),
            },
        );

        let mut modules = BTreeMap::new();
        modules.insert("my-service".to_string(), ModuleMetrics { reads, writes });

        Snapshot {
            version: buswatch_types::SchemaVersion::current(),
            timestamp_ms: 1703160000000,
            modules,
        }
    }

    #[test]
    fn test_format_prometheus_basic() {
        let snapshot = create_test_snapshot();
        let output = format_prometheus(&snapshot, None);

        assert!(output.contains("buswatch_read_count{module=\"my-service\",topic=\"events\"} 1000"));
        assert!(output.contains("buswatch_write_count{module=\"my-service\",topic=\"output\"} 500"));
        assert!(output.contains("buswatch_read_backlog{module=\"my-service\",topic=\"events\"} 50"));
        assert!(output.contains(
            "buswatch_read_pending_seconds{module=\"my-service\",topic=\"events\"} 0.100000"
        ));
        assert!(output.contains(
            "buswatch_read_rate_per_second{module=\"my-service\",topic=\"events\"} 50.50"
        ));
        assert!(output.contains(
            "buswatch_write_rate_per_second{module=\"my-service\",topic=\"output\"} 25.00"
        ));
    }

    #[test]
    fn test_format_prometheus_with_namespace() {
        let snapshot = create_test_snapshot();
        let output = format_prometheus(&snapshot, Some("myapp"));

        assert!(output.contains("myapp_buswatch_read_count"));
        assert!(output.contains("myapp_buswatch_write_count"));
        assert!(output.contains("# HELP myapp_buswatch_read_count"));
    }

    #[test]
    fn test_escape_label_value() {
        assert_eq!(escape_label_value("simple"), "simple");
        assert_eq!(escape_label_value("with\"quote"), "with\\\"quote");
        assert_eq!(escape_label_value("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_label_value("with\nnewline"), "with\\nnewline");
    }

    #[test]
    fn test_prometheus_config_builder() {
        let config = PrometheusConfig::builder()
            .listen_addr("127.0.0.1:8080")
            .metrics_path("/custom-metrics")
            .namespace("myapp")
            .build();

        assert_eq!(config.listen_addr, "127.0.0.1:8080");
        assert_eq!(config.metrics_path, "/custom-metrics");
        assert_eq!(config.namespace, Some("myapp".to_string()));
    }

    #[test]
    fn test_prometheus_config_defaults() {
        let config = PrometheusConfig::default();

        assert_eq!(config.listen_addr, "0.0.0.0:9090");
        assert_eq!(config.metrics_path, "/metrics");
        assert_eq!(config.namespace, None);
    }

    #[test]
    fn test_prometheus_exporter_record_and_render() {
        let config = PrometheusConfig::default();
        let exporter = PrometheusExporter::new(config);

        // Initially empty
        assert_eq!(exporter.render(), "");

        // Record a snapshot
        let snapshot = create_test_snapshot();
        exporter.record(&snapshot);

        // Should now have content
        let output = exporter.render();
        assert!(!output.is_empty());
        assert!(output.contains("buswatch_read_count"));
    }

    #[test]
    fn test_format_includes_help_and_type() {
        let snapshot = create_test_snapshot();
        let output = format_prometheus(&snapshot, None);

        assert!(output.contains("# HELP buswatch_read_count"));
        assert!(output.contains("# TYPE buswatch_read_count counter"));
        assert!(output.contains("# HELP buswatch_write_count"));
        assert!(output.contains("# TYPE buswatch_write_count counter"));
        assert!(output.contains("# TYPE buswatch_read_backlog gauge"));
    }

    #[test]
    fn test_format_includes_timestamp() {
        let snapshot = create_test_snapshot();
        let output = format_prometheus(&snapshot, None);

        assert!(output.contains("buswatch_snapshot_timestamp_seconds"));
        assert!(output.contains("1703160000.000"));
    }

    #[test]
    fn test_empty_snapshot() {
        let snapshot = Snapshot::builder().build();
        let output = format_prometheus(&snapshot, None);

        // Should still have HELP/TYPE headers and timestamp
        assert!(output.contains("# HELP"));
        assert!(output.contains("buswatch_snapshot_timestamp_seconds"));
    }

    #[test]
    fn test_multiple_modules_and_topics() {
        use std::collections::BTreeMap;

        let mut modules = BTreeMap::new();

        // Module 1
        let mut reads1 = BTreeMap::new();
        reads1.insert(
            "topic-a".to_string(),
            ReadMetrics {
                count: 100,
                backlog: None,
                pending: None,
                rate: None,
            },
        );
        reads1.insert(
            "topic-b".to_string(),
            ReadMetrics {
                count: 200,
                backlog: Some(10),
                pending: None,
                rate: None,
            },
        );
        modules.insert(
            "service-1".to_string(),
            ModuleMetrics {
                reads: reads1,
                writes: BTreeMap::new(),
            },
        );

        // Module 2
        let mut writes2 = BTreeMap::new();
        writes2.insert(
            "output".to_string(),
            WriteMetrics {
                count: 50,
                pending: None,
                rate: Some(10.0),
            },
        );
        modules.insert(
            "service-2".to_string(),
            ModuleMetrics {
                reads: BTreeMap::new(),
                writes: writes2,
            },
        );

        let snapshot = Snapshot {
            version: buswatch_types::SchemaVersion::current(),
            timestamp_ms: 1703160000000,
            modules,
        };

        let output = format_prometheus(&snapshot, None);

        assert!(output.contains("module=\"service-1\",topic=\"topic-a\""));
        assert!(output.contains("module=\"service-1\",topic=\"topic-b\""));
        assert!(output.contains("module=\"service-2\",topic=\"output\""));
        assert!(output.contains("buswatch_read_backlog{module=\"service-1\",topic=\"topic-b\"} 10"));
    }
}
