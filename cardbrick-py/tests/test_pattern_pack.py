"""JSON pattern-pack import for the 'pattern' drill card type."""

import json
import os

import pytest

from cardbrick.pattern_pack import PatternPackError, import_pattern_pack
from cardbrick.vocab_csv import _stable_id


def production_item(**overrides):
    item = {
        "pattern_id": "P012", "kind": "production", "variant": 1,
        "tier": "core", "gate": "A1", "priority_score": 0.95,
        "template": "¿[S] tener que [Inf]?",
        "tags": "topic:transactions",
        "prompt_en": "Ask whether you have to pay here.",
        "sibling_es": "¿Tengo que salir temprano?",
        "skeleton_es": "¿ ___ que pagar aquí?",
        "answer_es": "¿Tengo que pagar aquí?",
        "answer_audio": "", "sibling_audio": "",
    }
    item.update(overrides)
    return item


def mcq_item(**overrides):
    item = {
        "pattern_id": "P034", "kind": "mcq", "variant": 1,
        "tier": "core", "gate": "A1", "priority_score": 0.9,
        "template": "a [S] le gusta [N]",
        "tags": "topic:smalltalk",
        "cue_en": "Say that your daughter likes tacos. Which is correct?",
        "options": [
            {"text": "A mi hija le gustan los tacos.", "correct": True},
            {"text": "A mi hija le gusta los tacos.", "correct": False},
            {"text": "Mi hija gusta los tacos.", "correct": False},
        ],
        "constraint_note": "gustar agrees with the thing liked "
                           "(los tacos → gustan) and needs 'a … le'.",
    }
    item.update(overrides)
    return item


def write_pack(path, items, deck="Mexico City — Drills",
               fmt="cardbrick-pattern-pack"):
    pack = {"format": fmt, "version": 1, "deck": deck, "items": items}
    path.write_text(json.dumps(pack, ensure_ascii=False), encoding="utf-8")
    return str(path)


def test_production_item_imports_as_pattern_card(tmp_path, storage, service):
    pack = write_pack(tmp_path / "pack.json", [production_item()])
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.cards == 1

    card_id = _stable_id("P012:production:1")
    card = storage.get_card(card_id)
    assert card["card_type"] == "pattern"
    assert card["deck"] == "Mexico City — Drills"
    assert card["front"] == "Ask whether you have to pay here."
    assert card["back"] == "¿Tengo que pagar aquí?"
    assert card["audio_filename"] is None
    # tier/gate become tags automatically, alongside authored ones.
    assert set(card["tags"].split()) == {"topic:transactions",
                                         "tier:core", "gate:A1"}
    detail = storage.get_pattern_detail(card_id)
    assert detail["kind"] == "production"
    assert detail["sibling_es"] == "¿Tengo que salir temprano?"
    assert detail["skeleton_es"] == "¿ ___ que pagar aquí?"
    assert detail["answer_audio"] is None  # empty string stored as NULL
    assert storage.get_review_state(card_id) is not None


def test_mcq_item_imports_with_options_json(tmp_path, storage, service):
    pack = write_pack(tmp_path / "pack.json", [mcq_item()])
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.cards == 1

    card_id = _stable_id("P034:mcq:1")
    card = storage.get_card(card_id)
    # front/back mirror cue/correct answer so exports or fallbacks
    # degrade to a sensible basic card.
    assert card["back"] == "A mi hija le gustan los tacos."
    detail = storage.get_pattern_detail(card_id)
    assert detail["kind"] == "mcq"
    options = json.loads(detail["options_json"])
    assert len(options) == 3
    assert sum(1 for option in options if option["correct"]) == 1
    assert "gustan" in detail["constraint_note"]


def test_deck_name_override(tmp_path, storage, service):
    pack = write_pack(tmp_path / "pack.json", [production_item()])
    import_pattern_pack(pack, storage, service.scheduler,
                        deck_name="Drills (Papa)")
    card = storage.get_card(_stable_id("P012:production:1"))
    assert card["deck"] == "Drills (Papa)"


def test_reimport_updates_content_not_review_state(tmp_path, storage,
                                                   service):
    pack = write_pack(tmp_path / "pack.json", [production_item()])
    import_pattern_pack(pack, storage, service.scheduler)
    card_id = _stable_id("P012:production:1")
    service.answer_card(card_id, 3)
    state_before = dict(storage.get_review_state(card_id))

    pack = write_pack(tmp_path / "pack2.json", [production_item(
        sibling_es="¿Tengo que volver mañana?")])
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.cards == 1
    assert (storage.get_pattern_detail(card_id)["sibling_es"]
            == "¿Tengo que volver mañana?")
    assert dict(storage.get_review_state(card_id)) == state_before


def test_variants_are_distinct_cards(tmp_path, storage, service):
    pack = write_pack(tmp_path / "pack.json", [
        production_item(),
        production_item(variant=2,
                        prompt_en="Ask whether you have to leave now.",
                        sibling_es="¿Tengo que pagar aquí?",
                        skeleton_es="¿ ___ que salir ya?",
                        answer_es="¿Tengo que salir ya?"),
    ])
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.cards == 2
    assert (_stable_id("P012:production:1")
            != _stable_id("P012:production:2"))


@pytest.mark.parametrize("break_item, reason_part", [
    (lambda i: i.update(pattern_id=""), "pattern_id"),
    (lambda i: i.update(kind="essay"), "kind"),
    (lambda i: i.update(answer_es=""), "answer_es"),
    (lambda i: i.update(prompt_en=None), "prompt_en"),
])
def test_invalid_production_items_are_skipped(tmp_path, storage, service,
                                              break_item, reason_part):
    item = production_item()
    break_item(item)
    pack = write_pack(tmp_path / "pack.json", [item, mcq_item()])
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.cards == 1  # the valid mcq item still lands
    assert stats.skipped_cards == 1
    assert reason_part in stats.skipped[0][1]


@pytest.mark.parametrize("options", [
    # two options
    [{"text": "a", "correct": True}, {"text": "b", "correct": False}],
    # four options
    [{"text": "a", "correct": True}, {"text": "b", "correct": False},
     {"text": "c", "correct": False}, {"text": "d", "correct": False}],
    # zero correct
    [{"text": "a", "correct": False}, {"text": "b", "correct": False},
     {"text": "c", "correct": False}],
    # two correct
    [{"text": "a", "correct": True}, {"text": "b", "correct": True},
     {"text": "c", "correct": False}],
    # empty option text
    [{"text": "a", "correct": True}, {"text": "", "correct": False},
     {"text": "c", "correct": False}],
])
def test_invalid_mcq_options_are_skipped(tmp_path, storage, service,
                                         options):
    pack = write_pack(tmp_path / "pack.json", [mcq_item(options=options)])
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.cards == 0
    assert stats.skipped_cards == 1


def test_wrong_format_field_raises(tmp_path, storage, service):
    pack = write_pack(tmp_path / "pack.json", [production_item()],
                      fmt="some-other-format")
    with pytest.raises(PatternPackError):
        import_pattern_pack(pack, storage, service.scheduler)


def test_unreadable_json_raises(tmp_path, storage, service):
    path = tmp_path / "pack.json"
    path.write_text("{not json", encoding="utf-8")
    with pytest.raises(PatternPackError):
        import_pattern_pack(str(path), storage, service.scheduler)


def test_shipped_sample_pack_imports_cleanly(storage, service):
    pack = os.path.join(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))), "assets", "patterns",
        "sample_pack.json")
    stats = import_pattern_pack(pack, storage, service.scheduler)
    assert stats.skipped_cards == 0
    assert stats.cards == 18
    assert stats.decks == {"Mexico City — Drills"}
