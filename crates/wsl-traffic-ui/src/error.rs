//! Error types for native Windows UI boundary operations.

use thiserror::Error;
use wsl_traffic_monitor::MonitorError;
use wsl_traffic_storage::StorageError;
use wsl_traffic_windows::WindowsError;

/// Failure modes encountered during system tray or floating overlay UI operations.
#[derive(Error, Debug)]
pub enum UiError {
    /// Failed to register Win32 window class.
    #[error("Failed to register window class: {details}")]
    WindowClassRegistrationFailed {
        /// OS error details.
        details: String,
    },

    /// Failed to create Win32 window handle.
    #[error("Failed to create window: {details}")]
    WindowCreationFailed {
        /// OS error details.
        details: String,
    },

    /// Failed to initialize system tray icon.
    #[error("Failed to add system tray icon")]
    TrayIconInitializationFailed,

    /// Error from Windows platform boundary.
    #[error("Windows error: {0}")]
    Windows(#[from] WindowsError),

    /// Error from storage boundary.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Error from monitor boundary.
    #[error("Monitor error: {0}")]
    Monitor(#[from] MonitorError),
}
