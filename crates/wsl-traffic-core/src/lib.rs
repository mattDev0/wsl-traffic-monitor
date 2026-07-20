//! Core domain types for WSL Traffic Monitor.
//!
//! This crate is intentionally platform-neutral. It should not depend on Win32,
//! WSL command execution, UI frameworks, or storage backends.

use serde::{Deserialize, Serialize};

/// Human-readable product name used across crates.
pub const PRODUCT_NAME: &str = "WSL Traffic Monitor";

/// Current measurement confidence for a traffic sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MeasurementConfidence {
    /// WSL-only attribution has been validated for the active source.
    High,
    /// The selected source is likely WSL-related, but may include adjacent traffic.
    Medium,
    /// The selected source is experimental and not proven to isolate WSL traffic.
    Experimental,
    /// No source can currently isolate WSL traffic.
    Unsupported,
}

impl std::fmt::Display for MeasurementConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::High => write!(f, "High"),
            Self::Medium => write!(f, "Medium"),
            Self::Experimental => write!(f, "Experimental"),
            Self::Unsupported => write!(f, "Unsupported"),
        }
    }
}

/// The units used to display network speeds.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SpeedUnit {
    /// Display speed in Bytes per second (B/s, KiB/s, MiB/s).
    Bytes,
    /// Display speed in Bits per second (bps, Kbps, Mbps).
    Bits,
}

/// Format speed in bytes per second into a human-readable string given `SpeedUnit`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn format_speed(bytes_per_sec: f64, unit: SpeedUnit) -> String {
    match unit {
        SpeedUnit::Bytes => {
            if bytes_per_sec >= 1024.0 * 1024.0 * 1024.0 {
                format!("{:.2} GiB/s", bytes_per_sec / (1024.0 * 1024.0 * 1024.0))
            } else if bytes_per_sec >= 1024.0 * 1024.0 {
                format!("{:.2} MiB/s", bytes_per_sec / (1024.0 * 1024.0))
            } else if bytes_per_sec >= 1024.0 {
                format!("{:.2} KiB/s", bytes_per_sec / 1024.0)
            } else {
                format!("{bytes_per_sec:.0} B/s")
            }
        }
        SpeedUnit::Bits => {
            let bits_per_sec = bytes_per_sec * 8.0;
            if bits_per_sec >= 1000.0 * 1000.0 * 1000.0 {
                format!("{:.2} Gbps", bits_per_sec / (1000.0 * 1000.0 * 1000.0))
            } else if bits_per_sec >= 1000.0 * 1000.0 {
                format!("{:.2} Mbps", bits_per_sec / (1000.0 * 1000.0))
            } else if bits_per_sec >= 1000.0 {
                format!("{:.2} Kbps", bits_per_sec / 1000.0)
            } else {
                format!("{bits_per_sec:.0} bps")
            }
        }
    }
}

/// Format speed into a compact string suitable for icon rendering (e.g. "1.2M", "500K", "0").
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn format_speed_compact(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1}G", bytes_per_sec / (1024.0 * 1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024.0 * 1024.0 {
        format!("{:.1}M", bytes_per_sec / (1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.0}K", bytes_per_sec / 1024.0)
    } else {
        format!("{bytes_per_sec:.0}")
    }
}

/// Detailed information about a network adapter in the host system.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AdapterInfo {
    /// Interface LUID (Logical Unique Identifier)
    pub luid: u64,
    /// Interface index
    pub if_index: u32,
    /// Interface GUID (canonical string format)
    pub guid: String,
    /// Interface alias
    pub alias: String,
    /// Friendly name
    pub friendly_name: String,
    /// Interface description
    pub description: String,
    /// Interface Type (e.g. `IF_TYPE_ETHERNET_CSMACD` = 6)
    pub if_type: u32,
    /// Operational status
    pub oper_status: u32,
    /// MAC Address (formatted as hex octets separated by colons)
    pub mac_address: String,
    /// MTU (Maximum Transmission Unit)
    pub mtu: u32,
    /// Link speed in bits per second
    pub link_speed: u64,
    /// IPv4 addresses associated with the adapter
    pub ipv4_addresses: Vec<String>,
    /// IPv6 addresses associated with the adapter
    pub ipv6_addresses: Vec<String>,
    /// Current byte counter: Outgoing from host perspective
    pub bytes_sent: u64,
    /// Current byte counter: Incoming from host perspective
    pub bytes_recv: u64,
    /// Current packet counter: Outgoing from host perspective
    pub packets_sent: u64,
    /// Current packet counter: Incoming from host perspective
    pub packets_recv: u64,
}

/// Detailed information about a WSL distribution.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WslDistroInfo {
    /// Distribution name (e.g. "Ubuntu")
    pub name: String,
    /// WSL version (1 or 2)
    pub version: u32,
    /// Whether the distribution is currently running
    pub is_running: bool,
}

/// Detailed WSL installation and configuration state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WslInfo {
    /// Whether WSL is installed on the host
    pub is_installed: bool,
    /// Installed WSL version (queried from wsl.exe --version)
    pub wsl_version: Option<String>,
    /// WSL kernel version
    pub kernel_version: Option<String>,
    /// List of installed WSL distributions
    pub distributions: Vec<WslDistroInfo>,
    /// WSL networking mode (e.g. "nat", "mirrored")
    pub networking_mode: String,
    /// Parsed .wslconfig content (section -> key -> value)
    pub wslconfig_parsed:
        Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,
}

/// Docker Desktop installation and process state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DockerInfo {
    /// Whether Docker Desktop is installed on the host
    pub is_installed: bool,
    /// Whether the docker-desktop distribution is installed in WSL
    pub has_docker_desktop_distro: bool,
    /// Whether the docker-desktop-data distribution is installed in WSL
    pub has_docker_desktop_data_distro: bool,
    /// List of running Docker-related processes on the host
    pub running_processes: Vec<String>,
}

/// Classification of a candidate network adapter for WSL traffic monitoring.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CandidateClassification {
    /// LUID of the classified adapter
    pub adapter_luid: u64,
    /// Name or description of the adapter for display
    pub adapter_name: String,
    /// Classification confidence level
    pub confidence: MeasurementConfidence,
    /// Scoring value used to rank candidate
    pub score: i32,
    /// Explanation of why this classification and score was assigned
    pub explanation: String,
}

/// Complete diagnostic snapshot report.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiagnosticsReport {
    /// Schema version for the diagnostics report
    pub schema_version: u16,
    /// ISO-8601 timestamp when the report was generated
    pub timestamp: String,
    /// Windows version string
    pub windows_version: String,
    /// WSL state details
    pub wsl_info: WslInfo,
    /// Docker state details
    pub docker_info: DockerInfo,
    /// Inventory of network adapters
    pub adapters: Vec<AdapterInfo>,
    /// Classification and scoring of each adapter
    pub classifications: Vec<CandidateClassification>,
    /// Recommendation for which adapter to monitor
    pub recommendation: String,
    /// Advanced networking and experimental diagnostics (Phase 3)
    pub advanced_diagnostics: AdvancedNetworkingDiagnostics,
}

/// A calculated sample representing bandwidth usage over a measurement interval.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrafficSample {
    /// Upload speed in bytes per second from WSL user perspective
    pub upload_bytes_per_sec: f64,
    /// Download speed in bytes per second from WSL user perspective
    pub download_bytes_per_sec: f64,
    /// ISO-8601/RFC-3339 formatted UTC timestamp of the sample
    pub timestamp: String,
}

/// Advanced networking and experimental diagnostics for evaluation (Phase 3).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct AdvancedNetworkingDiagnostics {
    /// Whether mirrored mode was explicitly parsed from wslconfig
    pub wslconfig_mirrored: bool,
    /// Whether virtioproxy was explicitly parsed from wslconfig
    pub wslconfig_virtioproxy: bool,
    /// Whether DNS tunneling is enabled in wslconfig
    pub dns_tunneling_enabled: bool,
    /// Whether the host has access to run ETW network tracing (checks privileges/admin status)
    pub etw_tracing_accessible: bool,
    /// Whether the host has access to query WFP compartments/API (checks user permissions)
    pub wfp_accessible: bool,
    /// Additional evaluation details/findings for diagnostics
    pub evaluation_notes: String,
}

#[cfg(test)]
mod tests {
    use super::{MeasurementConfidence, PRODUCT_NAME};

    #[test]
    fn product_name_is_stable() {
        assert_eq!(PRODUCT_NAME, "WSL Traffic Monitor");
    }

    #[test]
    fn confidence_values_are_comparable() {
        assert_eq!(MeasurementConfidence::High, MeasurementConfidence::High);
        assert_ne!(
            MeasurementConfidence::High,
            MeasurementConfidence::Unsupported
        );
    }

    #[test]
    fn test_format_speed() {
        use super::{SpeedUnit, format_speed};
        assert_eq!(format_speed(500.0, SpeedUnit::Bytes), "500 B/s");
        assert_eq!(format_speed(2048.0, SpeedUnit::Bytes), "2.00 KiB/s");
        assert_eq!(format_speed(1000.0, SpeedUnit::Bits), "8.00 Kbps");
    }

    #[test]
    fn test_format_speed_compact() {
        use super::format_speed_compact;
        assert_eq!(format_speed_compact(0.0), "0");
        assert_eq!(format_speed_compact(500.0), "500");
        assert_eq!(format_speed_compact(1500.0), "1K");
        assert_eq!(format_speed_compact(1_500_000.0), "1.4M");
    }
}
