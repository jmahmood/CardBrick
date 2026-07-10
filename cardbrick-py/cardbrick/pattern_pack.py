"""JSON importer for sentence-pattern drill packs.

A pattern pack is a single JSON file compiled off-device (see
docs/PATTERN_DRILLING_PLAN.md) holding drill items for the 'pattern'
card type:

    {
      "format": "cardbrick-pattern-pack",
      "version": 1,
      "deck": "Mexico City — Drills",
      "items": [ ... ]
    }

Every item carries ``pattern_id`` (e.g. "P012"), ``kind``
("production" or "mcq"), an optional ``variant`` (default 1), and
optional ``tier``/``gate``/``priority_score``/``template``/``tags``
metadata. Production items add ``prompt_en``, ``sibling_es``,
``skeleton_es``, ``answer_es`` (and audio filenames, unused for now);
mcq items add ``cue_en``, exactly three ``options`` with exactly one
``"correct": true``, and a ``constraint_note`` shown after a wrong
answer.

Card identity: a stable hash of ``pattern_id:kind:variant`` — the
same scheme as the vocab CSV importer's Word hash. Re-importing a
rebuilt pack updates card content without resetting review progress;
**renaming a pattern_id (or changing kind/variant) creates a new
card** rather than updating the old one.

``tier:<tier>`` and ``gate:<gate>`` are auto-appended to each item's
tags so parent mode's category filters can gate drills without new UI.
"""

import json
import logging

from .importer import ImportStats
from .scheduler import iso, now_utc
from .vocab_csv import _stable_id

log = logging.getLogger(__name__)

PACK_FORMAT = "cardbrick-pattern-pack"
KINDS = ("production", "mcq")
MCQ_OPTION_COUNT = 3


class PatternPackError(Exception):
    """The file is not a usable pattern pack."""


def _item_tags(item):
    """The item's tags plus auto tier:/gate: tags (space-separated)."""
    tags = (item.get("tags") or "").split()
    for key in ("tier", "gate"):
        value = (item.get(key) or "").strip()
        auto = f"{key}:{value}"
        if value and auto not in tags:
            tags.append(auto)
    return " ".join(tags)


def _validate(item):
    """Reason this item must be skipped, or None if it is importable."""
    if not (item.get("pattern_id") or "").strip():
        return "missing pattern_id"
    kind = item.get("kind")
    if kind not in KINDS:
        return f"unknown kind {kind!r}"
    if kind == "production":
        for field in ("prompt_en", "sibling_es", "skeleton_es", "answer_es"):
            if not (item.get(field) or "").strip():
                return f"missing {field}"
        return None
    if not (item.get("cue_en") or "").strip():
        return "missing cue_en"
    if not (item.get("constraint_note") or "").strip():
        return "missing constraint_note"
    options = item.get("options")
    if not isinstance(options, list) or len(options) != MCQ_OPTION_COUNT:
        return f"needs exactly {MCQ_OPTION_COUNT} options"
    if any(not (option.get("text") or "").strip() for option in options):
        return "option with empty text"
    correct = sum(1 for option in options if option.get("correct"))
    if correct != 1:
        return f"needs exactly 1 correct option, found {correct}"
    return None


def import_pattern_pack(json_path, storage, scheduler, deck_name=None):
    """Import drill cards from a pattern pack. Returns an ImportStats."""
    try:
        with open(json_path, encoding="utf-8") as f:
            pack = json.load(f)
    except (OSError, json.JSONDecodeError) as exc:
        raise PatternPackError(f"cannot read pattern pack: {exc}") from exc
    if not isinstance(pack, dict) or pack.get("format") != PACK_FORMAT:
        raise PatternPackError(
            f'not a pattern pack (expected "format": "{PACK_FORMAT}")')

    deck = deck_name or pack.get("deck") or "Pattern Drills"
    stats = ImportStats()
    for i, item in enumerate(pack.get("items", [])):
        reason = _validate(item)
        pattern_id = (item.get("pattern_id") or "").strip()
        if reason is not None:
            stats.skip(pattern_id or f"item {i + 1}", reason)
            continue

        kind = item["kind"]
        variant = int(item.get("variant") or 1)
        card_id = _stable_id(f"{pattern_id}:{kind}:{variant}")
        tags = _item_tags(item)
        if kind == "production":
            front = item["prompt_en"].strip()
            back = item["answer_es"].strip()
        else:
            front = item["cue_en"].strip()
            back = next(option["text"].strip()
                        for option in item["options"]
                        if option.get("correct"))

        storage.upsert_card(
            card_id=card_id, note_id=card_id, deck=deck, front=front,
            back=back, tags=tags, audio_filename=None, audio_side=None,
            now_iso=iso(now_utc()), card_type="pattern")
        storage.upsert_pattern_card(
            card_id=card_id, pattern_id=pattern_id, kind=kind,
            tier=(item.get("tier") or "").strip() or None,
            gate=(item.get("gate") or "").strip() or None,
            priority_score=item.get("priority_score"),
            template=(item.get("template") or "").strip() or None,
            prompt_en=(item.get("prompt_en") or "").strip() or None,
            sibling_es=(item.get("sibling_es") or "").strip() or None,
            skeleton_es=(item.get("skeleton_es") or "").strip() or None,
            answer_es=(item.get("answer_es") or "").strip() or None,
            answer_audio=(item.get("answer_audio") or "").strip() or None,
            sibling_audio=(item.get("sibling_audio") or "").strip() or None,
            cue_en=(item.get("cue_en") or "").strip() or None,
            options_json=(json.dumps(item["options"], ensure_ascii=False)
                          if kind == "mcq" else None),
            constraint_note=(item.get("constraint_note") or "").strip()
            or None)
        storage.init_review_state(scheduler.initial_state(card_id))

        stats.cards += 1
        stats.notes += 1
        stats.decks.add(deck)
        stats.tags.update(tags.split())

    storage.commit()
    return stats
