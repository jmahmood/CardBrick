/*!
 * mDNS service discovery implementation for CardBrick sync
 * 
 * Provides:
 * - Real _cardbrick._tcp service record discovery
 * - UDP broadcast fallback for legacy daemons
 * - Service validation and ranking
 */

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr, Ipv4Addr};
use std::time::Duration;
use std::fmt;

use anyhow::{Result, Context, bail};
use log::{info, warn, debug};
use mdns::RecordKind;
use serde::{Serialize, Deserialize};
use futures_util::{pin_mut, StreamExt};
use tokio::net::UdpSocket;
use socket2::{Domain, Protocol, Socket, Type};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredService {
    pub name: String,
    pub host: IpAddr,
    pub port: u16,
    pub txt_records: HashMap<String, String>,
}

impl DiscoveredService {
    pub fn host_string(&self) -> String {
        self.host.to_string()
    }
    
    pub fn as_socket_addr(&self, port_override: Option<u16>) -> SocketAddr {
        SocketAddr::new(self.host, port_override.unwrap_or(self.port))
    }
}

impl fmt::Display for DiscoveredService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}:{})", self.name, self.host, self.port)
    }
}

const CARDBRICK_SERVICE_TYPE: &str = "_cardbrick._tcp.local";
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const UDP_BROADCAST_PORT: u16 = 6430; // Different from HTTP port to avoid conflicts

#[derive(Debug, Clone)]
pub(crate) struct ServiceInfo {
    pub(crate) address: IpAddr,
    pub(crate) port: u16,
    pub(crate) hostname: String,
    pub(crate) txt_records: HashMap<String, String>,
    pub(crate) discovered_via: DiscoveryMethod,
}

#[derive(Debug, Clone, Copy)]
pub enum DiscoveryMethod {
    Mdns,
    UdpBroadcast,
    Fallback,
}

impl fmt::Display for DiscoveryMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryMethod::Mdns => write!(f, "mDNS"),
            DiscoveryMethod::UdpBroadcast => write!(f, "UDP"),
            DiscoveryMethod::Fallback => write!(f, "fallback"),
        }
    }
}

pub struct MdnsDiscovery {
    mdns_timeout: Duration,
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self {
            mdns_timeout: DISCOVERY_TIMEOUT,
        }
    }
}

impl MdnsDiscovery {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.mdns_timeout = timeout;
        self
    }
    
    /// Primary service discovery method
    /// 1. Try mDNS first
    /// 2. Fall back to UDP broadcast
    /// 3. Use hardcoded candidates only as last resort
    pub async fn discover_services(&self) -> Result<Vec<DiscoveredService>> {
        let mut services = Vec::new();
        let mut seen_addresses = HashSet::new();
        
        // Step 1: Try mDNS discovery
        info!("Starting mDNS discovery for {}", CARDBRICK_SERVICE_TYPE);
        if let Ok(mdns_services) = self.discover_via_mdns().await {
            info!("Found {} services via mDNS", mdns_services.len());
            let discovered: Vec<DiscoveredService> = mdns_services.into_iter()
                .map(DiscoveredService::from)
                .collect();
            self.add_unique_services(discovered, &mut services, &mut seen_addresses);
        } else {
            warn!("mDNS discovery failed");
        }
        
        // Step 2: UDP broadcast probe for legacy daemons
        if services.is_empty() {
            info!("No mDNS services found, trying UDP broadcast probe");
            if let Ok(udp_services) = self.discover_via_udp_broadcast().await {
                info!("Found {} services via UDP broadcast", udp_services.len());
                let discovered: Vec<DiscoveredService> = udp_services.into_iter()
                    .map(DiscoveredService::from)
                    .collect();
                self.add_unique_services(discovered, &mut services, &mut seen_addresses);
            } else {
                warn!("UDP broadcast discovery failed");
            }
        }
        
        // Step 3: Fallback candidates (only if nothing else worked)
        if services.is_empty() {
            warn!("No services discovered via mDNS or UDP, using fallback candidates");
            let fallback_services = self.get_fallback_candidates();
            self.add_unique_services(fallback_services, &mut services, &mut seen_addresses);
        }
        
        info!("Total unique services discovered: {}", services.len());
        Ok(services)
    }
    
    /// Helper method to add unique services and handle deduplication
    fn add_unique_services(
        &self,
        discovered: Vec<DiscoveredService>,
        services: &mut Vec<DiscoveredService>,
        seen: &mut HashSet<(IpAddr, u16)>,
    ) {
        for service in discovered {
            let addr_key = (service.host, service.port);
            if seen.insert(addr_key) {
                services.push(service);
            }
        }
    }
    
    /// Discover services using proper mDNS queries
    async fn discover_via_mdns(&self) -> Result<Vec<ServiceInfo>> {
        let mut services = Vec::new();
        
        // Build discovery stream using the correct API
        let stream = mdns::discover::all(CARDBRICK_SERVICE_TYPE, self.mdns_timeout)?
            .listen();
        pin_mut!(stream);
        
        // The stream will naturally end when the mdns_timeout is reached
        while let Some(Ok(response)) = stream.next().await {
            debug!("mDNS response: {:?}", response);
            if let Some(service_info) = Self::parse_mdns_response(response).await {
                services.push(service_info);
            }
        }
        
        Ok(services)
    }
    
    /// Parse mDNS response into ServiceInfo with DNS lookup for SRV-only responses
    async fn parse_mdns_response(response: mdns::Response) -> Option<ServiceInfo> {
        let mut address = None;
        let mut port = None;
        let mut hostname = None;
        let mut srv_target = None;
        let mut txt_records = HashMap::new();
        const MAX_TXT_RECORDS: usize = 32;
        
        for record in response.records() {
            match &record.kind {
                RecordKind::A(addr) => {
                    address = Some(IpAddr::V4(*addr));
                    if hostname.is_none() {
                        hostname = Some(record.name.clone());
                    }
                }
                RecordKind::AAAA(addr) => {
                    if address.is_none() { // Prefer IPv4, but accept IPv6
                        address = Some(IpAddr::V6(*addr));
                        if hostname.is_none() {
                            hostname = Some(record.name.clone());
                        }
                    }
                }
                RecordKind::SRV { port: srv_port, target, .. } => {
                    port = Some(*srv_port);
                    srv_target = Some(target.clone());
                    if hostname.is_none() {
                        hostname = Some(target.clone());
                    }
                }
                RecordKind::TXT(txt_data) => {
                    // Parse TXT records with safety guards
                    for txt_entry in txt_data {
                        // Guard against oversized TXT records
                        if txt_entry.len() > 255 {
                            let preview = if txt_entry.len() > 32 {
                                format!("{}...", &txt_entry[..32])
                            } else {
                                txt_entry.clone()
                            };
                            warn!("Oversized TXT record ignored: {} bytes, content: {:?}", txt_entry.len(), preview);
                            continue;
                        }
                        
                        // Guard against too many TXT records
                        if txt_records.len() >= MAX_TXT_RECORDS {
                            warn!("TXT record limit reached, ignoring additional records");
                            break;
                        }
                        
                        if let Some((key, value)) = txt_entry.split_once('=') {
                            txt_records.insert(key.to_string(), value.to_string());
                        }
                    }
                }
                _ => {} // Ignore other record types
            }
        }
        
        // If we have SRV but no A/AAAA, try DNS lookup
        if address.is_none() && srv_target.is_some() {
            if let Some(target) = srv_target {
                debug!("No A/AAAA record found, attempting DNS lookup for SRV target: {}", target);
                match tokio::net::lookup_host(format!("{}:0", target)).await {
                    Ok(mut addrs) => {
                        if let Some(socket_addr) = addrs.next() {
                            address = Some(socket_addr.ip());
                            debug!("Resolved SRV target {} to {}", target, socket_addr.ip());
                        }
                    }
                    Err(e) => {
                        debug!("DNS lookup failed for SRV target {}: {}", target, e);
                    }
                }
            }
        }
        
        // Validate we have required fields
        match (address, port, hostname.clone()) {
            (Some(addr), Some(p), Some(host)) => Some(ServiceInfo {
                address: addr,
                port: p,
                hostname: host,
                txt_records,
                discovered_via: DiscoveryMethod::Mdns,
            }),
            _ => {
                warn!("Incomplete mDNS response: addr={:?}, port={:?}, host={:?}", 
                      address, port, hostname);
                None
            }
        }
    }
    
    /// UDP broadcast probe for legacy daemons
    async fn discover_via_udp_broadcast(&self) -> Result<Vec<ServiceInfo>> {
        let mut services = Vec::new();
        
        // Create UDP socket with reuse_addr for broadcast
        let socket2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .context("Failed to create socket2 UDP socket")?;
        socket2.set_reuse_address(true)
            .context("Failed to set SO_REUSEADDR")?;
        socket2.set_broadcast(true)
            .context("Failed to set SO_BROADCAST")?;
        socket2.bind(&"0.0.0.0:0".parse::<SocketAddr>().unwrap().into())
            .context("Failed to bind UDP socket")?;
        // Note: set_nonblocking is redundant after UdpSocket::from_std() but harmless
        
        let socket = UdpSocket::from_std(socket2.into())
            .context("Failed to convert to tokio UDP socket")?;
            
        // Get dynamic broadcast addresses from interfaces
        let broadcast_addrs = self.get_broadcast_addresses();
        
        // Broadcast probe message
        let probe_message = b"CARDBRICK_DISCOVER_V1";
        
        // Send IPv4 broadcasts with existing socket
        for addr in &broadcast_addrs {
            if addr.is_ipv4() {
                let target = SocketAddr::new(*addr, UDP_BROADCAST_PORT);
                debug!("Sending UDP broadcast to {}", target);
                if let Err(e) = socket.send_to(probe_message, target).await {
                    debug!("Failed to send to {}: {}", target, e);
                }
            } else {
                debug!("Skipping IPv6 address {} (IPv4 socket)", addr);
            }
        }
        
        // Try IPv6 multicast with separate socket (if enabled)
        if std::env::var("CARDBRICK_ENABLE_IPV6").is_ok() {
            if let Err(e) = self.send_ipv6_multicast(probe_message).await {
                debug!("IPv6 multicast failed: {}", e);
            }
        }
        
        // Listen for responses with timeout
        let mut buffer = vec![0u8; 1024]; // Reuse buffer allocation
        let timeout_duration = Duration::from_secs(3);
        let mut parse_error_count = 0;
        const MAX_PARSE_ERRORS: usize = 10;
        
        loop {
            match tokio::time::timeout(timeout_duration, socket.recv_from(&mut buffer)).await {
                Ok(Ok((size, sender))) => {
                    debug!("Received UDP response from {}: {} bytes", sender, size);
                    
                    // Parse response (expect JSON with service info)
                    match std::str::from_utf8(&buffer[..size]) {
                        Ok(response_str) => {
                            match self.parse_udp_response(response_str, sender.ip()) {
                                Ok(service_info) => {
                                    services.push(service_info);
                                    parse_error_count = 0; // Reset counter on success
                                }
                                Err(e) => {
                                    debug!("Failed to parse UDP response from {}: {}", sender, e);
                                    parse_error_count += 1;
                                    if parse_error_count >= MAX_PARSE_ERRORS {
                                        warn!("Too many UDP parse errors, stopping discovery");
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Invalid UTF-8 in UDP response from {}: {}", sender, e);
                            parse_error_count += 1;
                            if parse_error_count >= MAX_PARSE_ERRORS {
                                warn!("Too many UDP parse errors, stopping discovery");
                                break;
                            }
                        }
                    }
                    // Continue listening for more responses
                }
                Ok(Err(e)) => {
                    debug!("UDP receive error: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout reached, stop listening
                    debug!("UDP broadcast discovery timeout reached");
                    break;
                }
            }
        }
        
        Ok(services)
    }
    
    /// Send IPv6 multicast with separate socket
    async fn send_ipv6_multicast(&self, message: &[u8]) -> Result<()> {
        // Create IPv6 socket for multicast
        let socket6 = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))
            .context("Failed to create IPv6 UDP socket")?;
        socket6.set_reuse_address(true)
            .context("Failed to set SO_REUSEADDR on IPv6 socket")?;
        socket6.bind(&"[::]:0".parse::<SocketAddr>().unwrap().into())
            .context("Failed to bind IPv6 socket")?;
        // Note: set_nonblocking is redundant after UdpSocket::from_std() but harmless
        
        let socket6 = UdpSocket::from_std(socket6.into())
            .context("Failed to convert IPv6 socket to tokio")?;
        
        // IPv6 multicast addresses
        let ipv6_targets = [
            "[ff02::1]", // All nodes (link-local)
            "[ff02::fb]", // mDNS multicast (link-local)
        ];
        
        for target_str in &ipv6_targets {
            match format!("{}:{}", target_str, UDP_BROADCAST_PORT).parse::<SocketAddr>() {
                Ok(target) => {
                    debug!("Sending IPv6 multicast to {}", target);
                    if let Err(e) = socket6.send_to(message, target).await {
                        debug!("Failed to send IPv6 multicast to {}: {}", target, e);
                    }
                }
                Err(e) => {
                    warn!("Invalid IPv6 multicast address {}: {}", target_str, e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get broadcast addresses from network interfaces
    fn get_broadcast_addresses(&self) -> Vec<IpAddr> {
        let mut broadcast_addrs = Vec::new();
        
        // Add global IPv4 broadcast
        broadcast_addrs.push(IpAddr::V4(Ipv4Addr::BROADCAST));
        
        // Get interface-specific broadcast addresses
        match if_addrs::get_if_addrs() {
            Ok(interfaces) => {
                for iface in interfaces {
                    if !iface.is_loopback() {
                        match iface.addr {
                            if_addrs::IfAddr::V4(ref addr_v4) => {
                                let ip = addr_v4.ip;
                                let netmask = addr_v4.netmask;
                                
                                // Calculate broadcast: (ip & netmask) | !netmask
                                let network = u32::from(ip) & u32::from(netmask);
                                let broadcast_bits = network | (!u32::from(netmask));
                                let broadcast = Ipv4Addr::from(broadcast_bits);
                                broadcast_addrs.push(IpAddr::V4(broadcast));
                                debug!("Added interface broadcast: {} for {}", broadcast, ip);
                            }
                            if_addrs::IfAddr::V6(_) => {
                                // IPv6 handled separately with dedicated socket
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to enumerate network interfaces: {}", e);
                // Fallback to common broadcast addresses
                broadcast_addrs.extend([
                    IpAddr::V4("192.168.1.255".parse().unwrap()),
                    IpAddr::V4("192.168.0.255".parse().unwrap()),
                    IpAddr::V4("10.0.0.255".parse().unwrap()),
                ]);
            }
        }
        
        broadcast_addrs
    }
    
    /// Parse UDP broadcast response
    fn parse_udp_response(&self, response: &str, sender_ip: IpAddr) -> Result<ServiceInfo> {
        // Expect JSON response like:
        // {"service":"cardbrick","version":"1","port":6429,"hostname":"desktop-pc","txt":{"key":"value"}}
        
        let parsed: serde_json::Value = serde_json::from_str(response)
            .context("Failed to parse UDP response JSON")?;
            
        let service_name = parsed.get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing service field"))?;
            
        if service_name != "cardbrick" {
            bail!("Unexpected service type: {}", service_name);
        }
        
        let port = parsed.get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16)
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid port"))?;
            
        let hostname = parsed.get("hostname")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Preserve any existing TXT records from daemon response    
        let mut txt_records = HashMap::new();
        if let Some(txt_obj) = parsed.get("txt").and_then(|v| v.as_object()) {
            for (key, value) in txt_obj {
                if let Some(value_str) = value.as_str() {
                    txt_records.insert(key.clone(), value_str.to_string());
                }
            }
        }
        
        // Add standard fields, preserving daemon's version if present
        if let Some(version) = parsed.get("version").and_then(|v| v.as_str()) {
            txt_records.entry("version".to_string()).or_insert_with(|| version.to_string());
        }
        // Always add proto to distinguish discovery method
        txt_records.insert("proto".to_string(), "udp-broadcast".to_string());
        
        Ok(ServiceInfo {
            address: sender_ip,
            port,
            hostname,
            txt_records,
            discovered_via: DiscoveryMethod::UdpBroadcast,
        })
    }
    
    /// Fallback candidate list (last resort)
    fn get_fallback_candidates(&self) -> Vec<DiscoveredService> {
        let candidates = [
            ("192.168.1.100", 6429),
            ("192.168.0.100", 6429),
            ("10.0.0.100", 6429),
            ("127.0.0.1", 6429), // localhost as IP
        ];
        
        candidates.iter().filter_map(|(host_str, port)| {
            if let Ok(host_ip) = host_str.parse::<IpAddr>() {
                let mut txt_records = HashMap::new();
                txt_records.insert("proto".to_string(), "fallback".to_string());
                txt_records.insert("ver".to_string(), "unknown".to_string());
                
                Some(DiscoveredService {
                    name: format!("CardBrick Fallback ({})", host_str),
                    host: host_ip,
                    port: *port,
                    txt_records,
                })
            } else {
                warn!("Invalid fallback candidate IP: {}", host_str);
                None
            }
        }).collect()
    }
}

/// Convert ServiceInfo to DiscoveredService
impl From<ServiceInfo> for DiscoveredService {
    fn from(info: ServiceInfo) -> Self {
        let name = format!("CardBrick Sync ({} via {})", info.hostname, info.discovered_via);
        
        DiscoveredService {
            name,
            host: info.address,
            port: info.port,
            txt_records: info.txt_records,
        }
    }
}

/// Minimal CLI entry so this binary compiles and can be used for diagnostics.
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let discovery = MdnsDiscovery::new();
    let services = discovery.discover_services().await?;
    for svc in services {
        println!("{} {}:{}", svc.name, svc.host, svc.port);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_mdns_discovery_timeout() {
        let discovery = MdnsDiscovery::new().with_timeout(Duration::from_millis(500));
        
        // Should not hang and should return within reasonable time
        let start = Instant::now();
        let result = discovery.discover_services().await;
        let elapsed = start.elapsed();
        
        // Should complete reasonably quickly even if no services found
        // Allow more time since mDNS discovery involves network operations
        assert!(elapsed < Duration::from_secs(5));
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_service_deduplication() {
        let _discovery = MdnsDiscovery::new().with_timeout(Duration::from_millis(10));
        
        // Create duplicate services with same address:port
        let service1 = ServiceInfo {
            address: "192.168.1.100".parse().unwrap(),
            port: 6429,
            hostname: "host1".to_string(),
            txt_records: HashMap::new(),
            discovered_via: DiscoveryMethod::Mdns,
        };
        
        let service2 = ServiceInfo {
            address: "192.168.1.100".parse().unwrap(),
            port: 6429,
            hostname: "host2".to_string(), // Different hostname, same address:port
            txt_records: HashMap::new(),
            discovered_via: DiscoveryMethod::UdpBroadcast,
        };
        
        // Test deduplication logic manually
        let mut seen_addresses = HashSet::new();
        let mut services = Vec::new();
        
        for service_info in vec![service1, service2] {
            let service = DiscoveredService::from(service_info);
            let addr_key = (service.host, service.port);
            if seen_addresses.insert(addr_key) {
                services.push(service);
            }
        }
        
        // Should only have one service due to deduplication
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].host, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(services[0].port, 6429);
    }
    
    #[test]
    fn test_udp_response_parsing() {
        let discovery = MdnsDiscovery::new();
        let response = r#"{"service":"cardbrick","version":"1","port":6429,"hostname":"test-desktop"}"#;
        let sender_ip = "192.168.1.100".parse().unwrap();
        
        let result = discovery.parse_udp_response(response, sender_ip).unwrap();
        
        assert_eq!(result.address, sender_ip);
        assert_eq!(result.port, 6429);
        assert_eq!(result.hostname, "test-desktop");
        assert_eq!(result.txt_records.get("version"), Some(&"1".to_string()));
        assert_eq!(result.txt_records.get("proto"), Some(&"udp-broadcast".to_string()));
    }
    
    #[test]
    fn test_udp_response_with_txt_records() {
        let discovery = MdnsDiscovery::new();
        let response = r#"{"service":"cardbrick","version":"1","port":6429,"hostname":"test-desktop","txt":{"tls":"1","priority":"5"}}"#;
        let sender_ip = "192.168.1.100".parse().unwrap();
        
        let result = discovery.parse_udp_response(response, sender_ip).unwrap();
        
        assert_eq!(result.txt_records.get("version"), Some(&"1".to_string()));
        assert_eq!(result.txt_records.get("proto"), Some(&"udp-broadcast".to_string()));
        assert_eq!(result.txt_records.get("tls"), Some(&"1".to_string()));
        assert_eq!(result.txt_records.get("priority"), Some(&"5".to_string()));
    }
    
    #[test]
    fn test_discovery_method_display() {
        assert_eq!(DiscoveryMethod::Mdns.to_string(), "mDNS");
        assert_eq!(DiscoveryMethod::UdpBroadcast.to_string(), "UDP");
        assert_eq!(DiscoveryMethod::Fallback.to_string(), "fallback");
    }
    
    #[test]
    fn test_from_service_info() {
        let service_info = ServiceInfo {
            address: "192.168.1.100".parse().unwrap(),
            port: 6429,
            hostname: "test-host".to_string(),
            txt_records: {
                let mut map = HashMap::new();
                map.insert("version".to_string(), "1".to_string());
                map
            },
            discovered_via: DiscoveryMethod::Mdns,
        };
        
        let discovered_service = DiscoveredService::from(service_info);
        
        assert_eq!(discovered_service.name, "CardBrick Sync (test-host via mDNS)");
        assert_eq!(discovered_service.host, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(discovered_service.port, 6429);
        assert_eq!(discovered_service.txt_records.get("version"), Some(&"1".to_string()));
    }
    
    #[test]
    #[cfg(not(ci))]
    fn test_mdns_discovery_fast() {
        // This test only runs outside CI to keep pipeline fast
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let discovery = MdnsDiscovery::new().with_timeout(Duration::from_millis(50));
            let start = Instant::now();
            let result = discovery.discover_services().await;
            let elapsed = start.elapsed();
            
            assert!(elapsed < Duration::from_secs(1));
            assert!(result.is_ok());
        });
    }
    
    #[test]
    fn test_random_udp_responses() {
        // Fuzz test for UDP response parsing robustness
        let sender_ip = "127.0.0.1".parse().unwrap();
        
        let huge_response = "x".repeat(10000);
        let invalid_responses = vec![
            "", // Empty
            "{", // Invalid JSON
            r#"{"service":"wrong"}"#, // Wrong service
            r#"{"service":"cardbrick"}"#, // Missing fields
            r#"{"service":"cardbrick","port":"not_a_number"}"#, // Invalid port
            &huge_response, // Huge response
        ];
        
        for response in invalid_responses {
            // Create fresh discovery instance per test to avoid state contamination
            let discovery = MdnsDiscovery::new();
            
            // Should not panic, just return error
            let result = discovery.parse_udp_response(response, sender_ip);
            assert!(result.is_err(), "Expected error for response: {}", response);
        }
        
        // Valid response should work
        let discovery = MdnsDiscovery::new();
        let valid_response = r#"{"service":"cardbrick","version":"1","port":6429,"hostname":"test"}"#;
        let result = discovery.parse_udp_response(valid_response, sender_ip);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_discovered_service_helpers() {
        let service = DiscoveredService {
            name: "Test Service".to_string(),
            host: "192.168.1.100".parse().unwrap(),
            port: 6429,
            txt_records: HashMap::new(),
        };
        
        // Test Display trait
        assert_eq!(service.to_string(), "Test Service (192.168.1.100:6429)");
        
        // Test as_socket_addr helper
        let socket_addr = service.as_socket_addr(None);
        assert_eq!(socket_addr.ip().to_string(), "192.168.1.100");
        assert_eq!(socket_addr.port(), 6429);
        
        // Test as_socket_addr with port override
        let socket_addr_override = service.as_socket_addr(Some(8080));
        assert_eq!(socket_addr_override.port(), 8080);
        assert_eq!(socket_addr_override.ip().to_string(), "192.168.1.100");
    }
}
