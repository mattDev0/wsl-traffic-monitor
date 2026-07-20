# WSL Traffic Monitor

WSL Traffic Monitor is a lightweight native Windows application for measuring WSL2 network traffic and showing real-time upload/download speeds in a tray/taskbar interface.

This repository currently contains **Phase 0: Validation Harness**, a CLI diagnostics tool designed to query host network adapters, parse `.wslconfig` and WSL installations, check Docker Desktop backend processes, and classify/score each adapter for WSL monitoring suitability.

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
