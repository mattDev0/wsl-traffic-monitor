//! History storage boundary.
//!
//! Persistence backends and migrations will be introduced after counter
//! semantics are validated.

/// Logical storage schema version reserved for future migrations.
pub const STORAGE_SCHEMA_VERSION: u16 = 0;
