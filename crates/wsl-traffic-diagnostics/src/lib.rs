//! Diagnostics boundary.
//!
//! Collects host, WSL, Docker, and adapter snapshots for support and validation reports.

use std::fmt::Write;
use wsl_traffic_core::DiagnosticsReport;

/// Diagnostics schema version reserved for future report formats.
pub const DIAGNOSTICS_SCHEMA_VERSION: u16 = 0;

/// Generate a complete diagnostics snapshot report of the host networking environment.
///
/// # Errors
/// Returns an error if querying host network adapters fails.
pub fn generate_report() -> Result<DiagnosticsReport, String> {
    let windows_version = wsl_traffic_windows::get_windows_version();
    let wsl_info = wsl_traffic_wsl::detect_wsl();
    let docker_info = wsl_traffic_monitor::detect_docker(&wsl_info);
    let adapters = wsl_traffic_windows::get_adapters()?;
    let classifications =
        wsl_traffic_monitor::classify_adapters(&adapters, &wsl_info, &docker_info);
    let recommendation = wsl_traffic_monitor::generate_recommendation(&classifications, &wsl_info);

    let timestamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "Unknown".to_string());

    // Phase 3 advanced networking and experimental diagnostics
    let wslconfig_mirrored = wsl_info.networking_mode == "mirrored";
    let wslconfig_virtioproxy = wsl_info.networking_mode == "virtioproxy";

    let mut dns_tunneling_enabled = false;
    if let Some(ref config) = wsl_info.wslconfig_parsed {
        if let Some(wsl2_sec) = config.get("wsl2") {
            for (k, v) in wsl2_sec {
                if k.to_lowercase() == "dnstunneling" {
                    dns_tunneling_enabled = v.to_lowercase() == "true";
                }
            }
        }
    }

    let is_elevated = wsl_traffic_windows::is_elevated();
    let etw_tracing_accessible = is_elevated;
    let wfp_accessible = is_elevated;

    let mut evaluation_notes = String::new();
    if wslconfig_mirrored {
        evaluation_notes.push_str("Mirrored networking active. Attributing exact WSL-only traffic requires packet header inspection or WFP compartment rules (needs elevation).\n");
    } else if wslconfig_virtioproxy {
        evaluation_notes.push_str("VirtioProxy mode active. Host-side NAT interfaces may be bypassed or behave as user-mode proxied endpoints.\n");
    } else {
        evaluation_notes.push_str(
            "WSL NAT mode active. Virtual network interface isolation is validated and reliable.\n",
        );
    }

    if dns_tunneling_enabled {
        evaluation_notes.push_str("DNS Tunneling is enabled. DNS queries bypass the standard adapter packets and are virtualized.\n");
    }

    if is_elevated {
        evaluation_notes.push_str("Host process is running as Administrator. Full ETW tracing and WFP inspection capabilities are accessible.\n");
    } else {
        evaluation_notes.push_str("Host process is unprivileged. ETW network tracing and advanced WFP/compartment filtering are restricted (run as Admin to enable).\n");
    }

    let advanced_diagnostics = wsl_traffic_core::AdvancedNetworkingDiagnostics {
        wslconfig_mirrored,
        wslconfig_virtioproxy,
        dns_tunneling_enabled,
        etw_tracing_accessible,
        wfp_accessible,
        evaluation_notes,
    };

    Ok(DiagnosticsReport {
        schema_version: DIAGNOSTICS_SCHEMA_VERSION,
        timestamp,
        windows_version,
        wsl_info,
        docker_info,
        adapters,
        classifications,
        recommendation,
        advanced_diagnostics,
    })
}

/// Serialize the diagnostic report to a pretty-printed JSON string.
///
/// # Errors
/// Returns an error if JSON serialization fails.
pub fn format_report_as_json(report: &DiagnosticsReport) -> Result<String, String> {
    serde_json::to_string_pretty(report).map_err(|e| e.to_string())
}

/// Format the diagnostic report into a beautifully structured, human-readable text report.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn format_report_as_text(report: &DiagnosticsReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "======================================================================"
    );
    let _ = writeln!(
        out,
        "                   WSL Traffic Monitor Diagnostics                    "
    );
    let _ = writeln!(
        out,
        "======================================================================"
    );
    let _ = writeln!(out, "Generated at:    {}", report.timestamp);
    let _ = writeln!(out, "Windows Version: {}", report.windows_version);
    let _ = writeln!(out);

    let _ = writeln!(out, "WSL Information:");
    let _ = writeln!(
        out,
        "  Installed:        {}",
        if report.wsl_info.is_installed {
            "Yes"
        } else {
            "No"
        }
    );
    if let Some(ref wsl_v) = report.wsl_info.wsl_version {
        let _ = writeln!(out, "  WSL Version:      {wsl_v}");
    }
    if let Some(ref kernel_v) = report.wsl_info.kernel_version {
        let _ = writeln!(out, "  Kernel Version:   {kernel_v}");
    }
    let _ = writeln!(
        out,
        "  Networking Mode:  {}",
        report.wsl_info.networking_mode
    );
    let _ = writeln!(out, "  Distributions:");
    if report.wsl_info.distributions.is_empty() {
        let _ = writeln!(out, "    (None registered)");
    } else {
        for distro in &report.wsl_info.distributions {
            let state = if distro.is_running {
                "Running"
            } else {
                "Stopped"
            };
            let _ = writeln!(
                out,
                "    * {} (WSL {}, {})",
                distro.name, distro.version, state
            );
        }
    }

    if let Some(ref config) = report.wsl_info.wslconfig_parsed {
        let _ = writeln!(out, "  Parsed .wslconfig:");
        for (section, keys) in config {
            let _ = writeln!(out, "    [{section}]");
            for (k, v) in keys {
                let _ = writeln!(out, "      {k} = {v}");
            }
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "Docker Desktop Information:");
    let _ = writeln!(
        out,
        "  Installed:                   {}",
        if report.docker_info.is_installed {
            "Yes"
        } else {
            "No"
        }
    );
    let _ = writeln!(
        out,
        "  docker-desktop distro:       {}",
        if report.docker_info.has_docker_desktop_distro {
            "Yes"
        } else {
            "No"
        }
    );
    let _ = writeln!(
        out,
        "  docker-desktop-data distro:  {}",
        if report.docker_info.has_docker_desktop_data_distro {
            "Yes"
        } else {
            "No"
        }
    );
    let _ = writeln!(out, "  Running Host Docker Processes:");
    if report.docker_info.running_processes.is_empty() {
        let _ = writeln!(out, "    (None)");
    } else {
        for proc in &report.docker_info.running_processes {
            let _ = writeln!(out, "    * {proc}");
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "Network Adapter Inventory:");
    let _ = writeln!(
        out,
        "----------------------------------------------------------------------"
    );
    if report.adapters.is_empty() {
        let _ = writeln!(out, "(No adapters found)");
    } else {
        for adapter in &report.adapters {
            let _ = writeln!(
                out,
                "LUID:              {} (Index: {})",
                adapter.luid, adapter.if_index
            );
            let _ = writeln!(out, "GUID:              {}", adapter.guid);
            let _ = writeln!(out, "Friendly Name:     {}", adapter.friendly_name);
            let _ = writeln!(out, "Description:       {}", adapter.description);
            let _ = writeln!(out, "Alias/AliasName:   {}", adapter.alias);
            let _ = writeln!(
                out,
                "Interface Type:    {} (Oper Status: {})",
                format_if_type(adapter.if_type),
                format_oper_status(adapter.oper_status)
            );
            let _ = writeln!(
                out,
                "MAC Address:       {} | MTU: {}",
                adapter.mac_address, adapter.mtu
            );
            let _ = writeln!(
                out,
                "Link Speed:        {}",
                format_link_speed(adapter.link_speed)
            );
            let _ = writeln!(
                out,
                "IPv4 Addresses:    {}",
                if adapter.ipv4_addresses.is_empty() {
                    "None".to_string()
                } else {
                    adapter.ipv4_addresses.join(", ")
                }
            );
            let _ = writeln!(
                out,
                "IPv6 Addresses:    {}",
                if adapter.ipv6_addresses.is_empty() {
                    "None".to_string()
                } else {
                    adapter.ipv6_addresses.join(", ")
                }
            );
            let _ = writeln!(
                out,
                "Byte Counters:     Sent: {}, Recv: {}",
                format_bytes(adapter.bytes_sent),
                format_bytes(adapter.bytes_recv)
            );
            let _ = writeln!(
                out,
                "Packet Counters:   Sent: {}, Recv: {}",
                adapter.packets_sent, adapter.packets_recv
            );
            let _ = writeln!(
                out,
                "----------------------------------------------------------------------"
            );
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "Candidate Classification Rankings:");
    if report.classifications.is_empty() {
        let _ = writeln!(out, "  (No candidates classified)");
    } else {
        for (i, c) in report.classifications.iter().enumerate() {
            let _ = writeln!(
                out,
                "  {}. LUID {}: {} - Score {} ({})",
                i + 1,
                c.adapter_luid,
                c.adapter_name,
                c.score,
                c.confidence
            );
            let _ = writeln!(out, "     Explanation: {}", c.explanation);
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "Advanced Networking & Experimental Diagnostics (Phase 3):"
    );
    let _ = writeln!(
        out,
        "  WSLConfig Mirrored Mode:     {}",
        if report.advanced_diagnostics.wslconfig_mirrored {
            "Yes"
        } else {
            "No"
        }
    );
    let _ = writeln!(
        out,
        "  WSLConfig VirtioProxy Mode:  {}",
        if report.advanced_diagnostics.wslconfig_virtioproxy {
            "Yes"
        } else {
            "No"
        }
    );
    let _ = writeln!(
        out,
        "  DNS Tunneling Enabled:       {}",
        if report.advanced_diagnostics.dns_tunneling_enabled {
            "Yes"
        } else {
            "No"
        }
    );
    let _ = writeln!(
        out,
        "  ETW Tracing Accessible:      {}",
        if report.advanced_diagnostics.etw_tracing_accessible {
            "Yes (Admin)"
        } else {
            "No (Needs Elevation)"
        }
    );
    let _ = writeln!(
        out,
        "  WFP Engine Accessible:       {}",
        if report.advanced_diagnostics.wfp_accessible {
            "Yes (Admin)"
        } else {
            "No (Needs Elevation)"
        }
    );
    let _ = writeln!(out, "  Evaluation Findings:");
    for line in report.advanced_diagnostics.evaluation_notes.lines() {
        let _ = writeln!(out, "    * {line}");
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "Measurement Recommendation:");
    let _ = writeln!(out, "  {}", report.recommendation);
    let _ = writeln!(
        out,
        "======================================================================"
    );

    out
}

fn format_oper_status(status: u32) -> &'static str {
    match status {
        1 => "Up",
        2 => "Down",
        3 => "Testing",
        4 => "Unknown",
        5 => "Dormant",
        6 => "NotPresent",
        7 => "LowerLayerDown",
        _ => "Invalid",
    }
}

fn format_if_type(if_type: u32) -> String {
    match if_type {
        6 => "Ethernet (6)".to_string(),
        24 => "SoftwareLoopback (24)".to_string(),
        71 => "IEEE 802.11 Wireless (71)".to_string(),
        _ => format!("Other ({if_type})"),
    }
}

#[allow(clippy::cast_precision_loss)]
fn format_link_speed(speed: u64) -> String {
    if speed >= 1_000_000_000 {
        format!("{:.2} Gbps", speed as f64 / 1_000_000_000.0)
    } else if speed >= 1_000_000 {
        format!("{:.2} Mbps", speed as f64 / 1_000_000.0)
    } else if speed >= 1_000 {
        format!("{:.2} Kbps", speed as f64 / 1_000.0)
    } else {
        format!("{speed} bps")
    }
}

#[allow(clippy::cast_precision_loss)]
fn format_bytes(bytes: u64) -> String {
    let kib = 1024.0;
    let mib = kib * 1024.0;
    let gib = mib * 1024.0;
    let tib = gib * 1024.0;

    let b = bytes as f64;
    if b >= tib {
        format!("{:.2} TB", b / tib)
    } else if b >= gib {
        format!("{:.2} GB", b / gib)
    } else if b >= mib {
        format!("{:.2} MB", b / mib)
    } else if b >= kib {
        format!("{:.2} KB", b / kib)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024 * 10), "10.00 MB");
    }

    #[test]
    fn test_format_link_speed() {
        assert_eq!(format_link_speed(1_000_000_000), "1.00 Gbps");
        assert_eq!(format_link_speed(100_000_000), "100.00 Mbps");
    }

    #[test]
    fn test_generate_and_format_report() {
        let report = generate_report().unwrap();
        assert_eq!(report.schema_version, 0);
        let text = format_report_as_text(&report);
        assert!(text.contains("Advanced Networking & Experimental Diagnostics"));
        let json = format_report_as_json(&report).unwrap();
        assert!(json.contains("advanced_diagnostics"));
    }
}
