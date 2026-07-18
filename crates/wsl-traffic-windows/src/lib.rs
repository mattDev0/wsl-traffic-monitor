//! Windows platform boundary.
//!
//! Future code in this crate will wrap Win32 APIs such as IP Helper. Monitoring
//! logic should remain in higher-level crates.

/// Returns whether this build target is Windows.
#[must_use]
pub const fn is_windows_target() -> bool {
    cfg!(windows)
}
