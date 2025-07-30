"""
Sync operations for CardBrick daemon

Handles rsync operations to pull decks from handheld devices
"""

import asyncio
import logging
import shlex
import shutil
import sqlite3
import time
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional

try:
    from .config import Config
except ImportError:
    from config import Config

logger = logging.getLogger(__name__)

@dataclass
class SyncResult:
    """Result of a sync operation"""
    success: bool
    files_transferred: int = 0
    bytes_transferred: int = 0
    error_message: Optional[str] = None
    duration_seconds: float = 0.0
    rsync_output: str = ""
    
    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization"""
        return {
            "success": self.success,
            "files_transferred": self.files_transferred,
            "bytes_transferred": self.bytes_transferred,
            "error_message": self.error_message,
            "duration_seconds": self.duration_seconds,
            "rsync_output": self.rsync_output
        }

class SyncManager:
    """Manages rsync operations for deck synchronization"""
    
    def __init__(self, config: Config):
        self.config = config
        
    async def sync_from_device(
        self, 
        device_ip: str, 
        ssh_port: int,
        pubkey_fingerprint: str
    ) -> SyncResult:
        """
        Sync decks from handheld device using rsync over SSH
        
        Args:
            device_ip: IP address of the handheld device
            ssh_port: SSH port on the device  
            pubkey_fingerprint: Device public key fingerprint for logging
            
        Returns:
            SyncResult with operation details
        """
        start_time = asyncio.get_event_loop().time()
        
        try:
            # Prepare rsync command
            rsync_cmd = self._build_rsync_command(device_ip, ssh_port)
            
            logger.info(f"Starting sync from {device_ip}:{ssh_port} (fingerprint: {pubkey_fingerprint[:8]}...)")
            logger.debug(f"Rsync command: {' '.join(rsync_cmd)}")
            
            # Execute rsync
            process = await asyncio.create_subprocess_exec(
                *rsync_cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.STDOUT,
                env={"LC_ALL": "C"}  # Consistent output format
            )
            
            # Wait for completion with timeout
            try:
                stdout, _ = await asyncio.wait_for(
                    process.communicate(),
                    timeout=self.config.rsync_timeout_seconds
                )
            except asyncio.TimeoutError:
                process.kill()
                await process.wait()
                return SyncResult(
                    success=False,
                    error_message=f"Rsync timeout after {self.config.rsync_timeout_seconds} seconds",
                    duration_seconds=asyncio.get_event_loop().time() - start_time
                )
            
            duration = asyncio.get_event_loop().time() - start_time
            output = stdout.decode('utf-8', errors='replace')
            
            # Parse rsync output for statistics
            files_transferred, bytes_transferred = self._parse_rsync_stats(output)
            
            if process.returncode == 0:
                logger.info(f"Sync completed successfully from {device_ip}: {files_transferred} files, {bytes_transferred} bytes")
                
                # Apply conflict resolution after successful sync
                await self._resolve_conflicts()
                
                return SyncResult(
                    success=True,
                    files_transferred=files_transferred,
                    bytes_transferred=bytes_transferred,
                    duration_seconds=duration,
                    rsync_output=output
                )
            else:
                logger.warning(f"Rsync failed from {device_ip} (exit code {process.returncode})")
                return SyncResult(
                    success=False,
                    error_message=f"Rsync exit code {process.returncode}",
                    duration_seconds=duration,
                    rsync_output=output
                )
                
        except Exception as e:
            duration = asyncio.get_event_loop().time() - start_time
            logger.error(f"Sync error from {device_ip}: {e}")
            return SyncResult(
                success=False,
                error_message=str(e),
                duration_seconds=duration
            )
    
    def _build_rsync_command(self, device_ip: str, ssh_port: int) -> List[str]:
        """Build rsync command with proper SSH options"""
        
        # Base rsync options
        rsync_opts = [
            "rsync",
            "--archive",        # Preserve permissions, timestamps, etc.
            "--verbose",        # Verbose output for parsing
            "--compress",       # Compress during transfer
            "--stats",          # Show transfer statistics
            "--human-readable", # Human readable sizes
        ]
        
        # Add partial transfers if enabled
        if self.config.rsync_partial:
            rsync_opts.append("--partial")
            
        # SSH options for security and compatibility
        ssh_opts = [
            f"-i {self.config.ssh_key_path}",
            "-o StrictHostKeyChecking=yes",
            f"-o UserKnownHostsFile={self.config.known_hosts_path}",
            "-o BatchMode=yes",  # No interactive prompts
            "-o ConnectTimeout=30",
            f"-p {ssh_port}"
        ]
        
        # Combine SSH options
        ssh_command = f"ssh {' '.join(ssh_opts)}"
        rsync_opts.extend(["-e", ssh_command])
        
        # Source and destination
        source = f"cardbrick@{device_ip}:/flash/decks/"
        destination = str(self.config.inbox_dir) + "/"
        
        rsync_opts.extend([source, destination])
        
        return rsync_opts
    
    def _parse_rsync_stats(self, output: str) -> tuple[int, int]:
        """
        Parse rsync output to extract transfer statistics
        
        Returns:
            (files_transferred, bytes_transferred)
        """
        files_transferred = 0
        bytes_transferred = 0
        
        for line in output.split('\n'):
            line = line.strip()
            
            # Look for transfer summary lines
            if "Number of files transferred:" in line:
                try:
                    files_transferred = int(line.split(':')[1].strip())
                except (ValueError, IndexError):
                    pass
                    
            elif "Total transferred file size:" in line:
                try:
                    # Extract bytes (rsync shows human readable, we need bytes)
                    parts = line.split(':')[1].strip().split()
                    if len(parts) >= 2:
                        # Parse human readable format (e.g., "1.23M bytes")
                        size_str = parts[0]
                        if size_str.replace('.', '').replace(',', '').isdigit():
                            bytes_transferred = int(float(size_str.replace(',', '')))
                        elif size_str[-1].upper() in 'KMGT':
                            # Handle K, M, G, T suffixes
                            multipliers = {'K': 1024, 'M': 1024**2, 'G': 1024**3, 'T': 1024**4}
                            suffix = size_str[-1].upper()
                            if suffix in multipliers:
                                num = float(size_str[:-1].replace(',', ''))
                                bytes_transferred = int(num * multipliers[suffix])
                except (ValueError, IndexError):
                    pass
                    
            # Alternative: look for files actually transferred (not just matched)
            elif line.startswith('>') and not line.startswith('>.'):
                # This is a file being transferred
                files_transferred += 1
        
        return files_transferred, bytes_transferred
    
    async def test_device_connectivity(self, device_ip: str, ssh_port: int) -> bool:
        """
        Test if we can connect to a device via SSH
        
        Returns:
            True if connection successful, False otherwise
        """
        try:
            ssh_cmd = [
                "ssh",
                f"-i{self.config.ssh_key_path}",
                "-oStrictHostKeyChecking=yes",
                f"-oUserKnownHostsFile={self.config.known_hosts_path}",
                "-oBatchMode=yes",
                "-oConnectTimeout=10",
                f"-p{ssh_port}",
                f"cardbrick@{device_ip}",
                "echo 'connection_test'"
            ]
            
            process = await asyncio.create_subprocess_exec(
                *ssh_cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            
            stdout, stderr = await asyncio.wait_for(process.communicate(), timeout=15)
            
            if process.returncode == 0 and b'connection_test' in stdout:
                logger.info(f"Successfully tested connection to {device_ip}:{ssh_port}")
                return True
            else:
                logger.warning(f"Connection test failed to {device_ip}:{ssh_port}")
                return False
                
        except Exception as e:
            logger.warning(f"Connection test error to {device_ip}:{ssh_port}: {e}")
            return False
    
    async def _resolve_conflicts(self):
        """
        Resolve conflicts using last-writer-wins strategy
        
        Checks for files that exist in both PC and device with different timestamps
        """
        logger.info("Checking for sync conflicts...")
        
        inbox_dir = self.config.inbox_dir
        if not inbox_dir.exists():
            return
        
        conflicts_resolved = 0
        
        # Check each database file for conflicts
        for db_file in inbox_dir.rglob("*.db"):
            if await self._resolve_database_conflict(db_file):
                conflicts_resolved += 1
        
        # Check media files for conflicts
        for media_file in inbox_dir.rglob("*"):
            if media_file.is_file() and not media_file.name.endswith('.db'):
                if await self._resolve_media_conflict(media_file):
                    conflicts_resolved += 1
        
        if conflicts_resolved > 0:
            logger.info(f"Resolved {conflicts_resolved} sync conflicts")
    
    async def _resolve_database_conflict(self, db_file: Path) -> bool:
        """
        Handle database file conflicts using last-writer-wins
        
        Returns True if a conflict was resolved
        """
        try:
            # Check if there's a corresponding local version
            local_db = self._get_local_db_path(db_file)
            
            if not local_db or not local_db.exists():
                return False
            
            # Compare modification times
            device_mtime = db_file.stat().st_mtime
            local_mtime = local_db.stat().st_mtime
            
            if abs(device_mtime - local_mtime) < 1:  # Less than 1 second difference
                return False  # No meaningful conflict
            
            if device_mtime > local_mtime:
                # Device version is newer - keep it, backup local
                backup_path = local_db.with_suffix(f".conflict_{int(local_mtime)}.db")
                shutil.move(str(local_db), str(backup_path))
                shutil.move(str(db_file), str(local_db))
                
                logger.info(f"Conflict resolved: kept newer device version of {local_db.name}")
                logger.info(f"Backed up older local version to {backup_path.name}")
                
            else:
                # Local version is newer - keep it, backup device version
                backup_path = db_file.with_suffix(f".conflict_{int(device_mtime)}.db")
                shutil.move(str(db_file), str(backup_path))
                
                logger.info(f"Conflict resolved: kept newer local version of {local_db.name}")
                logger.info(f"Backed up older device version to {backup_path.name}")
            
            return True
            
        except Exception as e:
            logger.error(f"Failed to resolve database conflict for {db_file}: {e}")
            return False
    
    async def _resolve_media_conflict(self, media_file: Path) -> bool:
        """
        Handle media file conflicts using MD5 hash comparison
        
        Returns True if a conflict was resolved
        """
        try:
            # Check if there's a corresponding local version
            local_file = self._get_local_media_path(media_file)
            
            if not local_file or not local_file.exists():
                return False
            
            # Compare file hashes
            device_hash = await self._calculate_file_hash(media_file)
            local_hash = await self._calculate_file_hash(local_file)
            
            if device_hash == local_hash:
                # Files are identical - remove duplicate
                media_file.unlink()
                logger.debug(f"Removed duplicate media file: {media_file.name}")
                return True
            
            # Files are different - use modification time for conflict resolution
            device_mtime = media_file.stat().st_mtime
            local_mtime = local_file.stat().st_mtime
            
            if device_mtime > local_mtime:
                # Device version is newer
                backup_path = local_file.with_suffix(f".conflict_{int(local_mtime)}{local_file.suffix}")
                shutil.move(str(local_file), str(backup_path))
                shutil.move(str(media_file), str(local_file))
                
                logger.info(f"Media conflict resolved: kept newer device version of {local_file.name}")
                
            else:
                # Local version is newer
                backup_path = media_file.with_suffix(f".conflict_{int(device_mtime)}{media_file.suffix}")
                shutil.move(str(media_file), str(backup_path))
                
                logger.info(f"Media conflict resolved: kept newer local version of {local_file.name}")
            
            return True
            
        except Exception as e:
            logger.error(f"Failed to resolve media conflict for {media_file}: {e}")
            return False
    
    def _get_local_db_path(self, device_db: Path) -> Optional[Path]:
        """Get the corresponding local database path"""
        # This would need to be customized based on your directory structure
        # For now, assume a simple mapping
        local_data_dir = self.config.data_dir / "local_dbs"
        return local_data_dir / device_db.name
    
    def _get_local_media_path(self, device_media: Path) -> Optional[Path]:
        """Get the corresponding local media path"""
        # This would need to be customized based on your directory structure
        local_media_dir = self.config.data_dir / "local_media"
        return local_media_dir / device_media.name
    
    async def _calculate_file_hash(self, file_path: Path) -> str:
        """Calculate MD5 hash of a file"""
        import hashlib
        
        hash_md5 = hashlib.md5()
        try:
            with open(file_path, "rb") as f:
                for chunk in iter(lambda: f.read(4096), b""):
                    hash_md5.update(chunk)
            return hash_md5.hexdigest()
        except Exception as e:
            logger.error(f"Failed to calculate hash for {file_path}: {e}")
            return ""