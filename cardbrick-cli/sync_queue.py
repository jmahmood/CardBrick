"""
Sync Queue Management for CardBrick

Tracks decks that need to be synchronized to handheld devices.
Maintains state and handles "needs-sync" flags.
"""

import json
import logging
from pathlib import Path
from typing import List, Dict, Any, Optional
from datetime import datetime
from dataclasses import dataclass, asdict

logger = logging.getLogger(__name__)

class SyncQueueError(Exception):
    """Exception raised by sync queue operations"""
    pass

@dataclass
class QueuedDeck:
    """Represents a deck in the sync queue"""
    name: str
    path: str
    added_at: str
    status: str = "pending"  # pending, syncing, completed, error
    error_message: Optional[str] = None
    last_sync_attempt: Optional[str] = None
    sync_count: int = 0
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON storage"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'QueuedDeck':
        """Create from dictionary"""
        return cls(**data)

class SyncQueue:
    """Manages the sync queue for CardBrick decks"""
    
    def __init__(self, data_dir: Path):
        self.data_dir = Path(data_dir)
        self.queue_file = self.data_dir / 'sync_queue.json'
        
        # Ensure data directory exists
        self.data_dir.mkdir(parents=True, exist_ok=True)
        
        # Load existing queue
        self._queue: Dict[str, QueuedDeck] = {}
        self._load_queue()
    
    def add_deck(self, deck_path: Path, deck_name: Optional[str] = None) -> None:
        """Add a deck to the sync queue"""
        deck_path = Path(deck_path)
        
        if not deck_path.exists():
            raise SyncQueueError(f"Deck file not found: {deck_path}")
        
        # Use filename as name if not provided
        name = deck_name or deck_path.stem
        
        # Create queued deck entry
        queued_deck = QueuedDeck(
            name=name,
            path=str(deck_path.absolute()),
            added_at=datetime.now().isoformat(),
            status="pending"
        )
        
        # Add to queue (replace if exists)
        self._queue[name] = queued_deck
        
        # Save queue
        self._save_queue()
        
        logger.info(f"Added deck to sync queue: {name}")
    
    def remove_deck(self, deck_name: str) -> bool:
        """Remove a deck from the sync queue"""
        if deck_name in self._queue:
            del self._queue[deck_name]
            self._save_queue()
            logger.info(f"Removed deck from sync queue: {deck_name}")
            return True
        return False
    
    def update_deck_status(self, deck_name: str, status: str, 
                          error_message: Optional[str] = None) -> None:
        """Update the status of a deck in the queue"""
        if deck_name not in self._queue:
            raise SyncQueueError(f"Deck not found in queue: {deck_name}")
        
        deck = self._queue[deck_name]
        deck.status = status
        deck.error_message = error_message
        deck.last_sync_attempt = datetime.now().isoformat()
        
        if status == "syncing":
            deck.sync_count += 1
        
        self._save_queue()
        logger.info(f"Updated deck status: {deck_name} -> {status}")
    
    def mark_deck_completed(self, deck_name: str) -> None:
        """Mark a deck as successfully synced"""
        self.update_deck_status(deck_name, "completed")
    
    def mark_deck_error(self, deck_name: str, error_message: str) -> None:
        """Mark a deck as having sync errors"""
        self.update_deck_status(deck_name, "error", error_message)
    
    def list_pending(self) -> List[Dict[str, Any]]:
        """List all pending decks in the queue"""
        pending = []
        for deck in self._queue.values():
            if deck.status in ["pending", "error"]:
                pending.append(deck.to_dict())
        return pending
    
    def list_all(self) -> List[Dict[str, Any]]:
        """List all decks in the queue"""
        return [deck.to_dict() for deck in self._queue.values()]
    
    def get_next_pending(self) -> Optional[QueuedDeck]:
        """Get the next deck that needs to be synced"""
        for deck in self._queue.values():
            if deck.status == "pending":
                return deck
        return None
    
    def clear_completed(self) -> int:
        """Remove all completed decks from the queue"""
        completed_names = [
            name for name, deck in self._queue.items() 
            if deck.status == "completed"
        ]
        
        for name in completed_names:
            del self._queue[name]
        
        if completed_names:
            self._save_queue()
            logger.info(f"Cleared {len(completed_names)} completed decks from queue")
        
        return len(completed_names)
    
    def clear_all(self) -> int:
        """Clear all decks from the queue"""
        count = len(self._queue)
        self._queue.clear()
        self._save_queue()
        logger.info("Cleared all decks from sync queue")
        return count
    
    def _load_queue(self) -> None:
        """Load queue from JSON file"""
        if not self.queue_file.exists():
            logger.debug("No existing sync queue file found")
            return
        
        try:
            with open(self.queue_file, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            # Convert dictionaries back to QueuedDeck objects
            for name, deck_data in data.items():
                try:
                    self._queue[name] = QueuedDeck.from_dict(deck_data)
                except Exception as e:
                    logger.warning(f"Failed to load queued deck {name}: {e}")
            
            logger.debug(f"Loaded {len(self._queue)} decks from sync queue")
            
        except Exception as e:
            logger.error(f"Failed to load sync queue: {e}")
            # Start with empty queue if loading fails
            self._queue = {}
    
    def _save_queue(self) -> None:
        """Save queue to JSON file"""
        try:
            # Convert to serializable format
            data = {name: deck.to_dict() for name, deck in self._queue.items()}
            
            # Write atomically using temporary file
            temp_file = self.queue_file.with_suffix('.tmp')
            with open(temp_file, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
            
            # Atomic rename
            temp_file.replace(self.queue_file)
            
            logger.debug(f"Saved sync queue with {len(self._queue)} decks")
            
        except Exception as e:
            logger.error(f"Failed to save sync queue: {e}")
            raise SyncQueueError(f"Failed to save sync queue: {e}")
    
    def cleanup_missing_files(self) -> int:
        """Remove queue entries for files that no longer exist"""
        missing_names = []
        
        for name, deck in self._queue.items():
            if not Path(deck.path).exists():
                missing_names.append(name)
        
        for name in missing_names:
            del self._queue[name]
        
        if missing_names:
            self._save_queue()
            logger.info(f"Cleaned up {len(missing_names)} missing files from queue")
        
        return len(missing_names)
    
    def get_queue_stats(self) -> Dict[str, int]:
        """Get statistics about the sync queue"""
        stats = {
            "total": len(self._queue),
            "pending": 0,
            "syncing": 0, 
            "completed": 0,
            "error": 0
        }
        
        for deck in self._queue.values():
            if deck.status in stats:
                stats[deck.status] += 1
        
        return stats