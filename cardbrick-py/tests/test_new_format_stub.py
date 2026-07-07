"""Anki's default export (legacy-support checkbox left unchecked) writes
*only* collection.anki21b (zstd+protobuf, unsupported here) plus a
one-note collection.anki21 placeholder ("Please update to the latest
Anki version..."). Importing that placeholder as if it were the real
deck is a silent-data-loss trap; it must be detected and rejected."""

import json
import sqlite3
import zipfile

import pytest

from cardbrick.importer import ApkgError, import_apkg
from test_import import BASIC, COL_SCHEMA, DECKS, MODELS

REAL_ANKI_STUB_TEXT = "Please update to the latest Anki version, then " \
                     "import the .colpkg/.apkg file again."


def _build_collection(col_path, note_fields):
    conn = sqlite3.connect(col_path)
    conn.executescript(COL_SCHEMA)
    conn.execute(
        "INSERT INTO col VALUES (1, 0, 0, 0, 11, 0, 0, 0, '{}', ?, ?, "
        "'{}', '{}')", (json.dumps(MODELS), json.dumps(DECKS)))
    conn.execute(
        "INSERT INTO notes VALUES (1, 'g1', ?, 0, 0, '', ?, ?, 0, 0, '')",
        (BASIC, "\x1f".join(note_fields), note_fields[0]))
    conn.execute(
        "INSERT INTO cards VALUES (1, 1, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, "
        "0, 0, 0, 0, 0, '')")
    conn.commit()
    conn.close()


def test_stub_with_real_anki_placeholder_text_is_rejected(
        tmp_path, storage, service):
    apkg = tmp_path / "deck.apkg"
    col = str(apkg) + ".col"
    _build_collection(col, [REAL_ANKI_STUB_TEXT, ""])
    with zipfile.ZipFile(apkg, "w") as zf:
        zf.write(col, "collection.anki21")
        zf.writestr("collection.anki21b", b"\x28\xb5\x2f\xfd fake zstd")

    with pytest.raises(ApkgError, match="Support older Anki versions"):
        import_apkg(str(apkg), storage, service.scheduler,
                    str(tmp_path / "media"))


def test_single_note_alongside_anki21b_is_rejected_even_without_the_exact_text(
        tmp_path, storage, service):
    """Belt and braces: even if the placeholder wording ever changes,
    "exactly one note, and the real new-format file sits right there"
    is enough to refuse rather than silently import garbage."""
    apkg = tmp_path / "deck.apkg"
    col = str(apkg) + ".col"
    _build_collection(col, ["some other single note", ""])
    with zipfile.ZipFile(apkg, "w") as zf:
        zf.write(col, "collection.anki21")
        zf.writestr("collection.anki21b", b"\x28\xb5\x2f\xfd fake zstd")

    with pytest.raises(ApkgError, match="Support older Anki versions"):
        import_apkg(str(apkg), storage, service.scheduler,
                    str(tmp_path / "media"))


def test_single_note_without_anki21b_is_imported_normally(
        tmp_path, storage, service):
    """A genuinely tiny one-card deck (no new-format file present at
    all) is not a stub and must import fine."""
    apkg = tmp_path / "deck.apkg"
    col = str(apkg) + ".col"
    _build_collection(col, ["hola", "hello"])
    with zipfile.ZipFile(apkg, "w") as zf:
        zf.write(col, "collection.anki21")  # no collection.anki21b

    stats = import_apkg(str(apkg), storage, service.scheduler,
                        str(tmp_path / "media"))
    assert stats.cards == 1


def test_only_anki21b_present_still_gives_clear_error(tmp_path, storage,
                                                       service):
    apkg = tmp_path / "deck.apkg"
    with zipfile.ZipFile(apkg, "w") as zf:
        zf.writestr("collection.anki21b", b"\x28\xb5\x2f\xfd")
    with pytest.raises(ApkgError, match="Support older Anki versions"):
        import_apkg(str(apkg), storage, service.scheduler,
                    str(tmp_path / "media"))
