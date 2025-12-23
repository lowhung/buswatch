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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA_VERSION;

    #[test]
    fn current_version() {
        let v = SchemaVersion::current();
        assert_eq!(v.major, SCHEMA_VERSION);
        assert_eq!(v.minor, 0);
    }

    #[test]
    fn new_version() {
        let v = SchemaVersion::new(2, 5);
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 5);
    }

    #[test]
    fn is_compatible_same_major() {
        let v = SchemaVersion::new(SCHEMA_VERSION, 10);
        assert!(v.is_compatible());
    }

    #[test]
    fn is_incompatible_different_major() {
        let v = SchemaVersion::new(SCHEMA_VERSION + 1, 0);
        assert!(!v.is_compatible());

        // Test with major version 0 if current is > 0
        if SCHEMA_VERSION > 0 {
            let v2 = SchemaVersion::new(0, 0);
            assert!(!v2.is_compatible());
        }
    }

    #[test]
    fn default_is_current() {
        let v = SchemaVersion::default();
        assert_eq!(v, SchemaVersion::current());
        assert!(v.is_compatible());
    }

    #[test]
    fn equality() {
        let v1 = SchemaVersion::new(1, 0);
        let v2 = SchemaVersion::new(1, 0);
        let v3 = SchemaVersion::new(1, 1);
        let v4 = SchemaVersion::new(2, 0);

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
        assert_ne!(v1, v4);
    }

    #[test]
    fn hash_impl() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SchemaVersion::new(1, 0));
        set.insert(SchemaVersion::new(1, 1));
        set.insert(SchemaVersion::new(1, 0)); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn copy_semantics() {
        let v1 = SchemaVersion::new(1, 0);
        let v2 = v1; // Copy
        assert_eq!(v1, v2);
        assert_eq!(v1.major, 1); // v1 still usable
    }

    #[test]
    fn debug_format() {
        let v = SchemaVersion::new(1, 5);
        let debug = format!("{:?}", v);
        assert!(debug.contains("SchemaVersion"));
        assert!(debug.contains("1"));
        assert!(debug.contains("5"));
    }

    #[test]
    fn const_new() {
        // Verify new is const
        const V: SchemaVersion = SchemaVersion::new(1, 0);
        assert_eq!(V.major, 1);
        assert_eq!(V.minor, 0);
    }

    #[test]
    fn const_current() {
        // Verify current is const
        const V: SchemaVersion = SchemaVersion::current();
        assert_eq!(V.major, SCHEMA_VERSION);
    }
}
