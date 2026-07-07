"""CSV import for the four-phase vocab card, as an alternative to .apkg."""

import csv

from cardbrick.vocab_csv import _stable_id, import_vocab_csv

HEADER = ["Word", "Word Audio", "Gendered Forms", "Definitions", "Image",
          "Example ES", "Example Audio", "Example EN", "Report Link",
          "Word JP", "Example JP", "Tags"]


def write_csv(path, rows):
    with open(path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(HEADER)
        writer.writerows(rows)


def gato_row(**overrides):
    row = {
        "Word": "gato", "Word Audio": "gato.mp3",
        "Gendered Forms": "la gata", "Definitions": "Animal felino.",
        "Image": "gato.jpg", "Example ES": "El gato duerme.",
        "Example Audio": "ex1.mp3", "Example EN": "The cat sleeps.",
        "Report Link": "", "Word JP": "猫", "Example JP": "猫は眠る。",
        "Tags": "animals",
    }
    row.update(overrides)
    return [row[col] for col in HEADER]


def test_csv_row_imports_as_vocab_card(tmp_path, storage, service):
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row()])
    stats = import_vocab_csv(str(csv_path), storage, service.scheduler,
                             str(tmp_path / "media"))
    assert stats.cards == 1
    card_id = _stable_id("gato")
    card = storage.get_card(card_id)
    assert card["card_type"] == "vocab"
    assert card["front"] == "gato"
    assert card["tags"] == "animals"
    detail = storage.get_vocab_detail(card_id)
    assert detail["gendered_forms"] == "la gata"
    assert detail["example_en"] == "The cat sleeps."
    assert detail["word_jp"] == "猫"


def test_missing_word_is_skipped(tmp_path, storage, service):
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row(Word="")])
    stats = import_vocab_csv(str(csv_path), storage, service.scheduler,
                             str(tmp_path / "media"))
    assert stats.cards == 0
    assert stats.skipped_cards == 1


def test_header_is_case_and_order_insensitive(tmp_path, storage, service):
    csv_path = tmp_path / "deck.csv"
    shuffled = list(reversed(HEADER))  # different order
    with open(csv_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=[h.upper() for h in shuffled])
        writer.writeheader()
        writer.writerow({h.upper(): v for h, v in
                         zip(shuffled, reversed(gato_row()))})
    stats = import_vocab_csv(str(csv_path), storage, service.scheduler,
                             str(tmp_path / "media"))
    assert stats.cards == 1


def test_reimport_same_word_updates_not_duplicates(tmp_path, storage,
                                                    service):
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row()])
    import_vocab_csv(str(csv_path), storage, service.scheduler,
                     str(tmp_path / "media"))
    card_id = _stable_id("gato")
    service.answer_card(card_id, 3)
    state_before = dict(storage.get_review_state(card_id))

    write_csv(csv_path, [gato_row(Definitions="Corrected definition.")])
    stats = import_vocab_csv(str(csv_path), storage, service.scheduler,
                             str(tmp_path / "media"))
    assert stats.cards == 1
    total_cards = storage.conn.execute(
        "SELECT COUNT(*) AS n FROM cards").fetchone()["n"]
    assert total_cards == 1  # not duplicated
    assert storage.get_vocab_detail(card_id)["definitions"] == \
        "Corrected definition."
    assert dict(storage.get_review_state(card_id)) == state_before


def test_audio_and_image_cells_accept_bare_filenames(tmp_path, storage,
                                                      service):
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row(Word="perro", **{
        "Word Audio": "perro.mp3", "Image": "perro.png"})])
    import_vocab_csv(str(csv_path), storage, service.scheduler,
                     str(tmp_path / "media"))
    detail = storage.get_vocab_detail(_stable_id("perro"))
    assert detail["image_filename"] == "perro.png"
    card = storage.get_card(_stable_id("perro"))
    assert card["audio_filename"] == "perro.mp3"


def test_audio_and_image_cells_accept_anki_style_tags(tmp_path, storage,
                                                       service):
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row(Word="pajaro", **{
        "Word Audio": "[sound:pajaro.mp3]",
        "Image": '<img src="pajaro.jpg">'})])
    import_vocab_csv(str(csv_path), storage, service.scheduler,
                     str(tmp_path / "media"))
    detail = storage.get_vocab_detail(_stable_id("pajaro"))
    assert detail["image_filename"] == "pajaro.jpg"
    card = storage.get_card(_stable_id("pajaro"))
    assert card["audio_filename"] == "pajaro.mp3"


def test_media_copied_from_source_dir(tmp_path, storage, service):
    source = tmp_path / "source_media"
    source.mkdir()
    (source / "gato.mp3").write_bytes(b"word audio bytes")
    (source / "ex1.mp3").write_bytes(b"example audio bytes")
    (source / "gato.jpg").write_bytes(b"image bytes")

    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row()])
    media_dir = tmp_path / "media"
    stats = import_vocab_csv(str(csv_path), storage, service.scheduler,
                             str(media_dir), source_media_dir=str(source))
    assert stats.media_files == 3
    assert (media_dir / "gato.mp3").read_bytes() == b"word audio bytes"
    assert (media_dir / "gato.jpg").read_bytes() == b"image bytes"


def test_missing_source_media_logs_and_continues(tmp_path, storage,
                                                  service, caplog):
    source = tmp_path / "source_media"
    source.mkdir()  # empty: referenced files are absent
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row()])
    stats = import_vocab_csv(str(csv_path), storage, service.scheduler,
                             str(tmp_path / "media"),
                             source_media_dir=str(source))
    assert stats.cards == 1  # import still succeeds
    assert stats.media_files == 0


def test_deck_name_is_configurable(tmp_path, storage, service):
    csv_path = tmp_path / "deck.csv"
    write_csv(csv_path, [gato_row()])
    import_vocab_csv(str(csv_path), storage, service.scheduler,
                     str(tmp_path / "media"), deck_name="Mi Vocabulario")
    card = storage.get_card(_stable_id("gato"))
    assert card["deck"] == "Mi Vocabulario"


def test_stable_id_is_deterministic_and_case_insensitive():
    assert _stable_id("gato") == _stable_id("gato")
    assert _stable_id("gato") == _stable_id("  Gato ")
    assert _stable_id("gato") != _stable_id("perro")
