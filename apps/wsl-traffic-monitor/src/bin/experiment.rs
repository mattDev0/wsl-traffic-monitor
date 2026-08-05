//! Real-Time Interface Counter Logger (Phase 0).
//!
//! Polls the selected WSL virtual adapter and the host's internet-facing physical
//! adapter every second and prints raw byte deltas.
//!
//! Runs are self-annotating. Each phase is stamped into the output and closed with a
//! byte subtotal, so a figure quoted in a report traces back to a named window of the
//! log rather than having to be taken on trust.
//!
//! # Automated mode (preferred)
//!
//! ```text
//! experiment.exe --auto > run.log
//! ```
//!
//! Drives the full directionality and isolation protocol without operator input:
//! transfers are launched by the tool itself, phases are marked at exact boundaries,
//! and a verdict is computed at the end. Payload sizes are exact because the
//! Cloudflare endpoints echo a requested byte count.
//!
//! # Manual mode
//!
//! ```text
//! experiment.exe > run.log
//! ```
//!
//! Type a label + Enter to stamp a marker; type `done` to finish. Only use this if
//! the automated protocol cannot reach the network.

use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use wsl_traffic_core::AdapterInfo;

/// Endpoint that returns exactly the requested number of bytes.
const DOWNLOAD_ENDPOINT: &str = "https://speed.cloudflare.com/__down?bytes=";
/// Endpoint that accepts an arbitrary POST body.
const UPLOAD_ENDPOINT: &str = "https://speed.cloudflare.com/__up";
/// Seconds of quiet recorded before and between transfers.
const IDLE_SECS: u64 = 10;
/// Seconds of quiet recorded at the start of an automated run.
const BASELINE_SECS: u64 = 20;

/// Byte totals accumulated over a single labelled phase.
#[derive(Clone, Copy, Default)]
struct PhaseTotals {
    wsl_recv: u64,
    wsl_sent: u64,
    phys_recv: u64,
    phys_sent: u64,
}

/// Message from an input source to the sampling loop.
enum Control {
    /// Close the current phase and begin a new one under this label.
    Mark(String),
    /// Close the current phase, print the verdict, and exit.
    Stop,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }
    let auto = args.iter().any(|a| a == "--auto");
    let payload_bytes = parse_size_mb(&args) * 1024 * 1024;

    println!("======================================================================");
    println!("             WSL Traffic Monitor - Empirical Logger                   ");
    println!("======================================================================");

    let started_utc = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "Unknown".to_string());
    println!("Run started:      {started_utc}");
    println!(
        "Mode:             {}",
        if auto {
            "automated protocol"
        } else {
            "manual markers"
        }
    );

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

    if auto {
        println!("\nPayload per transfer: {payload_bytes} bytes");
        println!("Protocol will run unattended; do not use the network while it runs.");
    } else {
        println!("\nType a label + Enter to stamp a phase marker. Type `done` to finish.");
    }
    println!(
        "--------------------------------------------------------------------------------------------------"
    );
    println!("Time (s) | WSL Recv Delta | WSL Sent Delta | Phys Recv Delta | Phys Sent Delta");
    println!(
        "--------------------------------------------------------------------------------------------------"
    );

    let (tx, rx) = std::sync::mpsc::channel();
    if auto {
        spawn_auto_driver(tx, payload_bytes);
    } else {
        spawn_marker_reader(tx);
    }

    let completed = sample_loop(wsl_luid, phys_luid, &rx);
    print_verdict(&completed, payload_bytes);
}

/// Poll counters once per second until a `Stop` arrives, returning each closed phase.
fn sample_loop(
    wsl_luid: u64,
    phys_luid: Option<u64>,
    rx: &Receiver<Control>,
) -> Vec<(String, PhaseTotals)> {
    let mut last_wsl = get_counters_or_zero(wsl_luid);
    let mut last_phys = phys_luid.map(get_counters_or_zero);
    let start_time = Instant::now();

    let mut completed: Vec<(String, PhaseTotals)> = Vec::new();
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

        while let Ok(control) = rx.try_recv() {
            print_phase_summary(&phase_label, phase_start_secs, elapsed, &phase);
            completed.push((phase_label.clone(), phase));

            match control {
                Control::Stop => return completed,
                Control::Mark(label) => {
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
    }
}

/// Run the fixed directionality and isolation protocol without operator input.
///
/// Each phase is marked, then given a full sampling tick to take effect before the
/// transfer starts, so no transfer bytes land in the preceding phase's subtotal.
fn spawn_auto_driver(tx: Sender<Control>, payload_bytes: u64) {
    thread::spawn(move || {
        let download_url = format!("{DOWNLOAD_ENDPOINT}{payload_bytes}");

        begin(&tx, "idle-baseline");
        thread::sleep(Duration::from_secs(BASELINE_SECS));

        begin(&tx, "wsl-download");
        wsl_download(&download_url);

        begin(&tx, "idle-1");
        thread::sleep(Duration::from_secs(IDLE_SECS));

        begin(&tx, "wsl-upload");
        wsl_upload(payload_bytes);

        begin(&tx, "idle-2");
        thread::sleep(Duration::from_secs(IDLE_SECS));

        begin(&tx, "host-download");
        host_download(&download_url);

        begin(&tx, "idle-3");
        thread::sleep(Duration::from_secs(IDLE_SECS));

        begin(&tx, "both-download");
        both_downloads(&download_url);

        thread::sleep(Duration::from_secs(2));
        let _ = tx.send(Control::Stop);
    });
}

/// Mark the start of a phase and wait for the sampler to pick it up.
fn begin(tx: &Sender<Control>, label: &str) {
    let _ = tx.send(Control::Mark(label.to_string()));
    thread::sleep(Duration::from_millis(1200));
}

fn wsl_download(url: &str) {
    run_quiet(
        "wsl.exe",
        &["-e", "curl", "-sL", "-o", "/dev/null", url],
        "WSL download",
    );
}

fn wsl_upload(payload_bytes: u64) {
    let script = format!(
        "head -c {payload_bytes} /dev/urandom > /tmp/wsl_traffic_upload.bin && \
         curl -s -o /dev/null -X POST --data-binary @/tmp/wsl_traffic_upload.bin {UPLOAD_ENDPOINT}; \
         rm -f /tmp/wsl_traffic_upload.bin"
    );
    run_quiet("wsl.exe", &["-e", "sh", "-c", &script], "WSL upload");
}

fn host_download(url: &str) {
    run_quiet("curl.exe", &["-sL", "-o", "NUL", url], "host download");
}

/// Launch the WSL and host downloads concurrently and wait for both.
fn both_downloads(url: &str) {
    let url_owned = url.to_string();
    let wsl_handle = thread::spawn(move || wsl_download(&url_owned));
    host_download(url);
    let _ = wsl_handle.join();
}

/// Run a command with its output discarded so transfer noise cannot corrupt the log.
fn run_quiet(program: &str, args: &[&str], what: &str) {
    let result = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match result {
        Ok(status) if status.success() => {}
        Ok(status) => println!("--- WARNING: {what} exited with {status}; phase is not valid ---"),
        Err(e) => println!("--- WARNING: {what} could not start ({e}); phase is not valid ---"),
    }
}

/// Read phase labels from stdin on a background thread. `done` ends the run.
fn spawn_marker_reader(tx: Sender<Control>) {
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            match stdin.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let label = line.trim().to_string();
                    let stop = label.eq_ignore_ascii_case("done");
                    let msg = if stop {
                        Control::Stop
                    } else {
                        Control::Mark(label)
                    };
                    if tx.send(msg).is_err() || stop {
                        break;
                    }
                }
            }
        }
    });
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

/// Derive directionality and isolation conclusions from the recorded phases.
///
/// Directionality decides whether the UI labels upload and download correctly, so it
/// is stated as a conclusion here rather than left for a human to infer from a table.
fn print_verdict(completed: &[(String, PhaseTotals)], payload_bytes: u64) {
    let find = |name: &str| {
        completed
            .iter()
            .find(|(label, _)| label == name)
            .map(|(_, t)| *t)
    };

    println!(
        "\n======================================================================\n\
         Phase summary\n\
         ======================================================================"
    );
    println!(
        "{:<16} {:>14} {:>14} {:>14} {:>14}",
        "phase", "wsl_recv", "wsl_sent", "phys_recv", "phys_sent"
    );
    for (label, t) in completed {
        println!(
            "{:<16} {:>14} {:>14} {:>14} {:>14}",
            label, t.wsl_recv, t.wsl_sent, t.phys_recv, t.phys_sent
        );
    }

    println!("\nExpected payload per transfer: {payload_bytes} bytes\n");

    match (find("wsl-download"), find("wsl-upload")) {
        (Some(down), Some(up)) => {
            println!("Hypothesis 2 (directionality):");
            if down.wsl_sent > down.wsl_recv && up.wsl_recv > up.wsl_sent {
                println!(
                    "  CONFIRMS current mapping. During wsl-download the host SENT the bulk of \
                     bytes on the WSL adapter, and during wsl-upload it RECEIVED them."
                );
                println!("  Host OutOctets -> WSL download, host InOctets -> WSL upload.");
            } else if down.wsl_recv > down.wsl_sent && up.wsl_sent > up.wsl_recv {
                println!(
                    "  CONTRADICTS current mapping. During wsl-download the host RECEIVED the \
                     bulk of bytes on the WSL adapter, and during wsl-upload it SENT them."
                );
                println!(
                    "  The mapping in crates/wsl-traffic-monitor/src/lib.rs is inverted and the \
                     UI is reporting upload and download the wrong way round."
                );
            } else {
                println!(
                    "  INCONCLUSIVE. The two phases do not show opposing dominance; something \
                     else was using the network. Re-run on a quiet machine."
                );
            }
        }
        _ => println!("Hypothesis 2 (directionality): phases missing; cannot conclude."),
    }

    if let Some(host) = find("host-download") {
        let leak = host.wsl_recv + host.wsl_sent;
        println!("\nHypothesis 3 (isolation):");
        if host.phys_recv == 0 && host.phys_sent == 0 {
            println!(
                "  UNSUPPORTED. The physical adapter recorded no traffic during a host download, \
                 so the selected adapter is not carrying host internet traffic. Do not draw an \
                 isolation conclusion from this run."
            );
        } else {
            println!(
                "  Host transfer registered {} bytes on the physical adapter; WSL adapter moved \
                 {leak} bytes during the same window.",
                host.phys_recv + host.phys_sent
            );
        }
    }

    println!("\n======================================================================");
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

fn parse_size_mb(args: &[String]) -> u64 {
    args.iter()
        .position(|a| a == "--size-mb")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(10)
}

fn print_help() {
    println!("WSL Traffic Monitor - Empirical Logger");
    println!();
    println!("Usage: experiment [options]");
    println!();
    println!("Options:");
    println!("  --auto           Run the directionality/isolation protocol unattended");
    println!("  --size-mb <N>    Payload size per transfer in MiB (default: 10)");
    println!("  -h, --help       Show this help message");
    println!();
    println!("With no options, phases are marked by typing labels on stdin; `done` ends the run.");
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
