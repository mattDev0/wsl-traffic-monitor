//! Application entry point for WSL Traffic Monitor.

use std::env;
use std::process;
use wsl_traffic_diagnostics::{format_report_as_json, format_report_as_text, generate_report};

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut print_json = false;
    let mut run_diagnostics = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "-j" | "--json" => {
                print_json = true;
                run_diagnostics = true;
            }
            "-d" | "--diagnostics" => {
                run_diagnostics = true;
            }
            "-h" | "--help" => {
                print_help();
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_help();
                process::exit(1);
            }
        }
    }

    if run_diagnostics {
        match generate_report() {
            Ok(report) => {
                if print_json {
                    match format_report_as_json(&report) {
                        Ok(json) => println!("{json}"),
                        Err(e) => {
                            eprintln!("Error formatting report as JSON: {e}");
                            process::exit(1);
                        }
                    }
                } else {
                    let text = format_report_as_text(&report);
                    println!("{text}");
                }
            }
            Err(e) => {
                eprintln!("Error generating diagnostics report: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // Default behavior: Run the active monitoring service and the UI
    let settings = wsl_traffic_storage::load_settings();
    let mut service = wsl_traffic_monitor::WslTrafficMonitorService::new();

    // Start background sampling at config interval (default: 1000ms)
    let interval = std::time::Duration::from_millis(settings.poll_interval_ms);
    if let Err(e) = service.start(interval) {
        eprintln!("Failed to start monitor service: {e}");
        process::exit(1);
    }

    #[cfg(windows)]
    {
        println!("Starting native Windows system tray UI...");
        if let Err(e) = wsl_traffic_ui::run_ui(service, settings) {
            eprintln!("UI error: {e}");
            process::exit(1);
        }
    }

    #[cfg(not(windows))]
    {
        println!("System tray UI is only supported on Windows.");
        println!(
            "Starting real-time CLI monitor (polling every {}ms, Ctrl+C to exit)...",
            settings.poll_interval_ms
        );

        loop {
            std::thread::sleep(interval);
            let snapshot = service.get_snapshot();
            let label = snapshot.error_state.as_deref().unwrap_or("OK");
            println!(
                "Status: {:?} (Error: {}) | Down: {} | Up: {} | Confidence: {:?}",
                snapshot.status,
                label,
                format_speed(snapshot.download_speed, settings.speed_unit),
                format_speed(snapshot.upload_speed, settings.speed_unit),
                snapshot.confidence
            );
        }
    }
}

fn print_help() {
    println!("WSL Traffic Monitor");
    println!();
    println!("Usage: wsl-traffic-monitor [options]");
    println!();
    println!("Options:");
    println!("  -d, --diagnostics  Output the diagnostics report");
    println!("  -j, --json         Output the diagnostics report in structured JSON format");
    println!("  -h, --help         Show this help message");
}

#[allow(dead_code)]
fn format_speed(bytes_per_sec: f64, unit: wsl_traffic_storage::SpeedUnit) -> String {
    match unit {
        wsl_traffic_storage::SpeedUnit::Bytes => {
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
        wsl_traffic_storage::SpeedUnit::Bits => {
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
