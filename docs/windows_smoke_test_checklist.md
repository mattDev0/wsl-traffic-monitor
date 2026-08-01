# Windows Smoke-Test & Verification Checklist

This document tracks empirical Windows host verification steps for **WSL Traffic Monitor** before advancing to Phase 5 release packaging.

---

## Smoke-Test Execution Protocol

Run the application executable on a Windows host:
```powershell
.\wsl-traffic-monitor.exe
```

---

## Verification Matrix

| Test Case | Step / Action | Expected Result | Status |
| :--- | :--- | :--- | :---: |
| **1. Tray Startup** | Launch `wsl-traffic-monitor.exe` from PowerShell or File Explorer. | Tray icon loads in Windows System Tray area showing live speed text. Right-click context menu opens cleanly. | **PASS** |
| **2. Overlay Mechanics** | - Drag floating overlay across monitors.<br>- Right-click -> *Lock Overlay Position* and attempt to drag.<br>- Right-click -> *Show Floating Overlay* to toggle visibility. | - Window moves smoothly without lag or paint tearing.<br>- Dragging is disabled when locked.<br>- Overlay hides/shows cleanly. | **PASS** |
| **3. Settings Persistence** | Move overlay to $(X, Y)$, change speed unit in `settings.json` or toggle overlay, then restart app. | Overlay restores exact $(X, Y)$ position and visibility preferences from `settings.json`. | **PASS** |
| **4. History Persistence (10-30m)** | Run continuous download/upload traffic for 15+ minutes, then check *View Usage History...*. | Hourly and daily byte totals accumulate correctly in `redb` database without locks or corruption. | **PASS** |
| **5. WSL Lifecycle** | Run `wsl --shutdown` in PowerShell while app is running, then open WSL. | App transitions cleanly to `Disconnected` / `Offline`, then automatically reclassifies and reconnects when WSL restarts. | **PASS** |
| **6. Explorer Restart** | In Task Manager or PowerShell: `Taskkill /F /IM explorer.exe` then `start explorer.exe`. | App handles `TaskbarCreated` message and re-registers tray icon and overlay window smoothly without crashing or leaking GDI objects. | **PASS** |
| **7. Read-Only Config Safety** | Set `networkingMode=mirrored` in `%USERPROFILE%\.wslconfig` and restart app. | App detects mode, displays `[UNSUPPORTED]` overlay badge, and leaves `.wslconfig` **100% untouched**. | **PASS** |

## Extended Soak & Robustness Matrix

| Test Case | Step / Action | Expected Result | Status |
| :--- | :--- | :--- | :---: |
| **8. Extended Uptime (8h+)** | Run monitor continuously overnight under background WSL sampling. | Memory footprint stays <15MB RSS, CPU <0.1%, 0 handle/GDI leaks. | **PASS** |
| **9. High-DPI Display Scaling** | Move overlay window across monitors with different DPI scales (100% vs 150% vs 200%). | Window dimensions, text padding, and Segoe UI ClearType fonts scale dynamically without blurriness. | **PASS** |
| **10. Clean Exit Mechanics** | Right-click Exit from either the Tray Menu or Overlay Menu. | Utility window, overlay window, and background message pump terminate cleanly without leaving orphaned processes or stuck 0/0 windows. | **PASS** |
| **11. CLI Console Attachment** | Run `wsl-traffic-monitor.exe -d` or `wsl-traffic-monitor.exe -j` from Command Prompt. | Attaches to parent console, outputs text/JSON report, and exits cleanly without opening tray UI. | **PASS** |

---

## Lifecycle & GDI Resource Audit Notes

1. **GDI Objects**: Double-buffering in `win_tray.rs` and `win_overlay.rs` explicitly calls `DeleteObject` and `DeleteDC` for all created brushes, fonts, bitmaps, and DCs per paint pass.
2. **Win32 Message Routing**: Overlay window forwards `WM_RBUTTONUP` to the tray parent window via `WM_OVERLAY_RBUTTONUP` custom message to maintain unified menu control.
3. **Explorer Recovery**: Tray window registers `TaskbarCreated` message via `RegisterWindowMessageW(w!("TaskbarCreated"))` to re-mount icon upon shell crash.
4. **Window Exit Loop**: `WM_DESTROY` calls `PostQuitMessage(0)` and cleans up thread-local `OVERLAY_DATA` and tray handles to ensure standard Win32 message loop termination.
