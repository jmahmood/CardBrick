/*!
 * Sync client implementation for CardBrick handheld devices
 * 
 * Handles:
 * - mDNS service discovery
 * - ed25519 key management and signing
 * - HTTP communication with desktop daemon
 */

use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

use anyhow::{Result, Context, bail};
use log::{info, warn, debug};
use serde::{Serialize, Deserialize};
use reqwest::Client;
use ring::signature::{Ed25519KeyPair, KeyPair};
use ring::rand::SystemRandom;
// Note: mDNS implementation simplified for initial version

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredService {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub txt_records: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DoorbellRequest {
    pub device_ip: String,
    pub ssh_port: u16,
    pub pubkey_fingerprint: String,
    pub ready_until: u64,
    pub signature: String,
    pub backup_mode: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DoorbellResponse {
    pub status: String,
    pub message: String,
    pub job_id: Option<String>,
}

pub struct SyncClient {
    http_client: Client,
    key_pair: Ed25519KeyPair,
    device_ip: String,
}

impl SyncClient {
    pub async fn new() -> Result<Self> {
        // Initialize HTTP client with reasonable timeouts
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;
            
        // Load or generate ed25519 key pair
        let key_pair = load_or_generate_keypair()
            .context("Failed to initialize device keys")?;
            
        // Get device IP address
        let device_ip = get_local_ip()
            .context("Failed to determine device IP address")?;
            
        Ok(Self {
            http_client,
            key_pair,
            device_ip,
        })
    }
    
    pub async fn discover_services(&mut self) -> Vec<DiscoveredService> {
        info!("Starting service discovery for CardBrick sync daemons");
        
        // For initial implementation, try common IP ranges and broadcast addresses
        // A full mDNS implementation would be added later
        
        let mut discovered = Vec::new();
        
        // Common desktop IP addresses to try
        let candidate_hosts = vec![
            "192.168.1.100", "192.168.1.101", "192.168.1.102",
            "192.168.0.100", "192.168.0.101", "192.168.0.102",
            "10.0.0.100", "10.0.0.101", "10.0.0.102",
            "localhost", "cardbrick-desktop"
        ];
        
        for host in candidate_hosts {
            if let Ok(service) = self.probe_host(host, 6429).await {
                info!("Found CardBrick service at {}:{}", host, 6429);
                discovered.push(service);
            }
        }
        
        discovered
    }
    
    async fn probe_host(&self, host: &str, port: u16) -> Result<DiscoveredService> {
        let url = format!("http://{}:{}/health", host, port);
        
        // Quick health check with short timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.http_client.get(&url).send()
        ).await;
        
        match response {
            Ok(Ok(resp)) if resp.status().is_success() => {
                Ok(DiscoveredService {
                    name: format!("CardBrick Sync on {}", host),
                    host: host.to_string(),
                    port,
                    txt_records: {
                        let mut map = HashMap::new();
                        map.insert("proto".to_string(), "http".to_string());
                        map.insert("ver".to_string(), "1".to_string());
                        map
                    },
                })
            }
            _ => bail!("Host {} not responding", host)
        }
    }
    
    pub async fn create_doorbell_request(
        &self,
        _host: &str,
        ssh_port: u16,
    ) -> Result<DoorbellRequest> {
        // Create request timestamp (valid for 5 minutes)
        let ready_until = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs() + 300;
            
        // Get public key fingerprint
        let pubkey_fingerprint = get_key_fingerprint(&self.key_pair);
        
        // Create canonical message for signing
        let message_data = serde_json::json!({
            "device_ip": self.device_ip,
            "ssh_port": ssh_port,
            "pubkey_fingerprint": pubkey_fingerprint,
            "ready_until": ready_until,
            "backup_mode": false
        });
        
        let canonical_message = serde_json::to_string(&message_data)
            .context("Failed to serialize doorbell message")?;
            
        // Sign the message
        let signature = self.key_pair.sign(canonical_message.as_bytes());
        let signature_hex = hex::encode(signature.as_ref());
        
        Ok(DoorbellRequest {
            device_ip: self.device_ip.clone(),
            ssh_port,
            pubkey_fingerprint,
            ready_until,
            signature: signature_hex,
            backup_mode: false,
        })
    }
    
    pub async fn send_doorbell(
        &self,
        service: &DiscoveredService,
        request: DoorbellRequest,
    ) -> Result<DoorbellResponse> {
        let url = format!("http://{}:{}/doorbell", service.host, service.port);
        
        debug!("Sending doorbell to: {}", url);
        
        let response = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send doorbell request")?;
            
        if !response.status().is_success() {
            bail!("Doorbell request failed with status: {}", response.status());
        }
        
        let doorbell_response: DoorbellResponse = response
            .json()
            .await
            .context("Failed to parse doorbell response")?;
            
        Ok(doorbell_response)
    }
}

fn load_or_generate_keypair() -> Result<Ed25519KeyPair> {
    let key_path = get_key_path();
    
    // Try to load existing key
    if key_path.exists() {
        match std::fs::read(&key_path) {
            Ok(key_bytes) => {
                match Ed25519KeyPair::from_pkcs8(&key_bytes) {
                    Ok(key_pair) => {
                        info!("Loaded existing device key from {:?}", key_path);
                        return Ok(key_pair);
                    }
                    Err(e) => {
                        warn!("Failed to parse existing key, generating new one: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read existing key, generating new one: {}", e);
            }
        }
    }
    
    // Generate new key pair
    let rng = SystemRandom::new();
    let key_pair = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|e| anyhow::anyhow!("Failed to generate ed25519 key pair: {:?}", e))?;
        
    // Save the key
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create key directory")?;
    }
    
    std::fs::write(&key_path, key_pair.as_ref())
        .context("Failed to save device key")?;
        
    // Set secure permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&key_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&key_path, perms)?;
    }
    
    let key_pair = Ed25519KeyPair::from_pkcs8(key_pair.as_ref())
        .map_err(|e| anyhow::anyhow!("Failed to load generated key pair: {:?}", e))?;
        
    info!("Generated new device key at {:?}", key_path);
    Ok(key_pair)
}

fn get_key_fingerprint(key_pair: &Ed25519KeyPair) -> String {
    use ring::digest::{digest, SHA256};
    
    let public_key_bytes = key_pair.public_key().as_ref();
    let digest = digest(&SHA256, public_key_bytes);
    hex::encode(digest.as_ref())
}

fn get_key_path() -> std::path::PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("cardbrick").join("device_key.p8")
    } else {
        // Fallback for embedded systems
        std::path::PathBuf::from("/etc/cardbrick/device_key.p8")
    }
}

fn get_local_ip() -> Result<String> {
    // Simple approach: use first non-loopback interface
    // In practice, you might want more sophisticated network detection
    
    
    // Try to connect to a remote address to determine local IP
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .context("Failed to create UDP socket")?;
        
    socket.connect("8.8.8.8:80")
        .context("Failed to connect to determine local IP")?;
        
    let local_addr = socket.local_addr()
        .context("Failed to get local address")?;
        
    Ok(local_addr.ip().to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    info!("Starting CardBrick sync client");
    
    let mut client = SyncClient::new().await?;
    let services = client.discover_services().await;
    
    if services.is_empty() {
        warn!("No CardBrick sync services discovered");
        return Ok(());
    }
    
    for service in &services {
        info!("Found service: {} at {}:{}", service.name, service.host, service.port);
        
        // Create and send doorbell request
        match client.create_doorbell_request(&service.host, 22).await {
            Ok(request) => {
                match client.send_doorbell(service, request).await {
                    Ok(response) => {
                        info!("Doorbell response: {} - {}", response.status, response.message);
                    }
                    Err(e) => {
                        warn!("Failed to send doorbell to {}: {}", service.host, e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create doorbell request: {}", e);
            }
        }
    }
    
    Ok(())
}

