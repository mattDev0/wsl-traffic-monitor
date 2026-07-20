//! Real-Time Interface Counter Logger (Phase 0).
//!
//! Polls the selected WSL virtual adapter and the default host physical adapter
//! every 1 second and prints raw bytes received/sent deltas.

use std::thread;
use std::time::{Duration, Instant};

fn main() {
    println!("======================================================================");
    println!("             WSL Traffic Monitor - Empirical Logger                   ");
    println!("======================================================================");

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

    // Find the primary host physical interface
    let physical_adapter = adapters.iter().find(|a| {
        a.luid != wsl_luid
            && (a.if_type == 6 || a.if_type == 71)
            && a.oper_status == 1
            && !a.friendly_name.to_lowercase().contains("wsl")
            && !a.friendly_name.to_lowercase().contains("virtual")
    });

    let phys_luid = if let Some(a) = physical_adapter {
        println!("Physical NIC:     {} (LUID: {})", a.friendly_name, a.luid);
        Some(a.luid)
    } else {
        println!("Warning: No physical internet interface identified!");
        None
    };

    println!("\nStarting real-time counter logging (Ctrl+C to terminate)...");
    println!(
        "--------------------------------------------------------------------------------------------------"
    );
    println!("Time (s) | WSL Recv Delta | WSL Sent Delta | Phys Recv Delta | Phys Sent Delta");
    println!(
        "--------------------------------------------------------------------------------------------------"
    );

    let mut last_wsl = get_counters_or_zero(wsl_luid);
    let mut last_phys = phys_luid.map(get_counters_or_zero);
    let start_time = Instant::now();

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

        last_wsl = current_wsl;
        last_phys = current_phys;
    }
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
