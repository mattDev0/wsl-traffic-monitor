//! Native Windows system tray UI implementation.
#![cfg(windows)]
#![allow(non_snake_case)]
#![allow(
    clippy::borrow_as_ptr,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetMessageW, HICON, IDI_APPLICATION, KillTimer, LoadIconW, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, SetTimer, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY,
    WM_TIMER, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{PCWSTR, w};
use wsl_traffic_core::{format_speed, format_speed_compact};
use wsl_traffic_monitor::{NetworkProvider, WslTrafficMonitorService};

use crate::win_overlay;
use std::fmt::Write as _;

const WM_TRAY_ICON: u32 = WM_USER + 1;
const WM_OVERLAY_RBUTTONUP: u32 = WM_USER + 2;
const ID_TRAY_TITLE: usize = 1000;
const ID_TRAY_STATUS: usize = 1001;
const ID_TRAY_DOWNLOAD: usize = 1002;
const ID_TRAY_UPLOAD: usize = 1003;
const ID_TRAY_EXIT: usize = 1004;
const ID_TRAY_DIAGNOSTICS: usize = 1005;
const ID_TRAY_AUTOSTART: usize = 1006;
const ID_TRAY_HISTORY: usize = 1007;
const ID_TRAY_SETTINGS: usize = 1008;
const ID_TRAY_TOGGLE_OVERLAY: usize = 1009;
const ID_TRAY_LOCK_OVERLAY: usize = 1010;
const TRAY_TIMER_ID: usize = 1;

use windows::Win32::UI::WindowsAndMessaging::RegisterWindowMessageW;

static WM_TASKBARCREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

thread_local! {
    static TRAY_STATE: std::cell::RefCell<Option<Box<dyn TrayHandler>>> = const { std::cell::RefCell::new(None) };
}

trait TrayHandler {
    fn on_timer(&mut self, hwnd: HWND);
    fn on_tray_icon(&mut self, hwnd: HWND, lparam: LPARAM);
    fn on_command(&mut self, hwnd: HWND, wparam: WPARAM);
    fn on_destroy(&mut self, hwnd: HWND);
    fn set_overlay_hwnd(&mut self, overlay_hwnd: HWND);
}

pub struct TrayStateImpl<P: NetworkProvider> {
    pub(crate) service: WslTrafficMonitorService<P>,
    pub(crate) settings: std::sync::Arc<std::sync::Mutex<wsl_traffic_storage::UserSettings>>,
    pub(crate) current_icon: Option<HICON>,
    pub(crate) hwnd_overlay: Option<HWND>,
}

impl<P: NetworkProvider> TrayHandler for TrayStateImpl<P> {
    fn on_timer(&mut self, hwnd: HWND) {
        let snapshot = self.service.get_snapshot();
        let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();

        let tip_text =
            if snapshot.status == wsl_traffic_monitor::MonitorState::UnsupportedNetworkingMode {
                "WSL Network Traffic\nUnsupported Mode (Mirrored/VirtioProxy)".to_string()
            } else {
                format!(
                    "WSL Network Traffic\nDown: {}\nUp: {}",
                    format_speed(snapshot.download_speed, settings.speed_unit),
                    format_speed(snapshot.upload_speed, settings.speed_unit)
                )
            };
        let down_compact = format_speed_compact(snapshot.download_speed);
        let up_compact = format_speed_compact(snapshot.upload_speed);

        self.update_tray_icon(hwnd, &tip_text, &down_compact, &up_compact);

        if let Some(overlay_hwnd) = self.hwnd_overlay {
            win_overlay::update_overlay_snapshot(overlay_hwnd, snapshot);
        }
    }

    fn on_tray_icon(&mut self, hwnd: HWND, lparam: LPARAM) {
        let event = lparam.0 as u32;
        if event == windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP {
            let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
            show_context_menu(hwnd, &self.service, &settings);
        }
    }

    #[allow(unsafe_code)]
    fn on_command(&mut self, hwnd: HWND, wparam: WPARAM) {
        let control_id = wparam.0 & 0xffff;
        if control_id == ID_TRAY_EXIT {
            // Safety: hwnd is verified to be a valid handle before invocation.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        } else if control_id == ID_TRAY_HISTORY {
            show_history(hwnd);
        } else if control_id == ID_TRAY_SETTINGS {
            let settings = self.settings.lock().map(|s| s.clone()).unwrap_or_default();
            open_settings(hwnd, &settings);
        } else if control_id == ID_TRAY_DIAGNOSTICS {
            export_diagnostics(hwnd);
        } else if control_id == ID_TRAY_AUTOSTART {
            if let Ok(mut s) = self.settings.lock() {
                s.run_at_startup = !s.run_at_startup;
                let _ = wsl_traffic_storage::save_settings(&s);
                let _ = wsl_traffic_windows::set_autostart(s.run_at_startup);
            }
        } else if control_id == ID_TRAY_TOGGLE_OVERLAY {
            let show_overlay = if let Ok(mut s) = self.settings.lock() {
                s.show_overlay = !s.show_overlay;
                let _ = wsl_traffic_storage::save_settings(&s);
                s.show_overlay
            } else {
                false
            };
            if let Some(overlay_hwnd) = self.hwnd_overlay {
                win_overlay::set_overlay_visible(overlay_hwnd, show_overlay);
                win_overlay::update_overlay_snapshot(overlay_hwnd, self.service.get_snapshot());
            }
        } else if control_id == ID_TRAY_LOCK_OVERLAY {
            if let Ok(mut s) = self.settings.lock() {
                s.overlay_locked = !s.overlay_locked;
                let _ = wsl_traffic_storage::save_settings(&s);
            }
            if let Some(overlay_hwnd) = self.hwnd_overlay {
                win_overlay::update_overlay_snapshot(overlay_hwnd, self.service.get_snapshot());
            }
        }
    }

    #[allow(unsafe_code)]
    fn on_destroy(&mut self, _hwnd: HWND) {
        if let Some(overlay_hwnd) = self.hwnd_overlay.take() {
            unsafe {
                let _ = DestroyWindow(overlay_hwnd);
            }
        }
        if let Some(old_icon) = self.current_icon.take() {
            unsafe {
                let _ = DestroyIcon(old_icon);
            }
        }
        // Safety: calling standard post quit message loop exit.
        unsafe {
            PostQuitMessage(0);
        }
    }

    fn set_overlay_hwnd(&mut self, overlay_hwnd: HWND) {
        self.hwnd_overlay = Some(overlay_hwnd);
    }
}

impl<P: NetworkProvider> TrayStateImpl<P> {
    #[allow(unsafe_code)]
    fn update_tray_icon(&mut self, hwnd: HWND, tip: &str, down_compact: &str, up_compact: &str) {
        let down_text = format!("D {down_compact}");
        let up_text = format!("U {up_compact}");

        let new_icon = create_speed_icon(&down_text, &up_text);

        let mut flags = NIF_TIP | NIF_MESSAGE;
        let icon_to_set = if let Some(icon) = new_icon {
            flags |= NIF_ICON;
            icon
        } else {
            HICON(std::ptr::null_mut())
        };

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: flags,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: icon_to_set,
            ..Default::default()
        };

        let wide: Vec<u16> = tip.encode_utf16().collect();
        let len = wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&wide[..len]);
        nid.szTip[len] = 0;

        unsafe {
            let res = Shell_NotifyIconW(NIM_MODIFY, &nid);
            if !res.as_bool() {
                // If NIM_MODIFY failed (e.g. Explorer restarted), attempt to re-add the icon
                let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            }
            if new_icon.is_some() {
                if let Some(old_icon) = self.current_icon.take() {
                    let _ = DestroyIcon(old_icon);
                }
                self.current_icon = new_icon;
            }
        }
    }
}

#[allow(unsafe_code)]
unsafe extern "system" fn WndProc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            TRAY_STATE.with(|state| {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    if let Some(handler) = guard.as_mut() {
                        handler.on_timer(hwnd);
                    }
                }
            });
            LRESULT(0)
        }
        WM_TRAY_ICON => {
            TRAY_STATE.with(|state| {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    if let Some(handler) = guard.as_mut() {
                        handler.on_tray_icon(hwnd, lparam);
                    }
                }
            });
            LRESULT(0)
        }
        WM_COMMAND => {
            TRAY_STATE.with(|state| {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    if let Some(handler) = guard.as_mut() {
                        handler.on_command(hwnd, wparam);
                    }
                }
            });
            LRESULT(0)
        }
        WM_OVERLAY_RBUTTONUP => {
            TRAY_STATE.with(|state| {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    if let Some(handler) = guard.as_mut() {
                        handler.on_tray_icon(
                            hwnd,
                            LPARAM(windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP as isize),
                        );
                    }
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            TRAY_STATE.with(|state| {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    if let Some(handler) = guard.as_mut() {
                        handler.on_destroy(hwnd);
                    }
                }
            });
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => {
            let taskbar_msg = WM_TASKBARCREATED.load(std::sync::atomic::Ordering::Relaxed);
            if taskbar_msg != 0 && msg == taskbar_msg {
                TRAY_STATE.with(|state| {
                    if let Ok(mut guard) = state.try_borrow_mut() {
                        if let Some(handler) = guard.as_mut() {
                            handler.on_timer(hwnd);
                        }
                    }
                });
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

/// Resource id of the application icon embedded by the app crate's build script.
const IDI_APP_ICON: u16 = 1;

/// Load the embedded application icon, falling back to the system default.
///
/// This is the icon shown in Alt-Tab, the taskbar and the window's own frame. The
/// tray icon itself is rendered per tick by `create_speed_icon`. The fallback keeps
/// the UI usable if the executable was built without embedded resources.
#[allow(unsafe_code)]
fn load_app_icon(instance: windows::Win32::Foundation::HINSTANCE) -> HICON {
    unsafe {
        LoadIconW(Some(instance), PCWSTR(IDI_APP_ICON as *const u16))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or(HICON(std::ptr::null_mut()))
    }
}

/// Download glyph colour as (R, G, B).
const ICON_DOWN_RGB: (u32, u32, u32) = (0x00, 0xDC, 0xFF);
/// Upload glyph colour as (R, G, B).
const ICON_UP_RGB: (u32, u32, u32) = (0xFF, 0xD0, 0x00);

/// Render the speed readout into a 32bpp icon with a real per-pixel alpha channel.
///
/// A colour bitmap plus a 1bpp mask cannot express partial coverage, so the icon was
/// previously an opaque tile: on a light taskbar it read as a black box next to every
/// other tray glyph. GDI text routines do not write alpha, so the glyphs are drawn in
/// white onto a zeroed buffer and the coverage of each pixel is recovered from its
/// brightness, then recoloured and premultiplied.
#[allow(unsafe_code)]
fn create_speed_icon(down_text: &str, up_text: &str) -> Option<HICON> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap,
        CreateCompatibleDC, CreateDIBSection, CreateFontW, DIB_RGB_COLORS, DT_CENTER,
        DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FONT_CHARSET,
        FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FW_BOLD, HGDIOBJ, SelectObject, SetBkMode,
        SetTextColor, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};

    const WIDTH: i32 = 32;
    const HEIGHT: i32 = 32;

    unsafe {
        let hdc_mem = CreateCompatibleDC(None);
        if hdc_mem.is_invalid() {
            return None;
        }

        // Negative height requests a top-down DIB so row 0 is the top scanline.
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(40),
                biWidth: WIDTH,
                biHeight: -HEIGHT,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(
            Some(hdc_mem),
            &raw const bmi,
            DIB_RGB_COLORS,
            &raw mut bits,
            None,
            0,
        )
        .ok()?;

        // The mask is ignored for 32bpp alpha icons but ICONINFO still requires one.
        let mask_bytes = vec![0u8; ((WIDTH * HEIGHT) / 8) as usize];
        let hbm_mask = CreateBitmap(WIDTH, HEIGHT, 1, 1, Some(mask_bytes.as_ptr().cast()));

        if hbm_color.is_invalid() || hbm_mask.is_invalid() || bits.is_null() {
            if !hbm_color.is_invalid() {
                let _ = DeleteObject(hbm_color.into());
            }
            if !hbm_mask.is_invalid() {
                let _ = DeleteObject(hbm_mask.into());
            }
            let _ = DeleteDC(hdc_mem);
            return None;
        }

        let pixel_count = (WIDTH * HEIGHT) as usize;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixel_count);
        pixels.fill(0);

        let old_bm = SelectObject(hdc_mem, hbm_color.into());
        let _ = SetBkMode(hdc_mem, TRANSPARENT);

        // ClearType writes subpixel colour fringes that cannot be reduced to a single
        // coverage value; greyscale antialiasing keeps brightness proportional to coverage.
        let font_name: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
        let hfont = CreateFontW(
            -13,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            ANTIALIASED_QUALITY,
            0,
            PCWSTR(font_name.as_ptr()),
        );
        let old_font = SelectObject(hdc_mem, hfont.into());

        // Draw both rows in white; colour is applied per pixel afterwards.
        let _ = SetTextColor(hdc_mem, windows::Win32::Foundation::COLORREF(0x00FF_FFFF));

        let mut rect_down = RECT {
            left: 0,
            top: 0,
            right: WIDTH,
            bottom: HEIGHT / 2,
        };
        let mut down_wide: Vec<u16> = down_text.encode_utf16().chain(Some(0)).collect();
        let down_len = down_wide.len() - 1;
        let _ = DrawTextW(
            hdc_mem,
            &mut down_wide[..down_len],
            &mut rect_down,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );

        let mut rect_up = RECT {
            left: 0,
            top: HEIGHT / 2,
            right: WIDTH,
            bottom: HEIGHT,
        };
        let mut up_wide: Vec<u16> = up_text.encode_utf16().chain(Some(0)).collect();
        let up_len = up_wide.len() - 1;
        let _ = DrawTextW(
            hdc_mem,
            &mut up_wide[..up_len],
            &mut rect_up,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );

        // Detach the DIB before touching its pixels so GDI has flushed its writes.
        let _ = SelectObject(hdc_mem, old_font);
        let _ = DeleteObject(hfont.into());
        let _ = SelectObject(hdc_mem, old_bm as HGDIOBJ);
        let _ = DeleteDC(hdc_mem);

        colourise_coverage(pixels, WIDTH as usize, HEIGHT as usize);

        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let hicon = CreateIconIndirect(&icon_info).ok();

        let _ = DeleteObject(hbm_color.into());
        let _ = DeleteObject(hbm_mask.into());

        hicon
    }
}

/// Turn white-on-transparent glyph coverage into premultiplied coloured pixels.
///
/// Brightness of the rendered pixel is the antialiased coverage of the glyph, which
/// becomes the alpha value. The shell composites tray icons with `AlphaBlend`, so the
/// colour channels are premultiplied by that alpha.
fn colourise_coverage(pixels: &mut [u32], width: usize, height: usize) {
    for y in 0..height {
        let (r, g, b) = if y < height / 2 {
            ICON_DOWN_RGB
        } else {
            ICON_UP_RGB
        };

        for x in 0..width {
            let idx = y * width + x;
            let px = pixels[idx];
            // Top-down BI_RGB pixels are 0x00RRGGBB until we write alpha.
            let coverage = ((px >> 16) & 0xFF).max((px >> 8) & 0xFF).max(px & 0xFF);

            pixels[idx] = if coverage == 0 {
                0
            } else {
                let pm = |c: u32| (c * coverage) / 255;
                (coverage << 24) | (pm(r) << 16) | (pm(g) << 8) | pm(b)
            };
        }
    }
}

#[allow(unsafe_code)]
fn show_context_menu<P: NetworkProvider>(
    hwnd: HWND,
    service: &WslTrafficMonitorService<P>,
    settings: &wsl_traffic_storage::UserSettings,
) {
    let snapshot = service.get_snapshot();
    let download_str = format!(
        "Download: {}",
        format_speed(snapshot.download_speed, settings.speed_unit)
    );
    let upload_str = format!(
        "Upload: {}",
        format_speed(snapshot.upload_speed, settings.speed_unit)
    );
    let status_str = format!("Status: {:?}", snapshot.status);

    unsafe {
        // Safety: Creating context popup menu. Handle is verified and closed by standard TrackPopupMenu flow.
        let Ok(hmenu) = CreatePopupMenu() else {
            return;
        };

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

        let history_wide: Vec<u16> = "View Usage History..."
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_TRAY_HISTORY,
            PCWSTR(history_wide.as_ptr()),
        );

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

        let autostart_text = if settings.run_at_startup {
            "✓ Run at Startup"
        } else {
            "Run at Startup"
        };
        let autostart_wide: Vec<u16> = autostart_text.encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_TRAY_AUTOSTART,
            PCWSTR(autostart_wide.as_ptr()),
        );

        let overlay_text = if settings.show_overlay {
            "✓ Show Floating Overlay"
        } else {
            "Show Floating Overlay"
        };
        let overlay_wide: Vec<u16> = overlay_text.encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_TRAY_TOGGLE_OVERLAY,
            PCWSTR(overlay_wide.as_ptr()),
        );

        let lock_text = if settings.overlay_locked {
            "✓ Lock Overlay Position"
        } else {
            "Lock Overlay Position"
        };
        let lock_wide: Vec<u16> = lock_text.encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_TRAY_LOCK_OVERLAY,
            PCWSTR(lock_wide.as_ptr()),
        );

        let settings_wide: Vec<u16> = "Settings...".encode_utf16().chain(Some(0)).collect();
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_TRAY_SETTINGS,
            PCWSTR(settings_wide.as_ptr()),
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
        let _ = DestroyMenu(hmenu);
    }
}

use crate::UiError;

/// Start the Win32 message loop window and load the tray notification icon.
#[allow(unsafe_code)]
pub fn run_tray_ui<P: NetworkProvider>(
    service: WslTrafficMonitorService<P>,
    settings: wsl_traffic_storage::UserSettings,
) -> Result<(), UiError> {
    let shared_settings = std::sync::Arc::new(std::sync::Mutex::new(settings));

    TRAY_STATE.with(|state| {
        *state.borrow_mut() = Some(Box::new(TrayStateImpl {
            service,
            settings: shared_settings.clone(),
            current_icon: None,
            hwnd_overlay: None,
        }));
    });

    unsafe {
        // Enable Per-Monitor V2 DPI Awareness for crisp, un-scaled text and windows
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );

        // Register TaskbarCreated message to survive Explorer restarts
        let taskbar_msg = RegisterWindowMessageW(w!("TaskbarCreated"));
        WM_TASKBARCREATED.store(taskbar_msg, std::sync::atomic::Ordering::Relaxed);

        // Safety: retrieves module handle for the current running process to register window class.
        let instance = windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null())
            .map_err(|e| UiError::WindowCreationFailed {
                details: format!("GetModuleHandleW failed: {e}"),
            })?;

        let class_name: Vec<u16> = "WSLTrafficMonitorClass"
            .encode_utf16()
            .chain(Some(0))
            .collect();

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(WndProc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hIcon: load_app_icon(instance.into()),
            ..Default::default()
        };

        // Safety: registers class for the utility window.
        let atom = RegisterClassW(&wnd_class);
        if atom == 0 {
            return Err(UiError::WindowClassRegistrationFailed {
                details: "RegisterClassW returned 0".to_string(),
            });
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
        .map_err(|e| UiError::WindowCreationFailed {
            details: format!("{e}"),
        })?;

        // Create the floating display overlay window
        if let Ok(overlay_hwnd) = win_overlay::create_overlay_window(hwnd, &shared_settings) {
            TRAY_STATE.with(|state| {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    if let Some(handler) = guard.as_mut() {
                        handler.set_overlay_hwnd(overlay_hwnd);
                    }
                }
            });
        }

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: load_app_icon(instance.into()),
            ..Default::default()
        };
        let tip_wide: Vec<u16> = "WSL Traffic Monitor starting...".encode_utf16().collect();
        let len = tip_wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);
        nid.szTip[len] = 0;

        // Safety: Shell_NotifyIconW mounts the notification icon in the tray.
        let res = Shell_NotifyIconW(NIM_ADD, &nid);
        if !res.as_bool() {
            return Err(UiError::TrayIconInitializationFailed);
        }

        // Safety: SetTimer schedules 1-second ticks in WndProc timer.
        let timer = SetTimer(Some(hwnd), TRAY_TIMER_ID, 1000, None);
        if timer == 0 {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            return Err(UiError::WindowCreationFailed {
                details: "Failed to create timer".to_string(),
            });
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

fn show_history(hwnd: HWND) {
    let _ = wsl_traffic_storage::flush_history();
    let mut msg = String::new();

    let format_bytes = |bytes: f64| -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;
        const TB: f64 = GB * 1024.0;
        if bytes >= TB {
            format!("{:.2} TB", bytes / TB)
        } else if bytes >= GB {
            format!("{:.2} GB", bytes / GB)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes / MB)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes / KB)
        } else {
            format!("{bytes} B")
        }
    };

    match wsl_traffic_storage::get_daily_history(7) {
        Ok(history) if !history.is_empty() => {
            for (date, up, down) in history {
                let date_str = if date.len() >= 14 {
                    format!("{}-{}-{}", &date[6..10], &date[10..12], &date[12..14])
                } else {
                    date.clone()
                };
                let up_str = format_bytes(up as f64);
                let down_str = format_bytes(down as f64);
                let _ = writeln!(msg, "{date_str}: Up {up_str}, Down {down_str}");
            }
        }
        _ => {
            msg.push_str("No history available yet.");
        }
    }
    show_message_box(hwnd, &msg, "Usage History (Last 7 Days)", false);
}

#[allow(unsafe_code)]
fn open_settings(hwnd: HWND, settings: &wsl_traffic_storage::UserSettings) {
    let _ = wsl_traffic_storage::save_settings(settings);
    if let Some(path) = wsl_traffic_storage::get_settings_path() {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
        let file_wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let op_wide: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        unsafe {
            let _ = ShellExecuteW(
                Some(hwnd),
                PCWSTR(op_wide.as_ptr()),
                PCWSTR(file_wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOW,
            );
        }
    }
}
