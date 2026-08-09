# Windows Smoke-Test & Verification Checklist

This document tracks empirical Windows host verification steps for **WSL Traffic Monitor**.

**Execution record**

| Field | Value |
|---|---|
| Date | 2026-08-09 |
| Verified by | Maintainer, manual operator testing |
| Host | Windows 11, build 10.0.26200.8875 |
| WSL | 2.7.3.0 (kernel 6.6.114.1-1), NAT mode |
| Build | `x86_64-pc-windows-gnu` release, workspace v0.6.0 |
| Displays | Single display — see case 9 |

Rows marked VERIFIED were exercised by hand on the host above. Rows that could not be
exercised say so explicitly rather than being left to imply coverage; none of these are
automated, so a fresh run is required whenever the Win32 layer changes.

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
| **1. Tray Startup** | Launch `wsl-traffic-monitor.exe` from PowerShell or File Explorer. | Tray icon loads in Windows System Tray area showing live speed text. Right-click context menu opens cleanly. | **VERIFIED (maintainer, 2026-08-09)** |
| **2. Overlay Mechanics** | - Drag floating overlay across monitors.<br>- Right-click -> *Lock Overlay Position* and attempt to drag.<br>- Right-click -> *Show Floating Overlay* to toggle visibility. | - Window moves smoothly without lag or paint tearing.<br>- Dragging is disabled when locked.<br>- Overlay hides/shows cleanly. | **VERIFIED (maintainer, 2026-08-09)** — drag across monitors not exercised (single display) |
| **3. Settings Persistence** | Move overlay to $(X, Y)$, change speed unit in `settings.json` or toggle overlay, then restart app. | Overlay restores exact $(X, Y)$ position and visibility preferences from `settings.json`. | **VERIFIED (maintainer, 2026-08-09)** |
| **4. History Persistence** | Run continuous download/upload traffic for 15+ minutes, then check *View Usage History...*. | Hourly and daily byte totals accumulate correctly in `redb` database without locks or corruption. | **VERIFIED (maintainer, 2026-08-09)** |
| **5. WSL Lifecycle** | Run `wsl --shutdown` in PowerShell while app is running, then open WSL. | App transitions cleanly to `Disconnected` / `Offline`, then automatically reclassifies and reconnects when WSL restarts. | **VERIFIED (maintainer, 2026-08-09)** |
| **6. Explorer Restart** | In Task Manager or PowerShell: `Taskkill /F /IM explorer.exe` then `start explorer.exe`. | App handles `TaskbarCreated` message and re-registers tray icon and overlay window smoothly without crashing or leaking GDI objects. | **VERIFIED (maintainer, 2026-08-09)** |
| **7. Read-Only Config Safety** | Set `networkingMode=mirrored` in `%USERPROFILE%\.wslconfig` and restart app. | App detects mode, displays `[UNSUPPORTED]` overlay badge, and leaves `.wslconfig` **100% untouched**. | **VERIFIED (maintainer, 2026-08-09)** |

## Extended Soak & Robustness Matrix

| Test Case | Step / Action | Expected Result | Status |
| :--- | :--- | :--- | :---: |
| **8. Extended Uptime (Multi-Day)** | Run monitor continuously under background WSL sampling. | Memory footprint stays <15MB RSS, CPU <0.1%, 0 handle/GDI leaks. | **PLANNED (CI)** |
| **9. High-DPI Display Scaling** | Move overlay window across monitors with different DPI scales (100% vs 150% vs 200%). | Handles `WM_DPICHANGED` dynamically without clipping or scaling distortion. | **NOT TESTABLE ON THIS HOST** — single display; requires a mixed-DPI multi-monitor setup |
| **10. Clean Exit Mechanics** | Right-click Exit from either the Tray Menu or Overlay Menu. | Utility window, overlay window, menu handles (`DestroyMenu`), and background message pump terminate cleanly without leaking resources. | **VERIFIED (maintainer, 2026-08-09)** |
| **12. Single-Instance Guard** | Launch `wsl-traffic-monitor.exe` twice. | Second process exits silently; exactly one instance in Task Manager and one tray icon. | **VERIFIED (maintainer, 2026-08-09)** — confirmed via Task Manager process list |
| **13. Tray Icon Compositing** | Inspect the tray icon against a light taskbar/flyout background. | Glyphs composite with per-pixel alpha; no opaque background tile. | **VERIFIED (maintainer, 2026-08-09)** — black tile defect found and fixed, see PR #2 |
| **11. CLI Console Attachment** | Run `wsl-traffic-monitor.exe -d` or `wsl-traffic-monitor.exe -j` from Command Prompt. | Attaches to parent console, outputs text/JSON report, and exits cleanly without opening tray UI. | **VERIFIED (maintainer, 2026-08-09)** |

---

## Measured Resource Usage

Observed in Task Manager on 2026-08-09 while the monitor was running with WSL active:

| Metric | Observed | Target | Result |
|---|---|---|---|
| CPU | 0% | < 0.2% | PASS |
| Memory (Task Manager Processes column) | 2.2 MB | < 30 MB | PASS |
| Disk | 0 MB/s | — | — |
| Network | 0 Mbps | — | — |

This closes Hypothesis 4 in [experiment_report.md](./experiment_report.md) for steady-state
operation. Two caveats on how to read it: Task Manager rounds CPU to whole percent, so 0%
establishes "below rounding resolution" rather than a precise figure; and this is an
instantaneous reading, not an average over a long session. Sustained behaviour and any
handle or GDI growth remain the subject of case 8.

For scale, `VmmemWSL` was using 3,349.9 MB on the same host at the same moment.

---

## Lifecycle & GDI Resource Audit Notes

1. **GDI Objects & Menu Handles**: Double-buffering in `win_tray.rs` and `win_overlay.rs` explicitly calls `DeleteObject` and `DeleteDC` for all created brushes, fonts, bitmaps, and DCs per paint pass. `show_context_menu` explicitly calls `DestroyMenu(hmenu)` after invocation to prevent menu handle leaks.
2. **Win32 Message Routing**: Overlay window forwards `WM_RBUTTONUP` to the tray parent window via `WM_OVERLAY_RBUTTONUP` custom message to maintain unified menu control.
3. **Explorer Recovery**: Tray window registers `TaskbarCreated` message via `RegisterWindowMessageW(w!("TaskbarCreated"))` to re-mount icon upon shell crash.
4. **Window Exit Loop & Single Instance**: `WM_DESTROY` calls `PostQuitMessage(0)` and cleans up thread-local `OVERLAY_DATA` and tray handles to ensure standard Win32 message loop termination. Windows main entry point enforces a single-instance named Mutex (`Local\WslTrafficMonitorSingleInstanceMutex`).
