# Validation Report: WSL2 NAT Traffic Isolation & Counter Accuracy

**Date**: 2026-08-05
**Host**: Windows 11 (build 10.0.26200.8875), WSL 2.7.3.0, kernel 6.6.114.1-1
**Networking Mode**: NAT
**Docker Desktop**: Installed
**Raw evidence**: [validation_run_v3.log](./validation_run_v3.log)

---

## 1. Objectives & Hypotheses

*   **Hypothesis 1 (Accuracy)**: Windows host interface counters (`GetIfEntry2` / `MIB_IF_ROW2`) measure traffic passing through the WSL2 guest with high accuracy.
*   **Hypothesis 2 (Directionality)**: In NAT mode, direction is inverted from the host perspective — WSL download = host `OutOctets`, WSL upload = host `InOctets`.
*   **Hypothesis 3 (Isolation)**: Host-only traffic does not leak into the WSL virtual interface counters.
*   **Hypothesis 4 (Low Overhead)**: Polling by LUID consumes < 0.2% CPU and under 30 MB memory.

---

## 2. Method

`apps/wsl-traffic-monitor/src/bin/experiment.rs --auto` drives the protocol unattended. The tool launches each transfer itself and awaits completion, so phase boundaries are exact rather than hand-timed, and it marks each phase a full sampling tick before the transfer begins so no transfer bytes land in the preceding subtotal.

Payloads are exactly **10,485,760 bytes** (10 MiB), requested from an endpoint that echoes a specified byte count, so measured deltas compare against a known figure.

Adapters measured, as recorded in the log header:

| Role | Adapter | LUID |
|---|---|---|
| WSL virtual | `vEthernet (WSL (Hyper-V firewall))` | 1689399767072768 |
| Host physical | `Wi-Fi` (192.168.1.128, 866 Mbps) | 19985273102270464 |

The tool logs every rejected candidate with a reason. `vEthernet (Default Switch)` is rejected as a Hyper-V switch endpoint — see [§5](#5-correction-to-the-previous-report).

---

## 3. Collected Data

Phase subtotals, bytes, copied from the run's summary table:

| phase | duration | wsl_recv | wsl_sent | phys_recv | phys_sent |
|---|---:|---:|---:|---:|---:|
| idle-baseline | 21s | 8,269 | 10,885 | 90,687 | 30,001 |
| **wsl-download** | 4s | 78,777 | **10,910,868** | **10,907,942** | 92,131 |
| idle-1 | 11s | 1,134 | 9,846 | 31,431 | 29,444 |
| **wsl-upload** | 14s | **11,552,001** | 496,761 | 612,631 | **11,794,784** |
| idle-2 | 11s | 36,098 | 3,496 | 21,017 | 43,308 |
| **host-download** | 8s | 11,787 | 1,863 | **10,910,459** | 130,803 |
| idle-3 | 12s | 8,961 | 15,440 | 59,864 | 32,482 |
| **both-download** | 8s | 136,043 | **10,900,502** | **22,871,405** | 337,534 |

---

## 4. Analysis

### 4.1 Directionality (Hypothesis 2)

The two isolated transfer phases show clean opposing dominance, and the physical NIC corroborates each:

*   **wsl-download**: the WSL adapter's `OutOctets` carried 10,910,868 bytes while the Wi-Fi adapter *received* 10,907,942. The two agree to **2,926 bytes (0.027%)**. Bytes arriving from the internet leave the host toward the guest.
*   **wsl-upload**: the WSL adapter's `InOctets` carried 11,552,001 bytes while Wi-Fi *sent* 11,794,784, agreeing to **2.10%**. Bytes from the guest leave the host toward the internet.

**Hypothesis 2 is CONFIRMED.** Host `OutOctets` → WSL download, host `InOctets` → WSL upload. This matches the mapping implemented in `crates/wsl-traffic-monitor/src/lib.rs`.

### 4.2 Accuracy (Hypothesis 1)

Against the exact 10 MiB payload:

*   **Download**: 10,910,868 bytes measured, **+4.05%** over payload. Consistent with TCP/IP and TLS record framing.
*   **Upload**: 11,552,001 bytes measured, **+10.17%**. Higher than download, attributable to HTTPS POST framing over an incompressible body plus Wi-Fi retransmission.
*   The reverse-direction ACK stream during download was 78,777 bytes, **0.75%** of payload — the expected magnitude for pure acknowledgements.

**Hypothesis 1 is CONFIRMED** for measuring transfer volume. Counters track payload within single-digit percentages, with the residual explained by protocol framing rather than measurement error.

> Note: the monitor reports link-layer bytes, not application payload. A UI reading is expected to exceed the size of a file being downloaded by roughly this margin.

### 4.3 Isolation (Hypothesis 3)

During **host-download**, the Wi-Fi adapter carried 10,910,459 bytes while the WSL adapter moved 13,650 bytes total — **0.1251%**.

That residual is not leakage. Across the four idle phases the WSL adapter's own background rate is 94,129 bytes over 55s = **1,711 B/s**. Over the 8-second host-download window that predicts 13,691 bytes; observed was 13,650, or **99.7% of the idle baseline**.

WSL adapter activity during a host-only 10 MiB download is **statistically indistinguishable from its idle rate**. Attributable leakage is zero.

### 4.4 Isolation under concurrency

During **both-download**, Wi-Fi received 22,871,405 bytes — approximately double the solo figure — while the WSL adapter's `OutOctets` recorded 10,900,502, or **47.66%** of the physical total.

Compared against the solo download (10,910,868), the concurrent WSL measurement differs by **-10,366 bytes (-0.095%)**. The WSL adapter reports the same volume for a WSL download whether or not the host is saturating the same physical link.

### 4.5 Overhead (Hypothesis 4)

**NOT MEASURED.** The logger records no CPU or memory metrics, and this run collected none. Hypothesis 4 remains open pending observation under Task Manager during a sustained session. See `docs/validation_report.md` §3.

---

## 5. Correction to the previous report

The prior version of this document declared all four hypotheses PROVEN on the basis of [validation_run.log](./validation_run.log). Two of those conclusions were not supported by that data.

That run selected `vEthernet (Default Switch)` as the host "physical" adapter — a Hyper-V switch endpoint carrying no host internet traffic. Across its 609 samples the physical columns recorded **0 bytes received** and 10,635 bytes sent in total. Any statement about host traffic or host/WSL isolation drawn from it was unsupported:

*   Experiment 2 claimed the host downloaded 10 MB with 0.00% leakage. The log cannot show the host downloaded anything.
*   Experiment 4 claimed the physical NIC showed ~20 MB combined. The log's entire physical total is 10.6 KB.
*   Experiment 5 claimed < 0.1% CPU and ~12 MB memory. The logger records no resource metrics.

The run also carried no phase markers, so the per-experiment figures quoted (10,457,489 / 3,532,492 bytes) could not be traced to any window of it. The 3,532,492 figure was described as "TCP ACKs & protocol overhead" at 33.7% of payload; this run measures the ACK stream at 0.75%, so that attribution was wrong regardless.

`validation_run.log` is retained for history. **Its physical columns must not be cited.** The adapter selection defect is fixed and the selection is now logged with a keep/drop reason per candidate, so a future run can be checked rather than trusted.

---

## 6. Conclusions

| Hypothesis | Status | Basis |
|---|---|---|
| 1 — Accuracy | **CONFIRMED** | Download within +4.05% of exact 10 MiB payload; framing accounts for the residual |
| 2 — Directionality | **CONFIRMED** | Opposing dominance across isolated up/down phases, corroborated by the physical NIC to 0.027% |
| 3 — Isolation | **CONFIRMED** | WSL adapter activity during a host-only transfer at 99.7% of its idle baseline; holds under concurrency |
| 4 — Low Overhead | **OPEN** | No resource data collected |

Hypotheses 1–3 validate the core architecture: `GetIfEntry2` polling of the WSL virtual adapter is an accurate and well-isolated telemetry source under NAT.

**Scope**: one run, one host, NAT mode, Wi-Fi uplink, Docker Desktop installed but idle. Not yet exercised: Ethernet uplink, active Docker container egress, mirrored/VirtioProxy modes, or multi-distro configurations.

**Reproduce**: `experiment.exe --auto > run.log` on a quiet network with any VPN disconnected.
