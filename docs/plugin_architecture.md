# WSL Traffic Monitor Plugin Architecture Design

Status: Proposed Design (Phase 5 / v1.0)  
Date: 2026-07-22  

---

## 1. Overview & Goals

**WSL Traffic Monitor** core binary is designed to be ultra-lightweight ($< 0.1\%$ CPU, $< 30$ MB RAM) with zero external server or telemetry dependencies.

To support optional extensions—such as Prometheus metric exporters, Rainmeter skins, web dashboards, or enterprise logging—without polluting the core application with heavy dependencies (e.g. Tokio, Hyper, gRPC), Phase 5 defines a **Plugin IPC Architecture**.

---

## 2. Architectural Principles

1. **Core Binary Neutrality**: The core application binary MUST NOT depend on plugin frameworks, dynamic linkers (`.dll`), or external network servers.
2. **Out-of-Process Plugins**: Plugins run as separate processes on the host. If a plugin crashes or leaks memory, the core tray application remains completely unaffected.
3. **Low-Overhead IPC**: The core engine broadcasts lightweight JSON events over a local named pipe or socket when plugins are connected.

---

## 3. Communication Protocol

### Transport
- **Windows Host**: Local Named Pipe (`\\.\pipe\wsl-traffic-monitor`)
- **Protocol**: Single-direction or bi-directional JSON lines (JSONL) over pipe.

### Metric Event Payload (`TrafficSampleEvent`)

```json
{
  "version": 1,
  "timestamp": "2026-07-22T07:50:00Z",
  "download_speed_bps": 10457489.0,
  "upload_speed_bps": 124000.0,
  "status": "Active",
  "confidence": "High",
  "networking_mode": "nat",
  "active_adapter": "vEthernet (WSL)"
}
```

---

## 4. Candidate Plugin Ecosystem

| Plugin Category | Description | Example Target |
| :--- | :--- | :--- |
| **Metrics Exporters** | Exposes `/metrics` endpoint for Prometheus / Grafana. | `wsl-traffic-prometheus-exporter.exe` |
| **Desktop Widgets** | Custom desktop overlay skins / Rainmeter integration. | Rainmeter Plugin DLL / Web Widget |
| **System Event Loggers** | Logs daily network usage to Windows Event Log or CSV. | `wsl-traffic-csv-logger.exe` |

---

## 5. Security & Isolation

- Named pipe permissions restricted to the current Windows user SID (`SECURITY_RESTRICTED_CODE`).
- Zero administrative privileges required for plugin connection.
- Core app drops pipe connections if a plugin reader falls behind (unbounded queue protection).
