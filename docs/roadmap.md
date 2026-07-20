# Roadmap

## Phase 0: Validation Harness

- [x] Scaffold the workspace and CI.
- [x] Build a CLI diagnostic harness for adapter inventory and counter experiments.
- [ ] Validate WSL NAT-mode adapter selection.
- [ ] Validate upload/download direction mapping.
- [ ] Measure polling overhead.

## Phase 1: MVP

- [ ] Implement WSL NAT-mode interface-counter monitoring.
- [ ] Add native Windows tray/taskbar display.
- [ ] Add measurement confidence reporting.
- [ ] Add user settings for units and poll interval.

## Phase 2: History and Detection

- [ ] Add rolling usage history.
- [ ] Improve adapter scoring.
- [ ] Detect Docker Desktop interference.
- [ ] Add network-change and WSL lifecycle handling.
- [ ] Add diagnostics export.

## Phase 3: Advanced Networking Modes

- [ ] Experiment with mirrored networking attribution.
- [ ] Experiment with VirtioProxy fallback behavior.
- [ ] Evaluate ETW/WFP validation paths.

## Phase 4: Optional Guest Attribution

- [ ] Prototype a Linux-side helper.
- [ ] Evaluate per-distro and per-process feasibility.
- [ ] Define the helper protocol and permissions model.

## Phase 5: v1.0

- [ ] Harden installer/autostart behavior.
- [ ] Finalize storage migrations.
- [ ] Finalize plugin architecture.
- [ ] Add signed release pipeline.
