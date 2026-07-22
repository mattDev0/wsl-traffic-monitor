//! Monitoring orchestration and adapter classification logic.
//!
//! Evaluates candidate network interfaces for WSL traffic monitoring suitability.

pub mod error;

pub use error::MonitorError;

use wsl_traffic_core::{
    AdapterInfo, CandidateClassification, DockerInfo, MeasurementConfidence, TrafficSample, WslInfo,
};

/// Query Docker Desktop installation and process status.
#[must_use]
pub fn detect_docker(wsl_info: &WslInfo) -> DockerInfo {
    let is_installed = wsl_traffic_windows::is_docker_desktop_installed();

    let mut has_docker_desktop_distro = false;
    let mut has_docker_desktop_data_distro = false;

    for distro in &wsl_info.distributions {
        if distro.name == "docker-desktop" {
            has_docker_desktop_distro = true;
        } else if distro.name == "docker-desktop-data" {
            has_docker_desktop_data_distro = true;
        }
    }

    let running_processes = wsl_traffic_windows::get_running_docker_processes();

    DockerInfo {
        is_installed: is_installed || has_docker_desktop_distro,
        has_docker_desktop_distro,
        has_docker_desktop_data_distro,
        running_processes,
    }
}

/// Score and classify every adapter to evaluate its suitability as a WSL traffic source.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn classify_adapters(
    adapters: &[AdapterInfo],
    wsl_info: &WslInfo,
    docker_info: &DockerInfo,
) -> Vec<CandidateClassification> {
    let mut classifications = Vec::new();

    for adapter in adapters {
        let name_lower = adapter.friendly_name.to_lowercase();
        let desc_lower = adapter.description.to_lowercase();
        let alias_lower = adapter.alias.to_lowercase();

        let has_wsl_subnet_ip = adapter.ipv4_addresses.iter().any(|ip| {
            if ip.starts_with("192.168.") || ip.starts_with("10.") {
                return true;
            }
            if ip.starts_with("172.") {
                if let Some(second_octet) = ip.split('.').nth(1).and_then(|s| s.parse::<u8>().ok())
                {
                    return (16..=31).contains(&second_octet);
                }
            }
            false
        });

        // Classify and score adapter
        let (score, confidence, explanation) = if name_lower.contains("docker")
            || desc_lower.contains("docker")
            || alias_lower.contains("docker")
            || name_lower.contains("vbr")
            || desc_lower.contains("vbr")
        {
            (
                -10,
                MeasurementConfidence::Unsupported,
                "Identified as Docker virtual bridge / endpoint; excluded from WSL monitoring"
                    .to_string(),
            )
        } else if desc_lower.contains("hyper-v virtual ethernet adapter")
            || name_lower.contains("vethernet")
            || alias_lower.contains("vethernet")
        {
            if name_lower.contains("wsl")
                || desc_lower.contains("wsl")
                || alias_lower.contains("wsl")
            {
                if adapter.oper_status == 1 {
                    let (conf, expl) = if docker_info.is_installed
                        || !docker_info.running_processes.is_empty()
                    {
                        (MeasurementConfidence::Medium, "Hyper-V Virtual Ethernet adapter explicitly named 'WSL' and is UP. Confidence downgraded to Medium because Docker Desktop is installed/running".to_string())
                    } else {
                        (MeasurementConfidence::High, "Hyper-V Virtual Ethernet adapter explicitly named 'WSL' and is UP. High confidence NAT isolation".to_string())
                    };
                    let bonus = if has_wsl_subnet_ip { 5 } else { 0 };
                    (90 + bonus, conf, expl)
                } else {
                    let bonus = if has_wsl_subnet_ip { 5 } else { 0 };
                    (70 + bonus, MeasurementConfidence::Medium, "Hyper-V Virtual Ethernet adapter explicitly named 'WSL' but is currently offline/down".to_string())
                }
            } else {
                let (score, conf, expl) = if has_wsl_subnet_ip {
                    let (c, e) = if docker_info.is_installed
                        || !docker_info.running_processes.is_empty()
                    {
                        (MeasurementConfidence::Medium, "Hyper-V adapter without 'WSL' in name, but has WSL private subnet IP. Confidence is Medium because Docker Desktop is installed/running".to_string())
                    } else {
                        (MeasurementConfidence::High, "Hyper-V adapter without 'WSL' in name, but has WSL private subnet IP. High confidence NAT isolation".to_string())
                    };
                    (65, c, e)
                } else {
                    (50, MeasurementConfidence::Medium, "Hyper-V Virtual Ethernet adapter, possibly used by virtual machines, but not explicitly named 'WSL'".to_string())
                };
                (score, conf, expl)
            }
        } else if wsl_info.networking_mode == "mirrored" {
            let is_physical = adapter.if_type == 6 || adapter.if_type == 71;
            let has_ip = !adapter.ipv4_addresses.is_empty() || !adapter.ipv6_addresses.is_empty();

            if is_physical && has_ip && adapter.oper_status == 1 {
                (30, MeasurementConfidence::Experimental, "Mirrored networking mode is active. Host physical interface mirrors WSL traffic but will include host Windows traffic".to_string())
            } else {
                (
                    0,
                    MeasurementConfidence::Unsupported,
                    "Not a WSL-related network interface".to_string(),
                )
            }
        } else {
            (
                0,
                MeasurementConfidence::Unsupported,
                "Not a WSL-related network interface".to_string(),
            )
        };

        classifications.push(CandidateClassification {
            adapter_luid: adapter.luid,
            adapter_name: if adapter.friendly_name.is_empty() {
                adapter.description.clone()
            } else {
                adapter.friendly_name.clone()
            },
            confidence,
            score,
            explanation,
        });
    }

    // Sort: score descending, then LUID ascending
    classifications.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.adapter_luid.cmp(&b.adapter_luid))
    });

    classifications
}

/// Generate a measurement recommendation based on adapter classifications.
#[must_use]
pub fn generate_recommendation(
    classifications: &[CandidateClassification],
    wsl_info: &WslInfo,
) -> String {
    if !wsl_info.is_installed {
        return "WSL does not appear to be installed on this host. No monitoring is possible."
            .to_string();
    }

    if let Some(best) = classifications.first() {
        match best.confidence {
            MeasurementConfidence::High => {
                format!(
                    "RECOMMENDED: Monitor the high-confidence WSL adapter '{}' (LUID {}). NAT-mode isolation is fully validated.",
                    best.adapter_name, best.adapter_luid
                )
            }
            MeasurementConfidence::Medium => {
                format!(
                    "RECOMMENDED: Monitor the medium-confidence adapter '{}' (LUID {}). Warning: adjacent virtual machine or Docker traffic may be mixed in.",
                    best.adapter_name, best.adapter_luid
                )
            }
            MeasurementConfidence::Experimental => {
                format!(
                    "RECOMMENDED (EXPERIMENTAL): Mirrored networking is active. Monitor physical adapter '{}' (LUID {}). Warning: exact WSL traffic cannot be isolated from host Windows traffic.",
                    best.adapter_name, best.adapter_luid
                )
            }
            MeasurementConfidence::Unsupported => {
                "No suitable WSL traffic monitor adapter could be identified. WSL traffic monitoring is unsupported on this host configuration.".to_string()
            }
        }
    } else {
        "No network adapters were found. Cannot provide a monitoring recommendation.".to_string()
    }
}

/// The current connectivity state of the monitoring engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorState {
    /// Actively polling and computing deltas on the WSL interface.
    Active,
    /// Interface is missing or disconnected; attempting to reconnect.
    Disconnected,
    /// The configured .wslconfig networking mode (e.g., mirrored) is unsupported for host-side monitoring.
    UnsupportedNetworkingMode,
}

/// Abstract provider for retrieving OS-level network information (for testing and dependency injection).
pub trait NetworkProvider: Send + 'static {
    /// Query all network adapters on the host.
    ///
    /// # Errors
    /// Returns an error if querying host adapters fails.
    fn get_adapters(&self) -> Result<Vec<AdapterInfo>, MonitorError>;
    /// Query raw counters for a specific network interface.
    ///
    /// # Errors
    /// Returns an error if the interface is not found or querying fails.
    fn get_interface_counters(
        &self,
        luid: u64,
    ) -> Result<wsl_traffic_windows::RawInterfaceCounters, MonitorError>;
    /// Detect the WSL installation state.
    fn detect_wsl(&self) -> WslInfo;
    /// Query Docker Desktop integration state.
    fn detect_docker(&self, wsl_info: &WslInfo) -> DockerInfo;
}

/// Concrete production network provider wrapping real system calls.
pub struct SystemNetworkProvider;

impl NetworkProvider for SystemNetworkProvider {
    fn get_adapters(&self) -> Result<Vec<AdapterInfo>, MonitorError> {
        Ok(wsl_traffic_windows::get_adapters()?)
    }

    fn get_interface_counters(
        &self,
        luid: u64,
    ) -> Result<wsl_traffic_windows::RawInterfaceCounters, MonitorError> {
        Ok(wsl_traffic_windows::get_interface_counters(luid)?)
    }

    fn detect_wsl(&self) -> WslInfo {
        wsl_traffic_wsl::detect_wsl()
    }

    fn detect_docker(&self, wsl_info: &WslInfo) -> DockerInfo {
        detect_docker(wsl_info)
    }
}

/// Continuous monitoring engine that tracks a target adapter LUID and produces normalized traffic samples.
pub struct WslTrafficMonitor<P: NetworkProvider = SystemNetworkProvider> {
    provider: P,
    monitored_luid: Option<u64>,
    last_counters: Option<wsl_traffic_windows::RawInterfaceCounters>,
    last_time: Option<std::time::Instant>,
    state: MonitorState,
    networking_mode: String,
    confidence: MeasurementConfidence,
    last_error: Option<String>,
    ticks_since_reclassify: usize,
    reclassify_interval_ticks: usize,
}

impl WslTrafficMonitor<SystemNetworkProvider> {
    /// Create a new monitor using the host OS network provider.
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_provider(SystemNetworkProvider)
    }

    /// Create a new monitor with a pre-selected adapter LUID.
    #[must_use]
    pub fn new_with_luid(luid: u64) -> Self {
        Self {
            provider: SystemNetworkProvider,
            monitored_luid: Some(luid),
            last_counters: None,
            last_time: None,
            state: MonitorState::Active,
            networking_mode: "nat".to_string(),
            confidence: MeasurementConfidence::High,
            last_error: None,
            ticks_since_reclassify: 0,
            reclassify_interval_ticks: 10,
        }
    }
}

impl Default for WslTrafficMonitor<SystemNetworkProvider> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: NetworkProvider> WslTrafficMonitor<P> {
    /// Create a new monitor with a custom provider (e.g. for mock testing).
    pub fn new_with_provider(provider: P) -> Self {
        Self {
            provider,
            monitored_luid: None,
            last_counters: None,
            last_time: None,
            state: MonitorState::Disconnected,
            networking_mode: "nat".to_string(),
            confidence: MeasurementConfidence::Unsupported,
            last_error: None,
            ticks_since_reclassify: 0,
            reclassify_interval_ticks: 10,
        }
    }

    /// Retrieve the current monitored interface LUID, if any.
    #[must_use]
    pub fn monitored_luid(&self) -> Option<u64> {
        self.monitored_luid
    }

    /// Retrieve the current monitor status state.
    #[must_use]
    pub fn state(&self) -> MonitorState {
        self.state
    }

    /// Retrieve the current measurement confidence level.
    #[must_use]
    pub fn confidence(&self) -> MeasurementConfidence {
        self.confidence
    }

    /// Retrieve the last error message, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    /// Set the number of ticks between automatic reclassification checks.
    pub fn set_reclassify_interval_ticks(&mut self, ticks: usize) {
        self.reclassify_interval_ticks = ticks;
    }

    /// Perform a single tick check, using the current system monotonic clock.
    pub fn tick(&mut self) -> TrafficSample {
        self.tick_at(std::time::Instant::now())
    }

    /// Perform a single tick check, passing an explicit monotonic time (crucial for time-delta tests).
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    pub fn tick_at(&mut self, now: std::time::Instant) -> TrafficSample {
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "Unknown".to_string());

        // Increment ticks since reclassification
        self.ticks_since_reclassify += 1;

        // Check if we need to detect or redetect the target interface, or if reclassification is due
        let need_reclassify = self.monitored_luid.is_none()
            || self.state == MonitorState::Disconnected
            || self.ticks_since_reclassify >= self.reclassify_interval_ticks;

        if need_reclassify {
            self.ticks_since_reclassify = 0;
            if let Some(luid) = self.redetect(now) {
                if Some(luid) != self.monitored_luid {
                    // We switched to a new adapter!
                    self.monitored_luid = Some(luid);
                    self.state = MonitorState::Active;
                    // First tick on new adapter reports 0.0 to establish baseline
                    return TrafficSample {
                        upload_bytes_per_sec: 0.0,
                        download_bytes_per_sec: 0.0,
                        timestamp,
                    };
                }
            } else {
                // No supported interface found
                if self.networking_mode == "mirrored"
                    || self.networking_mode == "none"
                    || self.networking_mode == "virtioproxy"
                {
                    self.state = MonitorState::UnsupportedNetworkingMode;
                } else {
                    self.state = MonitorState::Disconnected;
                }
                self.monitored_luid = None;
                self.last_counters = None;
                self.last_time = None;

                return TrafficSample {
                    upload_bytes_per_sec: 0.0,
                    download_bytes_per_sec: 0.0,
                    timestamp,
                };
            }
        }

        let Some(luid) = self.monitored_luid else {
            return TrafficSample {
                upload_bytes_per_sec: 0.0,
                download_bytes_per_sec: 0.0,
                timestamp,
            };
        };

        let Ok(new_counters) = self.provider.get_interface_counters(luid) else {
            // Interface became unreachable (WSL shutdown or adapter destruction)
            self.state = MonitorState::Disconnected;
            self.monitored_luid = None;
            self.last_counters = None;
            self.last_time = None;
            self.last_error = Some(format!(
                "Failed to query interface counters for LUID {luid}"
            ));

            return TrafficSample {
                upload_bytes_per_sec: 0.0,
                download_bytes_per_sec: 0.0,
                timestamp,
            };
        };
        self.last_error = None;

        let Some(last_counters) = self.last_counters else {
            // First sample establishing the baseline
            self.last_counters = Some(new_counters);
            self.last_time = Some(now);
            return TrafficSample {
                upload_bytes_per_sec: 0.0,
                download_bytes_per_sec: 0.0,
                timestamp,
            };
        };

        let last_time = self.last_time.unwrap_or(now);
        let duration = now.duration_since(last_time);
        let duration_secs = duration.as_secs_f64();

        // 1. Detect sleep/resume or extremely long delay (> 5 seconds)
        if duration_secs > 5.0 {
            self.last_counters = Some(new_counters);
            self.last_time = Some(now);
            return TrafficSample {
                upload_bytes_per_sec: 0.0,
                download_bytes_per_sec: 0.0,
                timestamp,
            };
        }

        // 2. Detect counter reset (negative deltas)
        if new_counters.bytes_sent < last_counters.bytes_sent
            || new_counters.bytes_recv < last_counters.bytes_recv
        {
            self.last_counters = Some(new_counters);
            self.last_time = Some(now);
            return TrafficSample {
                upload_bytes_per_sec: 0.0,
                download_bytes_per_sec: 0.0,
                timestamp,
            };
        }

        // 3. Compute rates
        let bytes_sent_delta = new_counters.bytes_sent - last_counters.bytes_sent;
        let bytes_recv_delta = new_counters.bytes_recv - last_counters.bytes_recv;

        let (upload_rate, download_rate) = if duration_secs > 0.0 {
            let sent_rate = bytes_sent_delta as f64 / duration_secs;
            let recv_rate = bytes_recv_delta as f64 / duration_secs;

            if self.networking_mode == "mirrored" {
                // Mirrored mode: sharing host physical interface
                // Host sent = Upload, Host recv = Download
                (sent_rate, recv_rate)
            } else {
                // NAT mode: virtual interface (inverted directions)
                // Host sent = Download, Host recv = Upload
                (recv_rate, sent_rate)
            }
        } else {
            (0.0, 0.0)
        };

        self.last_counters = Some(new_counters);
        self.last_time = Some(now);

        TrafficSample {
            upload_bytes_per_sec: upload_rate,
            download_bytes_per_sec: download_rate,
            timestamp,
        }
    }

    #[allow(clippy::if_not_else)]
    fn redetect(&mut self, now: std::time::Instant) -> Option<u64> {
        let wsl_info = self.provider.detect_wsl();
        if !wsl_info.is_installed {
            self.confidence = MeasurementConfidence::Unsupported;
            self.last_error = Some("WSL is not installed".to_string());
            return None;
        }
        self.networking_mode.clone_from(&wsl_info.networking_mode);

        if self.networking_mode == "mirrored"
            || self.networking_mode == "none"
            || self.networking_mode == "virtioproxy"
        {
            self.confidence = MeasurementConfidence::Unsupported;
            self.last_error = Some(format!(
                "Networking mode '{}' is unsupported. Only 'nat' is fully supported for host-side tracking.",
                self.networking_mode
            ));
            return None;
        }
        let docker_info = self.provider.detect_docker(&wsl_info);

        match self.provider.get_adapters() {
            Ok(adapters) => {
                let classifications = crate::classify_adapters(&adapters, &wsl_info, &docker_info);
                if let Some(best) = classifications.first() {
                    if best.confidence != MeasurementConfidence::Unsupported {
                        let luid = best.adapter_luid;
                        match self.provider.get_interface_counters(luid) {
                            Ok(counters) => {
                                if Some(luid) != self.monitored_luid || self.last_counters.is_none()
                                {
                                    self.last_counters = Some(counters);
                                    self.last_time = Some(now);
                                }
                                self.confidence = best.confidence;
                                self.last_error = None;
                                return Some(luid);
                            }
                            Err(e) => {
                                self.confidence = best.confidence;
                                self.last_error = Some(format!(
                                    "Failed to get counters for redetected LUID {luid}: {e}"
                                ));
                            }
                        }
                    } else {
                        self.confidence = MeasurementConfidence::Unsupported;
                        self.last_error = Some(
                            "No supported network adapters identified for monitoring".to_string(),
                        );
                    }
                } else {
                    self.confidence = MeasurementConfidence::Unsupported;
                    self.last_error = Some("No network adapters found".to_string());
                }
            }
            Err(e) => {
                self.confidence = MeasurementConfidence::Unsupported;
                self.last_error = Some(format!("Failed to enumerate network adapters: {e}"));
            }
        }
        None
    }

    /// Spin up a background thread that periodically polls the sampler.
    ///
    /// The returned `ActiveMonitor` acts as a guard; dropping it halts the polling thread.
    pub fn start_background<F>(
        mut self,
        interval: std::time::Duration,
        mut callback: F,
    ) -> ActiveMonitor
    where
        F: FnMut(TrafficSample) + Send + 'static,
    {
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            loop {
                if stop_rx.recv_timeout(interval).is_ok() {
                    break;
                }
                let sample = self.tick();
                callback(sample);
            }
        });

        ActiveMonitor {
            stop_tx,
            thread_handle: Some(handle),
        }
    }
}

/// A handle to a running background monitor. HALTs the thread when dropped.
pub struct ActiveMonitor {
    stop_tx: std::sync::mpsc::Sender<()>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl ActiveMonitor {
    /// Signal the background thread to stop, and block until it joins.
    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ActiveMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A thread-safe, queryable snapshot of the active monitoring status and rates.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorStateSnapshot {
    /// Current upload speed in bytes per second.
    pub upload_speed: f64,
    /// Current download speed in bytes per second.
    pub download_speed: f64,
    /// Current status (Active or Disconnected).
    pub status: MonitorState,
    /// Current measurement confidence.
    pub confidence: MeasurementConfidence,
    /// The last error message, if in an error/disconnected state.
    pub error_state: Option<String>,
}

/// A thread-safe production monitor service that schedules background polling and exposes status/rates.
pub struct WslTrafficMonitorService<P: NetworkProvider = SystemNetworkProvider> {
    monitor: std::sync::Arc<std::sync::Mutex<WslTrafficMonitor<P>>>,
    state: std::sync::Arc<std::sync::RwLock<MonitorStateSnapshot>>,
    active_monitor: Option<ActiveMonitor>,
}

impl WslTrafficMonitorService<SystemNetworkProvider> {
    /// Create a new monitoring service with the host OS network provider.
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_provider(SystemNetworkProvider)
    }
}

impl Default for WslTrafficMonitorService<SystemNetworkProvider> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::missing_panics_doc)]
impl<P: NetworkProvider> WslTrafficMonitorService<P> {
    /// Create a new monitoring service with a custom network provider (e.g. for mock testing).
    pub fn new_with_provider(provider: P) -> Self {
        let initial_snapshot = MonitorStateSnapshot {
            upload_speed: 0.0,
            download_speed: 0.0,
            status: MonitorState::Disconnected,
            confidence: MeasurementConfidence::Unsupported,
            error_state: None,
        };

        Self {
            monitor: std::sync::Arc::new(std::sync::Mutex::new(
                WslTrafficMonitor::new_with_provider(provider),
            )),
            state: std::sync::Arc::new(std::sync::RwLock::new(initial_snapshot)),
            active_monitor: None,
        }
    }

    /// Start background polling at the specified interval.
    ///
    /// # Errors
    /// Returns an error if the monitor service is already running.
    pub fn start(&mut self, interval: std::time::Duration) -> Result<(), MonitorError> {
        if self.active_monitor.is_some() {
            return Err(MonitorError::AlreadyRunning);
        }

        let monitor = self.monitor.clone();
        let state = self.state.clone();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();

        let handle = std::thread::spawn(move || {
            loop {
                if stop_rx.recv_timeout(interval).is_ok() {
                    break;
                }
                let mut m = monitor.lock().unwrap();
                let sample = m.tick();
                let status = m.state();
                let confidence = m.confidence();
                let error_state = m.last_error();

                if let Ok(mut s) = state.write() {
                    *s = MonitorStateSnapshot {
                        upload_speed: sample.upload_bytes_per_sec,
                        download_speed: sample.download_bytes_per_sec,
                        status,
                        confidence,
                        error_state,
                    };
                }

                // Record usage history
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let up_bytes = (sample.upload_bytes_per_sec * interval.as_secs_f64()) as u64;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let down_bytes = (sample.download_bytes_per_sec * interval.as_secs_f64()) as u64;
                let _ = wsl_traffic_storage::record_usage(up_bytes, down_bytes);
            }
            // Flush any remaining history when loop exits
            let _ = wsl_traffic_storage::flush_history();
        });

        self.active_monitor = Some(ActiveMonitor {
            stop_tx,
            thread_handle: Some(handle),
        });

        Ok(())
    }

    /// Stop background polling.
    pub fn stop(&mut self) {
        if let Some(mut active) = self.active_monitor.take() {
            active.stop();
        }
        // Ensure data is written on stop
        let _ = wsl_traffic_storage::flush_history();
    }

    /// Retrieve the current snapshot of the monitoring metrics and status.
    #[must_use]
    pub fn get_snapshot(&self) -> MonitorStateSnapshot {
        self.state.read().unwrap().clone()
    }

    /// Retrieve the current status of the service (whether background polling is active).
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.active_monitor.is_some()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use wsl_traffic_core::{AdapterInfo, WslDistroInfo};

    #[test]
    fn test_classify_adapters_nat_high() {
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapters = vec![AdapterInfo {
            luid: 1,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 0,
            bytes_recv: 0,
            packets_sent: 0,
            packets_recv: 0,
        }];
        let classified = classify_adapters(&adapters, &wsl_info, &docker_info);
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].confidence, MeasurementConfidence::High);
        assert_eq!(classified[0].score, 90);
    }

    #[test]
    fn test_classify_adapters_docker_interference() {
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![WslDistroInfo {
                name: "docker-desktop".to_string(),
                version: 2,
                is_running: true,
            }],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = detect_docker(&wsl_info);
        let adapters = vec![AdapterInfo {
            luid: 1,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 0,
            bytes_recv: 0,
            packets_sent: 0,
            packets_recv: 0,
        }];
        let classified = classify_adapters(&adapters, &wsl_info, &docker_info);
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].confidence, MeasurementConfidence::Medium);
        assert!(classified[0].explanation.contains("Docker"));
    }

    #[test]
    fn test_classify_adapters_no_wsl_name_but_wsl_ip() {
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapters = vec![AdapterInfo {
            luid: 1,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (Default Switch)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec!["172.25.80.1".to_string()],
            ipv6_addresses: vec![],
            bytes_sent: 0,
            bytes_recv: 0,
            packets_sent: 0,
            packets_recv: 0,
        }];
        let classified = classify_adapters(&adapters, &wsl_info, &docker_info);
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].confidence, MeasurementConfidence::High);
        assert_eq!(classified[0].score, 65);
    }

    // --- SAMPLER & RESET TESTS ---

    struct MockNetworkProvider {
        adapters: std::sync::Mutex<Vec<AdapterInfo>>,
        counters: std::sync::Mutex<
            std::collections::HashMap<u64, wsl_traffic_windows::RawInterfaceCounters>,
        >,
        wsl_info: std::sync::Mutex<WslInfo>,
        docker_info: std::sync::Mutex<DockerInfo>,
    }

    impl NetworkProvider for MockNetworkProvider {
        fn get_adapters(&self) -> Result<Vec<AdapterInfo>, MonitorError> {
            Ok(self.adapters.lock().unwrap().clone())
        }

        fn get_interface_counters(
            &self,
            luid: u64,
        ) -> Result<wsl_traffic_windows::RawInterfaceCounters, MonitorError> {
            let counters_map = self.counters.lock().unwrap();
            if let Some(c) = counters_map.get(&luid) {
                Ok(*c)
            } else {
                Err(MonitorError::Windows(
                    wsl_traffic_windows::WindowsError::GetIfEntry2Failed { luid, code: 2 },
                ))
            }
        }

        fn detect_wsl(&self) -> WslInfo {
            self.wsl_info.lock().unwrap().clone()
        }

        fn detect_docker(&self, _wsl_info: &WslInfo) -> DockerInfo {
            self.docker_info.lock().unwrap().clone()
        }
    }

    #[test]
    fn test_monitor_normal_tick_nat_mode() {
        let base_instant = std::time::Instant::now();
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapter = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let mut counters_map = std::collections::HashMap::new();
        counters_map.insert(
            100,
            wsl_traffic_windows::RawInterfaceCounters {
                bytes_sent: 1000,
                bytes_recv: 2000,
                packets_sent: 0,
                packets_recv: 0,
            },
        );

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter]),
            counters: std::sync::Mutex::new(counters_map),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut monitor = WslTrafficMonitor::new_with_provider(provider);

        // First tick: establishing baseline
        let sample1 = monitor.tick_at(base_instant);
        assert_eq!(sample1.upload_bytes_per_sec, 0.0);
        assert_eq!(sample1.download_bytes_per_sec, 0.0);
        assert_eq!(monitor.state(), MonitorState::Active);
        assert_eq!(monitor.monitored_luid(), Some(100));

        // Update mock counters: 1 second elapsed, bytes_sent +5000, bytes_recv +10000
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                100,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 6000,  // OutOctets = Download in NAT mode
                    bytes_recv: 12000, // InOctets = Upload in NAT mode
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        // Second tick: 1 second later
        let sample2 = monitor.tick_at(base_instant + std::time::Duration::from_secs(1));
        // NAT mode: OutOctets (sent) = download, InOctets (recv) = upload
        assert_eq!(sample2.download_bytes_per_sec, 5000.0);
        assert_eq!(sample2.upload_bytes_per_sec, 10000.0);
    }

    #[test]
    fn test_monitor_unsupported_networking_mode() {
        let base_instant = std::time::Instant::now();
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "mirrored".to_string(), // unsupported mode
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapter = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "Wi-Fi".to_string(),
            description: "Intel Wi-Fi".to_string(),
            if_type: 71,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter]),
            counters: std::sync::Mutex::new(std::collections::HashMap::new()),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut monitor = WslTrafficMonitor::new_with_provider(provider);

        // Tick should fail to select the adapter and set state to UnsupportedNetworkingMode
        let sample = monitor.tick_at(base_instant);
        assert_eq!(sample.upload_bytes_per_sec, 0.0);
        assert_eq!(sample.download_bytes_per_sec, 0.0);
        assert_eq!(monitor.state(), MonitorState::UnsupportedNetworkingMode);
        assert_eq!(monitor.monitored_luid(), None);
    }

    #[test]
    fn test_monitor_counter_reset() {
        let base_instant = std::time::Instant::now();
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapter = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let mut counters_map = std::collections::HashMap::new();
        counters_map.insert(
            100,
            wsl_traffic_windows::RawInterfaceCounters {
                bytes_sent: 1000,
                bytes_recv: 2000,
                packets_sent: 0,
                packets_recv: 0,
            },
        );

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter]),
            counters: std::sync::Mutex::new(counters_map),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut monitor = WslTrafficMonitor::new_with_provider(provider);

        // Establish baseline
        let _ = monitor.tick_at(base_instant);

        // Counter reset happens: values drop below baseline
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                100,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 500,  // < 1000
                    bytes_recv: 1000, // < 2000
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        // Tick after reset
        let sample = monitor.tick_at(base_instant + std::time::Duration::from_secs(1));
        // Speed must be reported as 0.0, NOT negative or a huge spike
        assert_eq!(sample.download_bytes_per_sec, 0.0);
        assert_eq!(sample.upload_bytes_per_sec, 0.0);

        // Verify that the baseline was updated to the new lower values
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                100,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 1500, // diff = +1000
                    bytes_recv: 3000, // diff = +2000
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        let sample2 = monitor.tick_at(base_instant + std::time::Duration::from_secs(2));
        assert_eq!(sample2.download_bytes_per_sec, 1000.0);
        assert_eq!(sample2.upload_bytes_per_sec, 2000.0);
    }

    #[test]
    fn test_monitor_sleep_resume() {
        let base_instant = std::time::Instant::now();
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapter = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let mut counters_map = std::collections::HashMap::new();
        counters_map.insert(
            100,
            wsl_traffic_windows::RawInterfaceCounters {
                bytes_sent: 1000,
                bytes_recv: 2000,
                packets_sent: 0,
                packets_recv: 0,
            },
        );

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter]),
            counters: std::sync::Mutex::new(counters_map),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut monitor = WslTrafficMonitor::new_with_provider(provider);

        // Establish baseline
        let _ = monitor.tick_at(base_instant);

        // System goes to sleep and wakes up 10 seconds later
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                100,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 50000,
                    bytes_recv: 80000,
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        let sample = monitor.tick_at(base_instant + std::time::Duration::from_secs(10));
        // Due to sleep/resume interval > 5.0 seconds, we reset baseline and report 0.0
        assert_eq!(sample.download_bytes_per_sec, 0.0);
        assert_eq!(sample.upload_bytes_per_sec, 0.0);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_monitor_wsl_shutdown_and_restart() {
        let base_instant = std::time::Instant::now();
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapter = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let mut counters_map = std::collections::HashMap::new();
        counters_map.insert(
            100,
            wsl_traffic_windows::RawInterfaceCounters {
                bytes_sent: 1000,
                bytes_recv: 2000,
                packets_sent: 0,
                packets_recv: 0,
            },
        );

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter]),
            counters: std::sync::Mutex::new(counters_map),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut monitor = WslTrafficMonitor::new_with_provider(provider);

        // Establish baseline
        let _ = monitor.tick_at(base_instant);

        // WSL shutdown: adapter counters query fails
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.remove(&100);
        }

        let sample1 = monitor.tick_at(base_instant + std::time::Duration::from_secs(1));
        assert_eq!(monitor.state(), MonitorState::Disconnected);
        assert_eq!(sample1.download_bytes_per_sec, 0.0);

        // WSL restart: new adapter is created with different LUID (200) and starts with fresh counters
        let new_adapter = AdapterInfo {
            luid: 200,
            if_index: 4,
            guid: "new_guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 50,
            bytes_recv: 80,
            packets_sent: 0,
            packets_recv: 0,
        };

        // Update provider with new state
        {
            let mut a = monitor.provider.adapters.lock().unwrap();
            *a = vec![new_adapter];
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                200,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 50,
                    bytes_recv: 80,
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        // Next tick: will perform redetection, find new adapter, set baseline and return zero rates
        let sample2 = monitor.tick_at(base_instant + std::time::Duration::from_secs(2));
        assert_eq!(monitor.state(), MonitorState::Active);
        assert_eq!(monitor.monitored_luid(), Some(200));
        assert_eq!(sample2.download_bytes_per_sec, 0.0);

        // Subsequent tick: normal delta calculation on the new adapter
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                200,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 1050, // diff = 1000
                    bytes_recv: 2080, // diff = 2000
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        let sample3 = monitor.tick_at(base_instant + std::time::Duration::from_secs(3));
        assert_eq!(sample3.download_bytes_per_sec, 1000.0);
        assert_eq!(sample3.upload_bytes_per_sec, 2000.0);
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_monitor_service_lifecycle() {
        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!("test_history_{}.redb", std::process::id()));
        if test_db_path.exists() {
            let _ = std::fs::remove_file(&test_db_path);
        }
        unsafe {
            std::env::set_var("WSL_TRAFFIC_HISTORY_DB", &test_db_path);
        }

        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };
        let adapter = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let mut counters_map = std::collections::HashMap::new();
        counters_map.insert(
            100,
            wsl_traffic_windows::RawInterfaceCounters {
                bytes_sent: 1000,
                bytes_recv: 2000,
                packets_sent: 0,
                packets_recv: 0,
            },
        );

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter]),
            counters: std::sync::Mutex::new(counters_map),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut service = WslTrafficMonitorService::new_with_provider(provider);
        assert!(!service.is_running());

        // Check initial state
        let snapshot = service.get_snapshot();
        assert_eq!(snapshot.upload_speed, 0.0);
        assert_eq!(snapshot.download_speed, 0.0);
        assert_eq!(snapshot.status, MonitorState::Disconnected);

        // Start service
        let res = service.start(std::time::Duration::from_millis(50));
        assert!(res.is_ok());
        assert!(service.is_running());

        // Try to start again - should fail with MonitorError::AlreadyRunning
        let res_twice = service.start(std::time::Duration::from_millis(50));
        assert!(matches!(res_twice, Err(MonitorError::AlreadyRunning)));

        // Sleep to let background thread tick
        std::thread::sleep(std::time::Duration::from_millis(150));

        // After ticking, it should have performed discovery, set baseline, and transition to Active
        let snapshot2 = service.get_snapshot();
        assert_eq!(snapshot2.status, MonitorState::Active);
        assert_eq!(snapshot2.confidence, MeasurementConfidence::High);
        assert!(snapshot2.error_state.is_none());

        // Stop the service (which flushes history)
        let _ = wsl_traffic_storage::record_usage(1000, 2000);
        service.stop();
        assert!(!service.is_running());

        // Verify history was written!
        let hourly = wsl_traffic_storage::get_hourly_history(10).unwrap();
        assert!(!hourly.is_empty());
        let (key, up, down) = &hourly[0];
        assert!(key.starts_with("hourly:"));
        assert_eq!(*up, 1000);
        assert_eq!(*down, 2000);

        unsafe {
            std::env::remove_var("WSL_TRAFFIC_HISTORY_DB");
        }
        if test_db_path.exists() {
            let _ = std::fs::remove_file(&test_db_path);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_monitor_automatic_reclassification() {
        let base_instant = std::time::Instant::now();
        let wsl_info = WslInfo {
            is_installed: true,
            wsl_version: Some("2.0.0".to_string()),
            kernel_version: None,
            distributions: vec![],
            networking_mode: "nat".to_string(),
            wslconfig_parsed: None,
        };
        let docker_info = DockerInfo {
            is_installed: false,
            has_docker_desktop_distro: false,
            has_docker_desktop_data_distro: false,
            running_processes: vec![],
        };

        // Initially we have adapter 100
        let adapter100 = AdapterInfo {
            luid: 100,
            if_index: 1,
            guid: "guid100".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac100".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 1000,
            bytes_recv: 2000,
            packets_sent: 0,
            packets_recv: 0,
        };

        let mut counters_map = std::collections::HashMap::new();
        counters_map.insert(
            100,
            wsl_traffic_windows::RawInterfaceCounters {
                bytes_sent: 1000,
                bytes_recv: 2000,
                packets_sent: 0,
                packets_recv: 0,
            },
        );

        let provider = MockNetworkProvider {
            adapters: std::sync::Mutex::new(vec![adapter100]),
            counters: std::sync::Mutex::new(counters_map),
            wsl_info: std::sync::Mutex::new(wsl_info),
            docker_info: std::sync::Mutex::new(docker_info),
        };

        let mut monitor = WslTrafficMonitor::new_with_provider(provider);
        // Set reclassify interval to 3 ticks
        monitor.set_reclassify_interval_ticks(3);

        // Tick 1: trigger reclassification (interval starts at 0, tick increments it to 1.
        // Since monitored_luid is None, it runs redetect immediately!
        // This binds to adapter 100, establishes baseline, and returns 0.0 rates.
        let sample1 = monitor.tick_at(base_instant);
        assert_eq!(monitor.monitored_luid(), Some(100));
        assert_eq!(sample1.download_bytes_per_sec, 0.0);

        // Tick 2: ticks_since_reclassify becomes 1. Since 1 < 3, reclassification does not trigger.
        // It performs a normal query on adapter 100, preserving and calculating delta rates.
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                100,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 2000, // +1000 OutOctets (download in NAT)
                    bytes_recv: 4000, // +2000 InOctets (upload in NAT)
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }
        let sample2 = monitor.tick_at(base_instant + std::time::Duration::from_secs(1));
        // Verify rate calculation is preserved and calculated correctly!
        assert_eq!(sample2.download_bytes_per_sec, 1000.0);
        assert_eq!(sample2.upload_bytes_per_sec, 2000.0);

        // Now change topology: adapter 200 becomes the active adapter
        let adapter200 = AdapterInfo {
            luid: 200,
            if_index: 2,
            guid: "guid200".to_string(),
            alias: "alias".to_string(),
            friendly_name: "vEthernet (WSL)".to_string(),
            description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            if_type: 6,
            oper_status: 1,
            mac_address: "mac200".to_string(),
            mtu: 1500,
            link_speed: 100_000,
            ipv4_addresses: vec![],
            ipv6_addresses: vec![],
            bytes_sent: 5000,
            bytes_recv: 6000,
            packets_sent: 0,
            packets_recv: 0,
        };
        {
            let mut a = monitor.provider.adapters.lock().unwrap();
            *a = vec![adapter200];
            let mut c = monitor.provider.counters.lock().unwrap();
            c.clear();
            c.insert(
                200,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 5000,
                    bytes_recv: 6000,
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }

        // Tick 3: ticks_since_reclassify becomes 1. Since 1 < 2, it does NOT trigger reclassify.
        // It tries to query the old LUID 100.
        // But adapter 100 is gone, so the query on 100 fails!
        // This transitions state to Disconnected.
        let sample3 = monitor.tick_at(base_instant + std::time::Duration::from_secs(2));
        assert_eq!(monitor.state(), MonitorState::Disconnected);
        assert_eq!(sample3.download_bytes_per_sec, 0.0);

        // Tick 4: monitor is Disconnected, so reclassification / redetect triggers immediately!
        // It finds adapter 200, binds to it, sets baseline, and returns 0.0.
        let sample4 = monitor.tick_at(base_instant + std::time::Duration::from_secs(3));
        assert_eq!(monitor.state(), MonitorState::Active);
        assert_eq!(monitor.monitored_luid(), Some(200));
        assert_eq!(sample4.download_bytes_per_sec, 0.0);

        // Tick 5: ticks_since_reclassify becomes 1. Since 1 < 2, it does not reclassify.
        // Normal delta calculations on 200.
        {
            let mut c = monitor.provider.counters.lock().unwrap();
            c.insert(
                200,
                wsl_traffic_windows::RawInterfaceCounters {
                    bytes_sent: 6000, // +1000
                    bytes_recv: 8000, // +2000
                    packets_sent: 0,
                    packets_recv: 0,
                },
            );
        }
        let sample5 = monitor.tick_at(base_instant + std::time::Duration::from_secs(4));
        assert_eq!(sample5.download_bytes_per_sec, 1000.0);
        assert_eq!(sample5.upload_bytes_per_sec, 2000.0);
    }
}
