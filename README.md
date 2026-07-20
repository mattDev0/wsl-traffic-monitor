# WSL Traffic Monitor

WSL Traffic Monitor is a lightweight native Windows application for measuring WSL2 network traffic and showing real-time upload/download speeds in a tray/taskbar interface.

This repository contains a full **Phase 1 & 2 Prototype**. It includes the core NAT-mode traffic sampler, a native Windows system tray UI, rolling usage history (`redb` backed), and a diagnostic harness that handles WSL adapter discovery and Docker Desktop detection.

### Known Limitations (Phase 2)
- **Windows Smoke-Testing**: The tray icon and history persistence are feature-complete prototypes. Rigorous validation for long-running uptime, Explorer restarts, and Windows network-change events is pending.
- **Docker Desktop**: Docker installations and WSL backend VMs are detected to improve confidence scoring, but their NAT traffic is currently blended with regular WSL traffic and is not explicitly separated.

## Current Status

- **Phase 0, 1 & 2 (Completed)**: Fully native Windows tray application running in the background. It monitors WSL NAT traffic, records rolling history in a `redb` database, and offers a right-click UI for diagnostics, history, and settings.
- **Phase 3 (In Development)**: We are actively researching and preparing to support WSL's `mirrored` networking mode and VirtioProxy fallback behavior.
- **Cross-Compilation (Setup)**: Verified target compilation for both Linux and Windows (using `x86_64-pc-windows-gnu`).
- **Tests & Lints (Clean)**: All unit tests pass, and codebase is free of clippy/rustc warnings.

## Running the Validation Harness

To cross-compile the validation harness for Windows from a Linux environment:
```bash
cargo build --target x86_64-pc-windows-gnu
```
The binary will be generated at `target/x86_64-pc-windows-gnu/debug/wsl-traffic-monitor.exe`.

To run the diagnostic utility on a Windows host:
```cmd
# Print human-readable report:
wsl-traffic-monitor.exe

# Export report in structured JSON format:
wsl-traffic-monitor.exe --json
```

## Workspace Layout

- `apps/wsl-traffic-monitor`: placeholder desktop application entry point.
- `crates/wsl-traffic-core`: shared domain types and constants.
- `crates/wsl-traffic-windows`: future Windows API boundary.
- `crates/wsl-traffic-wsl`: future WSL discovery/config boundary.
- `crates/wsl-traffic-monitor`: future sampling/classification orchestration.
- `crates/wsl-traffic-ui`: future tray/taskbar UI boundary.
- `crates/wsl-traffic-storage`: future history storage boundary.
- `crates/wsl-traffic-diagnostics`: future diagnostics report boundary.
- `crates/wsl-traffic-agent-protocol`: future protocol shared with optional WSL guest helpers.

## Development

Required:

- Rust stable, 1.85 or newer.

Useful commands:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo doc --workspace --no-deps`

## Documentation

- [Research](docs/research.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Changelog](CHANGELOG.md)
