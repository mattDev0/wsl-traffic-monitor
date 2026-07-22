//! Error types for storage and persistence operations.

use thiserror::Error;

/// Failure modes encountered during settings or history database storage operations.
#[derive(Error, Debug)]
pub enum StorageError {
    /// Failed to determine configuration path.
    #[error("Could not determine settings path")]
    SettingsPathNotFound,

    /// Failed to determine history database path.
    #[error("Could not determine history database path")]
    HistoryDbPathNotFound,

    /// I/O error during settings or database directory creation or file operations.
    #[error("Storage I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Error serializing or deserializing settings JSON.
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error performing `redb` database transaction or table operations.
    #[error("Database error: {0}")]
    Database(Box<redb::Error>),

    /// Database error during database creation or opening.
    #[error("Database error: {0}")]
    DatabaseOpen(Box<redb::DatabaseError>),

    /// Transaction-specific `redb` error.
    #[error("Database transaction error: {0}")]
    Transaction(Box<redb::TransactionError>),

    /// Table-specific `redb` error.
    #[error("Database table error: {0}")]
    Table(Box<redb::TableError>),

    /// Storage database access error.
    #[error("Database storage error: {0}")]
    Storage(Box<redb::StorageError>),

    /// Commit error during database transaction commit.
    #[error("Commit error: {0}")]
    Commit(Box<redb::CommitError>),
}

impl From<redb::Error> for StorageError {
    fn from(err: redb::Error) -> Self {
        StorageError::Database(Box::new(err))
    }
}

impl From<redb::DatabaseError> for StorageError {
    fn from(err: redb::DatabaseError) -> Self {
        StorageError::DatabaseOpen(Box::new(err))
    }
}

impl From<redb::TransactionError> for StorageError {
    fn from(err: redb::TransactionError) -> Self {
        StorageError::Transaction(Box::new(err))
    }
}

impl From<redb::TableError> for StorageError {
    fn from(err: redb::TableError) -> Self {
        StorageError::Table(Box::new(err))
    }
}

impl From<redb::StorageError> for StorageError {
    fn from(err: redb::StorageError) -> Self {
        StorageError::Storage(Box::new(err))
    }
}

impl From<redb::CommitError> for StorageError {
    fn from(err: redb::CommitError) -> Self {
        StorageError::Commit(Box::new(err))
    }
}
