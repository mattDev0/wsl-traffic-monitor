//! Network adapter enumeration using Windows IP Helper API.

use wsl_traffic_core::AdapterInfo;

/// Query all network adapters on the host.
///
/// # Errors
/// Returns an error string if querying IP Helper APIs fails.
#[allow(clippy::too_many_lines)]
#[allow(unsafe_code)]
pub fn get_adapters() -> Result<Vec<AdapterInfo>, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
        use windows::Win32::NetworkManagement::IpHelper::{
            GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
            GetAdaptersAddresses, GetIfEntry2, MIB_IF_ROW2,
        };
        use windows::Win32::Networking::WinSock::{
            AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
        };

        let mut buf_size = 15000u32;
        let mut buf = vec![0u8; buf_size as usize];

        unsafe {
            let mut ret = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                None,
                Some(buf.as_mut_ptr() as *mut _),
                &mut buf_size,
            );

            if ret == ERROR_BUFFER_OVERFLOW.0 {
                buf.resize(buf_size as usize, 0);
                ret = GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                    None,
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut buf_size,
                );
            }

            if ret != ERROR_SUCCESS.0 {
                return Err(format!(
                    "GetAdaptersAddresses failed with error code: {ret}"
                ));
            }

            let mut adapters = Vec::new();
            let mut curr = buf.as_ptr()
                as *const windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH;

            while !curr.is_null() {
                let adapter = &*curr;

                let luid = adapter.Luid.Value;
                let if_index = adapter.Anonymous1.Anonymous.IfIndex;

                // Get GUID from AdapterName (PSTR)
                let guid_str = if adapter.AdapterName.is_null() {
                    String::new()
                } else {
                    let mut len = 0;
                    while *adapter.AdapterName.0.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(adapter.AdapterName.0, len);
                    String::from_utf8_lossy(slice).into_owned()
                };

                // Convert Wide String fields
                let friendly_name = if adapter.FriendlyName.is_null() {
                    String::new()
                } else {
                    let mut len = 0;
                    while *adapter.FriendlyName.0.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(adapter.FriendlyName.0, len);
                    String::from_utf16_lossy(slice)
                };

                let description = if adapter.Description.is_null() {
                    String::new()
                } else {
                    let mut len = 0;
                    while *adapter.Description.0.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(adapter.Description.0, len);
                    String::from_utf16_lossy(slice)
                };

                let alias = if adapter.FriendlyName.is_null() {
                    String::new()
                } else {
                    friendly_name.clone()
                };

                let mac_len = adapter.PhysicalAddressLength as usize;
                let mac_address = if mac_len > 0 && mac_len <= 8 {
                    adapter.PhysicalAddress[..mac_len]
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(":")
                } else {
                    String::new()
                };

                // Get IP Addresses
                let mut ipv4_addresses = Vec::new();
                let mut ipv6_addresses = Vec::new();
                let mut curr_addr = adapter.FirstUnicastAddress;
                while !curr_addr.is_null() {
                    let addr_info = &*curr_addr;
                    let sock_addr = addr_info.Address.lpSockaddr;
                    if !sock_addr.is_null() {
                        let family = (*sock_addr).sa_family;
                        if family == AF_INET {
                            let addr_in = &*(sock_addr as *const SOCKADDR_IN);
                            let b = addr_in.sin_addr.S_un.S_un_b;
                            ipv4_addresses
                                .push(format!("{}.{}.{}.{}", b.s_b1, b.s_b2, b.s_b3, b.s_b4));
                        } else if family == AF_INET6 {
                            let addr_in6 = &*(sock_addr as *const SOCKADDR_IN6);
                            let bytes = addr_in6.sin6_addr.u.Byte;
                            let ipv6 = std::net::Ipv6Addr::from(bytes);
                            ipv6_addresses.push(ipv6.to_string());
                        }
                    }
                    curr_addr = addr_info.Next;
                }

                // Query counters using GetIfEntry2
                let mut bytes_sent = 0;
                let mut bytes_recv = 0;
                let mut packets_sent = 0;
                let mut packets_recv = 0;
                let mut link_speed = adapter.TransmitLinkSpeed;

                let mut row = MIB_IF_ROW2::default();
                row.InterfaceLuid = adapter.Luid;
                if GetIfEntry2(&mut row) == ERROR_SUCCESS {
                    bytes_sent = row.OutOctets;
                    bytes_recv = row.InOctets;
                    packets_sent = row.OutUcastPkts + row.OutNUcastPkts;
                    packets_recv = row.InUcastPkts + row.InNUcastPkts;
                    link_speed = row.TransmitLinkSpeed;
                }

                adapters.push(AdapterInfo {
                    luid,
                    if_index,
                    guid: guid_str,
                    alias,
                    friendly_name,
                    description,
                    if_type: adapter.IfType,
                    oper_status: adapter.OperStatus.0 as u32,
                    mac_address,
                    mtu: adapter.Mtu,
                    link_speed,
                    ipv4_addresses,
                    ipv6_addresses,
                    bytes_sent,
                    bytes_recv,
                    packets_sent,
                    packets_recv,
                });

                curr = adapter.Next;
            }

            Ok(adapters)
        }
    }
    #[cfg(not(windows))]
    {
        // Mock list of adapters for testing on Linux
        Ok(vec![
            AdapterInfo {
                luid: 123_456_789,
                if_index: 2,
                guid: "{11111111-2222-3333-4444-555555555555}".to_string(),
                alias: "vEthernet (WSL)".to_string(),
                friendly_name: "vEthernet (WSL)".to_string(),
                description: "Hyper-V Virtual Ethernet Adapter".to_string(),
                if_type: 6,     // Ethernet
                oper_status: 1, // Up
                mac_address: "00:15:5d:77:f4:e4".to_string(),
                mtu: 1500,
                link_speed: 10_000_000_000,
                ipv4_addresses: vec!["172.24.2.1".to_string()],
                ipv6_addresses: vec!["fe80::1".to_string()],
                bytes_sent: 1024 * 512,
                bytes_recv: 1024 * 1024 * 20,
                packets_sent: 5000,
                packets_recv: 20000,
            },
            AdapterInfo {
                luid: 987_654_321,
                if_index: 3,
                guid: "{99999999-8888-7777-6666-555555555555}".to_string(),
                alias: "Ethernet".to_string(),
                friendly_name: "Ethernet".to_string(),
                description: "Intel(R) Ethernet Connection".to_string(),
                if_type: 6,
                oper_status: 1,
                mac_address: "aa:bb:cc:dd:ee:ff".to_string(),
                mtu: 1500,
                link_speed: 1_000_000_000,
                ipv4_addresses: vec!["192.168.1.50".to_string()],
                ipv6_addresses: vec![],
                bytes_sent: 1024 * 1024 * 10,
                bytes_recv: 1024 * 1024 * 50,
                packets_sent: 100_000,
                packets_recv: 500_000,
            },
        ])
    }
}

/// Raw counter values for a specific network interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RawInterfaceCounters {
    /// Number of bytes sent out of this interface from the host perspective.
    pub bytes_sent: u64,
    /// Number of bytes received into this interface from the host perspective.
    pub bytes_recv: u64,
    /// Number of packets sent out of this interface from the host perspective.
    pub packets_sent: u64,
    /// Number of packets received into this interface from the host perspective.
    pub packets_recv: u64,
}

/// Query only raw counters for a specific network interface by LUID.
///
/// # Errors
/// Returns an error string if the interface is not found or querying fails.
#[allow(unsafe_code)]
pub fn get_interface_counters(luid: u64) -> Result<RawInterfaceCounters, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::ERROR_SUCCESS;
        use windows::Win32::NetworkManagement::IpHelper::{GetIfEntry2, MIB_IF_ROW2};

        let mut row = MIB_IF_ROW2::default();
        row.InterfaceLuid.Value = luid;

        unsafe {
            let res = GetIfEntry2(&mut row);
            if res == ERROR_SUCCESS {
                Ok(RawInterfaceCounters {
                    bytes_sent: row.OutOctets,
                    bytes_recv: row.InOctets,
                    packets_sent: row.OutUcastPkts + row.OutNUcastPkts,
                    packets_recv: row.InUcastPkts + row.InNUcastPkts,
                })
            } else {
                Err(format!(
                    "GetIfEntry2 failed for LUID {luid} with error code {}",
                    res.0
                ))
            }
        }
    }
    #[cfg(not(windows))]
    {
        // Mock implementation for Linux development/testing
        if luid == 123_456_789 {
            Ok(RawInterfaceCounters {
                bytes_sent: 1024 * 512,
                bytes_recv: 1024 * 1024 * 20,
                packets_sent: 5000,
                packets_recv: 20000,
            })
        } else if luid == 987_654_321 {
            Ok(RawInterfaceCounters {
                bytes_sent: 1024 * 1024 * 10,
                bytes_recv: 1024 * 1024 * 50,
                packets_sent: 100_000,
                packets_recv: 500_000,
            })
        } else {
            Err(format!("LUID {luid} not found"))
        }
    }
}
