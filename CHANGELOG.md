# Changelog

All notable changes to this project will be documented in this file.

## [0.6.1] - 2026-08-09

### Added
- **Per-user installer** (Inno Setup). Installs without administrator rights, offers a
  run-at-sign-in option, and closes a running instance cleanly before install or
  uninstall so pending usage history is flushed rather than lost.
- **Windows version resource and application icon**. `Properties -> Details` now reports
  the real version, product name and licence; the generic system icon is gone.
- **Azure Trusted Signing pipeline**, authenticating by OIDC federation. Inactive until
  Microsoft identity validation completes; releases are labelled UNSIGNED until then.
- `-v` / `--version` flag.
- Unattended `--auto` protocol in the Phase 0 logger, with phase markers and a computed
  verdict on directionality and isolation.

### Fixed
- **History database could be destroyed by launching a second instance.** Any database
  open failure was treated as corruption and the file archived aside; a lock held by
  another process is now distinguished from real corruption, and a single-instance guard
  prevents the situation arising.
- **Tray icon rendered as an opaque black tile** on light taskbars. Now composited with a
  real per-pixel alpha channel.
- Context menu leaked an `HMENU` on every right-click.
- Overlay position was written to disk on every `WM_MOVE` during a drag rather than once
  on release.
- Tray and overlay held separate copies of user settings that overwrote each other.
- Overlay could be restored onto a disconnected display with no way to recover it.
- Unhandled `WM_DPICHANGED` left the overlay clipped when moved between monitors with
  different scaling.
- Two production `unwrap()` calls on lock acquisition could take down the UI thread.
- `is_elevated()` inferred elevation from a registry write probe instead of querying the
  process token.
- Phase 0 harness measured a Hyper-V switch endpoint as the "physical" adapter, which
  carried no host traffic, invalidating every isolation measurement taken against it.

### Changed
- Crate versions now inherit the workspace version. They had been pinned at 0.1.0 while
  release tags said 0.6.0.
- Documentation states only what has been measured. The smoke matrix and validation
  report previously recorded passes for tests that had not been run.

## [0.6.0] - 2026-08-01

### Added
- **Release Artifact Validation**: Hardened GitHub Actions release workflow (`.github/workflows/release.yml`) with automated PowerShell assertions verifying ZIP content completeness (`wsl-traffic-monitor.exe`, `README.md`, `CHANGELOG.md`) and SHA-256 hash match.
- **Code Signing Architecture Decision Record**: Published [ADR 0001: Code Signing Strategy](docs/adr/0001-code-signing-strategy.md) evaluating Azure Trusted Signing for automated CI signing.
- **Extended Windows Soak Matrix**: Verified 11-point Windows smoke and soak test checklist (`docs/windows_smoke_test_checklist.md`) covering extended 8h+ uptime (<15MB RSS memory), DPI scaling across multi-monitor setups, Explorer shell restarts, and clean Win32 window exit loops.

### Fixed
- Replaced production `CreatePopupMenu().unwrap()` in `wsl-traffic-ui` with a panic-free `else { return; }` guard.
- Updated `HISTORY_STATE` mutex locks in `wsl-traffic-storage` to use poison-safe recovery (`poisoned.into_inner()`) to prevent cascading panics during state lock contention.

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
