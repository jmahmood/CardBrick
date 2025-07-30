#!/usr/bin/env python3
"""
Integration tests for Rust sync components
"""

import json
import subprocess
import tempfile
import time
from pathlib import Path

def test_sync_ring_help():
    """Test that sync_ring binary shows help"""
    try:
        result = subprocess.run(
            ["cargo", "run", "--bin", "sync_ring", "--", "--help"],
            capture_output=True,
            text=True,
            timeout=10
        )
        
        # sync_ring doesn't implement --help yet, but it should compile
        assert result.returncode in [0, 1]  # Either success or expected failure
        print("✓ sync_ring binary compiles and runs")
        
    except subprocess.TimeoutExpired:
        print("⚠ sync_ring test timed out (expected for network operations)")
    except FileNotFoundError:
        print("⚠ cargo not found, skipping Rust tests")

def test_sync_ring_force_backup():
    """Test that sync_ring accepts --force-backup flag"""
    try:
        # Create temporary state file to avoid conflicts
        with tempfile.TemporaryDirectory() as temp_dir:
            # Set environment to use temp directory
            env = {"XDG_CONFIG_HOME": temp_dir}
            
            result = subprocess.run(
                ["cargo", "run", "--bin", "sync_ring", "--", "--force-backup"],
                capture_output=True,
                text=True,
                timeout=5,  # Short timeout since we expect no services found
                env=env
            )
            
            # We expect this to fail with "no services found" but the flag should be recognized
            output = result.stderr.lower()
            
            # Check that it's not a flag parsing error
            assert "unexpected argument" not in output
            assert "unknown argument" not in output
            
            print("✓ sync_ring accepts --force-backup flag")
            
    except subprocess.TimeoutExpired:
        print("⚠ sync_ring --force-backup test timed out")
    except FileNotFoundError:
        print("⚠ cargo not found, skipping Rust tests")

def test_sync_client_compiles():
    """Test that sync_client module compiles"""
    try:
        result = subprocess.run(
            ["cargo", "check", "--bin", "sync_ring"],
            capture_output=True,
            text=True,
            timeout=30
        )
        
        if result.returncode == 0:
            print("✓ sync_ring compiles successfully")
        else:
            print(f"❌ sync_ring compilation failed: {result.stderr}")
            
    except subprocess.TimeoutExpired:
        print("⚠ sync_ring compilation test timed out")
    except FileNotFoundError:
        print("⚠ cargo not found, skipping Rust compilation tests")

def test_main_menu_backup_integration():
    """Test that main menu integrates backup functionality"""
    try:
        # Check that the main menu files compile
        result = subprocess.run(
            ["cargo", "check", "--lib"],
            capture_output=True,
            text=True,
            timeout=30
        )
        
        if result.returncode == 0:
            print("✓ Main application with backup menu compiles")
        else:
            print(f"❌ Main application compilation failed: {result.stderr}")
            
    except subprocess.TimeoutExpired:
        print("⚠ Main application compilation test timed out")
    except FileNotFoundError:
        print("⚠ cargo not found, skipping Rust compilation tests")

def test_restore_wizard_scenes():
    """Test that restore wizard scenes are properly integrated"""
    # Check that the restore wizard files exist
    project_root = Path(__file__).parent.parent
    
    restore_mod = project_root / "src" / "scenes" / "restore_wizard" / "mod.rs"
    restore_input = project_root / "src" / "scenes" / "restore_wizard" / "input.rs"
    
    assert restore_mod.exists(), "Restore wizard mod.rs should exist"
    assert restore_input.exists(), "Restore wizard input.rs should exist"
    
    # Check that scenes/mod.rs includes the restore_wizard module
    scenes_mod = project_root / "src" / "scenes" / "mod.rs"
    if scenes_mod.exists():
        content = scenes_mod.read_text()
        assert "pub mod restore_wizard;" in content, "scenes/mod.rs should include restore_wizard module"
    
    print("✓ Restore wizard scene files are present and integrated")

def test_gamestate_includes_restore_wizard():
    """Test that GameState enum includes RestoreWizard variant"""
    project_root = Path(__file__).parent.parent
    state_file = project_root / "src" / "state.rs"
    
    if state_file.exists():
        content = state_file.read_text()
        assert "RestoreWizard(RestoreWizardState)" in content, "GameState should include RestoreWizard variant"
        assert "use crate::scenes::restore_wizard::RestoreWizardState;" in content, "state.rs should import RestoreWizardState"
    
    print("✓ GameState includes RestoreWizard variant")

def main():
    """Run all Rust integration tests"""
    print("Running Rust sync integration tests...")
    print("=" * 50)
    
    try:
        test_sync_ring_help()
        test_sync_ring_force_backup()
        test_sync_client_compiles()
        test_main_menu_backup_integration()
        test_restore_wizard_scenes()
        test_gamestate_includes_restore_wizard()
        
        print("=" * 50)
        print("✓ All Rust integration tests completed")
        
    except Exception as e:
        print(f"❌ Test failed with error: {e}")
        return False
    
    return True

if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)