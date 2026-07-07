"""Anki .apkg importer.

Reads an .apkg archive directly (it is a zip containing an SQLite
collection plus numbered media files) and converts it into the
application's own database and media folder. No Anki code is used.

Supported: the legacy collection formats `collection.anki2` and
`collection.anki21` (schema 11), which every Anki desktop can export
via "Support older Anki versions". The newer `collection.anki21b`
variant is zstd-compressed and is deliberately not supported, to avoid
native dependencies.

Note types: cards are reduced to a front/back pair from the note's
first two fields, honouring the card ordinal so "Basic (and reversed
card)" produces both directions. Cloze note types are skipped.
"""

import json
import os
import shutil
import sqlite3
import tempfile
import zipfile

from .scheduler import iso, now_utc
from .textutil import clean_html, extract_audio, extract_image_filename

AUDIO_EXTENSIONS = {".mp3", ".ogg", ".wav", ".flac", ".m4a", ".opus"}

CLOZE_MODEL_TYPE = 1

# A note type is recognised as the four-phase vocabulary card (see
# cardbrick/vocab_card.py) purely by field *names* — Anki model ids
# vary per collection, but "Word" first and an "Example ES" field
# somewhere are the two facts genanki/Anki always preserve.
VOCAB_REQUIRED_FIELDS = ("word", "example es")


class ApkgError(Exception):
    pass


class ImportStats:
    def __init__(self):
        self.decks = set()
        self.tags = set()
        self.cards = 0
        self.notes = 0
        self.media_files = 0
        self.skipped = []  # (card_id, reason) for auditing

    @property
    def skipped_cards(self):
        return len(self.skipped)

    def skip(self, card_id, reason):
        self.skipped.append((card_id, reason))

    def summary(self):
        text = (f"Imported {self.cards} cards from {self.notes} notes "
                f"into {len(self.decks)} deck(s); "
                f"{self.media_files} media files copied; "
                f"{self.skipped_cards} unsupported cards skipped.")
        if self.skipped:
            reasons = {}
            for _cid, reason in self.skipped:
                reasons[reason] = reasons.get(reason, 0) + 1
            details = ", ".join(f"{n}x {r}" for r, n in sorted(reasons.items()))
            text += f"\nSkipped: {details}"
        return text


def import_apkg(apkg_path, storage, scheduler, media_dir):
    """Import an .apkg into storage, copying media into media_dir."""
    stats = ImportStats()
    with zipfile.ZipFile(apkg_path) as zf:
        names = set(zf.namelist())
        collection = next(
            (n for n in ("collection.anki21", "collection.anki2")
             if n in names), None)
        if collection is None:
            if "collection.anki21b" in names:
                raise ApkgError(
                    "This .apkg uses the new compressed format. Re-export "
                    "it from Anki with 'Support older Anki versions' "
                    "checked.")
            raise ApkgError("No Anki collection found inside the archive.")

        with tempfile.TemporaryDirectory() as tmp:
            col_path = os.path.join(tmp, collection)
            with zf.open(collection) as src, open(col_path, "wb") as dst:
                shutil.copyfileobj(src, dst)
            _import_collection(col_path, storage, scheduler, stats)

        stats.media_files = _copy_media(zf, names, media_dir)

    storage.commit()
    return stats


def _import_collection(col_path, storage, scheduler, stats):
    conn = sqlite3.connect(col_path)
    conn.row_factory = sqlite3.Row
    try:
        col = conn.execute(
            "SELECT decks, models FROM col LIMIT 1").fetchone()
        decks = _deck_names(conn, col)
        models = _models(conn, col)

        notes = {}
        for row in conn.execute("SELECT id, mid, flds, tags FROM notes"):
            notes[row["id"]] = row

        for card in conn.execute("SELECT id, nid, did, ord FROM cards"):
            note = notes.get(card["nid"])
            if note is None:
                stats.skip(card["id"], "orphan card (missing note)")
                continue
            model = models.get(str(note["mid"])) or models.get(note["mid"])
            field_names = _model_field_names(model)
            if _is_vocab_model(field_names):
                deck = decks.get(str(card["did"]),
                                 decks.get(card["did"], "Default"))
                _import_vocab_note(card, note, field_names, deck, storage,
                                   scheduler, stats)
                continue
            if model and model.get("type") == CLOZE_MODEL_TYPE:
                stats.skip(card["id"], "cloze note type")
                continue

            fields = note["flds"].split("\x1f")
            if len(fields) < 2 or card["ord"] > 1:
                # Only front/back pairs from the first two fields are
                # supported (Basic and Basic (and reversed card)).
                stats.skip(card["id"], "unsupported template/fields")
                continue
            front_raw, back_raw = fields[0], fields[1]
            if card["ord"] == 1:
                front_raw, back_raw = back_raw, front_raw

            front_raw, front_audio = extract_audio(front_raw)
            back_raw, back_audio = extract_audio(back_raw)
            front = clean_html(front_raw)
            back = clean_html(back_raw)
            if not front and not back:
                stats.skip(card["id"], "empty after HTML stripping")
                continue

            audio_filename, audio_side = None, None
            if front_audio:
                audio_filename, audio_side = front_audio[0], "front"
            elif back_audio:
                audio_filename, audio_side = back_audio[0], "back"

            deck = decks.get(str(card["did"]),
                             decks.get(card["did"], "Default"))
            tags = note["tags"].strip()
            storage.upsert_card(
                card_id=card["id"], note_id=note["id"], deck=deck,
                front=front, back=back, tags=tags,
                audio_filename=audio_filename, audio_side=audio_side,
                now_iso=iso(now_utc()))
            storage.init_review_state(scheduler.initial_state(card["id"]))

            stats.cards += 1
            stats.decks.add(deck)
            stats.tags.update(tags.split())
            notes_seen = getattr(stats, "_notes_seen", set())
            if note["id"] not in notes_seen:
                notes_seen.add(note["id"])
                stats.notes += 1
            stats._notes_seen = notes_seen
    finally:
        conn.close()


def _model_field_names(model):
    """Field names in position order, or [] if unavailable.

    New (schema 18+) collections store note type config as protobuf,
    which this importer deliberately does not parse (no protobuf
    dependency) — vocab-card import from such collections falls back
    to being skipped like any other unrecognised template; use the CSV
    importer (cardbrick/vocab_csv.py) for those exports instead.
    """
    if not model:
        return []
    try:
        ordered = sorted(model.get("flds") or (), key=lambda f: f.get("ord", 0))
        return [f.get("name", "") for f in ordered]
    except (TypeError, AttributeError):
        return []


def _is_vocab_model(field_names):
    lowered = [n.strip().lower() for n in field_names]
    return (bool(lowered) and lowered[0] == "word" and
            "example es" in lowered)


def _import_vocab_note(card, note, field_names, deck, storage, scheduler,
                       stats):
    """Extract one 'Español MX (word + audio + example)' note.

    Unlike Basic/reversed notes there is exactly one template per note
    (Recognition), so no ordinal handling is needed. Fields are looked
    up by name so the optional Japanese columns can be present or
    absent without shifting anything else.
    """
    index = {name.strip().lower(): i for i, name in enumerate(field_names)}
    fields = note["flds"].split("\x1f")

    def field(name):
        i = index.get(name)
        return fields[i] if i is not None and i < len(fields) else ""

    word = clean_html(field("word"))
    if not word:
        stats.skip(card["id"], "vocab note missing Word")
        return

    _, word_audio_files = extract_audio(field("word audio"))
    _, example_audio_files = extract_audio(field("example audio"))
    definitions = clean_html(field("definitions"))
    example_en = clean_html(field("example en"))
    tags = note["tags"].strip()

    storage.upsert_card(
        card_id=card["id"], note_id=note["id"], deck=deck, front=word,
        back=definitions or example_en, tags=tags,
        audio_filename=word_audio_files[0] if word_audio_files else None,
        audio_side="front" if word_audio_files else None,
        now_iso=iso(now_utc()), card_type="vocab")
    storage.upsert_vocab_card(
        card_id=card["id"], word=word,
        word_jp=clean_html(field("word jp")) or None,
        gendered_forms=clean_html(field("gendered forms")),
        definitions=definitions,
        image_filename=extract_image_filename(field("image")),
        example_es=clean_html(field("example es")),
        example_audio=example_audio_files[0] if example_audio_files else None,
        example_en=example_en,
        example_jp=clean_html(field("example jp")) or None,
        report_link=field("report link").strip() or None)
    storage.init_review_state(scheduler.initial_state(card["id"]))

    stats.cards += 1
    stats.decks.add(deck)
    stats.tags.update(tags.split())
    notes_seen = getattr(stats, "_notes_seen", set())
    if note["id"] not in notes_seen:
        notes_seen.add(note["id"])
        stats.notes += 1
    stats._notes_seen = notes_seen


def _deck_names(conn, col):
    """Deck id -> name, from col JSON or the separate decks table."""
    try:
        decks_json = json.loads(col["decks"]) if col["decks"] else {}
    except (json.JSONDecodeError, TypeError):
        decks_json = {}
    if decks_json:
        return {str(k): v.get("name", "Default")
                for k, v in decks_json.items()}
    names = {}
    try:
        for row in conn.execute("SELECT id, name FROM decks"):
            # Newer schemas separate hierarchy levels with \x1f.
            names[str(row["id"])] = row["name"].replace("\x1f", "::")
    except sqlite3.OperationalError:
        pass
    return names


def _models(conn, col):
    """Model (note type) id -> model JSON dict."""
    try:
        models = json.loads(col["models"]) if col["models"] else {}
    except (json.JSONDecodeError, TypeError):
        models = {}
    if models:
        return models
    result = {}
    try:
        for row in conn.execute("SELECT id, config FROM notetypes"):
            # Config is protobuf in new schemas; assume non-cloze rather
            # than pulling in a protobuf dependency.
            result[str(row["id"])] = {"type": 0}
    except sqlite3.OperationalError:
        pass
    return result


def _copy_media(zf, names, media_dir):
    """Copy media files out of the archive under their real names."""
    if "media" not in names:
        return 0
    try:
        with zf.open("media") as f:
            mapping = json.load(f)
    except (json.JSONDecodeError, UnicodeDecodeError):
        # New export format stores the media map as protobuf; media from
        # such archives is skipped (the collection itself is anki21b and
        # already rejected earlier, so this is just belt and braces).
        return 0

    os.makedirs(media_dir, exist_ok=True)
    copied = 0
    for zip_name, real_name in mapping.items():
        if zip_name not in names:
            continue
        # Guard against path traversal from hostile archives.
        safe_name = os.path.basename(real_name)
        if not safe_name:
            continue
        with zf.open(zip_name) as src, \
                open(os.path.join(media_dir, safe_name), "wb") as dst:
            shutil.copyfileobj(src, dst)
        copied += 1
    return copied
