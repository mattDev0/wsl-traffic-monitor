//! Error types for diagnostics generation.

use thiserror::Error;
use wsl_traffic_windows::WindowsError;

/// Failure modes encountered during diagnostics snapshot generation or JSON formatting.
#[derive(Error, Debug)]
pub enum DiagnosticsError {
    /// Error querying Windows platform network adapter information.
    #[error("Windows error: {0}")]
    Windows(#[from] WindowsError),

    /// Error serializing report to JSON.
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
