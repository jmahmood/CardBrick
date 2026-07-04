"""Local SQLite storage.

Owns the application database: imported cards plus FSRS review state.
Nothing here knows about .apkg files, pygame, or the FSRS algorithm —
review_state rows are written from dicts produced by scheduler.py.
"""

import os
import sqlite3

SCHEMA = """
CREATE TABLE IF NOT EXISTS cards (
    id             INTEGER PRIMARY KEY,
    note_id        INTEGER NOT NULL,
    deck           TEXT NOT NULL,
    front          TEXT NOT NULL,
    back           TEXT NOT NULL,
    tags           TEXT NOT NULL DEFAULT '',
    audio_filename TEXT,
    audio_side     TEXT CHECK (audio_side IN ('front', 'back') OR audio_side IS NULL)
);

CREATE TABLE IF NOT EXISTS review_state (
    card_id        INTEGER PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    due            TEXT NOT NULL,
    stability      REAL,
    difficulty     REAL,
    elapsed_days   INTEGER NOT NULL DEFAULT 0,
    scheduled_days INTEGER NOT NULL DEFAULT 0,
    reps           INTEGER NOT NULL DEFAULT 0,
    lapses         INTEGER NOT NULL DEFAULT 0,
    state          INTEGER NOT NULL,
    fsrs_json      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cards_deck ON cards(deck);
CREATE INDEX IF NOT EXISTS idx_review_due ON review_state(due);
"""


class Storage:
    def __init__(self, db_path):
        os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)
        self.conn = sqlite3.connect(db_path)
        self.conn.row_factory = sqlite3.Row
        self.conn.execute("PRAGMA foreign_keys = ON")
        self.conn.executescript(SCHEMA)

    def close(self):
        self.conn.close()

    # -- import-side writes -------------------------------------------------

    def upsert_card(self, card_id, note_id, deck, front, back, tags,
                    audio_filename=None, audio_side=None):
        self.conn.execute(
            """INSERT INTO cards (id, note_id, deck, front, back, tags,
                                  audio_filename, audio_side)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                   note_id=excluded.note_id, deck=excluded.deck,
                   front=excluded.front, back=excluded.back,
                   tags=excluded.tags, audio_filename=excluded.audio_filename,
                   audio_side=excluded.audio_side""",
            (card_id, note_id, deck, front, back, tags,
             audio_filename, audio_side))

    def init_review_state(self, state):
        """Insert initial FSRS state for a card, unless one already exists.

        Re-importing a deck must never reset learning progress.
        """
        self.conn.execute(
            """INSERT OR IGNORE INTO review_state
               (card_id, due, stability, difficulty, elapsed_days,
                scheduled_days, reps, lapses, state, fsrs_json)
               VALUES (:card_id, :due, :stability, :difficulty, :elapsed_days,
                       :scheduled_days, :reps, :lapses, :state, :fsrs_json)""",
            state)

    def commit(self):
        self.conn.commit()

    # -- review-side reads/writes -------------------------------------------

    def save_review_state(self, state):
        self.conn.execute(
            """UPDATE review_state SET
                   due=:due, stability=:stability, difficulty=:difficulty,
                   elapsed_days=:elapsed_days, scheduled_days=:scheduled_days,
                   reps=:reps, lapses=:lapses, state=:state,
                   fsrs_json=:fsrs_json
               WHERE card_id=:card_id""",
            state)
        self.conn.commit()

    def next_due_card(self, now_iso, deck=None):
        """The most overdue card, joined with its review state."""
        query = """SELECT c.*, r.due, r.reps, r.lapses, r.state AS fsrs_state,
                          r.fsrs_json
                   FROM cards c JOIN review_state r ON r.card_id = c.id
                   WHERE r.due <= ?"""
        params = [now_iso]
        if deck:
            query += " AND c.deck = ?"
            params.append(deck)
        query += " ORDER BY r.due LIMIT 1"
        return self.conn.execute(query, params).fetchone()

    def next_due_time(self, deck=None):
        """ISO timestamp of the earliest due card, or None if no cards."""
        query = """SELECT MIN(r.due) AS next_due
                   FROM cards c JOIN review_state r ON r.card_id = c.id"""
        params = []
        if deck:
            query += " WHERE c.deck = ?"
            params.append(deck)
        row = self.conn.execute(query, params).fetchone()
        return row["next_due"] if row else None

    def due_count(self, now_iso, deck=None):
        query = """SELECT COUNT(*) AS n
                   FROM cards c JOIN review_state r ON r.card_id = c.id
                   WHERE r.due <= ?"""
        params = [now_iso]
        if deck:
            query += " AND c.deck = ?"
            params.append(deck)
        return self.conn.execute(query, params).fetchone()["n"]

    def decks(self, now_iso):
        """All deck names with total and currently-due card counts."""
        return self.conn.execute(
            """SELECT c.deck AS name, COUNT(*) AS total,
                      SUM(CASE WHEN r.due <= ? THEN 1 ELSE 0 END) AS due
               FROM cards c JOIN review_state r ON r.card_id = c.id
               GROUP BY c.deck ORDER BY c.deck""",
            (now_iso,)).fetchall()
