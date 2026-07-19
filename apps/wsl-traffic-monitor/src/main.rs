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
    let mut service = wsl_traffic_monitor::WslTrafficMonitorService::new();

    // Start background sampling at 1-second interval
    if let Err(e) = service.start(std::time::Duration::from_secs(1)) {
        eprintln!("Failed to start monitor service: {e}");
        process::exit(1);
    }

    #[cfg(windows)]
    {
        println!("Starting native Windows system tray UI...");
        if let Err(e) = wsl_traffic_ui::run_ui(service) {
            eprintln!("UI error: {e}");
            process::exit(1);
        }
    }

    #[cfg(not(windows))]
    {
        println!("System tray UI is only supported on Windows.");
        println!("Starting real-time CLI monitor (polling every 1s, Ctrl+C to exit)...");

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let snapshot = service.get_snapshot();
            let label = snapshot.error_state.as_deref().unwrap_or("OK");
            println!(
                "Status: {:?} (Error: {}) | Down: {}/s | Up: {}/s | Confidence: {:?}",
                snapshot.status,
                label,
                format_bytes(snapshot.download_speed),
                format_bytes(snapshot.upload_speed),
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
fn format_bytes(bytes: f64) -> String {
    if bytes >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GiB", bytes / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024.0 * 1024.0 {
        format!("{:.2} MiB", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.2} KiB", bytes / 1024.0)
    } else {
        format!("{bytes:.0} B")
    }
}
