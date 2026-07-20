# Changelog

All notable changes to this project will be documented in this file.

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
