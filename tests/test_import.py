#!/usr/bin/env python3
"""
Test script for import functionality
"""

import sys
import tempfile
from pathlib import Path

# Add CLI to path
sys.path.insert(0, str(Path(__file__).parent.parent / 'cardbrick-cli'))

from importer import CSVImporter, MarkdownImporter, import_file

def test_csv_import():
    """Test CSV import functionality"""
    print("Testing CSV import...")
    
    # Test CSV importer
    importer = CSVImporter()
    test_csv = Path(__file__).parent / 'test_data.csv'
    
    deck = importer.import_file(test_csv, "Test CSV Deck")
    
    assert deck.metadata.name == "Test CSV Deck"
    assert len(deck.cards) == 5
    assert deck.cards[0].front == "What is the capital of France?"
    assert deck.cards[0].back == "Paris"
    
    print("✓ CSV import test passed")

def test_markdown_import():
    """Test Markdown import functionality"""
    print("Testing Markdown import...")
    
    # Test Markdown importer
    importer = MarkdownImporter()
    test_md = Path(__file__).parent / 'test_data.md'
    
    deck = importer.import_file(test_md, "Test MD Deck")
    
    assert deck.metadata.name == "Test MD Deck"
    assert len(deck.cards) >= 1  # Should find at least one card
    
    print("✓ Markdown import test passed")

def test_full_import():
    """Test the full import workflow"""
    print("Testing full import workflow...")
    
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_path = Path(temp_dir)
        
        # Import CSV
        csv_path = Path(__file__).parent / 'test_data.csv'
        output_path = import_file(csv_path, temp_path, "Integration Test Deck")
        
        assert output_path.exists()
        assert output_path.suffix == '.cbdeck'
        
        # Verify content
        import json
        with open(output_path) as f:
            deck_data = json.load(f)
            
        assert deck_data['metadata']['name'] == "Integration Test Deck"
        assert len(deck_data['cards']) == 5
        
    print("✓ Full import test passed")

def main():
    """Run all tests"""
    try:
        test_csv_import()
        test_markdown_import()
        test_full_import()
        print("✓ All import tests passed!")
        return True
    except Exception as e:
        print(f"❌ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)