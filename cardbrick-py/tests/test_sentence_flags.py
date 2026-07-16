"""Learner sentence flags: storage, resolution, and the offline pipeline glue.

Covers TODO item #3 — flagging wrong/inappropriate example sentences and the
export -> pipeline -> apply loop that corrects them without disturbing FSRS
progress.
"""

import json
import os
import sys

import pytest

from conftest import seed_card

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))), "scripts"))

from cardbrick.storage import FLAG_REASONS  # noqa: E402


def _seed_vocab(storage, service, card_id=1, definitions="cat",
                example_es="Es un gato.", example_en="It is a cat."):
    """A vocab card with review state, so progress-preservation is testable."""
    seed_card(storage, service, card_id, reps=1)
    storage.upsert_vocab_card(
        card_id, "gato", None, None, definitions, None,
        example_es, None, example_en, None, None)
    storage.commit()
    return card_id


def test_add_and_list_open_flag(storage, service):
    _seed_vocab(storage, service, 1)
    flag_id = storage.add_sentence_flag(
        1, "meaning_not_listed", snapshot=json.dumps({"example_es": "Es un gato."}),
        now_iso="t1")

    rows = storage.open_flags()
    assert len(rows) == 1
    row = rows[0]
    assert row["flag_id"] == flag_id
    assert row["reason"] == "meaning_not_listed"
    # The join carries the card context the pipeline needs.
    assert row["word"] == "gato"
    assert row["definitions"] == "cat"
    assert row["example_en"] == "It is a cat."


def test_unknown_reason_rejected(storage, service):
    _seed_vocab(storage, service, 1)
    with pytest.raises(ValueError):
        storage.add_sentence_flag(1, "not_a_reason")


@pytest.mark.parametrize("reason", FLAG_REASONS)
def test_every_reason_accepted(storage, service, reason):
    _seed_vocab(storage, service, 1)
    storage.add_sentence_flag(1, reason, now_iso="t1")
    assert storage.open_flags()[0]["reason"] == reason


def test_resolve_and_dismiss_close_the_flag(storage, service):
    _seed_vocab(storage, service, 1)
    f1 = storage.add_sentence_flag(1, "wrong_translation", now_iso="t1")
    f2 = storage.add_sentence_flag(1, "inappropriate", now_iso="t2")
    assert len(storage.open_flags()) == 2

    storage.resolve_flag(f1, "card_updated", now_iso="t3")
    storage.dismiss_flag(f2, now_iso="t4")
    assert storage.open_flags() == []

    rows = {r["id"]: r for r in storage.flags_for_card(1)}
    assert rows[f1]["status"] == "resolved"
    assert rows[f1]["resolution"] == "card_updated"
    assert rows[f2]["status"] == "dismissed"
    assert rows[f2]["resolution"] == "no_change"


def test_update_vocab_fields_preserves_review_progress(storage, service):
    """Applying a correction rewrites content but never touches review_state
    — the same guarantee re-import gives."""
    _seed_vocab(storage, service, 1, definitions="cat")
    before = dict(storage.get_review_state(1))

    changed = storage.update_vocab_fields(
        1, {"definitions": "cat; (informal) a relaxed person",
            "example_es": "Ese gato es muy tranquilo."})
    assert changed == 1

    detail = storage.get_vocab_detail(1)
    assert detail["definitions"] == "cat; (informal) a relaxed person"
    assert detail["example_es"] == "Ese gato es muy tranquilo."
    assert dict(storage.get_review_state(1)) == before


def test_update_vocab_fields_ignores_unknown_columns(storage, service):
    _seed_vocab(storage, service, 1)
    # card_id is the key and must never be writable; bogus columns are dropped.
    assert storage.update_vocab_fields(1, {"card_id": 999, "bogus": "x"}) == 0
    assert storage.get_vocab_detail(1)["card_id"] == 1


# -- offline pipeline (scripts/flag_pipeline.py) ------------------------------

def test_pipeline_verdict_mapping():
    from flag_pipeline import verdict_to_resolution

    flag = {"flag_id": 5, "card_id": 42, "reason": "meaning_not_listed"}
    entry = verdict_to_resolution(flag, {
        "resolution": "card_updated",
        "vocab": {"definitions": "cat; slang for a cool person",
                  "example_es": ""},   # empty edits are dropped
        "explanation": "Added the slang sense.",
    })
    assert entry["flag_id"] == 5 and entry["card_id"] == 42
    assert entry["resolution"] == "card_updated"
    assert entry["vocab"] == {"definitions": "cat; slang for a cool person"}
    assert entry["unsuspend"] is True

    # An unknown resolution degrades safely and does not unsuspend.
    safe = verdict_to_resolution(flag, {"resolution": "hallucinated"})
    assert safe["resolution"] == "no_change"
    assert "unsuspend" not in safe


def test_pipeline_dry_backend_is_noop():
    from flag_pipeline import process

    flags = [{"flag_id": 1, "card_id": 1, "reason": "other"}]
    out = process(flags, backend="dry", model="x", base_url="http://unused")
    assert out == [{"flag_id": 1, "card_id": 1, "resolution": "no_change"}]
