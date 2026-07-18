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
}
