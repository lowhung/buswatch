//! Duration representation for serialization.
//!
//! We use microseconds as the canonical unit for durations to ensure
//! consistent serialization across formats and languages.

use core::time::Duration;

/// Duration in microseconds.
///
/// This wrapper provides consistent serialization of durations across
/// different formats. Microseconds offer good precision while fitting
/// in a u64 for durations up to ~584,000 years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[cfg_attr(feature = "minicbor", derive(minicbor::Encode, minicbor::Decode))]
#[cfg_attr(feature = "minicbor", cbor(transparent))]
pub struct Microseconds(#[cfg_attr(feature = "minicbor", n(0))] pub u64);

impl Microseconds {
    /// Create from microseconds.
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// Create from milliseconds.
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis * 1000)
    }

    /// Create from seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs * 1_000_000)
    }

    /// Get the value in microseconds.
    pub const fn as_micros(&self) -> u64 {
        self.0
    }

    /// Get the value in milliseconds (truncated).
    pub const fn as_millis(&self) -> u64 {
        self.0 / 1000
    }

    /// Get the value in seconds (truncated).
    pub const fn as_secs(&self) -> u64 {
        self.0 / 1_000_000
    }

    /// Convert to a standard Duration.
    pub const fn to_duration(&self) -> Duration {
        Duration::from_micros(self.0)
    }
}

impl From<Duration> for Microseconds {
    fn from(d: Duration) -> Self {
        Self(d.as_micros() as u64)
    }
}

impl From<Microseconds> for Duration {
    fn from(m: Microseconds) -> Self {
        Duration::from_micros(m.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversions() {
        let d = Duration::from_millis(1500);
        let m = Microseconds::from(d);
        assert_eq!(m.as_micros(), 1_500_000);
        assert_eq!(m.as_millis(), 1500);
        assert_eq!(m.as_secs(), 1);

        let d2: Duration = m.into();
        assert_eq!(d, d2);
    }
}
