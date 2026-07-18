//! Monitoring orchestration and adapter classification logic.
//!
//! Evaluates candidate network interfaces for WSL traffic monitoring suitability.

use wsl_traffic_core::{
    AdapterInfo, CandidateClassification, DockerInfo, MeasurementConfidence, WslInfo,
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
                    (90, conf, expl)
                } else {
                    (70, MeasurementConfidence::Medium, "Hyper-V Virtual Ethernet adapter explicitly named 'WSL' but is currently offline/down".to_string())
                }
            } else {
                (50, MeasurementConfidence::Medium, "Hyper-V Virtual Ethernet adapter, possibly used by virtual machines, but not explicitly named 'WSL'".to_string())
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

#[cfg(test)]
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
}
