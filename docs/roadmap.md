# Roadmap

## Phase 0: Validation Harness

- [x] Scaffold the workspace and CI.
- [x] Build a CLI diagnostic harness for adapter inventory and counter experiments.
- [x] Validate WSL NAT-mode adapter selection.
- [x] Validate upload/download direction mapping.
- [x] Measure polling overhead.

## Phase 1: MVP

- [x] Implement WSL NAT-mode interface-counter monitoring.
- [x] Add native Windows tray/taskbar display.
- [x] Add measurement confidence reporting.
- [x] Add user settings for units and poll interval.

## Phase 2: History and Detection

- [x] Add rolling usage history.
- [x] Improve adapter scoring.
- [x] Detect Docker Desktop interference.
- [x] Add network-change and WSL lifecycle handling.
- [x] Add diagnostics export.

## Phase 3: Advanced Networking Modes

- [x] Identify mirrored networking and VirtioProxy behaviors.
- [x] Update monitor to explicitly detect and reject mirrored/VirtioProxy/none modes.
- [x] Provide graceful read-only host-side fallbacks (Unsupported Networking Mode state).

## Phase 4: Optional Guest Attribution

- [ ] Prototype a Linux-side helper.
- [ ] Evaluate per-distro and per-process feasibility.
- [ ] Define the helper protocol and permissions model.

## Phase 5: v0.6 Stabilization Milestone

- [x] Reconcile status wording across `README.md`, `docs/roadmap.md`, and `CHANGELOG.md` for **v0.6.0**.
- [x] Audit production `unwrap()` calls and add poison-safe mutex lock recovery.
- [x] Execute full Windows soak & smoke test matrix (`docs/windows_smoke_test_checklist.md`).
- [x] Harden GitHub Actions release workflow artifact validation.
- [x] Publish Code-Signing Architecture Decision Record (`docs/adr/0001-code-signing-strategy.md`).

## Phase 6: v1.0 Release Sign-off

- [x] Harden autostart behavior (`is_autostart_enabled` query & path double-quoting).
- [x] Add storage recovery policy (`redb` timestamped corruption fallback).
- [x] Define plugin architecture design (`docs/plugin_architecture.md`).
- [x] Implement GitHub Actions release build & SHA-256 checksum pipeline (`.github/workflows/release.yml`).
- [ ] Implement Authenticode / Azure Trusted Signing code-signing pipeline.
