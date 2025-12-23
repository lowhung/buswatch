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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_http() {
        let err = AdapterError::Http("404 Not Found".to_string());
        assert_eq!(err.to_string(), "HTTP request failed: 404 Not Found");
    }

    #[test]
    fn error_display_parse() {
        let err = AdapterError::Parse("invalid JSON".to_string());
        assert_eq!(err.to_string(), "Failed to parse response: invalid JSON");
    }

    #[test]
    fn error_display_auth() {
        let err = AdapterError::Auth("bad credentials".to_string());
        assert_eq!(err.to_string(), "Authentication failed: bad credentials");
    }

    #[test]
    fn error_display_connection() {
        let err = AdapterError::Connection("connection refused".to_string());
        assert_eq!(err.to_string(), "Connection failed: connection refused");
    }

    #[test]
    fn error_display_timeout() {
        let err = AdapterError::Timeout;
        assert_eq!(err.to_string(), "Request timed out");
    }

    #[test]
    fn error_display_unsupported() {
        let err = AdapterError::Unsupported("feature X".to_string());
        assert_eq!(err.to_string(), "Feature not supported: feature X");
    }

    #[test]
    fn error_is_debug() {
        let err = AdapterError::Timeout;
        let debug = format!("{:?}", err);
        assert!(debug.contains("Timeout"));
    }
}
