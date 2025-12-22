//! Error types for adapters.

use thiserror::Error;

/// Errors that can occur when collecting metrics from adapters.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// Failed to parse response.
    #[error("Failed to parse response: {0}")]
    Parse(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Connection failed.
    #[error("Connection failed: {0}")]
    Connection(String),

    /// Timeout waiting for response.
    #[error("Request timed out")]
    Timeout,

    /// Feature not supported by this message bus version.
    #[error("Feature not supported: {0}")]
    Unsupported(String),
}

#[cfg(feature = "rabbitmq")]
impl From<reqwest::Error> for AdapterError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AdapterError::Timeout
        } else if err.is_connect() {
            AdapterError::Connection(err.to_string())
        } else {
            AdapterError::Http(err.to_string())
        }
    }
}
