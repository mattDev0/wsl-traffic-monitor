//! Floating display overlay window implementation for Windows.
#![cfg(windows)]
#![allow(non_snake_case)]
#![allow(
    clippy::borrow_as_ptr,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::match_same_arms,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use std::cell::RefCell;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush, DT_LEFT,
    DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FW_BOLD, FillRect, MONITOR_DEFAULTTONULL,
    MonitorFromRect, PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, DefWindowProcW, GetSystemMetrics, GetWindowRect, HTCAPTION,
    HWND_TOPMOST, LWA_ALPHA, PostMessageW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN,
    SPI_GETWORKAREA, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SendMessageW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, SystemParametersInfoW, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND,
    WM_EXITSIZEMOVE, WM_LBUTTONDOWN, WM_NCLBUTTONDOWN, WM_PAINT, WM_RBUTTONUP, WM_USER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};
use wsl_traffic_core::MeasurementConfidence;
use wsl_traffic_core::format_speed_compact;
use wsl_traffic_monitor::{MonitorState, MonitorStateSnapshot};

const BASE_OVERLAY_WIDTH: i32 = 160;
const BASE_OVERLAY_HEIGHT: i32 = 58;
const WM_OVERLAY_RBUTTONUP: u32 = WM_USER + 2;

thread_local! {
    static OVERLAY_DATA: RefCell<Option<OverlayData>> = const { RefCell::new(None) };
}

use std::sync::{Arc, Mutex};

pub struct OverlayData {
    pub hwnd_tray: HWND,
    pub snapshot: MonitorStateSnapshot,
    pub settings: Arc<Mutex<wsl_traffic_storage::UserSettings>>,
}

use crate::UiError;

#[allow(unsafe_code)]
fn get_dpi_for_window(hwnd: HWND) -> u32 {
    unsafe {
        let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd);
        if dpi == 0 { 96 } else { dpi }
    }
}

/// Create the floating overlay window.
#[allow(unsafe_code)]
pub fn create_overlay_window(
    hwnd_tray: HWND,
    shared_settings: &Arc<Mutex<wsl_traffic_storage::UserSettings>>,
) -> Result<HWND, UiError> {
    let settings = shared_settings
        .lock()
        .map_err(|_| UiError::WindowCreationFailed {
            details: "Failed to lock settings".to_string(),
        })?
        .clone();
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
        .map_err(|e| UiError::WindowCreationFailed {
            details: format!("GetModuleHandleW failed: {e}"),
        })?;

    let class_name = w!("WSLTrafficOverlayClass");

    let wnd_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(WndProcOverlay),
        hInstance: instance.into(),
        lpszClassName: class_name,
        ..Default::default()
    };

    unsafe {
        RegisterClassW(&wnd_class);
    }

    let dpi = unsafe {
        let d = windows::Win32::UI::HiDpi::GetDpiForSystem();
        if d == 0 { 96 } else { d }
    };

    let overlay_w = ((BASE_OVERLAY_WIDTH * dpi as i32) + 48) / 96;
    let overlay_h = ((BASE_OVERLAY_HEIGHT * dpi as i32) + 48) / 96;

    // Calculate default position (bottom-right above taskbar) using work area
    let work_area = unsafe {
        let mut work_rect = RECT::default();
        if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(std::ptr::from_mut(&mut work_rect).cast()),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            work_rect
        } else {
            RECT {
                left: 0,
                top: 0,
                right: GetSystemMetrics(SM_CXSCREEN),
                bottom: GetSystemMetrics(SM_CYSCREEN),
            }
        }
    };

    let default_x = work_area.right - overlay_w - 20;
    let default_y = work_area.bottom - overlay_h - 20;

    let (x, y) = if settings.overlay_x >= 0 && settings.overlay_y >= 0 {
        let check_rect = RECT {
            left: settings.overlay_x,
            top: settings.overlay_y,
            right: settings.overlay_x + overlay_w,
            bottom: settings.overlay_y + overlay_h,
        };
        let hmon = unsafe { MonitorFromRect(&check_rect, MONITOR_DEFAULTTONULL) };
        if hmon.0.is_null() {
            (default_x, default_y)
        } else {
            (settings.overlay_x, settings.overlay_y)
        }
    } else {
        (default_x, default_y)
    };

    let hwnd = unsafe {
        windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name,
            w!("WSL Traffic Overlay"),
            WS_POPUP,
            x,
            y,
            overlay_w,
            overlay_h,
            None,
            None,
            Some(windows::Win32::Foundation::HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|e| UiError::WindowCreationFailed {
        details: format!("Failed to create overlay window: {e}"),
    })?;

    // Store state
    OVERLAY_DATA.with(|data| {
        *data.borrow_mut() = Some(OverlayData {
            hwnd_tray,
            snapshot: MonitorStateSnapshot {
                download_speed: 0.0,
                upload_speed: 0.0,
                status: MonitorState::Disconnected,
                confidence: MeasurementConfidence::Unsupported,
                error_state: None,
            },
            settings: shared_settings.clone(),
        });
    });

    // Set initial opacity (0-255 scale from percentage)
    let alpha = ((u32::from(settings.overlay_opacity) * 255) / 100) as u8;
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }

    if settings.show_overlay {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }

    Ok(hwnd)
}

/// Update telemetry and trigger repaint for the overlay window.
#[allow(unsafe_code)]
pub fn update_overlay_snapshot(hwnd: HWND, snapshot: MonitorStateSnapshot) {
    OVERLAY_DATA.with(|data| {
        if let Some(d) = data.borrow_mut().as_mut() {
            d.snapshot = snapshot;
        }
    });

    unsafe {
        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
    }
}

/// Set overlay visibility.
#[allow(unsafe_code)]
pub fn set_overlay_visible(hwnd: HWND, visible: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
    }
}

/// Set overlay opacity (50-100%).
#[allow(dead_code, unsafe_code)]
pub fn set_overlay_opacity(hwnd: HWND, opacity: u8) {
    let alpha = ((u32::from(opacity) * 255) / 100) as u8;
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

#[allow(unsafe_code)]
unsafe extern "system" fn WndProcOverlay(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            let locked = OVERLAY_DATA.with(|data| {
                data.borrow()
                    .as_ref()
                    .and_then(|d| d.settings.lock().ok().map(|s| s.overlay_locked))
                    .unwrap_or(false)
            });

            if !locked {
                unsafe {
                    let _ = ReleaseCapture();
                    let _ = SendMessageW(
                        hwnd,
                        WM_NCLBUTTONDOWN,
                        Some(WPARAM(HTCAPTION as usize)),
                        Some(LPARAM(0)),
                    );
                }
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_EXITSIZEMOVE => {
            // Save updated window position when user finishes dragging
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                OVERLAY_DATA.with(|data| {
                    if let Some(d) = data.borrow_mut().as_mut() {
                        if let Ok(mut s) = d.settings.lock() {
                            s.overlay_x = rect.left;
                            s.overlay_y = rect.top;
                            let _ = wsl_traffic_storage::save_settings(&s);
                        }
                    }
                });
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_DPICHANGED => {
            let suggested_rect = unsafe { *(lparam.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    suggested_rect.left,
                    suggested_rect.top,
                    suggested_rect.right - suggested_rect.left,
                    suggested_rect.bottom - suggested_rect.top,
                    SWP_NOACTIVATE,
                );
            }
            LRESULT(0)
        }

        WM_RBUTTONUP => {
            // Forward right click to tray context menu handler
            let hwnd_tray = OVERLAY_DATA.with(|data| data.borrow().as_ref().map(|d| d.hwnd_tray));

            if let Some(tray_hwnd) = hwnd_tray {
                unsafe {
                    let _ =
                        PostMessageW(Some(tray_hwnd), WM_OVERLAY_RBUTTONUP, WPARAM(0), LPARAM(0));
                }
            }
            LRESULT(0)
        }

        WM_ERASEBKGND => LRESULT(1), // Prevent flickering

        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

            if !hdc.0.is_null() {
                paint_overlay(hwnd, hdc);
                unsafe {
                    let _ = EndPaint(hwnd, &ps);
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            OVERLAY_DATA.with(|data| {
                *data.borrow_mut() = None;
            });
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[allow(unsafe_code)]
fn paint_overlay(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    let mut rect = RECT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return;
    }

    let dpi = get_dpi_for_window(hwnd);

    // Double buffering to eliminate flicker
    unsafe {
        let hdc_mem = CreateCompatibleDC(Some(hdc));
        let hbm_mem = CreateCompatibleBitmap(hdc, width, height);
        let old_bm = SelectObject(hdc_mem, hbm_mem.into());

        // Background brush - sleek dark glass card (RGB: 20, 24, 32 -> 0x00201814 in COLORREF)
        let bg_brush = CreateSolidBrush(COLORREF(0x00201814));
        let _ = FillRect(hdc_mem, &rect, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        let _ = SetBkMode(hdc_mem, TRANSPARENT);

        // Fetch telemetry & settings snapshot
        OVERLAY_DATA.with(|data| {
            let guard = data.borrow();
            let Some(d) = guard.as_ref() else { return };

            let (badge_text, badge_color) = match d.snapshot.status {
                MonitorState::Active => match d.snapshot.confidence {
                    MeasurementConfidence::High => ("WSL  [NAT: HIGH]", COLORREF(0x0050E3C2)), // Soft Cyan / Emerald Green
                    MeasurementConfidence::Medium => ("WSL  [NAT: MED]", COLORREF(0x0000C8FF)), // Amber / Yellow
                    _ => ("WSL  [NAT: LOW]", COLORREF(0x00808080)),
                },
                MonitorState::UnsupportedNetworkingMode => {
                    ("WSL  [UNSUPPORTED]", COLORREF(0x005050FF))
                } // Red
                MonitorState::Disconnected => ("WSL  [OFFLINE]", COLORREF(0x00808080)), // Muted Gray
            };

            // Dynamic font height scaling using standard GDI MulDiv: - (pt * dpi) / 72
            let header_font_h = -(((9 * dpi as i32) + 36) / 72);
            let speed_font_h = -(((11 * dpi as i32) + 36) / 72);

            // 1. Render Header (WSL + Badge)
            let font_header_name: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
            let hfont_header = CreateFontW(
                header_font_h,
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
                windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY,
                0,
                PCWSTR(font_header_name.as_ptr()),
            );
            let old_font = SelectObject(hdc_mem, hfont_header.into());

            let pad_x = ((8 * dpi as i32) + 48) / 96;
            let header_h = ((16 * dpi as i32) + 48) / 96;
            let row_h = ((18 * dpi as i32) + 48) / 96;

            let mut header_rect = RECT {
                left: pad_x,
                top: 4,
                right: width - pad_x,
                bottom: 4 + header_h,
            };
            let _ = SetTextColor(hdc_mem, badge_color);
            let mut header_wide: Vec<u16> = badge_text.encode_utf16().chain(Some(0)).collect();
            let len = header_wide.len() - 1;
            let _ = DrawTextW(
                hdc_mem,
                &mut header_wide[..len],
                &mut header_rect,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            // 2. Render Speeds (Download Cyan, Upload Amber)
            let font_speed_name: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
            let hfont_speed = CreateFontW(
                speed_font_h,
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
                windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY,
                0,
                PCWSTR(font_speed_name.as_ptr()),
            );
            let _ = SelectObject(hdc_mem, hfont_speed.into());
            let _ = DeleteObject(hfont_header.into());

            let down_str = format!("D  {}", format_speed_compact(d.snapshot.download_speed));
            let up_str = format!("U  {}", format_speed_compact(d.snapshot.upload_speed));

            let mut down_rect = RECT {
                left: pad_x,
                top: 4 + header_h + 2,
                right: width - pad_x,
                bottom: 4 + header_h + 2 + row_h,
            };
            let mut up_rect = RECT {
                left: pad_x,
                top: 4 + header_h + 2 + row_h,
                right: width - pad_x,
                bottom: 4 + header_h + 2 + (row_h * 2),
            };

            // Cyan for download (0x00FFDC00)
            let _ = SetTextColor(hdc_mem, COLORREF(0x00FFDC00));
            let mut down_wide: Vec<u16> = down_str.encode_utf16().chain(Some(0)).collect();
            let down_len = down_wide.len() - 1;
            let _ = DrawTextW(
                hdc_mem,
                &mut down_wide[..down_len],
                &mut down_rect,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            // Orange/Amber for upload (0x0000D0FF)
            let _ = SetTextColor(hdc_mem, COLORREF(0x0000D0FF));
            let mut up_wide: Vec<u16> = up_str.encode_utf16().chain(Some(0)).collect();
            let up_len = up_wide.len() - 1;
            let _ = DrawTextW(
                hdc_mem,
                &mut up_wide[..up_len],
                &mut up_rect,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER,
            );

            let _ = SelectObject(hdc_mem, old_font);
            let _ = DeleteObject(hfont_speed.into());
        });

        // Copy bit block to screen
        let _ = windows::Win32::Graphics::Gdi::BitBlt(
            hdc,
            0,
            0,
            width,
            height,
            Some(hdc_mem),
            0,
            0,
            SRCCOPY,
        );

        let _ = SelectObject(hdc_mem, old_bm);
        let _ = DeleteObject(hbm_mem.into());
        let _ = DeleteDC(hdc_mem);
    }
}
