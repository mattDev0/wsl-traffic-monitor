//! Protocol definition for communication with the optional WSL guest agent helper.
//!
//! Exposes structures for querying network statistics, socket connections,
//! and metadata from within the Linux VM.

use serde::{Deserialize, Serialize};

/// Protocol version reserved for future helper negotiation.
pub const PROTOCOL_VERSION: u16 = 1;

/// Request sent from the host to the guest agent helper.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentRequest {
    /// Query active processes and their network connection mappings.
    QueryProcesses,
    /// Query basic metadata about the guest container environment.
    QueryMetadata,
}

/// Response returned by the guest agent helper to the host.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentResponse {
    /// Successful operation response.
    Ok(AgentResponseData),
    /// Failed operation response.
    Error {
        /// Explanation of the error.
        message: String,
    },
}

/// Success payload data for the agent response.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AgentResponseData {
    /// Payload for process network queries.
    Processes {
        /// List of active processes and their mapped connections.
        processes: Vec<ProcessConnectionInfo>,
    },
    /// Payload for metadata queries.
    Metadata {
        /// Name of the current WSL distribution.
        distro_name: String,
        /// Kernel version.
        kernel_version: String,
        /// VM uptime in seconds.
        uptime_secs: u64,
    },
}

/// Detailed information about a guest process and its network connections.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessConnectionInfo {
    /// Process ID.
    pub pid: u32,
    /// Process name/command.
    pub name: String,
    /// Username running the process.
    pub user: String,
    /// List of active network connections owned by this process.
    pub connections: Vec<SocketConnection>,
}

/// Details of a single active socket connection.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocketConnection {
    /// Protocol type (e.g. "tcp", "udp", "tcp6", "udp6").
    pub protocol: String,
    /// Local address in "IP:port" format.
    pub local_address: String,
    /// Remote address in "IP:port" format.
    pub remote_address: String,
    /// Connection state (e.g. "ESTABLISHED", "LISTEN", or "UNKNOWN").
    pub state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_serialization() {
        let req = AgentRequest::QueryProcesses;
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("query_processes"));

        let resp = AgentResponse::Ok(AgentResponseData::Metadata {
            distro_name: "Ubuntu".to_string(),
            kernel_version: "5.15".to_string(),
            uptime_secs: 100,
        });
        let json_resp = serde_json::to_string(&resp).unwrap();
        assert!(json_resp.contains("Ubuntu"));
    }
}
