import sqlite3
import sys
import shutil
import os
import time
import zipfile
import tempfile
import unittest
from bs4 import BeautifulSoup
from tqdm import tqdm
from multiprocessing import Pool, cpu_count

def simplify_ruby_html(note_data):
    """
    Worker function to simplify HTML. This version correctly handles
    malformed ruby tags and filters out link-only list items.
    """
    # The first element of the tuple is the note_id.
    # The second is the fields string.
    note_id, flds = note_data
    fields = flds.split('\x1f')
    
    # Ensure the field exists before trying to access it
    if len(fields) < 2:
        return None
    
    original_html = fields[1]

    if not original_html.strip():
        return None

    # --- HTML Parsing Logic ---
    soup = BeautifulSoup(original_html, 'html.parser')
    simplified_parts = []
    for element in soup.find_all(recursive=False):
        if element.name == 'h3':
            simplified_parts.append(f"<h3>{element.get_text().strip()}</h3>")
        elif element.name == 'p':
            paragraph_content = ""
            for content in element.contents:
                if content.name == 'ruby':
                    rt_tag = content.find('rt')
                    rb_tag = content.find('rb')
                    if rb_tag:
                        if rt_tag and rt_tag.find_parent() == rb_tag:
                            rt_tag.extract()
                        rt_text = rt_tag.get_text() if rt_tag else ""
                        rb_text = rb_tag.get_text()
                        if rt_text.strip():
                            paragraph_content += f"<ruby><rb>{rb_text}</rb><rt>{rt_text}</rt></ruby>"
                        else:
                            paragraph_content += rb_text
                elif content.string:
                    stripped_string = content.string.strip()
                    if stripped_string:
                        paragraph_content += stripped_string
            simplified_parts.append(f"<p>{paragraph_content}</p>")
        elif element.name == 'ul':
            # NEW LOGIC: Filter list items
            valid_list_items = []
            for li in element.find_all('li'):
                # Check if the list item is "link-only"
                # This is true if its only non-whitespace content is a single <a> tag.
                non_whitespace_children = [c for c in li.contents if not (isinstance(c, str) and c.strip() == '')]
                
                is_link_only = len(non_whitespace_children) == 1 and non_whitespace_children[0].name == 'a'
                
                if not is_link_only:
                    valid_list_items.append(f"<li>{li.get_text().strip()}</li>")

            # Only add the <ul> if it's not empty after filtering
            if valid_list_items:
                simplified_parts.append(f"<ul>{''.join(valid_list_items)}</ul>")
            # END OF NEW LOGIC
            
    cleaned_html = "\n".join(simplified_parts)
    # --- End HTML Logic ---

    if original_html != cleaned_html:
        fields[1] = cleaned_html
        new_flds = '\x1f'.join(fields)
        return (note_id, new_flds)
            
    return None


def clean_anki2_database(db_path):
    """Runs the cleaning logic on the provided .anki2 database file using a process pool."""
    try:
        with sqlite3.connect(db_path) as con:
            cur = con.cursor()
            cur.execute("SELECT id, flds FROM notes")
            notes_to_process = cur.fetchall()

        if not notes_to_process:
            print("No notes found in the database.")
            return

        print(f"Found {len(notes_to_process)} notes. Processing in parallel on {cpu_count()} cores...")
        
        with Pool() as pool:
            results = list(tqdm(pool.imap_unordered(simplify_ruby_html, notes_to_process), 
                                total=len(notes_to_process), 
                                desc="Cleaning notes"))

        updates_to_apply = [res for res in results if res is not None]

        if not updates_to_apply:
            print("No notes needed cleaning.")
            return
            
        print(f"Applying {len(updates_to_apply)} updates to the database...")
        with sqlite3.connect(db_path) as con:
            cur = con.cursor()
            timestamp = int(time.time())
            updates_with_timestamp = [(flds, timestamp, note_id) for note_id, flds in updates_to_apply]
            cur.executemany("UPDATE notes SET flds = ?, mod = ? WHERE id = ?", updates_with_timestamp)
            print(f"Updated {len(updates_to_apply)} notes in the database.")

    except sqlite3.Error as e:
        print(f"Database error during cleaning: {e}")
        raise

def process_apkg_file(apkg_path):
    """Unpacks, cleans (in parallel), and repacks an .apkg file."""
    output_path = apkg_path.replace('.apkg', '_cleaned.apkg')
    
    with tempfile.TemporaryDirectory() as temp_dir:
        try:
            print(f"Unpacking {os.path.basename(apkg_path)}...")
            with zipfile.ZipFile(apkg_path, 'r') as z:
                z.extractall(temp_dir)

            db_file_path = next((os.path.join(temp_dir, f) for f in os.listdir(temp_dir) if f.endswith('.anki2')), None)
            if not db_file_path:
                print("Error: Could not find an .anki2 database in the package.")
                return
            clean_anki2_database(db_file_path)

            print(f"Repacking into {os.path.basename(output_path)}...")
            with zipfile.ZipFile(output_path, 'w', zipfile.ZIP_DEFLATED) as z_out:
                files_to_zip = [os.path.join(r, f) for r, _, files in os.walk(temp_dir) for f in files]
                for file_path in tqdm(files_to_zip, desc="Repacking files", unit="file"):
                    arcname = os.path.relpath(file_path, temp_dir)
                    z_out.write(file_path, arcname)
            
            print(f"\nSuccess! Cleaned deck saved to: {output_path}")

        except Exception as e:
            print(f"\nAn error occurred during processing: {e}")
            print("No changes were saved.")

# --- Unit Test Suite ---
class TestHtmlSimplification(unittest.TestCase):
    def run_test_on_html(self, original_html):
        note_data = (1, f"Front\x1f{original_html}")
        result = simplify_ruby_html(note_data)
        return result[1].split('\x1f')[1] if result else original_html

    def test_fix_malformed_ruby(self):
        """Tests the specific bugfix for unclosed <rb> tags."""
        html = "<p><ruby><rb>結婚<rt>けっこん</ruby></p>"
        expected = "<p><ruby><rb>結婚</rb><rt>けっこん</rt></ruby></p>"
        self.assertEqual(self.run_test_on_html(html), expected)

    def test_removes_link_only_list(self):
        """Tests that a <ul> containing only links is completely removed."""
        html = "<h3>Actions:</h3><ul><li><a href='#'>Google</a></li><li><a href='#'>Midori</a></li></ul>"
        expected = "<h3>Actions:</h3>"
        self.assertEqual(self.run_test_on_html(html), expected)

    def test_keeps_mixed_content_list(self):
        """Tests that a <ul> with mixed content is kept, but links are stripped."""
        html = "<ul><li>Keep this.</li><li><a href='#'>Remove this</a></li></ul>"
        expected = "<ul><li>Keep this.</li></ul>"
        self.assertEqual(self.run_test_on_html(html), expected)

    def test_full_example_with_link_filtering(self):
        """Tests a complete, complex field with the new link filtering."""
        html = """<h3>Reading:</h3>
<p><ruby><rb>彼<rt>かれ</ruby></p>
<h3>Translation:</h3>
<ul><li>His shoes are brown.</li></ul>
<h3>Actions:</h3>
<ul><li><a href="#">Google Translate</a></li></ul>"""
        expected = """<h3>Reading:</h3>
<p><ruby><rb>彼</rb><rt>かれ</rt></ruby></p>
<h3>Translation:</h3>
<ul><li>His shoes are brown.</li></ul>
<h3>Actions:</h3>"""
        self.assertEqual(self.run_test_on_html(html), expected)

    def test_no_changes_needed(self):
        """Tests that HTML without relevant tags is returned as None by the worker."""
        note_data = (1, "Front\x1f<p>Hello world</p>")
        self.assertIsNone(simplify_ruby_html(note_data))

if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == '--test':
        print("Running unit tests...")
        sys.argv.pop(1)
        unittest.main(verbosity=2)
    elif len(sys.argv) == 2:
        process_apkg_file(sys.argv[1])
    else:
        print("Usage:")
        print(f"  python {sys.argv[0]} <path_to_deck.apkg>")
        print(f"  python {sys.argv[0]} --test")
        sys.exit(1)
