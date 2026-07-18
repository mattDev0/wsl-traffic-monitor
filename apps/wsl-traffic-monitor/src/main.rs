//! Application entry point for WSL Traffic Monitor.

use std::env;
use std::process;
use wsl_traffic_diagnostics::{format_report_as_json, format_report_as_text, generate_report};

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut print_json = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "-j" | "--json" => {
                print_json = true;
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
}

fn print_help() {
    println!("WSL Traffic Monitor - Validation Harness (Phase 0)");
    println!();
    println!("Usage: wsl-traffic-monitor [options]");
    println!();
    println!("Options:");
    println!("  -j, --json     Output the diagnostics report in structured JSON format");
    println!("  -h, --help     Show this help message");
}
