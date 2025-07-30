"""
CardBrick Deck Importer

Converts CSV and Markdown files to CardBrick's .cbdeck format.
Supports various input formats and produces decks compatible with the Rust handheld app.
"""

import csv
import json
import hashlib
import logging
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple
from dataclasses import dataclass, asdict
from datetime import datetime

import pandas as pd
import markdown
from pydantic import BaseModel, Field

logger = logging.getLogger(__name__)

@dataclass
class Card:
    """Represents a single flashcard"""
    front: str
    back: str
    tags: List[str] = None
    media: List[str] = None
    
    def __post_init__(self):
        if self.tags is None:
            self.tags = []
        if self.media is None:
            self.media = []

@dataclass
class DeckMetadata:
    """Metadata for a deck"""
    name: str
    description: str = ""
    version: str = "1.0"
    created_at: str = ""
    card_count: int = 0
    
    def __post_init__(self):
        if not self.created_at:
            self.created_at = datetime.now().isoformat()

@dataclass
class CBDeck:
    """CardBrick deck format"""
    metadata: DeckMetadata
    cards: List[Card]
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization"""
        return {
            "metadata": asdict(self.metadata),
            "cards": [asdict(card) for card in self.cards]
        }

class ImporterError(Exception):
    """Base exception for import errors"""
    pass

class CSVImporter:
    """Imports flashcards from CSV files"""
    
    def __init__(self):
        self.supported_formats = [
            "front,back",
            "question,answer", 
            "term,definition",
            "word,meaning",
            "front,back,tags"
        ]
    
    def import_file(self, file_path: Path, deck_name: Optional[str] = None) -> CBDeck:
        """
        Import CSV file to CBDeck format
        
        Expected CSV formats:
        - front,back
        - question,answer
        - term,definition  
        - word,meaning
        - front,back,tags (tags comma-separated in column)
        """
        if not file_path.exists():
            raise ImporterError(f"CSV file not found: {file_path}")
            
        logger.info(f"Importing CSV file: {file_path}")
        
        try:
            # Read CSV with pandas for better handling
            df = pd.read_csv(file_path, keep_default_na=False)
            
            if df.empty:
                raise ImporterError("CSV file is empty")
                
            # Normalize column names
            df.columns = df.columns.str.lower().str.strip()
            
            # Detect format and map columns
            front_col, back_col, tags_col = self._detect_csv_format(df.columns.tolist())
            
            cards = []
            for _, row in df.iterrows():
                front = str(row[front_col]).strip()
                back = str(row[back_col]).strip()
                
                # Skip empty cards
                if not front or not back:
                    continue
                    
                # Parse tags if available
                tags = []
                if tags_col and tags_col in row and row[tags_col]:
                    tags = [tag.strip() for tag in str(row[tags_col]).split(',') if tag.strip()]
                
                cards.append(Card(front=front, back=back, tags=tags))
            
            if not cards:
                raise ImporterError("No valid cards found in CSV")
                
            # Create deck metadata
            name = deck_name or file_path.stem
            metadata = DeckMetadata(
                name=name,
                description=f"Imported from {file_path.name}",
                card_count=len(cards)
            )
            
            logger.info(f"Successfully imported {len(cards)} cards from CSV")
            return CBDeck(metadata=metadata, cards=cards)
            
        except Exception as e:
            raise ImporterError(f"Failed to import CSV: {e}")
    
    def _detect_csv_format(self, columns: List[str]) -> Tuple[str, str, Optional[str]]:
        """Detect CSV format and return column mappings"""
        col_map = {col: col for col in columns}
        
        # Common front/back mappings
        front_mappings = {
            'front': ['front', 'question', 'term', 'word', 'prompt'],
            'back': ['back', 'answer', 'definition', 'meaning', 'response']
        }
        
        front_col = None
        back_col = None
        tags_col = None
        
        # Find front column
        for col in columns:
            for canonical, variants in front_mappings.items():
                if col in variants:
                    if canonical == 'front':
                        front_col = col
                        break
        
        # Find back column  
        for col in columns:
            for canonical, variants in front_mappings.items():
                if col in variants:
                    if canonical == 'back':
                        back_col = col
                        break
        
        # Find tags column
        if 'tags' in columns:
            tags_col = 'tags'
        elif 'tag' in columns:
            tags_col = 'tag'
            
        # Fallback to first two columns if detection fails
        if not front_col or not back_col:
            if len(columns) >= 2:
                front_col = columns[0]
                back_col = columns[1]
            else:
                raise ImporterError(f"Cannot detect CSV format. Expected at least 2 columns, got: {columns}")
                
        logger.debug(f"Detected CSV format - Front: {front_col}, Back: {back_col}, Tags: {tags_col}")
        return front_col, back_col, tags_col

class MarkdownImporter:
    """Imports flashcards from Markdown files"""
    
    def import_file(self, file_path: Path, deck_name: Optional[str] = None) -> CBDeck:
        """
        Import Markdown file to CBDeck format
        
        Expected formats:
        1. Q&A pairs separated by headers:
           ## Question 1
           What is...?
           
           ### Answer
           The answer is...
           
        2. Definition lists:
           **Term**: Definition
           
        3. Anki-style with separators:
           Front text
           ---
           Back text
           
           ===
           
           Next card front
           ---
           Next card back
        """
        if not file_path.exists():
            raise ImporterError(f"Markdown file not found: {file_path}")
            
        logger.info(f"Importing Markdown file: {file_path}")
        
        try:
            content = file_path.read_text(encoding='utf-8')
            
            # Try different parsing strategies
            cards = []
            
            # Strategy 1: Anki-style separators
            anki_cards = self._parse_anki_style(content)
            if anki_cards:
                cards.extend(anki_cards)
            
            # Strategy 2: Header-based Q&A
            if not cards:
                header_cards = self._parse_header_qa(content)
                if header_cards:
                    cards.extend(header_cards)
            
            # Strategy 3: Definition lists
            if not cards:
                def_cards = self._parse_definition_lists(content)
                if def_cards:
                    cards.extend(def_cards)
            
            if not cards:
                raise ImporterError("No flashcards found in Markdown file")
                
            # Create deck metadata
            name = deck_name or file_path.stem
            metadata = DeckMetadata(
                name=name,
                description=f"Imported from {file_path.name}",
                card_count=len(cards)
            )
            
            logger.info(f"Successfully imported {len(cards)} cards from Markdown")
            return CBDeck(metadata=metadata, cards=cards)
            
        except Exception as e:
            raise ImporterError(f"Failed to import Markdown: {e}")
    
    def _parse_anki_style(self, content: str) -> List[Card]:
        """Parse Anki-style cards with --- separator and === card separator"""
        cards = []
        
        # Split by card separator
        card_blocks = content.split('===')
        
        for block in card_blocks:
            block = block.strip()
            if not block:
                continue
                
            # Split front and back by ---
            if '---' in block:
                parts = block.split('---', 1)
                if len(parts) == 2:
                    front = parts[0].strip()
                    back = parts[1].strip()
                    
                    if front and back:
                        cards.append(Card(front=front, back=back))
        
        return cards
    
    def _parse_header_qa(self, content: str) -> List[Card]:
        """Parse Q&A pairs using markdown headers"""
        cards = []
        
        # Convert to HTML and parse
        md = markdown.Markdown()
        html = md.convert(content)
        
        # Simple parsing - look for header patterns
        lines = content.split('\n')
        current_question = None
        current_answer = None
        
        for line in lines:
            line = line.strip()
            
            # Question headers (## or ###)
            if line.startswith('##') and not line.startswith('###'):
                if current_question and current_answer:
                    cards.append(Card(front=current_question, back=current_answer))
                current_question = line.lstrip('#').strip()
                current_answer = None
                
            # Answer headers
            elif line.startswith('###') and 'answer' in line.lower():
                current_answer = ""
                
            # Content lines
            elif line and not line.startswith('#'):
                if current_answer is not None:
                    current_answer += line + '\n'
                elif current_question and current_answer is None:
                    current_answer = line + '\n'
        
        # Add final card
        if current_question and current_answer:
            cards.append(Card(front=current_question, back=current_answer.strip()))
            
        return cards
    
    def _parse_definition_lists(self, content: str) -> List[Card]:
        """Parse definition-style lists"""
        cards = []
        
        lines = content.split('\n')
        
        for line in lines:
            line = line.strip()
            
            # Look for bold term followed by colon
            if '**' in line and ':' in line:
                # Extract term and definition
                if line.count('**') >= 2:
                    start = line.find('**') + 2
                    end = line.find('**', start)
                    
                    if end > start:
                        term = line[start:end].strip()
                        colon_pos = line.find(':', end)
                        
                        if colon_pos > -1:
                            definition = line[colon_pos + 1:].strip()
                            
                            if term and definition:
                                cards.append(Card(front=term, back=definition))
        
        return cards

class DeckExporter:
    """Exports CBDeck format to .cbdeck files"""
    
    def export_deck(self, deck: CBDeck, output_path: Path) -> None:
        """Export deck to .cbdeck file"""
        try:
            # Ensure output directory exists
            output_path.parent.mkdir(parents=True, exist_ok=True)
            
            # Convert to JSON
            deck_json = json.dumps(deck.to_dict(), indent=2, ensure_ascii=False)
            
            # Write to file
            output_path.write_text(deck_json, encoding='utf-8')
            
            logger.info(f"Exported deck to: {output_path}")
            
        except Exception as e:
            raise ImporterError(f"Failed to export deck: {e}")
    
    def calculate_deck_hash(self, deck: CBDeck) -> str:
        """Calculate SHA256 hash of deck content for sync tracking"""
        deck_json = json.dumps(deck.to_dict(), sort_keys=True, ensure_ascii=False)
        return hashlib.sha256(deck_json.encode('utf-8')).hexdigest()

def import_file(file_path: Path, output_dir: Path, deck_name: Optional[str] = None) -> Path:
    """
    Import a file and convert to .cbdeck format
    
    Returns path to the created .cbdeck file
    """
    file_path = Path(file_path)
    output_dir = Path(output_dir)
    
    # Determine importer based on file extension
    if file_path.suffix.lower() == '.csv':
        importer = CSVImporter()
    elif file_path.suffix.lower() in ['.md', '.markdown']:
        importer = MarkdownImporter()
    else:
        raise ImporterError(f"Unsupported file format: {file_path.suffix}")
    
    # Import the file
    deck = importer.import_file(file_path, deck_name)
    
    # Create output filename
    output_name = deck_name or file_path.stem
    output_path = output_dir / f"{output_name}.cbdeck"
    
    # Export to .cbdeck format
    exporter = DeckExporter()
    exporter.export_deck(deck, output_path)
    
    # Calculate hash for sync tracking
    deck_hash = exporter.calculate_deck_hash(deck)
    logger.info(f"Deck hash: {deck_hash}")
    
    return output_path