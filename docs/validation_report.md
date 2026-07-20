# WSL Traffic Monitor - Host Validation Report

This report documents the experimental validation of the WSL Traffic Monitor on native Windows hosts. Use this template to log data for Phase 0 verification before proceeding with production UI or installer releases.

---

## 1. System Metadata

| Parameter | Value / Version |
|---|---|
| **OS Name & Build** | *e.g., Windows 11 Pro Build 22631* |
| **WSL Version** | *e.g., 2.2.4.0 (run `wsl --version`)* |
| **WSL Networking Mode** | *NAT / Mirrored / VirtioProxy (from `.wslconfig`)* |
| **Docker Desktop Status** | *Not Installed / Running / Stopped* |
| **Default Physical Adapter** | *e.g., Intel(R) Wi-Fi 6E AX211* |
| **Selected WSL Adapter (LUID)** | *e.g., vEthernet (WSL) (LUID 1234567)* |

---

## 2. Controlled Experiment Evidence Log

### Test Case 1: WSL-Only Download Traffic
*   **Procedure**: Keep Windows host idle. Run `curl -L -o /dev/null http://speedtest.tele2.net/100MB.zip` inside a WSL distribution (Ubuntu).
*   **Observation Point**: Inspect default physical NIC counters and selected WSL virtual NIC counters during download.
*   **Measurements**:
    *   Host Physical bytes delta:
    *   WSL Virtual bytes delta:
    *   Reported Rate by Sampler (B/s):
*   **Pass/Fail Criteria**: WSL virtual adapter delta should match within 5% of the downloaded payload. Reported download speed must correlate with the transfer rate.
*   **Result**: [ ] PASS / [ ] FAIL

### Test Case 2: Windows-Only Download Traffic
*   **Procedure**: Keep WSL completely idle. Run a large download in Windows PowerShell (e.g. `Invoke-WebRequest -Uri http://speedtest.tele2.net/100MB.zip -OutFile $null`).
*   **Observation Point**: Verify that host physical counters increment, while WSL virtual NIC counters remain zero.
*   **Measurements**:
    *   Host Physical bytes delta:
    *   WSL Virtual bytes delta:
    *   Reported Rate by Sampler (B/s):
*   **Pass/Fail Criteria**: WSL virtual adapter delta must remain 0 (or <0.1% of host download). Reported speed must remain 0 B/s.
*   **Result**: [ ] PASS / [ ] FAIL

### Test Case 3: Simultaneous Host & WSL Traffic
*   **Procedure**: Start a download in WSL and a download in Windows PowerShell at the same time.
*   **Observation Point**: Check if the sampler isolates and reports only the WSL traffic portion.
*   **Measurements**:
    *   Host Physical download rate (B/s):
    *   WSL virtual reported download rate (B/s):
*   **Pass/Fail Criteria**: The reported rate should align with the WSL download speed, not the sum of host and WSL downloads.
*   **Result**: [ ] PASS / [ ] FAIL

### Test Case 4: Docker Desktop Egress Attribution
*   **Procedure**: Launch a container inside WSL and perform an outbound transfer (e.g., `docker run --rm alpine wget -qO- http://speedtest.tele2.net/10MB.zip > /dev/null`).
*   **Observation Point**: Assess whether container traffic registers on the WSL virtual adapter or is bypassed via host processes.
*   **Measurements**:
    *   Docker container bytes transferred:
    *   WSL Virtual bytes delta:
*   **Pass/Fail Criteria**: Document if Docker container traffic is successfully measured on the WSL adapter.
*   **Result**: [ ] PASS / [ ] FAIL

### Test Case 5: WSL Lifecycle Transitions (Shutdown/Restart)
*   **Procedure**: Run `wsl --shutdown` on the host, wait 10 seconds, then start a WSL session (`wsl.exe`).
*   **Observation Point**: Verify the monitor transitions from `Active` -> `Disconnected` -> `Active` (reclassified LUID).
*   **Pass/Fail Criteria**: Service must not panic, should stop billing active speed during shutdown, and immediately binds to the new virtual adapter upon reboot.
*   **Result**: [ ] PASS / [ ] FAIL

### Test Case 6: System Sleep & Resume
*   **Procedure**: Put the Windows host to sleep during an active download/polling session, wait 30 seconds, and wake the host up.
*   **Observation Point**: Observe the first sample delta after wake-up.
*   **Pass/Fail Criteria**: Bandwidth speed must not report massive spikes (e.g. GB/s) due to elapsed time division.
*   **Result**: [ ] PASS / [ ] FAIL

---

## 3. Performance & Overhead Assessment
Measure resources using Windows Task Manager / Process Hacker while the application is active:

*   **Idle CPU Usage**: _______ % (Target: <0.2%)
*   **Active CPU Usage**: _______ %
*   **Private Working Set (Memory)**: _______ MB (Target: <30 MB)
