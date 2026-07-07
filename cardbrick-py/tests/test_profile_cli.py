"""CLI wiring for `python main.py profile --decks / --categories`."""

import os
from datetime import datetime, timezone

import main as cardbrick_main
from cardbrick.scheduler import ReviewScheduler
from cardbrick.storage import Storage


def _seed_card(db_path, card_id, deck="Spanish"):
    storage = Storage(db_path)
    scheduler = ReviewScheduler()
    now = datetime.now(timezone.utc)
    storage.upsert_card(card_id=card_id, note_id=card_id, deck=deck,
                       front=f"front{card_id}", back=f"back{card_id}",
                       tags="", now_iso=now.isoformat())
    storage.init_review_state(scheduler.initial_state(card_id, now=now))
    storage.commit()
    storage.close()


def test_profile_decks_flag_sets_active_decks(tmp_path, capsys):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1, deck="Spanish")
    _seed_card(db_path, 2, deck="French")

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "profile",
                             "--decks", "Spanish"])

    assert rc == 0
    storage = Storage(db_path)
    profile = storage.list_profiles()[0]
    assert profile["active_decks"] == ["Spanish"]
    storage.close()
    assert "Available decks: French, Spanish" in capsys.readouterr().out


def test_profile_decks_all_clears_the_filter(tmp_path):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1, deck="Spanish")
    cardbrick_main.main(["--data-dir", str(tmp_path), "profile",
                        "--decks", "Spanish"])

    cardbrick_main.main(["--data-dir", str(tmp_path), "profile",
                        "--decks", "all"])

    storage = Storage(db_path)
    assert storage.list_profiles()[0]["active_decks"] is None
    storage.close()


def test_profile_categories_flag_still_works(tmp_path):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    storage = Storage(db_path)
    scheduler = ReviewScheduler()
    now = datetime.now(timezone.utc)
    storage.upsert_card(card_id=1, note_id=1, deck="Spanish", front="hola",
                       back="hello", tags="greetings", now_iso=now.isoformat())
    storage.init_review_state(scheduler.initial_state(1, now=now))
    storage.commit()
    storage.close()

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "profile",
                             "--categories", "greetings"])

    assert rc == 0
    storage = Storage(db_path)
    assert storage.list_profiles()[0]["active_categories"] == ["greetings"]
    storage.close()


def test_profile_command_with_no_decks_imported_yet(tmp_path, capsys):
    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "profile",
                             "--name", "Maya"])
    assert rc == 0
    assert "Available decks" not in capsys.readouterr().out
