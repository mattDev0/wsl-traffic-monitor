# WSL Traffic Monitor Architecture

Status: proposed architecture  
Date: 2026-07-18  
Implementation status: Phase 0 (Validation Harness) implemented

## Product Goal

WSL Traffic Monitor is a lightweight native Windows desktop application that measures only WSL2 network traffic and displays real-time upload/download speeds in a tray/taskbar interface with minimal CPU and memory use.

The design prioritizes correctness over pretending that all WSL networking modes are equally observable. When traffic cannot be isolated, the app must say so.

## Recommended Architecture

### Core Principle

Use the cheapest reliable measurement point first:

- Primary data path: Windows interface counters for the WSL virtual networking path.
- Secondary diagnostics: WSL state, WSL config, adapter metadata, and Docker Desktop detection.
- Experimental attribution: ETW/WFP/Linux guest agents only after MVP validation.

### High-Level Components

- `wsl-detect`: discovers WSL installation, distro list, WSL versions, networking mode, and whether WSL2 is active.
- `net-enum`: enumerates Windows network interfaces and stable IDs using IP Helper API concepts: LUID, interface index, alias, description, GUID, operational state, link speed, addresses, and counters.
- `counter-sampler`: polls selected counter sources, handles resets, computes byte deltas, and emits normalized samples.
- `classifier`: decides whether a counter source represents WSL-only traffic and assigns a confidence level.
- `docker-detect`: detects Docker Desktop state and records whether Docker traffic may be included.
- `history`: stores rollups for recent and long-term usage.
- `presentation`: tray icon, taskbar display, context menu, settings, diagnostics.
- `diagnostics`: collects sanitized environment snapshots for bug reports and validation.

### Data Flow

1. Startup enumerates WSL state, `.wslconfig`, Windows build, WSL version, Docker Desktop state, and current network interfaces.
2. Classifier selects one or more candidate WSL counter sources.
3. Sampler records a baseline counter snapshot and monotonic timestamp.
4. Every poll interval, sampler reads counters, computes deltas, and emits upload/download rates.
5. Classifier continuously validates that selected counters still look like WSL traffic.
6. Presentation receives rates and confidence metadata.
7. History stores aggregated bytes using longer intervals than the live UI.
8. Network-change, WSL lifecycle, sleep/resume, and adapter-reset events trigger reclassification.

### Confidence Levels

- `High`: WSL NAT-mode virtual adapter identified and validated; Windows host traffic does not move the selected counters in validation tests.
- `Medium`: WSL-like virtual adapter found, but Docker Desktop or unusual adapter state may mix traffic.
- `Experimental`: mirrored/VirtioProxy/unknown mode where counters move with WSL traffic but isolation is not proven.
- `Unsupported`: no measurement point can isolate WSL from Windows host traffic.

The UI should expose confidence without overwhelming the primary speed display.

## Measurement Strategy

### MVP: Interface Counter Polling

Use IP Helper API interface counters as the MVP source.

Rationale:

- Lowest overhead.
- Stable Windows API.
- No packet capture dependency.
- No kernel driver.
- No admin requirement expected for normal reads.
- Sufficient for WSL NAT mode if adapter identification is correct.

Counter semantics:

- Download from WSL user's perspective should map to bytes received by the WSL VM from the outside network.
- Upload from WSL user's perspective should map to bytes transmitted by the WSL VM to the outside network.
- Host interface `InOctets`/`OutOctets` direction may need empirical mapping because the host-side virtual adapter's direction can be counterintuitive. The product must validate direction using controlled traffic.

### Adapter Selection

Do not depend on one localized adapter name. Score candidates using:

- Interface description containing Hyper-V virtual Ethernet concepts.
- Alias/name matching known WSL patterns as weak evidence.
- Operational state.
- Private WSL subnet/address relationship in NAT mode.
- Counter movement during WSL-only traffic.
- Lack of counter movement during Windows-only traffic.
- WSL networking mode from `.wslconfig` and runtime behavior.
- Exclusion of Docker-only adapters when Docker Desktop is active.

Store stable interface LUID after selection. Re-enumerate if `GetIfEntry2` fails, counters reset, or network-change events fire.

### Mirrored Networking

Mirrored mode should not be treated as fully supported until experiments prove a stable WSL-only counter source.

Recommended behavior before proof:

- Detect mirrored mode.
- Try experimental classification.
- If only physical interface counters move, report unsupported for exact WSL-only monitoring.
- Offer a diagnostics view explaining that mirrored mode improves networking compatibility but weakens host-only traffic isolation.

### Docker Desktop Awareness

Docker should be detected and surfaced because it changes interpretation:

- Docker Desktop may create or use WSL distributions.
- Docker Desktop may proxy network traffic through `com.docker.backend.exe`.
- Docker traffic may be counted as WSL VM traffic, Docker backend host traffic, or both depending on path.

Recommended MVP behavior:

- Detect `docker-desktop` distro and common Docker Desktop processes.
- Mark confidence as `Medium` or `Experimental` when Docker traffic may be mixed.
- Do not subtract Docker traffic until experiments show a reliable method.

### Per-Distribution Monitoring

Do not include in MVP.

Reason:

- WSL2 distributions share one network namespace.
- Host counters observe the VM/network path, not individual distributions.

Possible future architecture:

- Optional in-guest helper per distro.
- Distro-scoped process/cgroup accounting if stable.
- Explicit user opt-in because it changes permissions and trust boundaries.

### Per-Process Monitoring

Do not include in MVP.

Possible future architecture:

- Linux-side eBPF for socket/process byte accounting when available.
- `/proc` fallback for lower privilege but lower fidelity.
- Optional root/capability setup with clear user consent.
- Host UI consumes summarized metrics from the in-guest helper.

## Performance Design

Default polling:

- Live display: 1 second.
- Battery saver/background history: 2-5 seconds.
- Diagnostics/experiments: configurable down to 250-500 ms.

Efficiency rules:

- Poll selected interface by LUID after discovery.
- Avoid full interface enumeration on every tick.
- Use monotonic time for deltas.
- Use fixed-size history rings for live charts.
- Aggregate history before writing to disk.
- Keep UI redraws decoupled from raw counter sampling.
- Treat counter resets and adapter recreation as baseline resets, not traffic spikes.

Expected overhead:

- Interface counter polling should be near-zero CPU at 1 second intervals.
- ETW/WFP/packet capture modes are higher overhead and should be optional.
- Linux helper modes require separate performance budgets.

## Required Permissions

Expected standard-user operations:

- Read interface counters.
- Enumerate current user's WSL distros through `wsl.exe`.
- Read current user's `.wslconfig`.
- Display tray/taskbar UI.
- Store user-level settings/history.

Likely elevated or special-permission operations:

- Kernel ETW network tracing.
- WFP callouts or packet inspection.
- Hyper-V switch extension or driver installation.
- Linux eBPF, packet capture, or firewall accounting inside WSL.
- System-wide startup installation if using HKLM or services.

## Required Windows Versions

Recommended support:

- MVP: Windows 10/11 with WSL2 NAT mode.
- Preferred: Windows 11 22H2+ with current Store WSL for advanced networking detection.
- Mirrored networking: Windows 11 22H2+ only.
- WSL firewall integration: Windows 11 22H2+ and WSL 2.0.9+.
- WSL1: out of scope.

## Suggested Crate Structure

Status: scaffolded.

Workspace crates:

- `wsl-traffic-core`: domain types, samples, rates, confidence, history aggregation, error model.
- `wsl-traffic-windows`: Windows API wrappers for IP Helper, adapter metadata, network-change notifications, process checks, registry/config helpers.
- `wsl-traffic-wsl`: WSL discovery, distro inventory, `.wslconfig` parsing, runtime mode detection.
- `wsl-traffic-monitor`: sampling engine, classifier, Docker awareness, state machine.
- `wsl-traffic-ui`: native tray/taskbar UI and settings.
- `wsl-traffic-storage`: history persistence and migrations.
- `wsl-traffic-diagnostics`: environment snapshots and experiment reports.
- `wsl-traffic-agent-protocol`: future protocol shared with optional Linux helper.

If the project should start smaller, combine everything into one binary crate with modules matching these names, then split only when boundaries are stable.

## Suggested Dependencies

Keep dependencies conservative:

- `windows`: Win32 bindings for IP Helper API, Shell/Win32 UI, registry, eventing, and notifications. Prefer feature-scoped imports.
- `thiserror`: concise typed error definitions.
- `tracing`: internal diagnostics with low overhead.
- `tracing-subscriber`: dev/diagnostic logging setup.
- `serde`: settings, history records, diagnostics serialization.
- `toml` or `serde_ini`: parse `.wslconfig`-style configuration. Validate format choice against real files before committing.
- `time`: timestamps and history bucket boundaries.
- `directories`: user data/config paths.
- `rusqlite` or `redb`: history storage. Use SQLite if querying/reporting matters; use `redb` if embedded Rust-native storage and simple rollups are enough.
- `crossbeam-channel` or standard `std::sync::mpsc`: UI/sampler communication. Prefer standard library unless UI requirements need more.
- Native UI/tray choice requires a separate decision after prototype:
  - Direct Win32 via `windows` for minimum dependency and best control.
  - `tao`/`tray-icon` if they satisfy Windows taskbar/tray requirements with acceptable footprint.

Avoid for MVP:

- Packet capture crates.
- Async runtimes unless the selected UI framework requires one.
- Broad system-info crates if they duplicate small Win32 calls.
- Linux eBPF crates until per-process WSL monitoring is explicitly started.

## Development Roadmap

### Phase 0: Validation Harness

- Build a CLI-only diagnostic prototype outside production UI scope.
- Enumerate adapters and WSL state.
- Record counter deltas during scripted WSL/Windows/Docker traffic.
- Produce a machine-readable experiment report.

Exit criteria:

- NAT-mode adapter selection proven on at least two Windows/WSL configurations.
- Upload/download direction validated.
- Failure modes documented.

### Phase 1: MVP

- Native Windows tray/taskbar display.
- WSL NAT-mode traffic speeds.
- Confidence label and basic diagnostics.
- Manual refresh/re-detect.
- User settings for units and poll interval.

Exit criteria:

- Does not mislabel total host traffic as WSL traffic.
- CPU and memory remain low during idle and active traffic.
- Handles WSL shutdown/restart without spikes.

### Phase 2: History and Robust Detection

- Rolling hourly/daily usage.
- Better adapter scoring.
- Docker Desktop detection.
- Network-change event handling.
- Exportable diagnostics.

Exit criteria:

- Docker cases are clearly labeled.
- History survives restarts and counter resets.

### Phase 3: Advanced Networking Modes

- Mirrored-mode experiments.
- VirtioProxy fallback experiments.
- ETW validation prototype.
- Optional warning/remediation guidance for unsupported modes.

Exit criteria:

- Mirrored mode is either supported with proof or explicitly unsupported.

### Phase 4: Per-Distro and Per-Process Research

- Linux helper proof of concept.
- cgroup/eBPF feasibility tests.
- Security model and user consent design.
- Protocol between Windows UI and WSL helper.

Exit criteria:

- Accurate per-distro/per-process metrics on supported distros with measured overhead.

### Phase 5: v1.0 Hardening

- Installer/autostart.
- Crash-safe settings/history.
- Localization-safe adapter detection.
- Enterprise/fleet diagnostics.
- Plugin architecture design finalized.
- Signed release pipeline.

## Risks

- Mirrored networking may not expose a WSL-only host counter.
- Docker Desktop may make "WSL traffic" ambiguous.
- Adapter names and topology can change across WSL versions and Windows updates.
- Corporate VPN/firewall products can alter routes and visibility.
- Hyper-V/HNS metadata may require admin or be undocumented.
- Per-process and per-distro features may require Linux root/capabilities.
- UI/taskbar integration can dominate complexity if started before measurement is proven.
- Packet capture or driver approaches would increase support burden and reduce trust.

## Unknowns Requiring Experimentation

- NAT fallback to VirtioProxy counter behavior.
- Mirrored-mode isolation options.
- Docker Desktop inclusion/exclusion semantics.
- Non-admin access to useful Hyper-V/HNS/ETW metadata.
- Counter update granularity and direction across adapters.
- Whether WSL's VM creator ID can help classification outside firewall configuration.
- Whether Linux cgroups can provide stable distro attribution.

## Proof-of-Concept Milestones

1. Adapter inventory report:
   - list all interfaces, IDs, aliases, descriptions, addresses, counters, and WSL/Docker state.

2. NAT accuracy proof:
   - WSL-only download/upload changes selected WSL counters.
   - Windows-only download/upload does not change selected WSL counters materially.

3. Direction proof:
   - map host counter direction to user-facing WSL upload/download labels.

4. Lifecycle proof:
   - survive `wsl --shutdown`, distro start, sleep/resume, and adapter recreation.

5. Docker proof:
   - characterize Docker Desktop idle, pull, container egress, and published-port traffic.

6. Mirrored proof:
   - determine whether exact host-only measurement is possible.

7. Overhead proof:
   - measure CPU, memory, and wakeups at 500 ms, 1 s, 2 s, and 5 s intervals.

## ADRs

### ADR-001: Use Windows Interface Counters for MVP

Decision:

Use IP Helper interface counters as the MVP measurement source.

Status:

Accepted for MVP.

Context:

The product needs low CPU/memory use and should avoid drivers, packet capture, and elevated permissions. Microsoft exposes stable per-interface byte and packet counters through `MIB_IF_ROW2`.

Consequences:

- Excellent performance.
- Simple deployment.
- Accurate only when the selected interface is WSL-only.
- Requires strong adapter classification and honest confidence reporting.

### ADR-002: Treat Mirrored Networking as Experimental Until Proven

Decision:

Do not claim full mirrored-mode support until experiments prove WSL-only attribution.

Status:

Accepted.

Context:

Mirrored mode intentionally changes WSL networking by mirroring Windows interfaces into Linux. A physical NIC counter mixes Windows and WSL traffic.

Consequences:

- Prevents misleading users.
- MVP remains useful for NAT mode.
- Requires a visible unsupported/experimental state in the UI.

### ADR-003: Exclude Per-Distro Monitoring From MVP

Decision:

Do not implement per-distro monitoring in MVP.

Status:

Accepted.

Context:

Microsoft documents that WSL2 distributions share one network namespace. Host interface counters cannot naturally split traffic by distro.

Consequences:

- MVP remains host-only and lightweight.
- Future per-distro support likely requires an in-guest helper.

### ADR-004: Exclude Per-Process WSL Monitoring From MVP

Decision:

Do not implement per-process WSL monitoring in MVP.

Status:

Accepted.

Context:

Windows PID/socket APIs do not directly attribute Linux process traffic inside WSL. Linux-side observation requires extra permissions and overhead.

Consequences:

- Lower risk and simpler trust model for MVP.
- Future feature can be offered as an optional advanced mode.

### ADR-005: Docker Desktop Must Be Detected, Not Ignored

Decision:

Detect Docker Desktop and represent its effect on confidence and diagnostics.

Status:

Accepted.

Context:

Docker Desktop can use WSL2 and can route container traffic through `com.docker.backend.exe`, making WSL traffic semantics ambiguous.

Consequences:

- Users get honest readings.
- Exact Docker inclusion/exclusion is deferred until measured.

### ADR-006: No Kernel Driver or Packet Capture Dependency for MVP

Decision:

Avoid NDIS drivers, Hyper-V switch extensions, WFP callout drivers, and packet capture dependencies for MVP.

Status:

Accepted.

Context:

Those approaches may provide deeper attribution but add signing, admin rights, compatibility, and overhead concerns.

Consequences:

- Easier install and better trust.
- Some advanced attribution remains unavailable initially.

### ADR-007: Report Measurement Confidence

Decision:

Every displayed speed should have an associated internal confidence level, surfaced in diagnostics and compactly in the UI when confidence is not high.

Status:

Accepted.

Context:

WSL networking modes and Docker can make exact isolation impossible.

Consequences:

- Prevents false precision.
- Requires product copy and UX for unsupported/experimental states.
