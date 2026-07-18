# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-07-18

### Added
- Completed **Phase 0: Validation Harness**.
- Implemented **Adapter Inventory** using Win32 IP Helper API (`GetAdaptersAddresses`/`GetIfEntry2`) to retrieve adapter LUID, index, GUID, addresses, MAC, MTU, speed, and raw byte/packet counters.
- Implemented **WSL Detection** parsing user `.wslconfig` files and executing `wsl.exe` to inspect distros/versions (with non-blocking registry fallbacks).
- Implemented **Docker Desktop Detection** to identify Docker installations, WSL distros (`docker-desktop`), and running host processes (e.g. `vpnkit.exe`).
- Implemented **Candidate Classification** scoring and ranking algorithm matching adapters to WSL NAT and mirrored modes.
- Implemented **Diagnostics Report** generating text-formatted terminal reports and structured JSON exports.
- Configured Linux cross-compilation target `x86_64-pc-windows-gnu` in workspace and verified build success.
