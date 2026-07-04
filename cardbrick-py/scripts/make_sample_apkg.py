#!/usr/bin/env python3
"""Generate a small legacy-format .apkg for testing the importer.

Builds a schema-11 collection.anki2 by hand (no Anki involved) with a
"Basic" and a "Basic (and reversed card)" note type, a few Spanish
cards, one audio reference, and a tiny WAV file as media.

Usage: python scripts/make_sample_apkg.py [output.apkg]
"""

import json
import math
import os
import sqlite3
import struct
import sys
import tempfile
import time
import wave
import zipfile

SCHEMA = """
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

BASIC_MID = 1000
REVERSED_MID = 1001
DECK_ID = 1234

MODELS = {
    str(BASIC_MID): {
        "id": BASIC_MID, "name": "Basic", "type": 0,
        "flds": [{"name": "Front", "ord": 0}, {"name": "Back", "ord": 1}],
        "tmpls": [{"name": "Card 1", "ord": 0}],
    },
    str(REVERSED_MID): {
        "id": REVERSED_MID, "name": "Basic (and reversed card)", "type": 0,
        "flds": [{"name": "Front", "ord": 0}, {"name": "Back", "ord": 1}],
        "tmpls": [{"name": "Card 1", "ord": 0}, {"name": "Card 2", "ord": 1}],
    },
}

DECKS = {str(DECK_ID): {"id": DECK_ID, "name": "Spanish"}}

NOTES = [
    # (id, mid, fields, tags)
    (1, BASIC_MID,
     ["¿Dónde está el baño?[sound:hola.wav]",
      "Where is the bathroom?"], "travel"),
    (2, BASIC_MID,
     ["<b>Hola</b><br>informal greeting", "Hello"], "greetings"),
    (3, REVERSED_MID, ["gato", "cat"], "animals"),
]


def make_wav(path):
    """0.3s 440 Hz beep, standard library only."""
    rate = 22050
    with wave.open(path, "w") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        for i in range(int(rate * 0.3)):
            sample = int(12000 * math.sin(2 * math.pi * 440 * i / rate))
            w.writeframes(struct.pack("<h", sample))


def main(out_path):
    now = int(time.time())
    with tempfile.TemporaryDirectory() as tmp:
        col_path = os.path.join(tmp, "collection.anki2")
        conn = sqlite3.connect(col_path)
        conn.executescript(SCHEMA)
        conn.execute(
            "INSERT INTO col VALUES (1, ?, ?, ?, 11, 0, 0, 0, '{}', ?, ?, "
            "'{}', '{}')",
            (now, now, now, json.dumps(MODELS), json.dumps(DECKS)))
        card_id = 1
        for nid, mid, fields, tags in NOTES:
            conn.execute(
                "INSERT INTO notes VALUES (?, ?, ?, ?, 0, ?, ?, ?, 0, 0, '')",
                (nid, f"guid{nid}", mid, now, f" {tags} ",
                 "\x1f".join(fields), fields[0]))
            n_templates = len(MODELS[str(mid)]["tmpls"])
            for ordinal in range(n_templates):
                conn.execute(
                    "INSERT INTO cards VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, "
                    "0, 0, 0, 0, 0, 0, 0, 0, '')",
                    (card_id, nid, DECK_ID, ordinal, now))
                card_id += 1
        conn.commit()
        conn.close()

        wav_path = os.path.join(tmp, "beep.wav")
        make_wav(wav_path)

        with zipfile.ZipFile(out_path, "w") as zf:
            zf.write(col_path, "collection.anki2")
            zf.write(wav_path, "0")
            zf.writestr("media", json.dumps({"0": "hola.wav"}))
    print(f"Wrote {out_path}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "sample.apkg")
