//! Registry utility module for Windows.
#![allow(clippy::all, clippy::pedantic, clippy::restriction)]

#[cfg(windows)]
/// Direct Win32 registry calls.
pub mod win {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        HKEY, KEY_READ, REG_DWORD, REG_VALUE_TYPE, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
        RegQueryValueExW,
    };
    use windows::core::{PCWSTR, PWSTR};

    fn to_utf16_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Query a registry string value (REG_SZ, REG_EXPAND_SZ, or REG_MULTI_SZ).
    #[allow(unsafe_code)]
    pub fn query_reg_string(hkey: HKEY, subkey: &str, value: &str) -> Option<String> {
        unsafe {
            let mut hkresult = HKEY::default();
            let subkey_u16 = to_utf16_null(subkey);
            let res = RegOpenKeyExW(
                hkey,
                PCWSTR(subkey_u16.as_ptr()),
                None,
                KEY_READ,
                &mut hkresult,
            );
            if res != ERROR_SUCCESS {
                return None;
            }

            let value_u16 = to_utf16_null(value);
            let mut cbdata = 0u32;
            let mut val_type = REG_VALUE_TYPE::default();
            let res = RegQueryValueExW(
                hkresult,
                PCWSTR(value_u16.as_ptr()),
                None,
                Some(&mut val_type),
                None,
                Some(&mut cbdata),
            );

            if res != ERROR_SUCCESS || cbdata == 0 {
                let _ = RegCloseKey(hkresult);
                return None;
            }

            // Verify registry type is a string type before decoding
            use windows::Win32::System::Registry::{REG_EXPAND_SZ, REG_MULTI_SZ, REG_SZ};
            if val_type != REG_SZ && val_type != REG_EXPAND_SZ && val_type != REG_MULTI_SZ {
                let _ = RegCloseKey(hkresult);
                return None;
            }

            let mut data = vec![0u8; cbdata as usize];
            let res = RegQueryValueExW(
                hkresult,
                PCWSTR(value_u16.as_ptr()),
                None,
                None,
                Some(data.as_mut_ptr()),
                Some(&mut cbdata),
            );

            let _ = RegCloseKey(hkresult);

            if res != ERROR_SUCCESS {
                return None;
            }

            let u16_len = cbdata as usize / 2;
            let u16_slice = std::slice::from_raw_parts(data.as_ptr() as *const u16, u16_len);
            let clean_len = u16_slice.iter().position(|&x| x == 0).unwrap_or(u16_len);
            Some(String::from_utf16_lossy(&u16_slice[..clean_len]))
        }
    }

    /// Query a registry DWORD value (REG_DWORD).
    #[allow(unsafe_code)]
    pub fn query_reg_u32(hkey: HKEY, subkey: &str, value: &str) -> Option<u32> {
        unsafe {
            let mut hkresult = HKEY::default();
            let subkey_u16 = to_utf16_null(subkey);
            let res = RegOpenKeyExW(
                hkey,
                PCWSTR(subkey_u16.as_ptr()),
                None,
                KEY_READ,
                &mut hkresult,
            );
            if res != ERROR_SUCCESS {
                return None;
            }

            let value_u16 = to_utf16_null(value);
            let mut cbdata = 4u32;
            let mut data = 0u32;
            let mut val_type = REG_VALUE_TYPE::default();
            let res = RegQueryValueExW(
                hkresult,
                PCWSTR(value_u16.as_ptr()),
                None,
                Some(&mut val_type),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut cbdata),
            );

            let _ = RegCloseKey(hkresult);

            if res == ERROR_SUCCESS && val_type == REG_DWORD {
                Some(data)
            } else {
                None
            }
        }
    }

    /// Enumerate all subkey names under a given registry path.
    #[allow(unsafe_code)]
    pub fn enum_reg_keys(hkey: HKEY, subkey: &str) -> Option<Vec<String>> {
        unsafe {
            let mut hkresult = HKEY::default();
            let subkey_u16 = to_utf16_null(subkey);
            let res = RegOpenKeyExW(
                hkey,
                PCWSTR(subkey_u16.as_ptr()),
                None,
                KEY_READ,
                &mut hkresult,
            );
            if res != ERROR_SUCCESS {
                return None;
            }

            let mut keys = Vec::new();
            let mut index = 0u32;
            loop {
                let mut name_buf = [0u16; 256];
                let mut name_len = 256u32;
                let res = RegEnumKeyExW(
                    hkresult,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                );
                if res == ERROR_SUCCESS {
                    let len = name_len as usize;
                    let name = String::from_utf16_lossy(&name_buf[..len]);
                    keys.push(name);
                    index += 1;
                } else {
                    break;
                }
            }
            let _ = RegCloseKey(hkresult);
            Some(keys)
        }
    }
}

/// Retrieve the Windows OS version details from the registry.
#[must_use]
pub fn get_windows_version() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
        let subkey = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let product_name = win::query_reg_string(HKEY_LOCAL_MACHINE, subkey, "ProductName")
            .unwrap_or_else(|| "Windows".to_string());
        let current_build = win::query_reg_string(HKEY_LOCAL_MACHINE, subkey, "CurrentBuild")
            .unwrap_or_else(|| "Unknown".to_string());
        let display_version = win::query_reg_string(HKEY_LOCAL_MACHINE, subkey, "DisplayVersion")
            .unwrap_or_else(|| "".to_string());

        if display_version.is_empty() {
            format!("{product_name} (Build {current_build})")
        } else {
            format!("{product_name} Version {display_version} (Build {current_build})")
        }
    }
    #[cfg(not(windows))]
    {
        "Mock Windows 11 (Linux Dev Environment)".to_string()
    }
}

/// Enumerate WSL distributions registry entries under HKCU.
/// Returns a list of (`DistributionName`, Version).
#[must_use]
pub fn get_wsl_distros_from_registry() -> Vec<(String, u32)> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::HKEY_CURRENT_USER;
        let lxss_path = r"Software\Microsoft\Windows\CurrentVersion\Lxss";
        let mut results = Vec::new();

        if let Some(subkeys) = win::enum_reg_keys(HKEY_CURRENT_USER, lxss_path) {
            for subkey in subkeys {
                // The subkey is the GUID.
                let path = format!("{lxss_path}\\{subkey}");
                let name = win::query_reg_string(HKEY_CURRENT_USER, &path, "DistributionName");
                let version = win::query_reg_u32(HKEY_CURRENT_USER, &path, "Version");

                if let (Some(n), Some(v)) = (name, version) {
                    results.push((n, v));
                }
            }
        }
        results
    }
    #[cfg(not(windows))]
    {
        // Mock distributions for Linux target checks/tests
        vec![
            ("Ubuntu".to_string(), 2),
            ("docker-desktop".to_string(), 2),
            ("docker-desktop-data".to_string(), 2),
        ]
    }
}

/// Check if Docker Desktop is registered as installed.
#[must_use]
pub fn is_docker_desktop_installed() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let path1 = r"SOFTWARE\Docker Inc.\Docker Desktop";
        let path2 = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Docker Desktop";

        win::query_reg_string(HKEY_LOCAL_MACHINE, path1, "Version").is_some()
            || win::query_reg_string(HKEY_LOCAL_MACHINE, path2, "DisplayName").is_some()
            || win::query_reg_string(HKEY_CURRENT_USER, path1, "Version").is_some()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Check if the current process is running with administrative (elevated) privileges.
#[must_use]
#[allow(unsafe_code)]
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, KEY_WRITE, RegCloseKey, RegOpenKeyExW,
        };
        use windows::core::PCWSTR;

        unsafe {
            let mut key = windows::Win32::System::Registry::HKEY::default();
            let subkey: Vec<u16> = "Software".encode_utf16().chain(Some(0)).collect();
            let result = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                Some(0),
                KEY_WRITE,
                &mut key,
            );
            if result.is_ok() {
                let _ = RegCloseKey(key);
                true
            } else {
                false
            }
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

use crate::WindowsError;

/// Register or unregister the application for autostart on Windows.
///
/// # Errors
/// Returns an error string if writing to registry fails.
#[allow(unsafe_code)]
pub fn set_autostart(enabled: bool) -> Result<(), WindowsError> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Registry::{
            HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
            RegSetValueExW,
        };
        use windows::core::PCWSTR;

        let run_key_path: Vec<u16> = r"Software\Microsoft\Windows\CurrentVersion\Run"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let value_name: Vec<u16> = "WslTrafficMonitor".encode_utf16().chain(Some(0)).collect();

        let mut hkey = windows::Win32::System::Registry::HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(run_key_path.as_ptr()),
            Some(0),
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result.is_err() {
            return Err(WindowsError::OpenRunRegistryKeyFailed {
                details: format!("{:?}", result),
            });
        }

        if enabled {
            let exe_path =
                std::env::current_exe().map_err(|e| WindowsError::CurrentExePathFailed {
                    details: e.to_string(),
                })?;
            let exe_str = format!("\"{}\"", exe_path.to_string_lossy());
            let exe_wide: Vec<u16> = exe_str.encode_utf16().chain(Some(0)).collect();

            let bytes = std::slice::from_raw_parts(
                exe_wide.as_ptr() as *const u8,
                exe_wide.len() * std::mem::size_of::<u16>(),
            );

            let set_result =
                RegSetValueExW(hkey, PCWSTR(value_name.as_ptr()), None, REG_SZ, Some(bytes));

            let _ = RegCloseKey(hkey);

            if set_result.is_err() {
                return Err(WindowsError::WriteAutostartRegistryValueFailed {
                    details: format!("{:?}", set_result),
                });
            }
        } else {
            let del_result = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));
            let _ = RegCloseKey(hkey);

            if del_result.is_err() {
                // Ignore if it wasn't there
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}

/// Check if the application is configured to run at Windows startup.
#[must_use]
pub fn is_autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::HKEY_CURRENT_USER;
        let subkey = r"Software\Microsoft\Windows\CurrentVersion\Run";
        win::query_reg_string(HKEY_CURRENT_USER, subkey, "WslTrafficMonitor").is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
