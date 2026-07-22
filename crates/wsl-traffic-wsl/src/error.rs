//! Error types for WSL discovery and configuration boundary.

use thiserror::Error;

/// Failure modes encountered during WSL discovery and configuration parsing.
#[derive(Error, Debug)]
pub enum WslError {
    /// Failed to read or parse INI configuration file.
    #[error("Failed to read INI file: {0}")]
    IniIo(#[from] std::io::Error),
}
