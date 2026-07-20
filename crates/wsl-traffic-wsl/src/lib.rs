//! WSL discovery and configuration boundary.
//!
//! Handles parsing of `.wslconfig` and executing `wsl.exe` commands to discover
//! WSL distributions and networking states.
//! 
//! **Safety Rule**: This application is strictly READ-ONLY concerning WSL configuration.
//! It must NEVER automatically mutate `.wslconfig`, as changing networking modes
//! (e.g., to mirrored) can destructively break user environments, firewalls, and VPNs.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use wsl_traffic_core::{WslDistroInfo, WslInfo};

/// Parse a simple INI configuration file (like `.wslconfig`).
///
/// # Errors
/// Returns an error if reading the file fails.
pub fn parse_ini_file<P: AsRef<Path>>(
    path: P,
) -> io::Result<HashMap<String, HashMap<String, String>>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut config = HashMap::new();
    let mut current_section = String::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            continue;
        }
        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim().to_string(); // Keep key case or lowercase? Let's keep key case but let's query it case-insensitively or lowercase.
            let value = trimmed[pos + 1..].trim().to_string();
            config
                .entry(current_section.clone())
                .or_insert_with(HashMap::new)
                .insert(key, value);
        }
    }
    Ok(config)
}

/// Helper to decode wsl.exe process output which is typically UTF-16 encoded on Windows.
#[must_use]
pub fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && (bytes[0] == 0xff && bytes[1] == 0xfe) {
        let u16_chars: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&u16_chars)
    } else if bytes.len() >= 2 && (bytes[0] == 0xfe && bytes[1] == 0xff) {
        let u16_chars: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&u16_chars)
    } else if bytes.contains(&0) {
        let u16_chars: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&u16_chars)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Parse the output of `wsl.exe --version` to extract WSL and kernel versions.
#[must_use]
pub fn parse_wsl_version_output(output: &str) -> (Option<String>, Option<String>) {
    let mut wsl_ver = None;
    let mut kernel_ver = None;
    for line in output.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 2 {
            let key = parts[0].trim().to_lowercase();
            let val = parts[1].trim().to_string();
            if key == "wsl version" || key == "wsl" || key == "wsl-version" {
                wsl_ver = Some(val);
            } else if key == "kernel version" || key == "kernel-version" || key == "kernel" {
                kernel_ver = Some(val);
            }
        }
    }
    (wsl_ver, kernel_ver)
}

/// Parse the output of `wsl.exe -l -v` to extract installed distributions and their state.
#[must_use]
pub fn parse_wsl_list_output(output: &str) -> Vec<WslDistroInfo> {
    let mut distros = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.to_lowercase().contains("name") && trimmed.to_lowercase().contains("state") {
            continue;
        }
        let clean_line = if let Some(stripped) = trimmed.strip_prefix('*') {
            stripped.trim()
        } else {
            trimmed
        };
        let parts: Vec<&str> = clean_line.split_whitespace().collect();
        if parts.len() >= 3 {
            let name = parts[0].to_string();
            let state = parts[1].to_lowercase();
            let version_str = parts[2];
            let version = version_str.parse::<u32>().unwrap_or(2);
            let is_running = state.contains("running");
            distros.push(WslDistroInfo {
                name,
                version,
                is_running,
            });
        }
    }
    distros
}

/// Get the expected path to the `.wslconfig` file in the user's home directory.
#[must_use]
pub fn get_wslconfig_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf()))
        .map(|path| path.join(".wslconfig"))
}

/// Detect the active WSL configuration and installation status.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn detect_wsl() -> WslInfo {
    let wslconfig_path = get_wslconfig_path();
    let wslconfig_parsed = wslconfig_path.and_then(|path| parse_ini_file(path).ok());

    let mut networking_mode = "nat".to_string();
    if let Some(ref config) = wslconfig_parsed {
        if let Some(wsl2_sec) = config.get("wsl2") {
            // Check case-sensitively or case-insensitively. Let's do case-insensitive search
            for (k, v) in wsl2_sec {
                if k.to_lowercase() == "networkingmode" {
                    networking_mode = v.to_lowercase();
                }
            }
        }
    }

    // Run wsl.exe commands if on Windows
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let run_cmd = |args: &[&str]| -> Option<String> {
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("wsl.exe");
            cmd.args(args);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

            let child = cmd.spawn().ok()?;
            let (tx, rx) = mpsc::channel();
            let child_id = child.id();

            thread::spawn(move || {
                let res = child.wait_with_output();
                let _ = tx.send(res);
            });

            let output = if let Ok(Ok(out)) = rx.recv_timeout(Duration::from_secs(5)) {
                Some(out)
            } else {
                // Timeout or error: forcibly kill the child process
                let _ = Command::new("taskkill.exe")
                    .args(["/F", "/PID", &child_id.to_string()])
                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                    .output();
                None
            }?;

            if output.status.success() {
                Some(decode_wsl_output(&output.stdout))
            } else {
                None
            }
        };

        let ver_output = run_cmd(&["--version"]);
        let (wsl_version, kernel_version) =
            ver_output.map_or((None, None), |out| parse_wsl_version_output(&out));

        let list_output = run_cmd(&["-l", "-v"]);
        let distributions = list_output.map_or_else(
            || {
                // Fallback to registry if command fails
                wsl_traffic_windows::get_wsl_distros_from_registry()
                    .into_iter()
                    .map(|(name, version)| WslDistroInfo {
                        name,
                        version,
                        is_running: false, // Registry doesn't know running status
                    })
                    .collect()
            },
            |out| parse_wsl_list_output(&out),
        );

        // WSL is installed if registry contains distributions or wsl.exe executed successfully
        let is_installed = !distributions.is_empty() || wsl_version.is_some();

        WslInfo {
            is_installed,
            wsl_version,
            kernel_version,
            distributions,
            networking_mode,
            wslconfig_parsed,
        }
    }

    #[cfg(not(windows))]
    {
        // Mock data for development and testing on Linux
        let distros = vec![
            WslDistroInfo {
                name: "Ubuntu".to_string(),
                version: 2,
                is_running: true,
            },
            WslDistroInfo {
                name: "docker-desktop".to_string(),
                version: 2,
                is_running: true,
            },
            WslDistroInfo {
                name: "docker-desktop-data".to_string(),
                version: 2,
                is_running: true,
            },
        ];
        WslInfo {
            is_installed: true,
            wsl_version: Some("2.2.4.0".to_string()),
            kernel_version: Some("5.15.153.1-2".to_string()),
            distributions: distros,
            networking_mode,
            wslconfig_parsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wsl_version_output() {
        let sample = "WSL version: 2.2.4.0\nKernel version: 5.15.153.1-2\nWSLg version: 1.0.64";
        let (wsl, kernel) = parse_wsl_version_output(sample);
        assert_eq!(wsl, Some("2.2.4.0".to_string()));
        assert_eq!(kernel, Some("5.15.153.1-2".to_string()));
    }

    #[test]
    fn test_parse_wsl_list_output() {
        let sample = "  NAME                   STATE           VERSION\n* Ubuntu                 Running         2\n  docker-desktop         Stopped         2";
        let distros = parse_wsl_list_output(sample);
        assert_eq!(distros.len(), 2);
        assert_eq!(distros[0].name, "Ubuntu");
        assert!(distros[0].is_running);
        assert_eq!(distros[0].version, 2);
        assert_eq!(distros[1].name, "docker-desktop");
        assert!(!distros[1].is_running);
    }
}
