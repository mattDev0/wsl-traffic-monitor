# Changelog

All notable changes to this project will be documented in this file.

## [0.5.0] - 2026-07-24

### Added
- Implemented **Typed Error Architecture**: Replaced generic `Result<_, String>` with strongly-typed `thiserror` enums across 6 workspace crates (`WindowsError`, `WslError`, `StorageError`, `MonitorError`, `DiagnosticsError`, `UiError`).
- Implemented **Per-Monitor V2 High-DPI Awareness**: Added `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` support so floating overlays, context menus, and taskbar UI render at 100% crisp native display resolution without bitmap blurriness.
- Implemented **ClearType Subpixel Text Rendering**: Upgraded GDI font creation across overlay and tray icon rendering to use `CLEARTYPE_QUALITY` with point-scaled `Segoe UI` fonts.
- Implemented **Silent Windowless Application Subsystem**: Configured `#![windows_subsystem = "windows"]` so the application launches silently into the background without displaying a command prompt window, while preserving parent console attachment for CLI flags (`-d`, `-j`, `-h`).

### Fixed
- Fixed an issue where closing the application via the floating overlay context menu resulted in an orphaned window stuck at `0/0` throughput by posting `WM_QUIT` during `WM_DESTROY`.
- Corrected workspace `Cargo.toml` license metadata from `MIT OR Apache-2.0` to `GPL-3.0-or-later` to match root `LICENSE` file.

## [0.4.0] - 2026-07-21

### Added
- Implemented **Glanceable Floating Display Overlay**: Compact, borderless, dark glassmorphic card showing real-time upload/download throughput and confidence status (`[NAT: HIGH]`, `[NAT: MED]`, `[UNSUPPORTED]`).
- Added **Overlay Interactions**: Draggable window placement with position persistence (`overlay_x`, `overlay_y`) saved to `settings.json`.
- Added **Overlay Controls**: Toggle Show/Hide Floating Overlay and Lock Overlay Position via system tray and overlay right-click context menus.
- Added **Phase 3 Read-Only Safety**: Explicit detection of `mirrored`, `virtioproxy`, and `none` modes in `.wslconfig`, transitioning the engine to `MonitorState::UnsupportedNetworkingMode` with a visual UI warning instead of silent failure or config mutation.

### Fixed
- Fixed an issue where the overlay window handle was not properly bound to `TrayStateImpl`, preventing timer updates and toggle commands.

## [0.3.0] - 2026-07-20

### Added
- Implemented **Dynamic Tray Icon** to render real-time upload and download speeds directly in the taskbar via GDI.
- Added **View Usage History** dialog to the system tray context menu, exposing the `redb` hourly/daily history backend.
- Added **Settings...** option to the tray menu to quickly launch and edit `settings.json`.
- Implemented phase 0 validation experiments for polling overhead and traffic mapping.

### Fixed
- Fixed an issue where the tray UI could crash due to `RefCell` re-entrancy when opening the context menu.
- Fixed a bug where `wsl.exe` discovery failed to capture `stdout`, incorrectly transitioning the monitor to a "Disconnected" state.
- Deduplicated `format_speed()` functions by unifying them in `wsl-traffic-core`.

## [0.2.0] - 2026-07-19

### Added
- Implemented **Counter Sampler** (Phase 1).
- Added `get_interface_counters` in `wsl-traffic-windows` to poll raw counters of a single adapter by LUID using `GetIfEntry2`.
- Added stateful `WslTrafficMonitor` managing transition between `Active` and `Disconnected` states.
- Implemented automatic interface reclassification to dynamically transition between adapters and update confidence levels when topology changes, adapters appear/disappear, or WSL starts/stops.
- Implemented counter reset and sleep/resume protection preventing delta spikes.
- Added a mockable `NetworkProvider` trait for 100% deterministic test coverage of all connection, reset, and rate scenarios.
- Added a drop-safe background worker `ActiveMonitor` to run the sampling loop.
- Added thread-safe `WslTrafficMonitorService` running the counter sampler in a background thread and exposing real-time query snapshots.
- Implemented native Windows system tray UI displaying real-time upload and download speeds, subscribed to the monitor service.
- Added detailed error message tracking and measurement confidence levels dynamically throughout discovery and query operations.
- Added service lifecycle unit tests validating background ticks, snapshot queries, and double-start prevention.

## [0.1.0] - 2026-07-18

### Added
- Completed **Phase 0: Validation Harness**.
- Implemented **Adapter Inventory** using Win32 IP Helper API (`GetAdaptersAddresses`/`GetIfEntry2`) to retrieve adapter LUID, index, GUID, addresses, MAC, MTU, speed, and raw byte/packet counters.
- Implemented **WSL Detection** parsing user `.wslconfig` files and executing `wsl.exe` to inspect distros/versions (with non-blocking registry fallbacks).
- Implemented **Docker Desktop Detection** to identify Docker installations, WSL distros (`docker-desktop`), and running host processes (e.g. `vpnkit.exe`).
- Implemented **Candidate Classification** scoring and ranking algorithm matching adapters to WSL NAT and mirrored modes.
- Implemented **Diagnostics Report** generating text-formatted terminal reports and structured JSON exports.
- Configured Linux cross-compilation target `x86_64-pc-windows-gnu` in workspace and verified build success.
