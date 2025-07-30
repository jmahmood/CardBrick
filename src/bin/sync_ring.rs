/*!
 * CardBrick Sync Ring - Handheld Sync Client
 * 
 * Lightweight binary for handheld devices to trigger sync operations.
 * 
 * Features:
 * - mDNS discovery of desktop daemons
 * - ed25519 authentication 
 * - Rate limiting (max 1 ring per 15 minutes)
 * - Minimal resource usage for embedded devices
 */

use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::HashMap;

use anyhow::{Result, Context};
use log::{info, warn, error};
use serde::{Serialize, Deserialize};
use tokio::time::timeout;

mod sync_client;
use sync_client::{SyncClient, DiscoveredService, DoorbellResponse};

#[derive(Debug, Serialize, Deserialize)]
struct SyncState {
    last_ring_time: u64,
    known_services: HashMap<String, DiscoveredService>,
}

#[derive(Debug)]
struct Args {
    force_backup: bool,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            last_ring_time: 0,
            known_services: HashMap::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    // Parse command line arguments
    let args = parse_args();
    
    info!("CardBrick Sync Ring starting...");
    
    // Load or create sync state
    let mut state = load_sync_state().unwrap_or_default();
    
    // Check rate limiting (skip for force backup)
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();
        
    const RATE_LIMIT_SECONDS: u64 = 15 * 60; // 15 minutes
    
    if !args.force_backup && current_time - state.last_ring_time < RATE_LIMIT_SECONDS {
        let wait_time = RATE_LIMIT_SECONDS - (current_time - state.last_ring_time);
        warn!("Rate limited. Next sync allowed in {} minutes", wait_time / 60);
        return Ok(());
    }
    
    // Initialize sync client
    let mut client = SyncClient::new().await?;
    
    // Discover desktop services
    info!("Discovering CardBrick sync services...");
    let discovered = timeout(Duration::from_secs(10), client.discover_services())
        .await
        .context("Service discovery timeout")?;
        
    if discovered.is_empty() {
        warn!("No CardBrick sync services found on network");
        return Ok(());
    }
    
    info!("Found {} sync service(s)", discovered.len());
    
    // Try to sync with each discovered service
    let mut sync_success = false;
    
    for service in discovered {
        info!("Attempting sync with {} ({}:{})", 
              service.name, service.host, service.port);
              
        match attempt_sync(&mut client, &service, args.force_backup).await {
            Ok(response) => {
                info!("Sync request successful: {} - {}", 
                      response.status, response.message);
                if response.status == "accepted" {
                    sync_success = true;
                    break;
                }
            }
            Err(e) => {
                warn!("Sync failed with {} - {}", service.name, e);
                continue;
            }
        }
    }
    
    if sync_success {
        // Update rate limiting state (except for force backup)
        if !args.force_backup {
            state.last_ring_time = current_time;
            save_sync_state(&state)?;
        }
        let sync_type = if args.force_backup { "Manual backup" } else { "Sync" };
        info!("{} initiated successfully", sync_type);
    } else {
        error!("All sync attempts failed");
    }
    
    Ok(())
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    
    Args {
        force_backup: args.contains(&"--force-backup".to_string()),
    }
}

async fn attempt_sync(
    client: &mut SyncClient,
    service: &DiscoveredService,
    force_backup: bool
) -> Result<DoorbellResponse> {
    // Create doorbell request
    let mut request = client.create_doorbell_request(
        &service.host,
        22, // Default SSH port - could be configurable
    ).await?;
    
    // Mark as backup request if forced
    if force_backup {
        request.backup_mode = true;
    }
    
    // Send doorbell
    let response = client.send_doorbell(&service, request).await?;
    
    Ok(response)
}

fn load_sync_state() -> Result<SyncState> {
    let state_path = get_state_path();
    
    if !state_path.exists() {
        return Ok(SyncState::default());
    }
    
    let contents = std::fs::read_to_string(&state_path)
        .context("Failed to read sync state")?;
        
    let state: SyncState = serde_json::from_str(&contents)
        .context("Failed to parse sync state")?;
        
    Ok(state)
}

fn save_sync_state(state: &SyncState) -> Result<()> {
    let state_path = get_state_path();
    
    // Ensure parent directory exists
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create state directory")?;
    }
    
    let contents = serde_json::to_string_pretty(state)
        .context("Failed to serialize sync state")?;
        
    std::fs::write(&state_path, contents)
        .context("Failed to write sync state")?;
        
    Ok(())
}

fn get_state_path() -> std::path::PathBuf {
    // Use platform-appropriate state directory
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("cardbrick").join("sync_state.json")
    } else {
        // Fallback for embedded systems
        std::path::PathBuf::from("/tmp/cardbrick_sync_state.json")
    }
}