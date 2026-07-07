"""Local SQLite storage.

Owns the application database: imported cards, FSRS review state, the
append-only review log, study sessions, and child profiles. Nothing here
knows about .apkg files, pygame, or the FSRS algorithm — review_state
rows are written from dicts produced by scheduler.py.

The schema is versioned through the ``meta`` table and migrates in place,
so databases created by the original prototype keep their review
progress when opened by the CardBrick-style app.
"""

import json
import logging
import os
import shutil
import sqlite3

log = logging.getLogger(__name__)

SCHEMA_VERSION = 7

SCHEMA = """
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE IF NOT EXISTS cards (
    id             INTEGER PRIMARY KEY,
    note_id        INTEGER NOT NULL,
    deck           TEXT NOT NULL,
    front          TEXT NOT NULL,
    back           TEXT NOT NULL,
    tags           TEXT NOT NULL DEFAULT '',
    audio_filename TEXT,
    audio_side     TEXT CHECK (audio_side IN ('front', 'back') OR audio_side IS NULL),
    suspended      INTEGER NOT NULL DEFAULT 0,
    buried_until   TEXT,
    created_at     TEXT,
    updated_at     TEXT,
    card_type      TEXT NOT NULL DEFAULT 'basic'
);

-- One row per 'vocab' card_type card: the four-phase word/example/image/
-- definition layout (see cardbrick/vocab_card.py). Untouched for 'basic'
-- cards, which use only front/back/audio_filename above.
CREATE TABLE IF NOT EXISTS vocab_cards (
    card_id         INTEGER PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    word            TEXT NOT NULL,
    word_jp         TEXT,
    gendered_forms  TEXT,
    definitions     TEXT,
    image_filename  TEXT,
    example_es      TEXT,
    example_audio   TEXT,
    example_en      TEXT,
    example_jp      TEXT,
    report_link     TEXT
);

CREATE TABLE IF NOT EXISTS review_state (
    card_id          INTEGER PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    due              TEXT NOT NULL,
    stability        REAL,
    difficulty       REAL,
    elapsed_days     INTEGER NOT NULL DEFAULT 0,
    scheduled_days   INTEGER NOT NULL DEFAULT 0,
    reps             INTEGER NOT NULL DEFAULT 0,
    lapses           INTEGER NOT NULL DEFAULT 0,
    state            INTEGER NOT NULL,
    fsrs_json        TEXT NOT NULL,
    last_reviewed_at TEXT
);

CREATE TABLE IF NOT EXISTS review_log (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id             INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    reviewed_at         TEXT NOT NULL,
    rating              INTEGER NOT NULL,
    elapsed_ms          INTEGER,
    was_new             INTEGER NOT NULL DEFAULT 0,
    previous_state_json TEXT NOT NULL,
    new_state_json      TEXT NOT NULL,
    session_id          INTEGER,
    undone              INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id      INTEGER,
    started_at      TEXT NOT NULL,
    ended_at        TEXT,
    category_filter TEXT,
    cards_reviewed  INTEGER NOT NULL DEFAULT 0,
    new_cards       INTEGER NOT NULL DEFAULT 0,
    review_cards    INTEGER NOT NULL DEFAULT 0,
    again_count     INTEGER NOT NULL DEFAULT 0,
    hard_count      INTEGER NOT NULL DEFAULT 0,
    good_count      INTEGER NOT NULL DEFAULT 0,
    easy_count      INTEGER NOT NULL DEFAULT 0,
    buried_count    INTEGER NOT NULL DEFAULT 0,
    suspended_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS child_profiles (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL,
    active_categories    TEXT,
    active_decks         TEXT,
    daily_new_cards      INTEGER NOT NULL DEFAULT 0,
    daily_review_cards   INTEGER NOT NULL DEFAULT 40,
    daily_goal_cards     INTEGER NOT NULL DEFAULT 150,
    session_card_limit   INTEGER NOT NULL DEFAULT 20,
    session_time_minutes INTEGER NOT NULL DEFAULT 10,
    study_direction      TEXT NOT NULL DEFAULT 'normal',
    study_ahead_days     INTEGER NOT NULL DEFAULT 1,
    study_ahead_enabled  INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_cards_deck ON cards(deck);
CREATE INDEX IF NOT EXISTS idx_review_due ON review_state(due);
CREATE INDEX IF NOT EXISTS idx_log_session ON review_log(session_id);
CREATE INDEX IF NOT EXISTS idx_log_reviewed_at ON review_log(reviewed_at);
"""

# Columns added to prototype-era tables; applied with ALTER TABLE when a
# pre-versioned database is opened.
_CARD_UPGRADES = {
    "suspended": "INTEGER NOT NULL DEFAULT 0",
    "buried_until": "TEXT",
    "created_at": "TEXT",
    "updated_at": "TEXT",
    "card_type": "TEXT NOT NULL DEFAULT 'basic'",
}
_STATE_UPGRADES = {
    "last_reviewed_at": "TEXT",
}
_PROFILE_UPGRADES = {
    "active_decks": "TEXT",
    "daily_goal_cards": "INTEGER NOT NULL DEFAULT 150",
    "study_ahead_days": "INTEGER NOT NULL DEFAULT 1",
    "study_ahead_enabled": "INTEGER NOT NULL DEFAULT 1",
}


class Storage:
    def __init__(self, db_path):
        os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)
        self.db_path = db_path
        try:
            self.conn = sqlite3.connect(db_path)
            self.conn.row_factory = sqlite3.Row
            self.conn.execute("PRAGMA foreign_keys = ON")
        except sqlite3.Error:
            log.exception("cannot open database %s", db_path)
            raise
        try:
            self._migrate()
        except sqlite3.Error:
            log.exception("migration failed for %s", db_path)
            raise

    def schema_version(self):
        """Stored schema version, or None for a pre-versioned database."""
        try:
            row = self.conn.execute(
                "SELECT value FROM meta WHERE key='schema_version'"
            ).fetchone()
            return int(row["value"]) if row else None
        except sqlite3.OperationalError:
            return None  # no meta table yet

    def _migrate(self):
        stored = self.schema_version()
        has_cards = self.conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND "
            "name='cards'").fetchone() is not None
        needs_upgrade = has_cards and (stored is None or
                                       stored < SCHEMA_VERSION)
        if needs_upgrade:
            # An existing database is about to be altered: keep a
            # one-shot backup so a bad migration is recoverable.
            self._backup(stored)
        self._upgrade_table("cards", _CARD_UPGRADES)
        self._upgrade_table("review_state", _STATE_UPGRADES)
        added = self._upgrade_table("child_profiles", _PROFILE_UPGRADES)
        if "daily_goal_cards" in added:
            # Existing profiles were tuned around the old review+new caps;
            # seed the goal from them so nobody's day suddenly triples.
            self.conn.execute(
                """UPDATE child_profiles
                   SET daily_goal_cards = daily_new_cards +
                                          daily_review_cards""")
        has_profiles = self.conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND "
            "name='child_profiles'").fetchone() is not None
        if has_profiles and needs_upgrade and (stored is None or
                                               stored < 6):
            # v5 changed session_card_limit/session_time_minutes from
            # "one sitting" to "one sprint" but left the old sitting-
            # sized defaults (50 cards / 15 min) in place, so a whole
            # day fit in a single sprint. Re-baseline profiles still on
            # those defaults to sprint scale; custom values are kept.
            self.conn.execute(
                """UPDATE child_profiles SET session_card_limit = 20
                   WHERE session_card_limit = 50""")
            self.conn.execute(
                """UPDATE child_profiles SET session_time_minutes = 10
                   WHERE session_time_minutes = 15""")
        if has_profiles and needs_upgrade and (stored is None or
                                               stored < 7):
            # v7: new-card intake is paced by the daily goal itself
            # (0 = auto) instead of a fixed drip. Profiles still on the
            # old default of 10/day switch to auto so a fresh deck can
            # fill a whole goal-sized day; deliberate caps are kept.
            # (Runs after the goal seeding above, which reads the old
            # daily_new_cards value.)
            self.conn.execute(
                """UPDATE child_profiles SET daily_new_cards = 0
                   WHERE daily_new_cards = 10""")
        self.conn.executescript(SCHEMA)
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES "
            "('schema_version', ?)", (str(SCHEMA_VERSION),))
        self.conn.commit()
        if needs_upgrade:
            log.info("database migrated: schema v%s -> v%s",
                     stored if stored is not None else "pre-1",
                     SCHEMA_VERSION)

    def _backup(self, from_version):
        label = f"v{from_version}" if from_version is not None else "pre1"
        backup_path = f"{self.db_path}.backup-{label}"
        if os.path.exists(backup_path):
            return  # keep the oldest backup for this version jump
        try:
            shutil.copyfile(self.db_path, backup_path)
            log.info("database backed up to %s before migration",
                     backup_path)
        except OSError as exc:
            log.warning("could not back up database before migration: %s",
                        exc)

    def _upgrade_table(self, table, upgrades):
        """ALTER TABLE in any missing columns; returns the ones added."""
        row = self.conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            (table,)).fetchone()
        if row is None:
            return []
        existing = {r["name"] for r in
                    self.conn.execute(f"PRAGMA table_info({table})")}
        added = []
        for column, decl in upgrades.items():
            if column not in existing:
                self.conn.execute(
                    f"ALTER TABLE {table} ADD COLUMN {column} {decl}")
                added.append(column)
        return added

    def close(self):
        self.conn.close()

    def commit(self):
        self.conn.commit()

    # -- import-side writes -------------------------------------------------

    def upsert_card(self, card_id, note_id, deck, front, back, tags,
                    audio_filename=None, audio_side=None, now_iso=None,
                    card_type="basic"):
        self.conn.execute(
            """INSERT INTO cards (id, note_id, deck, front, back, tags,
                                  audio_filename, audio_side,
                                  created_at, updated_at, card_type)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                   note_id=excluded.note_id, deck=excluded.deck,
                   front=excluded.front, back=excluded.back,
                   tags=excluded.tags, audio_filename=excluded.audio_filename,
                   audio_side=excluded.audio_side,
                   updated_at=excluded.updated_at,
                   card_type=excluded.card_type""",
            (card_id, note_id, deck, front, back, tags,
             audio_filename, audio_side, now_iso, now_iso, card_type))

    def upsert_vocab_card(self, card_id, word, word_jp, gendered_forms,
                          definitions, image_filename, example_es,
                          example_audio, example_en, example_jp,
                          report_link):
        """Insert or refresh the phase-content row for a 'vocab' card.

        Re-importing updates the displayed content only; review_state
        (FSRS progress) is untouched, same guarantee as basic cards.
        """
        self.conn.execute(
            """INSERT INTO vocab_cards
               (card_id, word, word_jp, gendered_forms, definitions,
                image_filename, example_es, example_audio, example_en,
                example_jp, report_link)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(card_id) DO UPDATE SET
                   word=excluded.word, word_jp=excluded.word_jp,
                   gendered_forms=excluded.gendered_forms,
                   definitions=excluded.definitions,
                   image_filename=excluded.image_filename,
                   example_es=excluded.example_es,
                   example_audio=excluded.example_audio,
                   example_en=excluded.example_en,
                   example_jp=excluded.example_jp,
                   report_link=excluded.report_link""",
            (card_id, word, word_jp, gendered_forms, definitions,
             image_filename, example_es, example_audio, example_en,
             example_jp, report_link))

    def get_vocab_detail(self, card_id):
        """The phase-content row for a 'vocab' card, or None."""
        return self.conn.execute(
            "SELECT * FROM vocab_cards WHERE card_id = ?",
            (card_id,)).fetchone()

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

    # -- card reads -----------------------------------------------------------

    def get_card(self, card_id):
        """A card joined with its review state (the shape the UI consumes)."""
        return self.conn.execute(
            """SELECT c.*, r.due, r.reps, r.lapses, r.state AS fsrs_state,
                      r.fsrs_json
               FROM cards c JOIN review_state r ON r.card_id = c.id
               WHERE c.id = ?""", (card_id,)).fetchone()

    def get_review_state(self, card_id):
        return self.conn.execute(
            "SELECT * FROM review_state WHERE card_id = ?",
            (card_id,)).fetchone()

    def all_tags(self):
        """Sorted distinct tags across all cards."""
        tags = set()
        for row in self.conn.execute("SELECT DISTINCT tags FROM cards"):
            tags.update(row["tags"].split())
        return sorted(tags)

    def queue_candidates(self, now_iso, new_cards=False, decks=None):
        """Cards eligible for the review queue, before tag filtering.

        Excludes suspended cards and cards buried until later than now.
        ``new_cards`` selects never-reviewed cards (reps = 0) ordered by
        id; otherwise cards that are due now, most overdue first.
        ``decks`` of None means every deck; otherwise a list of deck
        names to restrict to (unlike tags, a card has exactly one deck,
        so this filters in SQL rather than in Python).
        """
        base = """SELECT c.*, r.due, r.reps, r.lapses,
                         r.state AS fsrs_state, r.fsrs_json
                  FROM cards c JOIN review_state r ON r.card_id = c.id
                  WHERE c.suspended = 0
                    AND (c.buried_until IS NULL OR c.buried_until <= :now)"""
        if decks is not None and len(decks) == 0:
            return []  # explicitly "no decks active": nothing is due
        params = {"now": now_iso}
        if decks is not None:
            placeholders = ",".join(f":deck{i}" for i in range(len(decks)))
            base += f" AND c.deck IN ({placeholders})"
            params.update({f"deck{i}": name for i, name in enumerate(decks)})
        if new_cards:
            query = base + " AND r.reps = 0 ORDER BY c.id"
        else:
            query = base + " AND r.reps > 0 AND r.due <= :now ORDER BY r.due"
        return self.conn.execute(query, params).fetchall()

    def ahead_candidates(self, now_iso, horizon_iso, decks=None):
        """Cards due after now but within the study-ahead horizon.

        Same eligibility rules as queue_candidates (no suspended cards,
        no buried cards, optional deck restriction), ordered soonest-due
        first so the most urgent cards are pulled forward first. Used to
        top up a sprint when today's due pool runs dry.
        """
        if decks is not None and len(decks) == 0:
            return []  # explicitly "no decks active": nothing to pull
        query = """SELECT c.*, r.due, r.reps, r.lapses,
                          r.state AS fsrs_state, r.fsrs_json
                   FROM cards c JOIN review_state r ON r.card_id = c.id
                   WHERE c.suspended = 0
                     AND (c.buried_until IS NULL OR c.buried_until <= :now)
                     AND r.reps > 0 AND r.due > :now AND r.due <= :horizon"""
        params = {"now": now_iso, "horizon": horizon_iso}
        if decks is not None:
            placeholders = ",".join(f":deck{i}" for i in range(len(decks)))
            query += f" AND c.deck IN ({placeholders})"
            params.update({f"deck{i}": name for i, name in enumerate(decks)})
        query += " ORDER BY r.due"
        return self.conn.execute(query, params).fetchall()

    def suspended_cards(self):
        return self.conn.execute(
            "SELECT * FROM cards WHERE suspended = 1 ORDER BY deck, id"
        ).fetchall()

    # -- review-side writes ---------------------------------------------------

    def save_review_state(self, state, last_reviewed_at=None):
        state = dict(state)
        state["last_reviewed_at"] = last_reviewed_at
        self.conn.execute(
            """UPDATE review_state SET
                   due=:due, stability=:stability, difficulty=:difficulty,
                   elapsed_days=:elapsed_days, scheduled_days=:scheduled_days,
                   reps=:reps, lapses=:lapses, state=:state,
                   fsrs_json=:fsrs_json,
                   last_reviewed_at=:last_reviewed_at
               WHERE card_id=:card_id""",
            state)
        self.conn.commit()

    def restore_review_state(self, state):
        """Overwrite a card's review state from a snapshot dict (undo)."""
        keys = ("due", "stability", "difficulty", "elapsed_days",
                "scheduled_days", "reps", "lapses", "state", "fsrs_json",
                "last_reviewed_at")
        snapshot = {k: state.get(k) for k in keys}
        snapshot["card_id"] = state["card_id"]
        self.conn.execute(
            """UPDATE review_state SET
                   due=:due, stability=:stability, difficulty=:difficulty,
                   elapsed_days=:elapsed_days, scheduled_days=:scheduled_days,
                   reps=:reps, lapses=:lapses, state=:state,
                   fsrs_json=:fsrs_json,
                   last_reviewed_at=:last_reviewed_at
               WHERE card_id=:card_id""",
            snapshot)

    def set_suspended(self, card_id, suspended, now_iso=None):
        self.conn.execute(
            "UPDATE cards SET suspended=?, updated_at=? WHERE id=?",
            (1 if suspended else 0, now_iso, card_id))
        self.conn.commit()

    def set_buried_until(self, card_id, buried_until_iso, now_iso=None):
        self.conn.execute(
            "UPDATE cards SET buried_until=?, updated_at=? WHERE id=?",
            (buried_until_iso, now_iso, card_id))
        self.conn.commit()

    # -- review log -----------------------------------------------------------

    def append_review_log(self, card_id, reviewed_at, rating, elapsed_ms,
                          was_new, previous_state_json, new_state_json,
                          session_id):
        cur = self.conn.execute(
            """INSERT INTO review_log
               (card_id, reviewed_at, rating, elapsed_ms, was_new,
                previous_state_json, new_state_json, session_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (card_id, reviewed_at, rating, elapsed_ms, 1 if was_new else 0,
             previous_state_json, new_state_json, session_id))
        return cur.lastrowid

    def last_review_log(self, session_id):
        """Most recent not-undone log entry for a session."""
        return self.conn.execute(
            """SELECT * FROM review_log
               WHERE session_id = ? AND undone = 0
               ORDER BY id DESC LIMIT 1""", (session_id,)).fetchone()

    def mark_log_undone(self, log_id):
        self.conn.execute(
            "UPDATE review_log SET undone = 1 WHERE id = ?", (log_id,))

    def daily_counts(self, day_start_iso):
        """Distinct cards introduced/reviewed since the local day start.

        Returns (new_count, review_count, new_card_ids). A card whose
        first-ever review happened today counts only as new, even if it
        was answered again later the same day.
        """
        new_ids = {row["card_id"] for row in self.conn.execute(
            """SELECT DISTINCT card_id FROM review_log
               WHERE undone = 0 AND was_new = 1 AND reviewed_at >= ?""",
            (day_start_iso,))}
        all_ids = {row["card_id"] for row in self.conn.execute(
            """SELECT DISTINCT card_id FROM review_log
               WHERE undone = 0 AND reviewed_at >= ?""",
            (day_start_iso,))}
        return len(new_ids), len(all_ids - new_ids), new_ids

    def cards_answered_since(self, day_start_iso):
        """Set of card ids with a not-undone answer since day start.

        Used to tell "fresh" cards (which still advance the distinct-
        card daily goal) from repeats of cards already counted today.
        """
        return {row["card_id"] for row in self.conn.execute(
            """SELECT DISTINCT card_id FROM review_log
               WHERE undone = 0 AND reviewed_at >= ?""",
            (day_start_iso,))}

    def daily_history(self, since_iso):
        """Raw (reviewed_at, was_new, card_id) tuples for progress views."""
        return self.conn.execute(
            """SELECT reviewed_at, was_new, card_id FROM review_log
               WHERE undone = 0 AND reviewed_at >= ?
               ORDER BY reviewed_at""", (since_iso,)).fetchall()

    # -- sessions ---------------------------------------------------------------

    def create_session(self, profile_id, started_at, category_filter):
        cur = self.conn.execute(
            """INSERT INTO sessions (profile_id, started_at, category_filter)
               VALUES (?, ?, ?)""",
            (profile_id, started_at,
             json.dumps(category_filter) if category_filter else None))
        self.conn.commit()
        return cur.lastrowid

    def bump_session_counter(self, session_id, field):
        if field not in ("buried_count", "suspended_count"):
            raise ValueError(field)
        self.conn.execute(
            f"UPDATE sessions SET {field} = {field} + 1 WHERE id = ?",
            (session_id,))
        self.conn.commit()

    def session_row(self, session_id):
        return self.conn.execute(
            "SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()

    def sessions_in_range(self, profile_id, start_iso, end_iso):
        """(started_at, cards_reviewed) for a profile's sessions whose
        start falls in [start_iso, end_iso). Used by the stamp calendar;
        callers group by local day and decide what counts as a "logged"
        session (see ReviewService.sessions_per_day)."""
        return self.conn.execute(
            """SELECT started_at, cards_reviewed FROM sessions
               WHERE profile_id = ? AND started_at >= ? AND started_at < ?
               ORDER BY started_at""",
            (profile_id, start_iso, end_iso)).fetchall()

    def session_counts(self, session_id):
        """Aggregate review counters for a session from the review log.

        Deriving these from the log (rather than mutating counters in
        memory) makes them automatically correct across undo and crashes.
        """
        rows = self.conn.execute(
            """SELECT card_id, rating, was_new, elapsed_ms FROM review_log
               WHERE session_id = ? AND undone = 0""",
            (session_id,)).fetchall()
        counts = {"cards_reviewed": len(rows), "new_cards": 0,
                  "review_cards": 0, "again_count": 0, "hard_count": 0,
                  "good_count": 0, "easy_count": 0, "elapsed_ms": 0,
                  "unique_cards": 0}
        rating_field = {1: "again_count", 2: "hard_count",
                        3: "good_count", 4: "easy_count"}
        new_ids, all_ids = set(), set()
        for row in rows:
            counts[rating_field[row["rating"]]] += 1
            counts["elapsed_ms"] += row["elapsed_ms"] or 0
            all_ids.add(row["card_id"])
            if row["was_new"]:
                new_ids.add(row["card_id"])
        counts["new_cards"] = len(new_ids)
        counts["review_cards"] = len(all_ids - new_ids)
        counts["unique_cards"] = len(all_ids)
        return counts

    def finish_session(self, session_id, ended_at):
        """Stamp the end time and persist final aggregates from the log."""
        counts = self.session_counts(session_id)
        self.conn.execute(
            """UPDATE sessions SET ended_at=?, cards_reviewed=?, new_cards=?,
                   review_cards=?, again_count=?, hard_count=?, good_count=?,
                   easy_count=?
               WHERE id=?""",
            (ended_at, counts["cards_reviewed"], counts["new_cards"],
             counts["review_cards"], counts["again_count"],
             counts["hard_count"], counts["good_count"],
             counts["easy_count"], session_id))
        self.conn.commit()

    def close_dangling_sessions(self, fallback_ended_at):
        """Finish sessions left open by a crash or power-off.

        Reviews already answered were committed one by one, so nothing is
        lost; the session just never got an end stamp.
        """
        open_ids = [row["id"] for row in self.conn.execute(
            "SELECT id FROM sessions WHERE ended_at IS NULL")]
        for session_id in open_ids:
            last = self.conn.execute(
                """SELECT MAX(reviewed_at) AS t FROM review_log
                   WHERE session_id = ?""", (session_id,)).fetchone()
            self.finish_session(session_id,
                                last["t"] or fallback_ended_at)
        return open_ids

    # -- child profiles ---------------------------------------------------------

    _PROFILE_LIST_FIELDS = ("active_categories", "active_decks")

    def create_profile(self, name, **fields):
        allowed = {"active_categories", "active_decks", "daily_new_cards",
                   "daily_review_cards", "daily_goal_cards",
                   "session_card_limit", "session_time_minutes",
                   "study_direction", "study_ahead_days",
                   "study_ahead_enabled"}
        unknown = set(fields) - allowed
        if unknown:
            raise ValueError(f"Unknown profile fields: {unknown}")
        self._serialize_profile_lists(fields)
        columns = ["name"] + list(fields)
        values = [name] + list(fields.values())
        cur = self.conn.execute(
            f"""INSERT INTO child_profiles ({', '.join(columns)})
                VALUES ({', '.join('?' * len(values))})""", values)
        self.conn.commit()
        return cur.lastrowid

    def update_profile(self, profile_id, **fields):
        allowed = {"name", "active_categories", "active_decks",
                   "daily_new_cards", "daily_review_cards",
                   "daily_goal_cards", "session_card_limit",
                   "session_time_minutes", "study_direction",
                   "study_ahead_days", "study_ahead_enabled"}
        unknown = set(fields) - allowed
        if unknown:
            raise ValueError(f"Unknown profile fields: {unknown}")
        self._serialize_profile_lists(fields)
        sets = ", ".join(f"{k}=?" for k in fields)
        self.conn.execute(
            f"UPDATE child_profiles SET {sets} WHERE id=?",
            list(fields.values()) + [profile_id])
        self.conn.commit()

    @classmethod
    def _serialize_profile_lists(cls, fields):
        """None means "everything active" and is stored as NULL; an
        explicit list (including []) is JSON-encoded in place."""
        for key in cls._PROFILE_LIST_FIELDS:
            if key in fields and fields[key] is not None:
                fields[key] = json.dumps(fields[key])

    def get_profile(self, profile_id):
        row = self.conn.execute(
            "SELECT * FROM child_profiles WHERE id=?",
            (profile_id,)).fetchone()
        return self._profile_dict(row) if row else None

    def list_profiles(self):
        return [self._profile_dict(row) for row in self.conn.execute(
            "SELECT * FROM child_profiles ORDER BY id")]

    def ensure_default_profile(self, name="Student"):
        """Return the first profile, creating one if none exist."""
        profiles = self.list_profiles()
        if profiles:
            return profiles[0]
        profile_id = self.create_profile(name)
        return self.get_profile(profile_id)

    @classmethod
    def _profile_dict(cls, row):
        profile = dict(row)
        for key in cls._PROFILE_LIST_FIELDS:
            raw = profile.get(key)
            profile[key] = json.loads(raw) if raw else None
        return profile

    # -- legacy prototype queries (main.py review / decks) ------------------------

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

    # -- admin (destructive) -----------------------------------------------------

    def deck_names_list(self):
        """Distinct deck names, regardless of due status."""
        return [row["deck"] for row in self.conn.execute(
            "SELECT DISTINCT deck FROM cards ORDER BY deck")]

    def count_cards_in_decks(self, deck_names=None):
        """Card count for the given decks, or every card if None."""
        if deck_names is None:
            return self.conn.execute(
                "SELECT COUNT(*) AS n FROM cards").fetchone()["n"]
        placeholders = ",".join("?" for _ in deck_names)
        return self.conn.execute(
            f"SELECT COUNT(*) AS n FROM cards WHERE deck IN ({placeholders})",
            deck_names).fetchone()["n"]

    def purge_decks(self, deck_names=None):
        """Permanently delete cards for the given decks (or all decks).

        Relies on ON DELETE CASCADE (foreign_keys=ON, set at connect
        time) to remove the matching review_state, review_log, and
        vocab_cards rows along with each card. Child profiles, app
        settings, and historical session rows are untouched. Returns
        the number of cards deleted. Callers are responsible for
        confirmation and backups (see main.py's admin command) —
        this method does not ask twice.
        """
        count = self.count_cards_in_decks(deck_names)
        if deck_names is None:
            self.conn.execute("DELETE FROM cards")
        else:
            placeholders = ",".join("?" for _ in deck_names)
            self.conn.execute(
                f"DELETE FROM cards WHERE deck IN ({placeholders})",
                deck_names)
        self.conn.commit()
        return count
