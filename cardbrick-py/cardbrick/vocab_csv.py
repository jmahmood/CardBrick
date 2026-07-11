"""CSV importer for the "Español MX (word + audio + example)" vocab deck.

An alternative to .apkg for this specific card design — useful when the
source data is a spreadsheet rather than an Anki export. Expected
header (case-insensitive, any column order, extra columns ignored):

    Word, Word Audio, Gendered Forms, Definitions, Image,
    Example ES, Example Audio, Example EN, Report Link,
    Word EN, Word JP, Example JP, Tags

Audio/Image cells may be a bare filename ("hola.mp3", "gato.jpg") or an
Anki-style tag (`[sound:hola.mp3]`, `<img src="gato.jpg">`) pasted
straight out of Anki — either is accepted. There is no media zip to
draw from as there is with .apkg, so referenced files must already
exist in the app's media folder, or be found in ``source_media_dir``
and copied in by this importer.

Card identity: a CSV has no Anki note id, so the card id is a stable
hash of the (lowercased, trimmed) Word field. This means **the Word
field must stay unique and unchanged across re-imports** — renaming a
word in the spreadsheet creates a new card rather than updating the
old one, since there is nothing else to key on.
"""

import csv
import hashlib
import logging
import os
import shutil

from .importer import ImportStats
from .scheduler import iso, now_utc
from .textutil import clean_html, extract_audio, extract_image_filename

log = logging.getLogger(__name__)


def _stable_id(word):
    """Deterministic positive integer id from the Word text.

    Re-importing the same CSV must update existing cards rather than
    duplicate them; a content hash is the only stable key available
    without an Anki note id. 60 bits comfortably fits SQLite INTEGER
    and collisions are not a practical concern at deck scale.
    """
    digest = hashlib.sha1(word.strip().lower().encode("utf-8")).hexdigest()
    return int(digest[:15], 16)


def _audio_filename(cell):
    cell = (cell or "").strip()
    if not cell:
        return None
    if cell.lower().startswith("[sound:"):
        _, files = extract_audio(cell)
        return files[0] if files else None
    return os.path.basename(cell)


def _image_filename(cell):
    cell = (cell or "").strip()
    if not cell:
        return None
    if "<img" in cell.lower():
        return extract_image_filename(cell)
    return os.path.basename(cell)


def _copy_if_present(filename, source_dir, media_dir, stats):
    if not filename or not source_dir:
        return
    src = os.path.join(source_dir, filename)
    if not os.path.exists(src):
        log.warning("vocab CSV: media file not found: %s", src)
        return
    os.makedirs(media_dir, exist_ok=True)
    dst = os.path.join(media_dir, filename)
    if not os.path.exists(dst):
        shutil.copyfile(src, dst)
    stats.media_files += 1


def _normalize_row(row):
    return {(k or "").strip().lower(): (v or "").strip()
            for k, v in row.items()}


def import_vocab_csv(csv_path, storage, scheduler, media_dir,
                     source_media_dir=None, deck_name="Vocabulario"):
    """Import vocab cards from a CSV export. Returns an ImportStats."""
    stats = ImportStats()
    with open(csv_path, newline="", encoding="utf-8-sig") as f:
        reader = csv.DictReader(f)
        rows = [_normalize_row(row) for row in reader]

    for i, row in enumerate(rows):
        word = clean_html(row.get("word", ""))
        if not word:
            stats.skip(f"csv row {i + 2}", "missing Word")  # +2: header + 1-index
            continue

        card_id = _stable_id(word)
        tags = row.get("tags", "")
        word_audio = _audio_filename(row.get("word audio", ""))
        example_audio = _audio_filename(row.get("example audio", ""))
        image_filename = _image_filename(row.get("image", ""))
        _copy_if_present(word_audio, source_media_dir, media_dir, stats)
        _copy_if_present(example_audio, source_media_dir, media_dir, stats)
        _copy_if_present(image_filename, source_media_dir, media_dir, stats)

        definitions = clean_html(row.get("definitions", ""))
        word_en = clean_html(row.get("word en", ""))
        word_jp = clean_html(row.get("word jp", ""))
        example_en = clean_html(row.get("example en", ""))
        example_jp = clean_html(row.get("example jp", ""))

        storage.upsert_card(
            card_id=card_id, note_id=card_id, deck=deck_name, front=word,
            back=definitions or word_en or example_en or word_jp or example_jp,
            tags=tags,
            audio_filename=word_audio,
            audio_side="front" if word_audio else None,
            now_iso=iso(now_utc()), card_type="vocab",
            front_jp=word_jp or None, back_jp=example_jp or None)
        storage.upsert_vocab_card(
            card_id=card_id, word=word,
            word_en=word_en or None,
            word_jp=word_jp or None,
            gendered_forms=clean_html(row.get("gendered forms", "")),
            definitions=definitions,
            image_filename=image_filename,
            example_es=clean_html(row.get("example es", "")),
            example_audio=example_audio,
            example_en=example_en,
            example_jp=example_jp or None,
            report_link=row.get("report link", "").strip() or None)
        storage.init_review_state(scheduler.initial_state(card_id))

        stats.cards += 1
        stats.notes += 1
        stats.decks.add(deck_name)
        stats.tags.update(tags.split())

    storage.commit()
    return stats
