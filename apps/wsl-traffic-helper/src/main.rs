//! WSL guest agent helper utility.
//!
//! Listens on stdin for JSON-encoded requests, collects guest OS process
//! and connection information, and writes JSON-encoded responses to stdout.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use wsl_traffic_agent_protocol::{
    AgentRequest, AgentResponse, AgentResponseData, ProcessConnectionInfo, SocketConnection,
};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: AgentRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let resp = AgentResponse::Error {
                    message: format!("Invalid JSON request: {e}"),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let response = match request {
            AgentRequest::QueryProcesses => AgentResponse::Ok(handle_query_processes()),
            AgentRequest::QueryMetadata => AgentResponse::Ok(handle_query_metadata()),
        };

        let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
        let _ = stdout.flush();
    }
}

fn handle_query_processes() -> AgentResponseData {
    // 1. Get mapping of active sockets (inodes -> details)
    let mut sockets = parse_proc_net_file("/proc/net/tcp", "tcp");
    sockets.extend(parse_proc_net_file("/proc/net/tcp6", "tcp6"));
    sockets.extend(parse_proc_net_file("/proc/net/udp", "udp"));
    sockets.extend(parse_proc_net_file("/proc/net/udp6", "udp6"));

    // 2. Map inodes to active Linux processes
    let inode_map = get_inode_to_process_map();

    // 3. Group connections by process
    let mut proc_groups: HashMap<u32, (String, String, Vec<SocketConnection>)> = HashMap::new();

    for sock in sockets {
        if let Some(&(pid, ref comm, ref user)) = inode_map.get(&sock.inode) {
            let entry = proc_groups
                .entry(pid)
                .or_insert_with(|| (comm.clone(), user.clone(), Vec::new()));
            entry.2.push(SocketConnection {
                protocol: sock.protocol,
                local_address: sock.local_address,
                remote_address: sock.remote_address,
                state: sock.state,
            });
        }
    }

    let mut processes = Vec::new();
    for (pid, (comm, user, conns)) in proc_groups {
        processes.push(ProcessConnectionInfo {
            pid,
            name: comm,
            user,
            connections: conns,
        });
    }

    // Sort by PID for deterministic output
    processes.sort_by_key(|p| p.pid);

    AgentResponseData::Processes { processes }
}

fn handle_query_metadata() -> AgentResponseData {
    let distro_name = get_distro_name().unwrap_or_else(|| "Unknown".to_string());
    let kernel_version = get_kernel_version().unwrap_or_else(|| "Unknown".to_string());
    let uptime_secs = get_uptime().unwrap_or(0);

    AgentResponseData::Metadata {
        distro_name,
        kernel_version,
        uptime_secs,
    }
}

// OS Parsing Functions
struct SocketInfo {
    inode: u64,
    protocol: String,
    local_address: String,
    remote_address: String,
    state: String,
}

fn parse_proc_net_file(path: &str, protocol: &str) -> Vec<SocketInfo> {
    let mut sockets = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return sockets;
    };
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().map_while(Result::ok).skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let local = parse_address_hex(parts[1]);
        let remote = parse_address_hex(parts[2]);
        let state = parse_state_hex(parts[3], protocol);
        let inode = parts[9].parse::<u64>().unwrap_or(0);

        if inode > 0 {
            sockets.push(SocketInfo {
                inode,
                protocol: protocol.to_string(),
                local_address: local.unwrap_or_else(|| "0.0.0.0:0".to_string()),
                remote_address: remote.unwrap_or_else(|| "0.0.0.0:0".to_string()),
                state,
            });
        }
    }
    sockets
}

fn parse_address_hex(hex: &str) -> Option<String> {
    let parts: Vec<&str> = hex.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let ip_hex = parts[0];
    let port = u16::from_str_radix(parts[1], 16).ok()?;

    let ip = if ip_hex.len() == 8 {
        std::net::IpAddr::V4(parse_hex_ipv4(ip_hex)?)
    } else if ip_hex.len() == 32 {
        std::net::IpAddr::V6(parse_hex_ipv6(ip_hex)?)
    } else {
        return None;
    };

    Some(format!("{ip}:{port}"))
}

fn parse_hex_ipv4(hex: &str) -> Option<std::net::Ipv4Addr> {
    let val = u32::from_str_radix(hex, 16).ok()?;
    let bytes = val.to_ne_bytes();
    Some(std::net::Ipv4Addr::new(
        bytes[0], bytes[1], bytes[2], bytes[3],
    ))
}

fn parse_hex_ipv6(hex: &str) -> Option<std::net::Ipv6Addr> {
    if hex.len() != 32 {
        return None;
    }
    let mut segments = [0u16; 8];
    for i in 0..4 {
        let part = &hex[i * 8..(i + 1) * 8];
        let val = u32::from_str_radix(part, 16).ok()?;
        let bytes = val.to_ne_bytes();
        segments[i * 2] = u16::from_be_bytes([bytes[1], bytes[0]]);
        segments[i * 2 + 1] = u16::from_be_bytes([bytes[3], bytes[2]]);
    }
    Some(std::net::Ipv6Addr::from(segments))
}

fn parse_state_hex(hex: &str, protocol: &str) -> String {
    if protocol.starts_with("udp") {
        return "UNCONN".to_string();
    }
    match hex {
        "01" => "ESTABLISHED".to_string(),
        "02" => "SYN_SENT".to_string(),
        "03" => "SYN_RECV".to_string(),
        "04" => "FIN_WAIT1".to_string(),
        "05" => "FIN_WAIT2".to_string(),
        "06" => "TIME_WAIT".to_string(),
        "07" => "CLOSE".to_string(),
        "08" => "CLOSE_WAIT".to_string(),
        "09" => "LAST_ACK".to_string(),
        "0A" => "LISTEN".to_string(),
        "0B" => "CLOSING".to_string(),
        _ => "UNKNOWN".to_string(),
    }
}

fn get_inode_to_process_map() -> HashMap<u64, (u32, String, String)> {
    let mut map = HashMap::new();
    let Ok(proc_dir) = std::fs::read_dir("/proc") else {
        return map;
    };

    for entry in proc_dir.flatten() {
        let path = entry.path();
        let filename = entry.file_name();
        let pid_str = filename.to_string_lossy();
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        let comm_path = path.join("comm");
        let name = std::fs::read_to_string(comm_path)
            .map_or_else(|_| "unknown".to_string(), |s| s.trim().to_string());

        let user = get_process_owner(&path);

        let fd_dir = path.join("fd");
        let Ok(entries) = std::fs::read_dir(fd_dir) else {
            continue;
        };

        for fd_entry in entries.flatten() {
            if let Ok(link) = std::fs::read_link(fd_entry.path()) {
                let link_str = link.to_string_lossy();
                if let Some(stripped) = link_str.strip_prefix("socket:[") {
                    if let Some(inode_str) = stripped.strip_suffix(']') {
                        if let Ok(inode) = inode_str.parse::<u64>() {
                            map.insert(inode, (pid, name.clone(), user.clone()));
                        }
                    }
                }
            }
        }
    }
    map
}

#[cfg(target_os = "linux")]
fn get_process_owner(path: &std::path::Path) -> String {
    use std::os::unix::fs::MetadataExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let uid = metadata.uid();
        get_username_from_uid(uid)
    } else {
        "unknown".to_string()
    }
}

#[cfg(not(target_os = "linux"))]
fn get_process_owner(_path: &std::path::Path) -> String {
    "unknown".to_string()
}

#[cfg(target_os = "linux")]
fn get_username_from_uid(uid: u32) -> String {
    let Ok(file) = std::fs::File::open("/etc/passwd") else {
        return uid.to_string();
    };
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            if let Ok(parsed_uid) = parts[2].parse::<u32>() {
                if parsed_uid == uid {
                    return parts[0].to_string();
                }
            }
        }
    }
    uid.to_string()
}

fn get_distro_name() -> Option<String> {
    let file = std::fs::File::open("/etc/os-release").ok()?;
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(stripped) = line.strip_prefix("NAME=") {
            let val = stripped.trim_matches('"');
            return Some(val.to_string());
        }
    }
    None
}

fn get_kernel_version() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .ok()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn get_uptime() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/uptime").ok()?;
    let first_part = content.split_whitespace().next()?;
    first_part.parse::<f64>().map(|f| f as u64).ok()
}
