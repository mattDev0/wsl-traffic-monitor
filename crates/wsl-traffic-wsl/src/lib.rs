//! WSL discovery and configuration boundary.
//!
//! This crate will eventually parse WSL configuration and discover installed
//! distributions. It does not shell out or inspect the host yet.

/// WSL networking modes recognized by the architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WslNetworkingMode {
    /// WSL networking is disabled.
    None,
    /// Default NAT-based WSL2 networking.
    Nat,
    /// Deprecated bridged networking mode.
    Bridged,
    /// Windows interfaces are mirrored into Linux.
    Mirrored,
    /// `VirtioProxy` networking mode or fallback.
    VirtioProxy,
    /// Runtime mode is not known.
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::WslNetworkingMode;

    #[test]
    fn unknown_mode_is_available_for_unclassified_hosts() {
        assert_eq!(WslNetworkingMode::Unknown, WslNetworkingMode::Unknown);
    }
}
