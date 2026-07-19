//! History and settings storage boundary.

#![deny(missing_docs)]

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Logical storage schema version reserved for future migrations.
pub const STORAGE_SCHEMA_VERSION: u16 = 0;

/// The units used to display network speeds.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SpeedUnit {
    /// Display speed in Bytes per second (B/s, KiB/s, MiB/s).
    Bytes,
    /// Display speed in Bits per second (bps, Kbps, Mbps).
    Bits,
}

/// User configuration settings for WSL Traffic Monitor.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UserSettings {
    /// Polling interval in milliseconds (default: 1000).
    pub poll_interval_ms: u64,
    /// Speed display units (default: `SpeedUnit::Bytes`).
    pub speed_unit: SpeedUnit,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            speed_unit: SpeedUnit::Bytes,
        }
    }
}

/// Retrieve the absolute path to the settings file.
#[must_use]
pub fn get_settings_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "wsl-traffic-monitor", "WSL Traffic Monitor")
        .map(|proj| proj.config_dir().join("settings.json"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_load_save() {
        let settings = UserSettings {
            poll_interval_ms: 2000,
            speed_unit: SpeedUnit::Bits,
        };
        // Verify roundtrip serialization
        let serialized = serde_json::to_string(&settings).unwrap();
        let deserialized: UserSettings = serde_json::from_str(&serialized).unwrap();
        assert_eq!(settings, deserialized);
    }
}
