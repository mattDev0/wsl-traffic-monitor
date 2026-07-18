# WSL Traffic Monitor

WSL Traffic Monitor is planned as a lightweight native Windows application for measuring WSL2 network traffic and showing real-time upload/download speeds in a tray/taskbar interface.

This repository currently contains the project scaffold and technical research only. Monitoring logic has not been implemented yet.

## Current Status

- Cargo workspace scaffolded.
- Research and architecture documents written.
- CI, formatting, linting, and test configuration added.
- Runtime crates contain placeholders only.

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
