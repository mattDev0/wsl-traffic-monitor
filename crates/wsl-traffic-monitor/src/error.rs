//! Error types for monitoring orchestration.

use thiserror::Error;
use wsl_traffic_storage::StorageError;
use wsl_traffic_windows::WindowsError;
use wsl_traffic_wsl::WslError;

/// Failure modes encountered during traffic monitoring orchestration.
#[derive(Error, Debug)]
pub enum MonitorError {
    /// Monitoring service is already active and running.
    #[error("Monitoring service is already running")]
    AlreadyRunning,

    /// Monitoring service is parked due to unsupported networking mode.
    #[error("Monitoring service cannot start in unsupported mode")]
    UnsupportedMode,

    /// Error from Windows platform boundary.
    #[error("Windows error: {0}")]
    Windows(#[from] WindowsError),

    /// Error from storage boundary.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Error from WSL boundary.
    #[error("WSL error: {0}")]
    Wsl(#[from] WslError),
}
