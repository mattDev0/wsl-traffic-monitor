//! Native Windows system tray UI implementation.
#![allow(clippy::all, clippy::pedantic, clippy::restriction)]
#![cfg(windows)]
#![allow(non_snake_case)]

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, HICON,
    IDI_APPLICATION, KillTimer, LoadIconW, MF_GRAYED, MF_SEPARATOR, MF_STRING, PostMessageW,
    PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY, WM_TIMER, WM_USER,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{PCWSTR, w};
use wsl_traffic_monitor::{NetworkProvider, WslTrafficMonitorService};

use wsl_traffic_storage::SpeedUnit;

fn format_speed(bytes_per_sec: f64, unit: SpeedUnit) -> String {
    match unit {
        SpeedUnit::Bytes => {
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
        SpeedUnit::Bits => {
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

const WM_TRAY_ICON: u32 = WM_USER + 1;
const ID_TRAY_TITLE: usize = 1000;
const ID_TRAY_STATUS: usize = 1001;
const ID_TRAY_DOWNLOAD: usize = 1002;
const ID_TRAY_UPLOAD: usize = 1003;
const ID_TRAY_EXIT: usize = 1004;
const ID_TRAY_DIAGNOSTICS: usize = 1005;
const TRAY_TIMER_ID: usize = 1;

thread_local! {
    static TRAY_STATE: std::cell::RefCell<Option<Box<dyn TrayHandler>>> = std::cell::RefCell::new(None);
}

trait TrayHandler {
    fn on_timer(&mut self, hwnd: HWND);
    fn on_tray_icon(&mut self, hwnd: HWND, lparam: LPARAM);
    fn on_command(&mut self, hwnd: HWND, wparam: WPARAM);
    fn on_destroy(&mut self, hwnd: HWND);
}

pub struct TrayStateImpl<P: NetworkProvider> {
    pub(crate) service: WslTrafficMonitorService<P>,
    pub(crate) settings: wsl_traffic_storage::UserSettings,
}

impl<P: NetworkProvider> TrayHandler for TrayStateImpl<P> {
    fn on_timer(&mut self, hwnd: HWND) {
        let snapshot = self.service.get_snapshot();
        let tip_text = format!(
            "WSL Network Traffic\nDown: {}\nUp: {}",
            format_speed(snapshot.download_speed, self.settings.speed_unit),
            format_speed(snapshot.upload_speed, self.settings.speed_unit)
        );
        update_tray_icon_tip(hwnd, &tip_text);
    }

    fn on_tray_icon(&mut self, hwnd: HWND, lparam: LPARAM) {
        let event = lparam.0 as u32;
        if event == windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP {
            show_context_menu(hwnd, &self.service, self.settings.speed_unit);
        }
    }

    #[allow(unsafe_code)]
    fn on_command(&mut self, hwnd: HWND, wparam: WPARAM) {
        let control_id = (wparam.0 & 0xffff) as usize;
        if control_id == ID_TRAY_EXIT {
            // Safety: hwnd is verified to be a valid handle before invocation.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        } else if control_id == ID_TRAY_DIAGNOSTICS {
            export_diagnostics(hwnd);
        }
    }

    #[allow(unsafe_code)]
    fn on_destroy(&mut self, _hwnd: HWND) {
        // Safety: calling standard post quit message loop exit.
        unsafe {
            PostQuitMessage(0);
        }
    }
}

#[allow(unsafe_code)]
unsafe extern "system" fn WndProc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            TRAY_STATE.with(|state| {
                if let Some(handler) = state.borrow_mut().as_mut() {
                    handler.on_timer(hwnd);
                }
            });
            LRESULT(0)
        }
        WM_TRAY_ICON => {
            TRAY_STATE.with(|state| {
                if let Some(handler) = state.borrow_mut().as_mut() {
                    handler.on_tray_icon(hwnd, lparam);
                }
            });
            LRESULT(0)
        }
        WM_COMMAND => {
            TRAY_STATE.with(|state| {
                if let Some(handler) = state.borrow_mut().as_mut() {
                    handler.on_command(hwnd, wparam);
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            TRAY_STATE.with(|state| {
                if let Some(handler) = state.borrow_mut().as_mut() {
                    handler.on_destroy(hwnd);
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[allow(unsafe_code)]
fn update_tray_icon_tip(hwnd: HWND, tip: &str) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_TIP,
        ..Default::default()
    };

    let wide: Vec<u16> = tip.encode_utf16().collect();
    let len = wide.len().min(nid.szTip.len() - 1);
    nid.szTip[..len].copy_from_slice(&wide[..len]);
    nid.szTip[len] = 0;

    // Safety: modifying existing registered tray icon by valid window handle and index.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

#[allow(unsafe_code)]
fn show_context_menu<P: NetworkProvider>(
    hwnd: HWND,
    service: &WslTrafficMonitorService<P>,
    unit: SpeedUnit,
) {
    let snapshot = service.get_snapshot();
    let download_str = format!("Download: {}", format_speed(snapshot.download_speed, unit));
    let upload_str = format!("Upload: {}", format_speed(snapshot.upload_speed, unit));
    let status_str = format!("Status: {:?}", snapshot.status);

    unsafe {
        // Safety: Creating context popup menu. Handle is verified and closed by standard TrackPopupMenu flow.
        let hmenu = CreatePopupMenu().unwrap();

        let title_wide: Vec<u16> = "WSL Traffic Monitor"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_GRAYED,
            ID_TRAY_TITLE,
            PCWSTR(title_wide.as_ptr()),
        );

        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());

        let status_wide: Vec<u16> = status_str.encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_GRAYED,
            ID_TRAY_STATUS,
            PCWSTR(status_wide.as_ptr()),
        );

        let down_wide: Vec<u16> = download_str.encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_GRAYED,
            ID_TRAY_DOWNLOAD,
            PCWSTR(down_wide.as_ptr()),
        );

        let up_wide: Vec<u16> = upload_str.encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_GRAYED,
            ID_TRAY_UPLOAD,
            PCWSTR(up_wide.as_ptr()),
        );

        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());

        let diag_wide: Vec<u16> = "Export Diagnostics..."
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_TRAY_DIAGNOSTICS,
            PCWSTR(diag_wide.as_ptr()),
        );

        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());

        let exit_wide: Vec<u16> = "Exit".encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, PCWSTR(exit_wide.as_ptr()));

        let mut pos = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut pos);

        // Required call to permit keyboard focus and click-away dismiss on tray menus.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            hmenu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            pos.x,
            pos.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), 0, WPARAM(0), LPARAM(0));
    }
}

/// Start the Win32 message loop window and load the tray notification icon.
#[allow(unsafe_code)]
pub fn run_tray_ui<P: NetworkProvider>(
    service: WslTrafficMonitorService<P>,
    settings: wsl_traffic_storage::UserSettings,
) -> Result<(), String> {
    TRAY_STATE.with(|state| {
        *state.borrow_mut() = Some(Box::new(TrayStateImpl { service, settings }));
    });

    unsafe {
        // Safety: retrieves module handle for the current running process to register window class.
        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null())
            .map_err(|e| format!("Failed to get module handle: {e}"))?;

        let class_name: Vec<u16> = "WSLTrafficMonitorClass"
            .encode_utf16()
            .chain(Some(0))
            .collect();

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(WndProc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON(std::ptr::null_mut())),
            ..Default::default()
        };

        // Safety: registers class for the utility window.
        let atom = RegisterClassW(&wnd_class);
        if atom == 0 {
            return Err("Failed to register window class".to_string());
        }

        // Safety: CreateWindowExW creates a hidden window to anchor standard messages.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            w!("WSL Traffic Monitor Utility Window"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(windows::Win32::Foundation::HINSTANCE(instance.0)),
            None,
        )
        .map_err(|e| format!("Failed to create window: {e}"))?;

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or(HICON(std::ptr::null_mut())),
            ..Default::default()
        };
        let tip_wide: Vec<u16> = "WSL Traffic Monitor starting...".encode_utf16().collect();
        let len = tip_wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);
        nid.szTip[len] = 0;

        // Safety: Shell_NotifyIconW mounts the notification icon in the tray.
        let res = Shell_NotifyIconW(NIM_ADD, &nid);
        if !res.as_bool() {
            return Err("Failed to add tray icon".to_string());
        }

        // Safety: SetTimer schedules 1-second ticks in WndProc timer.
        let timer = SetTimer(Some(hwnd), TRAY_TIMER_ID, 1000, None);
        if timer == 0 {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            return Err("Failed to create timer".to_string());
        }

        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        // Safety: standard Win32 message pump processing.
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Safety: standard timer and tray icon resource release.
        let _ = KillTimer(Some(hwnd), TRAY_TIMER_ID);
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }

    TRAY_STATE.with(|state| {
        *state.borrow_mut() = None;
    });

    Ok(())
}

fn export_diagnostics(hwnd: HWND) {
    match wsl_traffic_diagnostics::generate_report() {
        Ok(report) => {
            let text = wsl_traffic_diagnostics::format_report_as_text(&report);
            if let Some(proj) =
                directories::ProjectDirs::from("com", "wsl-traffic-monitor", "WSL Traffic Monitor")
            {
                let path = proj.config_dir().join("diagnostics_report.txt");
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&path, &text).is_ok() {
                    let msg = format!(
                        "Diagnostics report exported successfully to:\n{}",
                        path.to_string_lossy()
                    );
                    show_message_box(hwnd, &msg, "Diagnostics Exported", false);
                    return;
                }
            }
            show_message_box(
                hwnd,
                "Failed to write diagnostics report file.",
                "Export Error",
                true,
            );
        }
        Err(e) => {
            let msg = format!("Failed to generate diagnostics report:\n{e}");
            show_message_box(hwnd, &msg, "Export Error", true);
        }
    }
}

#[allow(unsafe_code)]
fn show_message_box(hwnd: HWND, message: &str, title: &str, is_error: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW,
    };
    let msg_wide: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title_wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let flags = if is_error {
        MB_ICONERROR
    } else {
        MB_ICONINFORMATION
    } | MB_OK;
    unsafe {
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(msg_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            flags,
        );
    }
}
