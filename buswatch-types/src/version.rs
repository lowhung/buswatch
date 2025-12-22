//! Schema versioning for forward compatibility.

use crate::SCHEMA_VERSION;

/// Schema version information embedded in snapshots.
///
/// This allows consumers to detect and handle format changes gracefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "minicbor", derive(minicbor::Encode, minicbor::Decode))]
pub struct SchemaVersion {
    /// Major version - breaking changes increment this.
    #[cfg_attr(feature = "minicbor", n(0))]
    pub major: u32,

    /// Minor version - backwards-compatible additions increment this.
    #[cfg_attr(feature = "minicbor", n(1))]
    pub minor: u32,
}

impl SchemaVersion {
    /// Create a new schema version.
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// The current schema version used by this library.
    pub const fn current() -> Self {
        Self {
            major: SCHEMA_VERSION,
            minor: 0,
        }
    }

    /// Check if this version is compatible with the current library version.
    ///
    /// Returns true if the major version matches (minor differences are OK).
    pub fn is_compatible(&self) -> bool {
        self.major == SCHEMA_VERSION
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::current()
    }
}
