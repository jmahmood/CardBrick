#!/usr/bin/env python3
"""
CardBrick CLI Tool

Command-line interface for managing CardBrick decks and sync operations.
"""

import sys
import logging
import asyncio
from pathlib import Path

import click

from .importer import import_file, ImporterError
from .sync_queue import SyncQueue, SyncQueueError
from .wormhole_backup import WormholeBackup, WormholeError

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

@click.group()
@click.option('--verbose', '-v', is_flag=True, help='Enable verbose logging')
@click.option('--data-dir', '-d', type=click.Path(), 
              default='/var/lib/cardbrick', help='Data directory path')
@click.pass_context
def cli(ctx, verbose, data_dir):
    """CardBrick CLI - Manage decks and sync operations"""
    if verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Store common options in context
    ctx.ensure_object(dict)
    ctx.obj['data_dir'] = Path(data_dir)
    ctx.obj['verbose'] = verbose

@cli.command()
@click.argument('file_path', type=click.Path(exists=True))
@click.option('--output-dir', '-o', type=click.Path(), 
              help='Output directory (default: data_dir/inbox)')
@click.option('--deck-name', '-n', help='Custom deck name')
@click.option('--auto-sync', is_flag=True, 
              help='Mark deck for automatic sync to handheld devices')
@click.pass_context
def import_deck(ctx, file_path, output_dir, deck_name, auto_sync):
    """Import CSV or Markdown file to CardBrick deck format"""
    try:
        data_dir = ctx.obj['data_dir']
        
        # Default output directory
        if not output_dir:
            output_dir = data_dir / 'inbox'
        else:
            output_dir = Path(output_dir)
        
        # Ensure output directory exists
        output_dir.mkdir(parents=True, exist_ok=True)
        
        click.echo(f"Importing {file_path}...")
        
        # Import the file
        output_path = import_file(Path(file_path), output_dir, deck_name)
        
        # Add to sync queue if requested
        if auto_sync:
            try:
                sync_queue = SyncQueue(data_dir)
                sync_queue.add_deck(output_path)
                click.echo(f"✓ Added to sync queue: {output_path.name}")
            except SyncQueueError as e:
                click.echo(f"Warning: Failed to add to sync queue: {e}", err=True)
        
        click.echo(f"✓ Successfully imported: {output_path}")
        
    except ImporterError as e:
        click.echo(f"Import failed: {e}", err=True)
        sys.exit(1)
    except Exception as e:
        click.echo(f"Unexpected error: {e}", err=True)
        if ctx.obj['verbose']:
            import traceback
            traceback.print_exc()
        sys.exit(1)

@cli.command()
@click.pass_context
def list_queue(ctx):
    """List decks in the sync queue"""
    try:
        data_dir = ctx.obj['data_dir']
        sync_queue = SyncQueue(data_dir)
        
        pending_decks = sync_queue.list_pending()
        
        if not pending_decks:
            click.echo("No decks in sync queue")
            return
        
        click.echo("Pending sync decks:")
        for deck_info in pending_decks:
            status_icon = "⏳" if deck_info['status'] == 'pending' else "❌"
            click.echo(f"  {status_icon} {deck_info['name']} ({deck_info['path']})")
            if deck_info.get('error'):
                click.echo(f"    Error: {deck_info['error']}")
                
    except SyncQueueError as e:
        click.echo(f"Failed to list queue: {e}", err=True)
        sys.exit(1)

@cli.command()
@click.argument('deck_name')
@click.pass_context  
def remove_from_queue(ctx, deck_name):
    """Remove a deck from the sync queue"""
    try:
        data_dir = ctx.obj['data_dir']
        sync_queue = SyncQueue(data_dir)
        
        if sync_queue.remove_deck(deck_name):
            click.echo(f"✓ Removed {deck_name} from sync queue")
        else:
            click.echo(f"Deck {deck_name} not found in queue")
            
    except SyncQueueError as e:
        click.echo(f"Failed to remove from queue: {e}", err=True)
        sys.exit(1)

@cli.command()
@click.pass_context
def clear_queue(ctx):
    """Clear all decks from sync queue"""
    try:
        data_dir = ctx.obj['data_dir']
        sync_queue = SyncQueue(data_dir)
        
        cleared_count = sync_queue.clear_all()
        click.echo(f"✓ Cleared {cleared_count} deck(s) from sync queue")
        
    except SyncQueueError as e:
        click.echo(f"Failed to clear queue: {e}", err=True)
        sys.exit(1)

@cli.command()
@click.pass_context
def status(ctx):
    """Show sync daemon and queue status"""
    try:
        data_dir = ctx.obj['data_dir']
        
        click.echo("CardBrick Sync Status")
        click.echo("===================")
        
        # Check if directories exist
        click.echo(f"Data directory: {data_dir}")
        click.echo(f"  Exists: {'✓' if data_dir.exists() else '❌'}")
        
        inbox_dir = data_dir / 'inbox'
        click.echo(f"Inbox directory: {inbox_dir}")
        click.echo(f"  Exists: {'✓' if inbox_dir.exists() else '❌'}")
        
        if inbox_dir.exists():
            deck_files = list(inbox_dir.glob('*.cbdeck'))
            click.echo(f"  Deck files: {len(deck_files)}")
        
        # Sync queue status
        try:
            sync_queue = SyncQueue(data_dir)
            pending_count = len(sync_queue.list_pending())
            click.echo(f"Sync queue: {pending_count} pending")
        except SyncQueueError:
            click.echo("Sync queue: Not accessible")
        
        # Try to check daemon status
        try:
            import subprocess
            result = subprocess.run(['systemctl', 'is-active', 'cardbrick-syncd'], 
                                  capture_output=True, text=True)
            daemon_status = result.stdout.strip()
            status_icon = "✓" if daemon_status == "active" else "❌"
            click.echo(f"Sync daemon: {status_icon} {daemon_status}")
        except Exception:
            click.echo("Sync daemon: Status unknown")
            
    except Exception as e:
        click.echo(f"Status check failed: {e}", err=True)
        if ctx.obj['verbose']:
            import traceback
            traceback.print_exc()

@cli.command()
@click.argument('learner_uuid')
@click.pass_context
def send_backup(ctx, learner_uuid):
    """Send a learner's backup via magic-wormhole"""
    try:
        data_dir = ctx.obj['data_dir']
        wormhole = WormholeBackup(data_dir)
        
        # Run async function
        code = asyncio.run(wormhole.send_backup(learner_uuid))
        
        click.echo("✓ Backup sent successfully!")
        click.echo(f"Wormhole code: {code}")
        click.echo("Share this code with the recipient to download the backup.")
        
    except WormholeError as e:
        click.echo(f"Backup send failed: {e}", err=True)
        sys.exit(1)
    except Exception as e:
        click.echo(f"Unexpected error: {e}", err=True)
        if ctx.obj['verbose']:
            import traceback
            traceback.print_exc()
        sys.exit(1)

@cli.command()
@click.argument('wormhole_code')
@click.option('--uuid', '-u', help='Specify UUID for restored backup (optional)')
@click.pass_context
def receive_backup(ctx, wormhole_code, uuid):
    """Receive and restore a backup via magic-wormhole"""
    try:
        data_dir = ctx.obj['data_dir']
        wormhole = WormholeBackup(data_dir)
        
        click.echo(f"Receiving backup with code: {wormhole_code}")
        click.echo("This may take a few minutes...")
        
        # Run async function
        restored_uuid = asyncio.run(wormhole.receive_backup(wormhole_code, uuid))
        
        click.echo("✓ Backup restored successfully!")
        click.echo(f"Learner UUID: {restored_uuid}")
        click.echo(f"Data location: {data_dir / 'learners' / restored_uuid}")
        
    except WormholeError as e:
        click.echo(f"Backup receive failed: {e}", err=True)
        sys.exit(1)
    except Exception as e:
        click.echo(f"Unexpected error: {e}", err=True)
        if ctx.obj['verbose']:
            import traceback
            traceback.print_exc()
        sys.exit(1)

@cli.command()
@click.pass_context
def list_backups(ctx):
    """List available learner backups"""
    try:
        data_dir = ctx.obj['data_dir']
        wormhole = WormholeBackup(data_dir)
        
        backups = wormhole.list_learner_backups()
        
        if not backups:
            click.echo("No learner backups found")
            return
        
        click.echo("Available learner backups:")
        click.echo("=" * 50)
        
        for backup in backups:
            size_mb = backup['size_bytes'] / (1024 * 1024)
            click.echo(f"UUID: {backup['uuid']}")
            click.echo(f"  Size: {size_mb:.1f} MB")
            
            metadata = backup.get('metadata', {})
            if 'restored_at' in metadata:
                click.echo(f"  Restored: {metadata['restored_at']}")
            if 'original_uuid' in metadata:
                click.echo(f"  Original: {metadata['original_uuid']}")
            
            click.echo()
            
    except Exception as e:
        click.echo(f"Failed to list backups: {e}", err=True)
        if ctx.obj['verbose']:
            import traceback
            traceback.print_exc()
        sys.exit(1)

def main():
    """Entry point for the CLI"""
    cli()

if __name__ == '__main__':
    main()
