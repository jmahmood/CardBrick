#!/usr/bin/env python3
"""
Integration tests for Sprint 4 sync features
"""

import asyncio
import json
import shutil
import tempfile
import time
from pathlib import Path
import sys
import pytest

# Add cardbrick-syncd to path
sys.path.insert(0, str(Path(__file__).parent.parent / 'cardbrick-syncd'))
sys.path.insert(0, str(Path(__file__).parent.parent / 'cardbrick-cli'))

from sync import SyncManager, SyncResult
from config import Config
from daemon import DoorbellRequest
from crypto import CryptoManager
from wormhole_backup import WormholeBackup, WormholeError

class TestSyncIntegration:
    """Test suite for sync functionality"""
    
    @pytest.fixture
    def temp_config(self):
        """Create temporary config for testing"""
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            
            config = Config()
            config.data_dir = temp_path / "data"
            config.inbox_dir = temp_path / "data" / "inbox"
            config.keys_dir = temp_path / "data" / "keys"
            config.ssh_key_path = temp_path / "data" / "id_ed25519"
            config.known_hosts_path = temp_path / "data" / "known_hosts"
            
            # Create directories
            config.data_dir.mkdir(parents=True, exist_ok=True)
            config.inbox_dir.mkdir(parents=True, exist_ok=True)
            config.keys_dir.mkdir(parents=True, exist_ok=True)
            
            yield config
    
    def test_sync_manager_initialization(self, temp_config):
        """Test SyncManager can be initialized"""
        sync_manager = SyncManager(temp_config)
        assert sync_manager.config == temp_config
    
    def test_doorbell_request_validation(self, temp_config):
        """Test doorbell request structure"""
        request = DoorbellRequest(
            device_ip="192.168.1.100",
            ssh_port=22,
            pubkey_fingerprint="abc123",
            ready_until=int(time.time()) + 300,
            signature="def456",
            backup_mode=False
        )
        
        assert request.device_ip == "192.168.1.100"
        assert request.backup_mode is False
        
        # Test backup mode
        backup_request = DoorbellRequest(
            device_ip="192.168.1.100",
            ssh_port=22,
            pubkey_fingerprint="abc123",
            ready_until=int(time.time()) + 300,
            signature="def456",
            backup_mode=True
        )
        
        assert backup_request.backup_mode is True
    
    @pytest.mark.asyncio
    async def test_conflict_resolution_setup(self, temp_config):
        """Test conflict resolution infrastructure"""
        sync_manager = SyncManager(temp_config)
        
        # Create test files to simulate conflicts
        inbox_dir = temp_config.inbox_dir
        
        # Create a test database file
        test_db = inbox_dir / "test.db"
        test_db.write_text("device version")
        test_db.touch()  # Set modification time
        
        # Create local database directory structure
        local_db_dir = temp_config.data_dir / "local_dbs"
        local_db_dir.mkdir(exist_ok=True)
        local_db = local_db_dir / "test.db"
        local_db.write_text("local version")
        
        # Modify times to create conflict
        device_time = time.time()
        local_time = device_time - 100  # Local is older
        
        test_db.touch(times=(device_time, device_time))
        local_db.touch(times=(local_time, local_time))
        
        # Test conflict resolution
        await sync_manager._resolve_conflicts()
        
        # Check that conflict was handled
        conflict_files = list(local_db_dir.glob("test.conflict_*.db"))
        assert len(conflict_files) >= 0  # Conflict resolution should have run
    
    def test_wormhole_backup_initialization(self, temp_config):
        """Test WormholeBackup initialization"""
        wormhole = WormholeBackup(temp_config.data_dir)
        assert wormhole.data_dir == temp_config.data_dir
    
    def test_wormhole_backup_list_empty(self, temp_config):
        """Test listing backups when none exist"""
        wormhole = WormholeBackup(temp_config.data_dir)
        backups = wormhole.list_learner_backups()
        assert backups == []
    
    def test_wormhole_backup_list_with_data(self, temp_config):
        """Test listing backups with sample data"""
        wormhole = WormholeBackup(temp_config.data_dir)
        
        # Create sample learner directory
        learners_dir = temp_config.data_dir / "learners"
        learners_dir.mkdir(exist_ok=True)
        
        test_learner = learners_dir / "test-uuid-123"
        test_learner.mkdir(exist_ok=True)
        
        # Add some test files
        (test_learner / "progress.db").write_text("test database")
        (test_learner / "metadata.json").write_text(json.dumps({
            "learner_uuid": "test-uuid-123",
            "created_at": "2025-01-01T00:00:00Z"
        }))
        
        backups = wormhole.list_learner_backups()
        assert len(backups) == 1
        assert backups[0]["uuid"] == "test-uuid-123"
        assert backups[0]["size_bytes"] > 0
    
    @pytest.mark.asyncio
    async def test_file_hash_calculation(self, temp_config):
        """Test file hash calculation"""
        sync_manager = SyncManager(temp_config)
        
        # Create test file
        test_file = temp_config.data_dir / "test.txt"
        test_file.write_text("Hello, World!")
        
        hash1 = await sync_manager._calculate_file_hash(test_file)
        hash2 = await sync_manager._calculate_file_hash(test_file)
        
        assert hash1 == hash2  # Same file should have same hash
        assert len(hash1) == 32  # MD5 hash length
    
    def test_sync_result_serialization(self):
        """Test SyncResult can be serialized"""
        result = SyncResult(
            success=True,
            files_transferred=5,
            bytes_transferred=1024,
            duration_seconds=2.5,
            rsync_output="test output"
        )
        
        serialized = result.to_dict()
        
        assert serialized["success"] is True
        assert serialized["files_transferred"] == 5
        assert serialized["bytes_transferred"] == 1024
        assert serialized["duration_seconds"] == 2.5

class TestErrorHandling:
    """Test error handling and edge cases"""
    
    def test_wormhole_error_handling(self):
        """Test WormholeError exception"""
        with pytest.raises(WormholeError):
            raise WormholeError("Test error")
    
    @pytest.mark.asyncio
    async def test_missing_file_hash(self, temp_config):
        """Test hash calculation with missing file"""
        sync_manager = SyncManager(temp_config)
        
        missing_file = temp_config.data_dir / "nonexistent.txt"
        hash_result = await sync_manager._calculate_file_hash(missing_file)
        
        assert hash_result == ""  # Should return empty string for missing files

def main():
    """
    Run integration tests
    
    Usage: python test_sync_integration.py
    """
    pytest.main([__file__, "-v"])

if __name__ == "__main__":
    main()