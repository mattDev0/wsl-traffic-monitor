//! Error types for Windows platform boundary operations.

use thiserror::Error;

/// Failure modes encountered during Windows API or system operations.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WindowsError {
    /// Failed to query network adapter addresses from IP Helper API.
    #[error("GetAdaptersAddresses failed with error code: {code}")]
    GetAdaptersAddressesFailed {
        /// Windows error status code.
        code: u32,
    },

    /// Failed to query interface counters for a specific LUID.
    #[error("GetIfEntry2 failed for LUID {luid} with code: {code}")]
    GetIfEntry2Failed {
        /// Interface LUID identifier.
        luid: u64,
        /// Windows error status code.
        code: u32,
    },

    /// Failed to open Windows registry key for autostart configuration.
    #[error("Failed to open Run registry key: {details}")]
    OpenRunRegistryKeyFailed {
        /// Detailed OS error message.
        details: String,
    },

    /// Failed to set Windows registry value for autostart configuration.
    #[error("Failed to write autostart registry value: {details}")]
    WriteAutostartRegistryValueFailed {
        /// Detailed OS error message.
        details: String,
    },

    /// Failed to resolve current executable path.
    #[error("Failed to get current executable path: {details}")]
    CurrentExePathFailed {
        /// Detailed OS error message.
        details: String,
    },
}
