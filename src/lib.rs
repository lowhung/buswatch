//! # buswatch
//!
//! A diagnostic TUI and library for monitoring Caryatid message bus activity.
//!
//! This crate re-exports the `buswatch-tui` crate for backwards compatibility.
//! For new code, consider depending on `buswatch-tui` directly.
//!
//! See the [`buswatch_tui`] crate documentation for full API details.

pub use buswatch_tui::*;
pub use buswatch_types;
