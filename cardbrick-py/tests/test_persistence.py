"""Durability: restarts, crash recovery, log consistency, migration."""

import json
import sqlite3

from conftest import seed_card
from cardbrick.scheduler import ReviewScheduler, iso
from cardbrick.service import ReviewService
from cardbrick.session import StudySession
from cardbrick.storage import Storage


def test_restart_preserves_review_state(tmp_path, clock):
    db = str(tmp_path / "app.db")
    storage = Storage(db)
    service = ReviewService(storage, ReviewScheduler(), now_fn=clock.now)
    seed_card(storage, service, 1)
    service.answer_card(1, 3)
    state = dict(storage.get_review_state(1))
    storage.close()

    reopened = Storage(db)
    assert dict(reopened.get_review_state(1)) == state
    reopened.close()


def test_review_log_is_consistent_with_state(storage, service, clock):
    seed_card(storage, service, 1)
    session_id = storage.create_session(None, iso(clock.now()), None)
    service.answer_card(1, 3, elapsed_ms=800, session_id=session_id)

    entry = storage.conn.execute("SELECT * FROM review_log").fetchone()
    logged_new = json.loads(entry["new_state_json"])
    current = dict(storage.get_review_state(1))
    # The logged new state matches what is stored (bar the audit stamp).
    for key in ("due", "reps", "lapses", "state", "fsrs_json"):
        assert logged_new[key] == current[key]
    prev = json.loads(entry["previous_state_json"])
    assert prev["reps"] == 0


def test_interrupted_session_recovers_on_next_start(tmp_path, clock):
    db = str(tmp_path / "app.db")
    storage = Storage(db)
    service = ReviewService(storage, ReviewScheduler(), now_fn=clock.now)
    profile = storage.ensure_default_profile()
    for i in range(1, 4):
        seed_card(storage, service, i)
    session = StudySession(storage, service, profile)
    session.answer(4)  # Easy: graduates, no requeue
    session_id = session.session_id
    storage.close()  # simulated crash: no finish(), no ended_at

    storage = Storage(db)
    service = ReviewService(storage, ReviewScheduler(), now_fn=clock.now)
    closed = storage.close_dangling_sessions(iso(clock.now()))
    assert closed == [session_id]
    row = storage.session_row(session_id)
    assert row["ended_at"] is not None
    assert row["cards_reviewed"] == 1  # completed review survived

    # Daily accounting still reflects the interrupted work.
    queue = service.get_due_cards(limits={"daily_new_cards": 3})
    assert len(queue) == 2  # 1 of 3 new already used today
    storage.close()


def test_answer_is_durable_without_explicit_commit(tmp_path, clock):
    """Auto-save after every answer: a second connection sees it."""
    db = str(tmp_path / "app.db")
    storage = Storage(db)
    service = ReviewService(storage, ReviewScheduler(), now_fn=clock.now)
    seed_card(storage, service, 1)
    service.answer_card(1, 3)

    other = sqlite3.connect(db)
    reps = other.execute(
        "SELECT reps FROM review_state WHERE card_id=1").fetchone()[0]
    n_log = other.execute("SELECT COUNT(*) FROM review_log").fetchone()[0]
    other.close()
    storage.close()
    assert reps == 1
    assert n_log == 1


def test_prototype_database_migrates_in_place(tmp_path):
    """A DB created by the original prototype schema opens cleanly and
    keeps its cards and progress."""
    db = str(tmp_path / "old.db")
    conn = sqlite3.connect(db)
    conn.executescript("""
        CREATE TABLE cards (
            id INTEGER PRIMARY KEY, note_id INTEGER NOT NULL,
            deck TEXT NOT NULL, front TEXT NOT NULL, back TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '', audio_filename TEXT,
            audio_side TEXT
        );
        CREATE TABLE review_state (
            card_id INTEGER PRIMARY KEY, due TEXT NOT NULL,
            stability REAL, difficulty REAL,
            elapsed_days INTEGER NOT NULL DEFAULT 0,
            scheduled_days INTEGER NOT NULL DEFAULT 0,
            reps INTEGER NOT NULL DEFAULT 0,
            lapses INTEGER NOT NULL DEFAULT 0,
            state INTEGER NOT NULL, fsrs_json TEXT NOT NULL
        );
    """)
    conn.execute("INSERT INTO cards VALUES (1, 1, 'Spanish', 'hola', "
                 "'hello', 'greetings', NULL, NULL)")
    conn.execute("INSERT INTO review_state VALUES (1, "
                 "'2026-01-01T00:00:00+00:00', 2.5, 5.0, 0, 0, 7, 1, 2, "
                 "'{}')")
    conn.commit()
    conn.close()

    storage = Storage(db)
    card = storage.get_card(1)
    assert card["front"] == "hola"
    assert card["suspended"] == 0       # migrated column, default applied
    assert card["reps"] == 7            # progress preserved
    # New tables exist and work.
    storage.ensure_default_profile()
    assert storage.list_profiles()
    storage.close()
