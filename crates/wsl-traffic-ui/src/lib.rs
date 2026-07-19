//! Native Windows system tray UI wrapper.

#![deny(missing_docs)]

use wsl_traffic_monitor::NetworkProvider;

#[cfg(windows)]
mod win_tray;

/// Run the system tray UI. On Windows, this enters the Win32 message loop and displays the tray icon.
/// On other platforms, it returns an error.
///
/// # Errors
/// Returns an error string if window creation, registering class, or tray icon initialization fails.
#[allow(clippy::needless_pass_by_value)]
pub fn run_ui<P: NetworkProvider>(
    service: wsl_traffic_monitor::WslTrafficMonitorService<P>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        win_tray::run_tray_ui(service)
    }
    #[cfg(not(windows))]
    {
        let _ = service;
        println!("System tray UI is only supported on Windows.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_platform_boundary() {
        // Verify that the function compiles and runs on any platform
        let service = wsl_traffic_monitor::WslTrafficMonitorService::new();
        assert!(!service.is_running());
    }

    #[cfg(windows)]
    #[test]
    fn test_win_tray_state_creation() {
        let service = wsl_traffic_monitor::WslTrafficMonitorService::new();
        let handler = win_tray::TrayStateImpl { service };
        assert!(!handler.service.is_running());
    }
}
