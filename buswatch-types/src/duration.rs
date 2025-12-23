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

    #[test]
    fn from_micros() {
        let m = Microseconds::from_micros(1_500_000);
        assert_eq!(m.as_micros(), 1_500_000);
        assert_eq!(m.as_millis(), 1500);
        assert_eq!(m.as_secs(), 1);
    }

    #[test]
    fn from_millis() {
        let m = Microseconds::from_millis(2500);
        assert_eq!(m.as_micros(), 2_500_000);
        assert_eq!(m.as_millis(), 2500);
        assert_eq!(m.as_secs(), 2);
    }

    #[test]
    fn from_secs() {
        let m = Microseconds::from_secs(5);
        assert_eq!(m.as_micros(), 5_000_000);
        assert_eq!(m.as_millis(), 5000);
        assert_eq!(m.as_secs(), 5);
    }

    #[test]
    fn to_duration() {
        let m = Microseconds::from_millis(567);
        let d = m.to_duration();
        assert_eq!(d, Duration::from_millis(567));
    }

    #[test]
    fn default_is_zero() {
        let m = Microseconds::default();
        assert_eq!(m.as_micros(), 0);
        assert_eq!(m.to_duration(), Duration::ZERO);
    }

    #[test]
    fn ordering() {
        let a = Microseconds::from_millis(100);
        let b = Microseconds::from_millis(200);
        let c = Microseconds::from_millis(100);

        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, c);
        assert!(a <= c);
        assert!(a >= c);
    }

    #[test]
    fn truncation_behavior() {
        // 1,500,999 microseconds should truncate to 1500 millis and 1 sec
        let m = Microseconds::from_micros(1_500_999);
        assert_eq!(m.as_millis(), 1500); // truncated, not rounded
        assert_eq!(m.as_secs(), 1); // truncated
    }

    #[test]
    fn large_values() {
        // Test with very large values (days worth of microseconds)
        let days_in_micros = 86_400_000_000u64 * 30; // 30 days
        let m = Microseconds::from_micros(days_in_micros);
        assert_eq!(m.as_secs(), 86_400 * 30);
    }

    #[test]
    fn zero_values() {
        let m = Microseconds::from_micros(0);
        assert_eq!(m.as_micros(), 0);
        assert_eq!(m.as_millis(), 0);
        assert_eq!(m.as_secs(), 0);
    }

    #[test]
    fn hash_impl() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Microseconds::from_secs(1));
        set.insert(Microseconds::from_secs(2));
        set.insert(Microseconds::from_secs(1)); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn copy_semantics() {
        let m1 = Microseconds::from_secs(5);
        let m2 = m1; // Copy
        assert_eq!(m1, m2);
        assert_eq!(m1.as_secs(), 5); // m1 still usable
    }

    #[test]
    fn debug_format() {
        let m = Microseconds::from_millis(1500);
        let debug = format!("{:?}", m);
        assert!(debug.contains("1500000"));
    }
}
