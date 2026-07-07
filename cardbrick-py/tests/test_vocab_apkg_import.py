"""Importing the 'Español MX (word + audio + example)' note type from
an .apkg — detected by field names, not by a hardcoded model id."""

import json
import os
import sqlite3
import zipfile

import pytest

from cardbrick.importer import import_apkg
from test_import import COL_SCHEMA

VOCAB_MID = 2000
BASIC_MID = 1000
DECK_ID = 55

# Matches the spec order exactly (including the optional JP fields).
VOCAB_FIELDS = ["Word", "Word Audio", "Gendered Forms", "Definitions",
                "Image", "Example ES", "Example Audio", "Example EN",
                "Report Link", "Word JP", "Example JP"]

MODELS = {
    str(VOCAB_MID): {
        "id": VOCAB_MID, "name": "Español MX (word + audio + example)",
        "type": 0,
        "flds": [{"name": n, "ord": i} for i, n in enumerate(VOCAB_FIELDS)],
        "tmpls": [{"ord": 0}],
    },
    str(BASIC_MID): {
        "id": BASIC_MID, "name": "Basic", "type": 0,
        "flds": [{"name": "Front", "ord": 0}, {"name": "Back", "ord": 1}],
        "tmpls": [{"ord": 0}],
    },
}
DECKS = {str(DECK_ID): {"id": DECK_ID, "name": "Spanish Vocab"}}

VOCAB_NOTE_FIELDS = [
    "gato",                                # Word (plain text, no audio)
    "[sound:word_gato.mp3]",              # Word Audio
    "<div class='gendered_forms'>la gata</div>",  # Gendered Forms
    "Animal doméstico felino.",           # Definitions
    '<img src="gato.jpg">',               # Image
    "El <span>gato</span> duerme mucho.",  # Example ES
    "[sound:ex1.mp3]",                    # Example Audio
    "The cat sleeps a lot.",              # Example EN
    "",                                    # Report Link
    "猫",                                  # Word JP
    "猫はよく眠る。",                        # Example JP
]


def build_vocab_apkg(path):
    tmp_col = str(path) + ".col"
    conn = sqlite3.connect(tmp_col)
    conn.executescript(COL_SCHEMA)
    conn.execute(
        "INSERT INTO col VALUES (1, 0, 0, 0, 11, 0, 0, 0, '{}', ?, ?, "
        "'{}', '{}')", (json.dumps(MODELS), json.dumps(DECKS)))
    conn.execute(
        "INSERT INTO notes VALUES (1, 'g1', ?, 0, 0, ' animals ', ?, "
        "'gato', 0, 0, '')",
        (VOCAB_MID, "\x1f".join(VOCAB_NOTE_FIELDS)))
    conn.execute(
        "INSERT INTO cards VALUES (1, 1, ?, 0, 0, 0, 0, 0, 0, 0, 0, 0, "
        "0, 0, 0, 0, 0, '')", (DECK_ID,))
    # A plain Basic note in the same deck must still import normally.
    conn.execute(
        "INSERT INTO notes VALUES (2, 'g2', ?, 0, 0, '', ?, 'hola', 0, "
        "0, '')", (BASIC_MID, "hola\x1fhello"))
    conn.execute(
        "INSERT INTO cards VALUES (2, 2, ?, 0, 0, 0, 0, 0, 0, 0, 0, 0, "
        "0, 0, 0, 0, 0, '')", (DECK_ID,))
    conn.commit()
    conn.close()

    with zipfile.ZipFile(path, "w") as zf:
        zf.write(tmp_col, "collection.anki2")
        zf.writestr("0", b"fake jpg bytes")
        zf.writestr("1", b"fake example audio bytes")
        zf.writestr("2", b"fake word audio bytes")
        zf.writestr("media", json.dumps({
            "0": "gato.jpg", "1": "ex1.mp3", "2": "word_gato.mp3"}))
    os.remove(tmp_col)


@pytest.fixture
def imported(tmp_path, storage, service):
    apkg = tmp_path / "vocab.apkg"
    build_vocab_apkg(apkg)
    media_dir = tmp_path / "media"
    stats = import_apkg(str(apkg), storage, service.scheduler,
                        str(media_dir))
    return stats, media_dir


def test_vocab_note_detected_and_imported(imported, storage):
    stats, _ = imported
    assert stats.cards == 2  # 1 vocab + 1 basic
    card = storage.get_card(1)
    assert card["card_type"] == "vocab"
    assert card["front"] == "gato"
    assert card["audio_filename"] == "word_gato.mp3"  # from Word Audio
    assert card["audio_side"] == "front"


def test_vocab_detail_fields_extracted(imported, storage):
    detail = storage.get_vocab_detail(1)
    assert detail["word"] == "gato"
    assert detail["gendered_forms"] == "la gata"
    assert detail["definitions"] == "Animal doméstico felino."
    assert detail["image_filename"] == "gato.jpg"
    assert detail["example_es"] == "El gato duerme mucho."
    assert detail["example_audio"] == "ex1.mp3"
    assert detail["example_en"] == "The cat sleeps a lot."
    assert detail["word_jp"] == "猫"
    assert detail["example_jp"] == "猫はよく眠る。"
    assert detail["report_link"] is None


def test_vocab_media_copied(imported):
    _stats, media_dir = imported
    assert (media_dir / "gato.jpg").read_bytes() == b"fake jpg bytes"
    assert (media_dir / "ex1.mp3").read_bytes() == b"fake example audio bytes"
    assert (media_dir / "word_gato.mp3").read_bytes() == \
        b"fake word audio bytes"


def test_basic_note_in_same_deck_still_imports(imported, storage):
    card = storage.get_card(2)
    assert card["card_type"] == "basic"
    assert card["front"] == "hola" and card["back"] == "hello"


def test_vocab_note_missing_word_is_skipped(tmp_path, storage, service):
    apkg = tmp_path / "empty_word.apkg"
    tmp_col = str(apkg) + ".col"
    conn = sqlite3.connect(tmp_col)
    conn.executescript(COL_SCHEMA)
    conn.execute(
        "INSERT INTO col VALUES (1, 0, 0, 0, 11, 0, 0, 0, '{}', ?, ?, "
        "'{}', '{}')", (json.dumps(MODELS), json.dumps(DECKS)))
    blank_fields = [""] * len(VOCAB_FIELDS)
    conn.execute(
        "INSERT INTO notes VALUES (1, 'g1', ?, 0, 0, '', ?, '', 0, 0, "
        "'')", (VOCAB_MID, "\x1f".join(blank_fields)))
    conn.execute(
        "INSERT INTO cards VALUES (1, 1, ?, 0, 0, 0, 0, 0, 0, 0, 0, 0, "
        "0, 0, 0, 0, 0, '')", (DECK_ID,))
    conn.commit()
    conn.close()
    with zipfile.ZipFile(apkg, "w") as zf:
        zf.write(tmp_col, "collection.anki2")
    os.remove(tmp_col)

    stats = import_apkg(str(apkg), storage, service.scheduler,
                        str(tmp_path / "media"))
    assert stats.cards == 0
    assert any("Word" in reason for _cid, reason in stats.skipped)
