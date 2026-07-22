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
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, FW_BOLD, FillRect, PAINTSTRUCT,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, DefWindowProcW, GetSystemMetrics, GetWindowRect, HTCAPTION, LWA_ALPHA,
    PostMessageW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, SendMessageW,
    SetLayeredWindowAttributes, ShowWindow, WM_DESTROY, WM_ERASEBKGND, WM_EXITSIZEMOVE,
    WM_LBUTTONDOWN, WM_MOVE, WM_NCLBUTTONDOWN, WM_PAINT, WM_RBUTTONUP, WM_USER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};
use wsl_traffic_core::MeasurementConfidence;
use wsl_traffic_core::format_speed_compact;
use wsl_traffic_monitor::{MonitorState, MonitorStateSnapshot};

const OVERLAY_WIDTH: i32 = 148;
const OVERLAY_HEIGHT: i32 = 54;
const WM_OVERLAY_RBUTTONUP: u32 = WM_USER + 2;

thread_local! {
    static OVERLAY_DATA: RefCell<Option<OverlayData>> = const { RefCell::new(None) };
}

pub struct OverlayData {
    pub hwnd_tray: HWND,
    pub snapshot: MonitorStateSnapshot,
    pub settings: wsl_traffic_storage::UserSettings,
}

/// Create the floating overlay window.
#[allow(unsafe_code)]
pub fn create_overlay_window(
    hwnd_tray: HWND,
    settings: &wsl_traffic_storage::UserSettings,
) -> Result<HWND, String> {
    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
        .map_err(|e| format!("Failed to get module handle: {e}"))?;

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

    // Calculate default position (bottom-right above taskbar) if not saved
    let (x, y) = if settings.overlay_x >= 0 && settings.overlay_y >= 0 {
        (settings.overlay_x, settings.overlay_y)
    } else {
        unsafe {
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            (
                screen_w - OVERLAY_WIDTH - 20,
                screen_h - OVERLAY_HEIGHT - 60,
            )
        }
    };

    let hwnd = unsafe {
        windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name,
            w!("WSL Traffic Overlay"),
            WS_POPUP,
            x,
            y,
            OVERLAY_WIDTH,
            OVERLAY_HEIGHT,
            None,
            None,
            Some(windows::Win32::Foundation::HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|e| format!("Failed to create overlay window: {e}"))?;

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
            settings: settings.clone(),
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
pub fn update_overlay_snapshot(
    hwnd: HWND,
    snapshot: MonitorStateSnapshot,
    settings: &wsl_traffic_storage::UserSettings,
) {
    OVERLAY_DATA.with(|data| {
        if let Some(d) = data.borrow_mut().as_mut() {
            d.snapshot = snapshot;
            d.settings = settings.clone();
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
                    .is_some_and(|d| d.settings.overlay_locked)
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

        WM_EXITSIZEMOVE | WM_MOVE => {
            // Save updated window position
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                OVERLAY_DATA.with(|data| {
                    if let Some(d) = data.borrow_mut().as_mut() {
                        d.settings.overlay_x = rect.left;
                        d.settings.overlay_y = rect.top;
                        let _ = wsl_traffic_storage::save_settings(&d.settings);
                    }
                });
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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

        WM_DESTROY => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[allow(unsafe_code)]
fn paint_overlay(_hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    let rect = RECT {
        left: 0,
        top: 0,
        right: OVERLAY_WIDTH,
        bottom: OVERLAY_HEIGHT,
    };

    // Double buffering to eliminate flicker
    unsafe {
        let hdc_mem = CreateCompatibleDC(Some(hdc));
        let hbm_mem = CreateCompatibleBitmap(hdc, OVERLAY_WIDTH, OVERLAY_HEIGHT);
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

            // 1. Render Header (WSL + Badge)
            let font_header_name: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
            let hfont_header = CreateFontW(
                11,
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
                FONT_QUALITY(0),
                0,
                PCWSTR(font_header_name.as_ptr()),
            );
            let old_font = SelectObject(hdc_mem, hfont_header.into());

            let mut header_rect = RECT {
                left: 8,
                top: 4,
                right: OVERLAY_WIDTH - 8,
                bottom: 18,
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
                13,
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
                FONT_QUALITY(0),
                0,
                PCWSTR(font_speed_name.as_ptr()),
            );
            let _ = SelectObject(hdc_mem, hfont_speed.into());
            let _ = DeleteObject(hfont_header.into());

            let down_str = format!("D  {}", format_speed_compact(d.snapshot.download_speed));
            let up_str = format!("U  {}", format_speed_compact(d.snapshot.upload_speed));

            let mut down_rect = RECT {
                left: 8,
                top: 20,
                right: OVERLAY_WIDTH - 8,
                bottom: 35,
            };
            let mut up_rect = RECT {
                left: 8,
                top: 35,
                right: OVERLAY_WIDTH - 8,
                bottom: 50,
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
            OVERLAY_WIDTH,
            OVERLAY_HEIGHT,
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
