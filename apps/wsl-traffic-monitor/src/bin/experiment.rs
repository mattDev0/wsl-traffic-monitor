//! Real-Time Interface Counter Logger (Phase 0).
//!
//! Polls the selected WSL virtual adapter and the host's internet-facing physical
//! adapter every second and prints raw byte deltas.
//!
//! The run is self-annotating: type a label followed by Enter at any point to stamp
//! a phase marker into the output. Each marker closes the preceding phase with a
//! byte subtotal, so figures quoted in a report can be traced back to a window of
//! the log. Without markers a captured run cannot be audited after the fact.
//!
//! Typical directionality experiment:
//! ```text
//! wsl-experiment.exe > run.log
//! > idle-baseline           (wait ~30s with nothing running)
//! > wsl-download            (run a 10 MB download inside WSL, wait for completion)
//! > idle                    (wait ~10s)
//! > wsl-upload              (send 10 MB out of WSL, wait for completion)
//! > host-download           (run the same download on the Windows host)
//! ```

use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use wsl_traffic_core::AdapterInfo;

/// Byte totals accumulated over a single labelled phase.
#[derive(Default)]
struct PhaseTotals {
    wsl_recv: u64,
    wsl_sent: u64,
    phys_recv: u64,
    phys_sent: u64,
}

fn main() {
    println!("======================================================================");
    println!("             WSL Traffic Monitor - Empirical Logger                   ");
    println!("======================================================================");

    let started_utc = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "Unknown".to_string());
    println!("Run started:      {started_utc}");

    // 1. Detect environment
    let wsl_info = wsl_traffic_wsl::detect_wsl();
    let docker_info = wsl_traffic_monitor::detect_docker(&wsl_info);
    let adapters = match wsl_traffic_windows::get_adapters() {
        Ok(a) => a,
        Err(e) => {
            println!("Error: Failed to retrieve adapters: {e}");
            return;
        }
    };

    println!("WSL Mode:         {}", wsl_info.networking_mode);
    println!(
        "Docker Desktop:   {}",
        if docker_info.is_installed {
            "Installed"
        } else {
            "Not Present"
        }
    );

    // 2. Classify and select adapters
    let classifications =
        wsl_traffic_monitor::classify_adapters(&adapters, &wsl_info, &docker_info);
    let best_candidate = classifications.iter().max_by_key(|c| c.score);

    let wsl_luid = match best_candidate {
        Some(c) if c.score > 0 => {
            println!(
                "Target WSL NIC:   {} (LUID: {})",
                c.adapter_name, c.adapter_luid
            );
            c.adapter_luid
        }
        _ => {
            println!("Error: No suitable WSL interface detected!");
            return;
        }
    };

    let phys_luid = report_physical_selection(&adapters, wsl_luid);

    println!("\nType a label + Enter at any time to stamp a phase marker. Ctrl+C to terminate.");
    println!(
        "--------------------------------------------------------------------------------------------------"
    );
    println!("Time (s) | WSL Recv Delta | WSL Sent Delta | Phys Recv Delta | Phys Sent Delta");
    println!(
        "--------------------------------------------------------------------------------------------------"
    );

    let markers = spawn_marker_reader();

    let mut last_wsl = get_counters_or_zero(wsl_luid);
    let mut last_phys = phys_luid.map(get_counters_or_zero);
    let start_time = Instant::now();

    let mut phase_label = "unlabelled".to_string();
    let mut phase_start_secs = 0u64;
    let mut phase = PhaseTotals::default();

    loop {
        thread::sleep(Duration::from_secs(1));

        let current_wsl = get_counters_or_zero(wsl_luid);
        let current_phys = phys_luid.map(get_counters_or_zero);

        let wsl_recv = current_wsl.bytes_recv.saturating_sub(last_wsl.bytes_recv);
        let wsl_sent = current_wsl.bytes_sent.saturating_sub(last_wsl.bytes_sent);

        let phys_recv = match (last_phys, current_phys) {
            (Some(lp), Some(cp)) => cp.bytes_recv.saturating_sub(lp.bytes_recv),
            _ => 0,
        };
        let phys_sent = match (last_phys, current_phys) {
            (Some(lp), Some(cp)) => cp.bytes_sent.saturating_sub(lp.bytes_sent),
            _ => 0,
        };

        let elapsed = start_time.elapsed().as_secs();

        println!("{elapsed:8} | {wsl_recv:14} | {wsl_sent:14} | {phys_recv:15} | {phys_sent:15}");

        phase.wsl_recv += wsl_recv;
        phase.wsl_sent += wsl_sent;
        phase.phys_recv += phys_recv;
        phase.phys_sent += phys_sent;

        last_wsl = current_wsl;
        last_phys = current_phys;

        // Drain any phase markers entered since the last tick.
        while let Ok(label) = markers.try_recv() {
            print_phase_summary(&phase_label, phase_start_secs, elapsed, &phase);
            phase_label = if label.is_empty() {
                "unlabelled".to_string()
            } else {
                label
            };
            phase_start_secs = elapsed;
            phase = PhaseTotals::default();
            println!("--- MARK t={elapsed}s BEGIN: {phase_label} ---");
        }
    }
}

/// Print the closing subtotal for a phase so report figures map onto a log window.
fn print_phase_summary(label: &str, from_secs: u64, to_secs: u64, totals: &PhaseTotals) {
    println!(
        "--- PHASE END: {label} (t={from_secs}s..{to_secs}s, {}s) \
         wsl_recv={} wsl_sent={} phys_recv={} phys_sent={} ---",
        to_secs.saturating_sub(from_secs),
        totals.wsl_recv,
        totals.wsl_sent,
        totals.phys_recv,
        totals.phys_sent
    );
}

/// Read phase labels from stdin on a background thread.
fn spawn_marker_reader() -> Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            match stdin.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if tx.send(line.trim().to_string()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}

/// Select the host's internet-facing adapter and log why each candidate was kept or dropped.
///
/// The previous heuristic rejected only friendly names containing "wsl" or "virtual", which
/// let `vEthernet (Default Switch)` through as the "physical" NIC. That adapter carries no
/// host internet traffic, so every measurement taken against it read zero and any conclusion
/// drawn about host/WSL isolation was unsupported. Selection is now logged so a captured run
/// can be checked rather than trusted.
fn report_physical_selection(adapters: &[AdapterInfo], wsl_luid: u64) -> Option<u64> {
    println!("\nPhysical NIC candidate evaluation:");

    let mut candidates: Vec<&AdapterInfo> = Vec::new();

    for a in adapters {
        if a.luid == wsl_luid {
            continue;
        }
        if let Some(reason) = physical_rejection_reason(a) {
            println!("  [skip] {:<40} {reason}", a.friendly_name);
        } else {
            println!(
                "  [ ok ] {:<40} ipv4={} link={} Mbps",
                a.friendly_name,
                a.ipv4_addresses.join(","),
                a.link_speed / 1_000_000
            );
            candidates.push(a);
        }
    }

    // Prefer the fastest link, then lowest LUID so repeat runs pick the same adapter.
    candidates.sort_by(|a, b| b.link_speed.cmp(&a.link_speed).then(a.luid.cmp(&b.luid)));

    if let Some(a) = candidates.first() {
        println!(
            "Physical NIC:     {} (LUID: {}) [{}]",
            a.friendly_name,
            a.luid,
            a.ipv4_addresses.join(",")
        );
        Some(a.luid)
    } else {
        println!(
            "Warning: No host internet interface identified. Physical columns will read zero \
             and MUST NOT be used to support any isolation or concurrency claim."
        );
        None
    }
}

/// Return a reason string if this adapter cannot be the host's internet-facing NIC.
fn physical_rejection_reason(a: &AdapterInfo) -> Option<&'static str> {
    if a.if_type != 6 && a.if_type != 71 {
        return Some("not ethernet/wireless");
    }
    if a.oper_status != 1 {
        return Some("not up");
    }

    let desc = a.description.to_lowercase();
    let name = a.friendly_name.to_lowercase();
    let alias = a.alias.to_lowercase();

    // Hyper-V host vNICs (vEthernet (WSL), vEthernet (Default Switch), ...) are switch
    // endpoints, not internet-facing adapters. This is the check the old filter lacked.
    if desc.contains("hyper-v virtual ethernet") {
        return Some("Hyper-V virtual switch endpoint");
    }

    for needle in [
        "vethernet",
        "wsl",
        "virtual",
        "docker",
        "vmware",
        "virtualbox",
        "loopback",
        "tap-",
        "tunnel",
        "bluetooth",
    ] {
        if name.contains(needle) || desc.contains(needle) || alias.contains(needle) {
            return Some("virtual/non-internet adapter");
        }
    }

    // An internet-facing NIC holds a routable IPv4 lease. Link-local means no DHCP.
    if a.ipv4_addresses.is_empty() {
        return Some("no IPv4 address");
    }
    if a.ipv4_addresses.iter().all(|ip| ip.starts_with("169.254.")) {
        return Some("link-local IPv4 only");
    }

    None
}

fn get_counters_or_zero(luid: u64) -> wsl_traffic_windows::RawInterfaceCounters {
    wsl_traffic_windows::get_interface_counters(luid).unwrap_or(
        wsl_traffic_windows::RawInterfaceCounters {
            bytes_sent: 0,
            bytes_recv: 0,
            packets_sent: 0,
            packets_recv: 0,
        },
    )
}
