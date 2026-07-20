//! History and settings storage boundary.

#![deny(missing_docs)]

use directories::ProjectDirs;
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Logical storage schema version reserved for future migrations.
pub const STORAGE_SCHEMA_VERSION: u16 = 0;

pub use wsl_traffic_core::SpeedUnit;

/// User configuration settings for WSL Traffic Monitor.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UserSettings {
    /// Polling interval in milliseconds (default: 1000).
    pub poll_interval_ms: u64,
    /// Speed display units (default: `SpeedUnit::Bytes`).
    pub speed_unit: SpeedUnit,
    /// Whether the application runs at Windows startup (default: false).
    pub run_at_startup: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            speed_unit: SpeedUnit::Bytes,
            run_at_startup: false,
        }
    }
}

/// Retrieve the absolute path to the settings file.
#[must_use]
pub fn get_settings_path() -> Option<PathBuf> {
    if cfg!(test) {
        Some(std::env::temp_dir().join("wsl_traffic_settings_test.json"))
    } else if let Ok(val) = std::env::var("WSL_TRAFFIC_SETTINGS") {
        Some(PathBuf::from(val))
    } else {
        ProjectDirs::from("com", "wsl-traffic-monitor", "WSL Traffic Monitor")
            .map(|proj| proj.config_dir().join("settings.json"))
    }
}

/// Load user settings from the configuration file. Falls back to defaults on failure or missing file.
#[must_use]
pub fn load_settings() -> UserSettings {
    if let Some(path) = get_settings_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str::<UserSettings>(&content) {
                    return settings;
                }
            }
        }
    }
    UserSettings::default()
}

/// Save user settings to the configuration file.
///
/// # Errors
/// Returns an error string if directory creation or file writing fails.
pub fn save_settings(settings: &UserSettings) -> Result<(), String> {
    let Some(path) = get_settings_path() else {
        return Err("Could not determine settings path".to_string());
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

// --- HISTORY PERSISTENCE SYSTEM ---

const HOURLY_TABLE: TableDefinition<&str, [u8; 16]> = TableDefinition::new("hourly_usage");
const DAILY_TABLE: TableDefinition<&str, [u8; 16]> = TableDefinition::new("daily_usage");

struct HistoryState {
    pending_up: u64,
    pending_down: u64,
    last_flush: Instant,
}

static HISTORY_STATE: Mutex<Option<HistoryState>> = Mutex::new(None);

/// Retrieve the absolute path to the history database.
#[must_use]
pub fn get_history_db_path() -> Option<PathBuf> {
    if cfg!(test) {
        Some(std::env::temp_dir().join("wsl_traffic_history_test.redb"))
    } else if let Ok(val) = std::env::var("WSL_TRAFFIC_HISTORY_DB") {
        Some(PathBuf::from(val))
    } else {
        ProjectDirs::from("com", "wsl-traffic-monitor", "WSL Traffic Monitor")
            .map(|proj| proj.data_dir().join("history.redb"))
    }
}

fn pack_totals(up: u64, down: u64) -> [u8; 16] {
    let mut buf = [0u8; 16];
    buf[..8].copy_from_slice(&up.to_le_bytes());
    buf[8..].copy_from_slice(&down.to_le_bytes());
    buf
}

fn unpack_totals(buf: [u8; 16]) -> (u64, u64) {
    let up = u64::from_le_bytes(buf[..8].try_into().unwrap_or([0u8; 8]));
    let down = u64::from_le_bytes(buf[8..].try_into().unwrap_or([0u8; 8]));
    (up, down)
}

/// Record usage data in memory. Accumulates stats and periodically flushes to the database.
///
/// # Errors
/// Returns an error string if database flush fails.
#[allow(clippy::missing_panics_doc)]
pub fn record_usage(upload_bytes: u64, download_bytes: u64) -> Result<(), String> {
    let mut lock = HISTORY_STATE.lock().unwrap();
    let state = lock.get_or_insert_with(|| HistoryState {
        pending_up: 0,
        pending_down: 0,
        last_flush: Instant::now(),
    });

    state.pending_up += upload_bytes;
    state.pending_down += download_bytes;

    // Flush every 60 seconds
    if state.last_flush.elapsed() >= Duration::from_secs(60) {
        let up = state.pending_up;
        let down = state.pending_down;
        state.pending_up = 0;
        state.pending_down = 0;
        state.last_flush = Instant::now();

        // Release lock before doing blocking disk I/O
        drop(lock);

        flush_to_db(up, down)?;
    }

    Ok(())
}

/// Forcibly flush any pending in-memory accumulated usage metrics to the database.
///
/// # Errors
/// Returns an error string if database write fails.
#[allow(clippy::missing_panics_doc)]
pub fn flush_history() -> Result<(), String> {
    let mut lock = HISTORY_STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        let up = state.pending_up;
        let down = state.pending_down;
        state.pending_up = 0;
        state.pending_down = 0;
        state.last_flush = Instant::now();

        drop(lock);

        flush_to_db(up, down)?;
    }
    Ok(())
}

fn flush_to_db(up: u64, down: u64) -> Result<(), String> {
    if up == 0 && down == 0 {
        return Ok(());
    }

    let Some(db_path) = get_history_db_path() else {
        return Err("Could not determine history database path".to_string());
    };

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Open/create database.
    let db = Database::create(&db_path).map_err(|e| e.to_string())?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;

    let now = time::OffsetDateTime::now_utc();
    let hour_key = format!(
        "hourly:{:04}{:02}{:02}{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour()
    );
    let day_key = format!(
        "daily:{:04}{:02}{:02}",
        now.year(),
        now.month() as u8,
        now.day()
    );

    {
        // 1. Update hourly table
        let mut hourly_table = write_txn
            .open_table(HOURLY_TABLE)
            .map_err(|e| e.to_string())?;
        let existing_hourly = hourly_table
            .get(hour_key.as_str())
            .map_err(|e| e.to_string())?
            .map_or((0, 0), |v| unpack_totals(v.value()));
        hourly_table
            .insert(
                hour_key.as_str(),
                &pack_totals(existing_hourly.0 + up, existing_hourly.1 + down),
            )
            .map_err(|e| e.to_string())?;

        // 2. Update daily table
        let mut daily_table = write_txn
            .open_table(DAILY_TABLE)
            .map_err(|e| e.to_string())?;
        let existing_daily = daily_table
            .get(day_key.as_str())
            .map_err(|e| e.to_string())?
            .map_or((0, 0), |v| unpack_totals(v.value()));
        daily_table
            .insert(
                day_key.as_str(),
                &pack_totals(existing_daily.0 + up, existing_daily.1 + down),
            )
            .map_err(|e| e.to_string())?;
    }

    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Retrieve the hourly history records. Returns a list of `(timestamp_key, upload_bytes, download_bytes)`.
///
/// # Errors
/// Returns an error string if database query fails.
pub fn get_hourly_history(limit: usize) -> Result<Vec<(String, u64, u64)>, String> {
    get_history_from_table(&HOURLY_TABLE, limit)
}

/// Retrieve the daily history records. Returns a list of `(timestamp_key, upload_bytes, download_bytes)`.
///
/// # Errors
/// Returns an error string if database query fails.
pub fn get_daily_history(limit: usize) -> Result<Vec<(String, u64, u64)>, String> {
    get_history_from_table(&DAILY_TABLE, limit)
}

fn get_history_from_table(
    table_def: &TableDefinition<&str, [u8; 16]>,
    limit: usize,
) -> Result<Vec<(String, u64, u64)>, String> {
    let Some(db_path) = get_history_db_path() else {
        return Ok(Vec::new());
    };
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let db = Database::open(&db_path).map_err(|e| e.to_string())?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;

    let Ok(table) = read_txn.open_table(*table_def) else {
        return Ok(Vec::new());
    };

    let mut results = Vec::new();
    let range = table.iter().map_err(|e| e.to_string())?;
    for item in range {
        let (key, val) = item.map_err(|e| e.to_string())?;
        let (up, down) = unpack_totals(val.value());
        results.push((key.value().to_string(), up, down));
    }

    // Sort by key (chronological)
    results.sort_by(|a, b| a.0.cmp(&b.0));

    // Limit to the last N items
    if results.len() > limit {
        let drain_len = results.len() - limit;
        results.drain(0..drain_len);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_load_save() {
        let settings = UserSettings {
            poll_interval_ms: 2000,
            speed_unit: SpeedUnit::Bits,
            run_at_startup: true,
        };
        let serialized = serde_json::to_string(&settings).unwrap();
        let deserialized: UserSettings = serde_json::from_str(&serialized).unwrap();
        assert_eq!(settings, deserialized);
    }

    #[test]
    fn test_history_packing() {
        let packed = pack_totals(100, 200);
        let (up, down) = unpack_totals(packed);
        assert_eq!(up, 100);
        assert_eq!(down, 200);
    }

    #[test]
    fn test_history_db_operations() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_history.redb");
        if db_path.exists() {
            let _ = std::fs::remove_file(&db_path);
        }

        let db = Database::create(&db_path).unwrap();
        let write_txn = db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(HOURLY_TABLE).unwrap();
            table
                .insert("hourly:2026071900", &pack_totals(500, 600))
                .unwrap();
        }
        write_txn.commit().unwrap();

        let read_txn = db.begin_read().unwrap();
        let table = read_txn.open_table(HOURLY_TABLE).unwrap();
        let val = table.get("hourly:2026071900").unwrap().unwrap().value();
        let (up, down) = unpack_totals(val);
        assert_eq!(up, 500);
        assert_eq!(down, 600);

        let _ = std::fs::remove_file(&db_path);
    }
}
