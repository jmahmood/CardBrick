"""
Wormhole backup functionality for CardBrick

Handles sending and receiving backup packages via magic-wormhole
"""

import asyncio
import json
import logging
import shutil
import tarfile
import tempfile
from pathlib import Path
from typing import Optional, Dict, Any

from wormhole import wormhole

logger = logging.getLogger(__name__)

class WormholeError(Exception):
    """Base exception for wormhole operations"""
    pass

class WormholeBackup:
    """Manages wormhole backup operations"""
    
    def __init__(self, data_dir: Path):
        self.data_dir = Path(data_dir)
        
    async def send_backup(self, learner_uuid: str) -> str:
        """
        Send a learner's complete backup via wormhole
        
        Returns the wormhole code for the receiver
        """
        logger.info(f"Creating backup package for learner {learner_uuid}")
        
        # Create learner directory path
        learner_dir = self.data_dir / "learners" / learner_uuid
        
        if not learner_dir.exists():
            raise WormholeError(f"Learner directory not found: {learner_dir}")
        
        # Create temporary backup archive
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            archive_path = temp_path / f"cardbrick_backup_{learner_uuid}.tar.zst"
            
            # Create compressed archive
            self._create_backup_archive(learner_dir, archive_path)
            
            # Send via wormhole
            logger.info("Sending backup via magic-wormhole...")
            code = await self._send_file_via_wormhole(archive_path)
            
            logger.info(f"Backup sent successfully. Wormhole code: {code}")
            return code
    
    async def receive_backup(self, wormhole_code: str, target_uuid: Optional[str] = None) -> str:
        """
        Receive a backup via wormhole and restore it
        
        Returns the UUID of the restored learner
        """
        logger.info(f"Receiving backup with code: {wormhole_code}")
        
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            
            # Receive file via wormhole
            received_file = await self._receive_file_via_wormhole(wormhole_code, temp_path)
            
            if not received_file or not received_file.exists():
                raise WormholeError("Failed to receive backup file")
            
            # Extract and restore
            restored_uuid = self._restore_backup_archive(received_file, target_uuid)
            
            logger.info(f"Backup restored successfully as learner: {restored_uuid}")
            return restored_uuid
    
    def _create_backup_archive(self, learner_dir: Path, archive_path: Path):
        """Create a compressed backup archive"""
        try:
            # Create tar.zst archive (fall back to tar.gz if zstd not available)
            try:
                import zstandard as zstd
                # Use zstd compression
                with open(archive_path, 'wb') as f:
                    cctx = zstd.ZstdCompressor(level=3)
                    with cctx.stream_writer(f) as compressor:
                        with tarfile.open(fileobj=compressor, mode='w|') as tar:
                            tar.add(learner_dir, arcname='cardbrick_backup')
            except ImportError:
                # Fall back to gzip
                archive_path = archive_path.with_suffix('.tar.gz')
                with tarfile.open(archive_path, 'w:gz') as tar:
                    tar.add(learner_dir, arcname='cardbrick_backup')
                    
            logger.info(f"Created backup archive: {archive_path} ({archive_path.stat().st_size} bytes)")
            
        except Exception as e:
            raise WormholeError(f"Failed to create backup archive: {e}")
    
    def _restore_backup_archive(self, archive_path: Path, target_uuid: Optional[str] = None) -> str:
        """Extract and restore a backup archive"""
        try:
            # Determine compression type
            if archive_path.suffix == '.zst':
                # zstd compressed
                import zstandard as zstd
                with open(archive_path, 'rb') as f:
                    dctx = zstd.ZstdDecompressor()
                    with dctx.stream_reader(f) as reader:
                        with tarfile.open(fileobj=reader, mode='r|') as tar:
                            return self._extract_backup(tar, target_uuid)
            else:
                # Standard tar
                with tarfile.open(archive_path, 'r:*') as tar:
                    return self._extract_backup(tar, target_uuid)
                    
        except Exception as e:
            raise WormholeError(f"Failed to restore backup: {e}")
    
    def _extract_backup(self, tar: tarfile.TarFile, target_uuid: Optional[str] = None) -> str:
        """Extract backup and return the learner UUID"""
        # Create temporary extraction directory
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            
            # Extract archive
            tar.extractall(temp_path)
            
            # Find the backup directory
            backup_dir = temp_path / 'cardbrick_backup'
            if not backup_dir.exists():
                raise WormholeError("Invalid backup archive - missing cardbrick_backup directory")
            
            # Read metadata to get original UUID
            metadata_file = backup_dir / 'metadata.json'
            original_uuid = None
            
            if metadata_file.exists():
                try:
                    with open(metadata_file) as f:
                        metadata = json.load(f)
                        original_uuid = metadata.get('learner_uuid')
                except Exception as e:
                    logger.warning(f"Could not read backup metadata: {e}")
            
            # Determine target UUID
            restore_uuid = target_uuid or original_uuid or self._generate_uuid()
            
            # Create target directory
            target_dir = self.data_dir / "learners" / restore_uuid
            target_dir.parent.mkdir(parents=True, exist_ok=True)
            
            # Remove existing directory if it exists
            if target_dir.exists():
                shutil.rmtree(target_dir)
            
            # Move backup to target location
            shutil.move(str(backup_dir), str(target_dir))
            
            # Update metadata with new UUID if changed
            if restore_uuid != original_uuid:
                self._update_backup_metadata(target_dir, restore_uuid)
            
            return restore_uuid
    
    def _update_backup_metadata(self, learner_dir: Path, new_uuid: str):
        """Update metadata with new UUID"""
        metadata_file = learner_dir / 'metadata.json'
        metadata = {
            'learner_uuid': new_uuid,
            'restored_at': self._current_timestamp(),
        }
        
        if metadata_file.exists():
            try:
                with open(metadata_file) as f:
                    existing = json.load(f)
                    metadata.update(existing)
            except Exception:
                pass  # Use new metadata if existing is corrupted
        
        metadata['learner_uuid'] = new_uuid  # Always update UUID
        
        with open(metadata_file, 'w') as f:
            json.dump(metadata, f, indent=2)
    
    async def _send_file_via_wormhole(self, file_path: Path) -> str:
        """Send file via magic wormhole"""
        try:
            # Create wormhole sender
            w = wormhole.create(
                appid="cardbrick.backup.transfer",
                relay_url="ws://relay.magic-wormhole.io:4000/v1",
                reactor=None  # Use default reactor
            )
            
            # Allocate code
            code = await w.get_code()
            logger.info(f"Wormhole code generated: {code}")
            
            # Send file
            await w.send_file(str(file_path))
            
            # Close wormhole
            await w.close()
            
            return code
            
        except Exception as e:
            raise WormholeError(f"Failed to send via wormhole: {e}")
    
    async def _receive_file_via_wormhole(self, code: str, target_dir: Path) -> Optional[Path]:
        """Receive file via magic wormhole"""
        try:
            # Create wormhole receiver
            w = wormhole.create(
                appid="cardbrick.backup.transfer",
                relay_url="ws://relay.magic-wormhole.io:4000/v1",
                reactor=None
            )
            
            # Set code
            await w.set_code(code)
            
            # Receive file
            received_file = await w.receive_file(str(target_dir))
            
            # Close wormhole
            await w.close()
            
            return Path(received_file) if received_file else None
            
        except Exception as e:
            raise WormholeError(f"Failed to receive via wormhole: {e}")
    
    def _generate_uuid(self) -> str:
        """Generate a new UUID for restored backups"""
        import uuid
        return str(uuid.uuid4())
    
    def _current_timestamp(self) -> str:
        """Get current timestamp as ISO string"""
        from datetime import datetime, timezone
        return datetime.now(timezone.utc).isoformat()
    
    def list_learner_backups(self) -> list[Dict[str, Any]]:
        """List available learner directories for backup"""
        learners_dir = self.data_dir / "learners"
        
        if not learners_dir.exists():
            return []
        
        backups = []
        for item in learners_dir.iterdir():
            if item.is_dir():
                metadata_file = item / "metadata.json"
                metadata = {}
                
                if metadata_file.exists():
                    try:
                        with open(metadata_file) as f:
                            metadata = json.load(f)
                    except Exception:
                        pass
                
                # Calculate directory size
                total_size = sum(f.stat().st_size for f in item.rglob('*') if f.is_file())
                
                backups.append({
                    'uuid': item.name,
                    'path': str(item),
                    'size_bytes': total_size,
                    'metadata': metadata
                })
        
        return backups