"""Storage support for the 'pattern' drill card type."""

import json
import os
import sqlite3

from cardbrick.scheduler import ReviewScheduler
from cardbrick.storage import SCHEMA_VERSION, Storage


def _production_row(storage, card_id=1):
    storage.upsert_card(card_id=card_id, note_id=card_id,
                        deck="Mexico City — Drills",
                        front="Ask whether you have to pay here.",
                        back="¿Tengo que pagar aquí?",
                        tags="topic:transactions",
                        card_type="pattern")
    storage.upsert_pattern_card(
        card_id=card_id, pattern_id="P012", kind="production",
        tier="core", gate="A1", priority_score=0.95,
        template="¿[S] tener que [Inf]?",
        prompt_en="Ask whether you have to pay here.",
        sibling_es="¿Tengo que salir temprano?",
        skeleton_es="¿ ___ que pagar aquí?",
        answer_es="¿Tengo que pagar aquí?",
        answer_audio="", sibling_audio="")


def test_pattern_production_card_round_trips(storage):
    _production_row(storage)
    storage.init_review_state(ReviewScheduler().initial_state(1))
    storage.commit()

    card = storage.get_card(1)
    assert card["card_type"] == "pattern"
    detail = storage.get_pattern_detail(1)
    assert detail["pattern_id"] == "P012"
    assert detail["kind"] == "production"
    assert detail["sibling_es"] == "¿Tengo que salir temprano?"
    assert detail["skeleton_es"] == "¿ ___ que pagar aquí?"
    assert detail["answer_es"] == "¿Tengo que pagar aquí?"
    assert detail["cue_en"] is None


def test_pattern_mcq_card_round_trips(storage):
    options = json.dumps([
        {"text": "A mi hija le gustan los tacos.", "correct": True},
        {"text": "A mi hija le gusta los tacos.", "correct": False},
        {"text": "Mi hija gusta los tacos.", "correct": False},
    ])
    storage.upsert_card(card_id=2, note_id=2, deck="Mexico City — Drills",
                        front="Say that your daughter likes tacos.",
                        back="A mi hija le gustan los tacos.",
                        tags="topic:smalltalk", card_type="pattern")
    storage.upsert_pattern_card(
        card_id=2, pattern_id="P034", kind="mcq",
        cue_en="Say that your daughter likes tacos. Which is correct?",
        options_json=options,
        constraint_note="gustar agrees with the thing liked.")
    storage.commit()

    detail = storage.get_pattern_detail(2)
    assert detail["kind"] == "mcq"
    assert json.loads(detail["options_json"])[0]["correct"] is True
    assert detail["prompt_en"] is None


def test_missing_pattern_detail_is_none(storage, service):
    from conftest import seed_card
    seed_card(storage, service, 1)
    assert storage.get_pattern_detail(1) is None


def test_reimporting_pattern_card_updates_content_not_review_state(storage,
                                                                   service):
    _production_row(storage)
    storage.init_review_state(service.scheduler.initial_state(1))
    storage.commit()
    service.answer_card(1, 3)
    state_before = dict(storage.get_review_state(1))

    # Re-import: content changes, review progress must not reset.
    storage.upsert_pattern_card(
        card_id=1, pattern_id="P012", kind="production",
        tier="core", gate="A1", priority_score=0.95,
        template="¿[S] tener que [Inf]?",
        prompt_en="Ask whether you have to pay here.",
        sibling_es="¿Tengo que volver mañana?",
        skeleton_es="¿ ___ que pagar aquí?",
        answer_es="¿Tengo que pagar aquí?")
    storage.init_review_state(service.scheduler.initial_state(1))
    storage.commit()

    assert (storage.get_pattern_detail(1)["sibling_es"]
            == "¿Tengo que volver mañana?")
    assert dict(storage.get_review_state(1)) == state_before


def test_pattern_kind_is_constrained(storage):
    _production_row(storage)
    try:
        storage.upsert_pattern_card(card_id=1, pattern_id="P012",
                                    kind="essay")
        raised = False
    except sqlite3.IntegrityError:
        raised = True
    assert raised


def test_purge_decks_cascades_pattern_cards(storage):
    _production_row(storage)
    storage.init_review_state(ReviewScheduler().initial_state(1))
    storage.commit()

    assert storage.purge_decks(["Mexico City — Drills"]) == 1
    assert storage.get_card(1) is None
    assert storage.get_pattern_detail(1) is None


def test_v8_database_migrates_to_v9_with_backup(tmp_path):
    """A schema-8 database (pre pattern_cards) gains the table on open,
    after a backup copy is taken."""
    db = str(tmp_path / "v8.db")
    storage = Storage(db)
    storage.close()

    conn = sqlite3.connect(db)
    conn.execute("DROP TABLE pattern_cards")
    conn.execute("UPDATE meta SET value = '8' WHERE key = 'schema_version'")
    conn.commit()
    conn.close()

    storage = Storage(db)
    try:
        assert storage.schema_version() == SCHEMA_VERSION
        tables = {row["name"] for row in storage.conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table'")}
        assert "pattern_cards" in tables
        assert os.path.exists(db + ".backup-v8")
    finally:
        storage.close()
