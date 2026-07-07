"""`admin reset`: the clean-slate command behind `python main.py admin
reset` — wipes decks, progress, profiles, and media, keeps device
calibration."""

import json
import os
from datetime import datetime, timezone

import main as cardbrick_main
from cardbrick.scheduler import ReviewScheduler
from cardbrick.storage import Storage


def _seed(data_dir):
    db_path = os.path.join(data_dir, "cardbrick.db")
    storage = Storage(db_path)
    scheduler = ReviewScheduler()
    now = datetime.now(timezone.utc)
    storage.upsert_card(1, 1, "Spanish", "hola", "hello", "",
                        now_iso=now.isoformat())
    storage.init_review_state(scheduler.initial_state(1, now=now))
    profile = storage.ensure_default_profile("Maya")
    storage.update_profile(profile["id"], daily_goal_cards=99)
    storage.commit()
    storage.close()

    media_dir = os.path.join(data_dir, "media")
    os.makedirs(media_dir, exist_ok=True)
    with open(os.path.join(media_dir, "hola.mp3"), "wb") as f:
        f.write(b"fake audio")
    for name in ("settings.json", "input_mapping.json"):
        with open(os.path.join(data_dir, name), "w",
                  encoding="utf-8") as f:
            json.dump({"kept": True}, f)
    return db_path, media_dir


def test_reset_wipes_context_but_keeps_calibration(tmp_path):
    data_dir = str(tmp_path)
    db_path, media_dir = _seed(data_dir)

    rc = cardbrick_main.main(["--data-dir", data_dir, "admin", "reset",
                              "--yes"])

    assert rc == 0
    assert not os.path.exists(db_path)
    assert os.listdir(media_dir) == []
    # Calibration survives; a timestamped backup of the database exists.
    assert os.path.exists(os.path.join(data_dir, "settings.json"))
    assert os.path.exists(os.path.join(data_dir, "input_mapping.json"))
    backups = [n for n in os.listdir(data_dir)
               if n.startswith("cardbrick.db.backup-reset-")]
    assert len(backups) == 1

    # The next open really is a clean slate: empty, fresh default profile.
    storage = Storage(db_path)
    try:
        assert storage.deck_names_list() == []
        profile = storage.ensure_default_profile("Student")
        assert profile["name"] == "Student"
        assert profile["daily_goal_cards"] == 150  # not Maya's 99
    finally:
        storage.close()


def test_reset_without_confirmation_deletes_nothing(tmp_path, monkeypatch):
    data_dir = str(tmp_path)
    db_path, media_dir = _seed(data_dir)
    monkeypatch.setattr("builtins.input", lambda *args: "no")

    rc = cardbrick_main.main(["--data-dir", data_dir, "admin", "reset"])

    assert rc == 1
    assert os.path.exists(db_path)
    assert os.listdir(media_dir) == ["hola.mp3"]
