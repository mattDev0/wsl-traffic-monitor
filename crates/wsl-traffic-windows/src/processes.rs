//! Process enumeration utility for Windows.

/// Query running Docker-related processes on the host.
#[must_use]
#[allow(unsafe_code)]
pub fn get_running_docker_processes() -> Vec<String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        };

        let mut running = Vec::new();
        let target_processes = [
            "com.docker.backend.exe",
            "docker desktop.exe",
            "dockerd.exe",
            "vpnkit.exe",
        ];

        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if let Ok(handle) = snapshot {
                if handle != INVALID_HANDLE_VALUE {
                    let mut entry = PROCESSENTRY32W::default();
                    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                    if Process32FirstW(handle, &mut entry).is_ok() {
                        loop {
                            let len = entry
                                .szExeFile
                                .iter()
                                .position(|&x| x == 0)
                                .unwrap_or(entry.szExeFile.len());
                            let exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                            let exe_lower = exe_name.to_lowercase();

                            for target in &target_processes {
                                if exe_lower == *target {
                                    running.push(exe_name.clone());
                                    break;
                                }
                            }

                            if Process32NextW(handle, &mut entry).is_err() {
                                break;
                            }
                        }
                    }
                    let _ = CloseHandle(handle);
                }
            }
        }
        running
    }
    #[cfg(not(windows))]
    {
        // Mock running Docker processes on Linux development system
        vec![
            "com.docker.backend.exe".to_string(),
            "vpnkit.exe".to_string(),
        ]
    }
}
