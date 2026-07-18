//! Core domain types for WSL Traffic Monitor.
//!
//! This crate is intentionally platform-neutral. It should not depend on Win32,
//! WSL command execution, UI frameworks, or storage backends.

/// Human-readable product name used across crates.
pub const PRODUCT_NAME: &str = "WSL Traffic Monitor";

/// Current measurement confidence for a traffic sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementConfidence {
    /// WSL-only attribution has been validated for the active source.
    High,
    /// The selected source is likely WSL-related, but may include adjacent traffic.
    Medium,
    /// The selected source is experimental and not proven to isolate WSL traffic.
    Experimental,
    /// No source can currently isolate WSL traffic.
    Unsupported,
}

#[cfg(test)]
mod tests {
    use super::{MeasurementConfidence, PRODUCT_NAME};

    #[test]
    fn product_name_is_stable() {
        assert_eq!(PRODUCT_NAME, "WSL Traffic Monitor");
    }

    #[test]
    fn confidence_values_are_comparable() {
        assert_eq!(MeasurementConfidence::High, MeasurementConfidence::High);
        assert_ne!(
            MeasurementConfidence::High,
            MeasurementConfidence::Unsupported
        );
    }
}
