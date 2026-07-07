"""CLI wiring for `python main.py admin purge-decks`."""

import glob
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


def test_purge_aborts_without_confirmation(tmp_path, monkeypatch, capsys):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1)
    monkeypatch.setattr("builtins.input", lambda _prompt: "no")

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks"])

    assert rc == 1
    assert "Aborted" in capsys.readouterr().out
    storage = Storage(db_path)
    assert storage.count_cards_in_decks(None) == 1  # untouched
    storage.close()


def test_purge_with_yes_flag_skips_prompt(tmp_path, monkeypatch):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1)

    def fail_if_called(_prompt):
        raise AssertionError("should not prompt when --yes is given")
    monkeypatch.setattr("builtins.input", fail_if_called)

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks", "--yes"])

    assert rc == 0
    storage = Storage(db_path)
    assert storage.count_cards_in_decks(None) == 0
    storage.close()


def test_purge_confirmed_interactively(tmp_path, monkeypatch):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1)
    monkeypatch.setattr("builtins.input", lambda _prompt: "yes")

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks"])

    assert rc == 0
    storage = Storage(db_path)
    assert storage.count_cards_in_decks(None) == 0
    storage.close()


def test_purge_specific_deck_leaves_others(tmp_path):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1, deck="Spanish")
    _seed_card(db_path, 2, deck="French")

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks", "--deck", "Spanish", "--yes"])

    assert rc == 0
    storage = Storage(db_path)
    assert storage.deck_names_list() == ["French"]
    storage.close()


def test_purge_unknown_deck_errors_without_deleting(tmp_path, capsys):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1)

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks", "--deck", "Nonexistent",
                             "--yes"])

    assert rc == 1
    assert "Unknown deck" in capsys.readouterr().err
    storage = Storage(db_path)
    assert storage.count_cards_in_decks(None) == 1
    storage.close()


def test_purge_creates_backup_before_deleting(tmp_path):
    db_path = os.path.join(str(tmp_path), "cardbrick.db")
    _seed_card(db_path, 1)

    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks", "--yes"])

    assert rc == 0
    backups = glob.glob(db_path + ".backup-purge-*")
    assert len(backups) == 1
    restored = Storage(backups[0])
    assert restored.count_cards_in_decks(None) == 1  # pre-purge snapshot
    restored.close()


def test_purge_with_no_decks_is_a_noop(tmp_path):
    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin",
                             "purge-decks", "--yes"])
    assert rc == 0  # "No decks to purge." — not an error


def test_admin_without_subcommand_prints_usage(tmp_path, capsys):
    rc = cardbrick_main.main(["--data-dir", str(tmp_path), "admin"])
    assert rc == 1
    assert "purge-decks" in capsys.readouterr().err
