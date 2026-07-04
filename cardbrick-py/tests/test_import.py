"""Importer behaviour: basic notes, media, audio refs, safe degradation."""

import json
import os
import sqlite3
import zipfile

import pytest

from cardbrick.importer import ApkgError, import_apkg

COL_SCHEMA = """
CREATE TABLE col (
    id INTEGER PRIMARY KEY, crt INTEGER, mod INTEGER, scm INTEGER,
    ver INTEGER, dty INTEGER, usn INTEGER, ls INTEGER,
    conf TEXT, models TEXT, decks TEXT, dconf TEXT, tags TEXT
);
CREATE TABLE notes (
    id INTEGER PRIMARY KEY, guid TEXT, mid INTEGER, mod INTEGER,
    usn INTEGER, tags TEXT, flds TEXT, sfld TEXT, csum INTEGER,
    flags INTEGER, data TEXT
);
CREATE TABLE cards (
    id INTEGER PRIMARY KEY, nid INTEGER, did INTEGER, ord INTEGER,
    mod INTEGER, usn INTEGER, type INTEGER, queue INTEGER, due INTEGER,
    ivl INTEGER, factor INTEGER, reps INTEGER, lapses INTEGER,
    left INTEGER, odue INTEGER, odid INTEGER, flags INTEGER, data TEXT
);
"""

BASIC, REVERSED, CLOZE = 1000, 1001, 1002
MODELS = {
    str(BASIC): {"id": BASIC, "name": "Basic", "type": 0,
                 "tmpls": [{"ord": 0}]},
    str(REVERSED): {"id": REVERSED, "name": "Basic (and reversed card)",
                    "type": 0, "tmpls": [{"ord": 0}, {"ord": 1}]},
    str(CLOZE): {"id": CLOZE, "name": "Cloze", "type": 1,
                 "tmpls": [{"ord": 0}]},
}
DECKS = {"77": {"id": 77, "name": "Spanish"}}

NOTES = [
    # (note_id, model, fields, tags, n_cards)
    (1, BASIC, ["¿Dónde está el baño?[sound:bano.mp3]",
                "Where is the bathroom?"], "bathroom travel", 1),
    (2, BASIC, ["<b>Hola</b><br>saludo", "Hello"], "greetings", 1),
    (3, REVERSED, ["gato", "cat"], "animals", 2),
    (4, CLOZE, ["El {{c1::perro}} ladra", ""], "animals", 1),
    (5, BASIC, ["<style>p{}</style>", "<div></div>"], "", 1),  # empty
]


def build_apkg(path, collection_name="collection.anki2", media=True):
    tmp_col = str(path) + ".col"
    conn = sqlite3.connect(tmp_col)
    conn.executescript(COL_SCHEMA)
    conn.execute(
        "INSERT INTO col VALUES (1, 0, 0, 0, 11, 0, 0, 0, '{}', ?, ?, "
        "'{}', '{}')", (json.dumps(MODELS), json.dumps(DECKS)))
    card_id = 1
    for nid, mid, fields, tags, n_cards in NOTES:
        conn.execute(
            "INSERT INTO notes VALUES (?, ?, ?, 0, 0, ?, ?, ?, 0, 0, '')",
            (nid, f"g{nid}", mid, f" {tags} ", "\x1f".join(fields),
             fields[0]))
        for ordinal in range(n_cards):
            conn.execute(
                "INSERT INTO cards VALUES (?, ?, 77, ?, 0, 0, 0, 0, 0, 0, "
                "0, 0, 0, 0, 0, 0, 0, '')", (card_id, nid, ordinal))
            card_id += 1
    conn.commit()
    conn.close()

    with zipfile.ZipFile(path, "w") as zf:
        zf.write(tmp_col, collection_name)
        if media:
            zf.writestr("0", b"fake mp3 bytes")
            zf.writestr("media", json.dumps({"0": "bano.mp3"}))
    os.remove(tmp_col)


@pytest.fixture
def imported(tmp_path, storage, service):
    apkg = tmp_path / "deck.apkg"
    build_apkg(apkg)
    media_dir = tmp_path / "media"
    stats = import_apkg(str(apkg), storage, service.scheduler,
                        str(media_dir))
    return stats, media_dir


def test_basic_apkg_imports_correctly(imported, storage):
    stats, _ = imported
    # 1 + 1 + 2 (reversed) importable; cloze + empty skipped.
    assert stats.cards == 4
    assert stats.decks == {"Spanish"}
    cards = {row["id"]: row for row in
             storage.conn.execute("SELECT * FROM cards")}
    assert len(cards) == 4
    # Reversed note produced both directions.
    fronts = {row["front"] for row in cards.values()}
    assert "gato" in fronts and "cat" in fronts
    # HTML stripped.
    assert any(row["front"].startswith("Hola") for row in cards.values())


def test_sound_reference_detected(imported, storage):
    row = storage.conn.execute(
        "SELECT * FROM cards WHERE audio_filename IS NOT NULL").fetchone()
    assert row["audio_filename"] == "bano.mp3"
    assert row["audio_side"] == "front"
    assert "[sound:" not in row["front"]


def test_media_mapping_applied(imported):
    stats, media_dir = imported
    assert stats.media_files == 1
    assert (media_dir / "bano.mp3").read_bytes() == b"fake mp3 bytes"


def test_unsupported_cards_skipped_with_reasons(imported):
    stats, _ = imported
    assert stats.skipped_cards == 2
    reasons = {reason for _cid, reason in stats.skipped}
    assert "cloze note type" in reasons
    assert "empty after HTML stripping" in reasons


def test_tags_become_categories(imported, storage):
    assert set(storage.all_tags()) == {"bathroom", "travel", "greetings",
                                       "animals"}


def test_reimport_preserves_review_progress(tmp_path, storage, service):
    apkg = tmp_path / "deck.apkg"
    build_apkg(apkg)
    import_apkg(str(apkg), storage, service.scheduler, str(tmp_path / "m"))
    service.answer_card(1, 3)
    state = dict(storage.get_review_state(1))
    import_apkg(str(apkg), storage, service.scheduler, str(tmp_path / "m"))
    assert dict(storage.get_review_state(1)) == state


def test_new_compressed_format_rejected_with_hint(tmp_path, storage,
                                                  service):
    apkg = tmp_path / "new.apkg"
    with zipfile.ZipFile(apkg, "w") as zf:
        zf.writestr("collection.anki21b", b"\x28\xb5\x2f\xfd")
    with pytest.raises(ApkgError, match="Support older Anki versions"):
        import_apkg(str(apkg), storage, service.scheduler,
                    str(tmp_path / "m"))
