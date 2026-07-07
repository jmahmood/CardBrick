"""
Configuration management for CardBrick sync daemon
"""

import os
from pathlib import Path
from dataclasses import dataclass

@dataclass
class Config:
    """Configuration for the sync daemon"""
    
    # Network settings
    http_port: int = 6429
    service_name: str = "_cardbrick._tcp"
    
    # Storage paths
    data_dir: Path = Path("/var/lib/cardbrick")
    inbox_dir: Path = Path("/var/lib/cardbrick/inbox")
    keys_dir: Path = Path("/var/lib/cardbrick/keys")
    
    # SSH settings
    ssh_key_path: Path = Path("/var/lib/cardbrick/id_ed25519")
    known_hosts_path: Path = Path("/var/lib/cardbrick/known_hosts")
    
    # Security settings
    max_doorbell_age_seconds: int = 300  # 5 minutes
    rate_limit_window_seconds: int = 900  # 15 minutes
    
    # Rsync settings
    rsync_timeout_seconds: int = 300  # 5 minutes
    rsync_partial: bool = True
    
    @classmethod
    def load(cls) -> 'Config':
        """Load configuration from environment variables and defaults"""
        config = cls()
        
        # Override with environment variables if present
        if port := os.getenv('CARDBRICK_HTTP_PORT'):
            config.http_port = int(port)
            
        if data_dir := os.getenv('CARDBRICK_DATA_DIR'):
            config.data_dir = Path(data_dir)
            config.inbox_dir = config.data_dir / "inbox"
            config.keys_dir = config.data_dir / "keys"
            config.ssh_key_path = config.data_dir / "id_ed25519"
            config.known_hosts_path = config.data_dir / "known_hosts"
            
        if timeout := os.getenv('CARDBRICK_RSYNC_TIMEOUT'):
            config.rsync_timeout_seconds = int(timeout)
            
        # Ensure directories exist
        config.data_dir.mkdir(parents=True, exist_ok=True)
        config.inbox_dir.mkdir(parents=True, exist_ok=True)
        config.keys_dir.mkdir(parents=True, exist_ok=True)
        
        return config