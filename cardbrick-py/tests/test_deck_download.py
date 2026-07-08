"""Tests for deck_download.py against a real local HTTP server."""

import http.server
import json
import os
import threading
import zipfile

import pytest

from cardbrick.deck_download import (DownloadError, download_deck,
                                     fetch_deck_list, format_size,
                                     safe_filename)


@pytest.fixture
def server(tmp_path):
    """Serve tmp_path/www on an ephemeral localhost port."""
    www = tmp_path / "www"
    www.mkdir()

    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(www), **kwargs)

        def log_message(self, *args):
            pass

    class QuietServer(http.server.ThreadingHTTPServer):
        def handle_error(self, request, client_address):
            pass  # client aborts (size-cap test) are expected

    httpd = QuietServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{httpd.server_address[1]}"
    yield www, base
    httpd.shutdown()
    thread.join()


def make_apkg(path, payload=b"x" * 1000):
    with zipfile.ZipFile(path, "w") as zf:
        zf.writestr("collection.anki21", payload)
    return path


# -- fetch_deck_list -------------------------------------------------------------


def test_direct_apkg_url_is_a_single_entry_list():
    decks = fetch_deck_list("http://example.com/decks/JLPT%20N5.apkg")
    assert decks == [{"name": "JLPT N5.apkg",
                      "url": "http://example.com/decks/JLPT%20N5.apkg",
                      "size": None}]


def test_manifest_with_decks_object_and_relative_urls(server):
    www, base = server
    (www / "decks.json").write_text(json.dumps({"decks": [
        {"name": "Spanish N5", "file": "n5.apkg", "size": 1234},
        {"name": "Spanish N4", "url": f"{base}/other/n4.apkg"},
    ]}))
    decks = fetch_deck_list(f"{base}/decks.json")
    assert decks == [
        {"name": "Spanish N5", "url": f"{base}/n5.apkg", "size": 1234},
        {"name": "Spanish N4", "url": f"{base}/other/n4.apkg",
         "size": None},
    ]


def test_manifest_bare_list_and_missing_names(server):
    www, base = server
    (www / "decks.json").write_text(json.dumps([
        {"url": "vocab.apkg", "size": -5},
    ]))
    decks = fetch_deck_list(f"{base}/decks.json")
    assert decks == [{"name": "vocab.apkg", "url": f"{base}/vocab.apkg",
                      "size": None}]


def test_manifest_skips_unusable_entries(server):
    www, base = server
    (www / "decks.json").write_text(json.dumps({"decks": [
        "not-a-dict",
        {"name": "no url at all"},
        {"name": "evil", "url": "file:///etc/passwd"},
        {"name": "ok", "url": "good.apkg"},
    ]}))
    decks = fetch_deck_list(f"{base}/decks.json")
    assert [d["name"] for d in decks] == ["ok"]


def test_manifest_with_no_usable_decks_is_an_error(server):
    www, base = server
    (www / "decks.json").write_text("[]")
    with pytest.raises(DownloadError, match="no.*usable decks"):
        fetch_deck_list(f"{base}/decks.json")


def test_non_json_manifest_is_a_friendly_error(server):
    www, base = server
    (www / "decks.json").write_text("<html>login page</html>")
    with pytest.raises(DownloadError, match="not a deck list"):
        fetch_deck_list(f"{base}/decks.json")


def test_missing_manifest_reports_http_status(server):
    _www, base = server
    with pytest.raises(DownloadError, match="404"):
        fetch_deck_list(f"{base}/nope.json")


def test_unreachable_server_is_a_download_error():
    # RFC 5737 TEST-NET address; nothing listens there.
    with pytest.raises(DownloadError):
        fetch_deck_list("http://192.0.2.1:9/decks.json", timeout=0.2)


def test_empty_and_non_http_urls_are_rejected():
    with pytest.raises(DownloadError):
        fetch_deck_list("")
    with pytest.raises(DownloadError, match="http"):
        fetch_deck_list("ftp://example.com/decks.json")
    with pytest.raises(DownloadError, match="http"):
        download_deck("file:///etc/passwd", "/tmp/never-used")


# -- download_deck ---------------------------------------------------------------


def test_download_success(server, tmp_path):
    www, base = server
    make_apkg(www / "n5.apkg")
    dest = tmp_path / "import"

    seen = []
    path = download_deck(f"{base}/n5.apkg", str(dest),
                         progress=lambda got, total: seen.append((got,
                                                                  total)))
    assert os.path.basename(path) == "n5.apkg"
    assert zipfile.is_zipfile(path)
    assert seen and seen[-1][0] == os.path.getsize(path)
    assert seen[-1][1] == os.path.getsize(path)  # Content-Length passed
    assert os.listdir(dest) == ["n5.apkg"]  # no .part left behind


def test_download_uses_entry_name_for_the_filename(server, tmp_path):
    www, base = server
    make_apkg(www / "x.apkg")
    path = download_deck(f"{base}/x.apkg", str(tmp_path),
                         name="JLPT N5 Vocab")
    assert os.path.basename(path) == "JLPT N5 Vocab.apkg"


def test_download_rejects_non_zip(server, tmp_path):
    www, base = server
    (www / "fake.apkg").write_text("<html>captive portal</html>")
    dest = tmp_path / "import"
    with pytest.raises(DownloadError, match="not a zip"):
        download_deck(f"{base}/fake.apkg", str(dest))
    assert os.listdir(dest) == []  # nothing plausible left behind


def test_download_enforces_size_cap(server, tmp_path):
    www, base = server
    make_apkg(www / "big.apkg", payload=os.urandom(300_000))
    dest = tmp_path / "import"
    with pytest.raises(DownloadError, match="limit"):
        download_deck(f"{base}/big.apkg", str(dest),
                      max_bytes=100_000)
    assert os.listdir(dest) == []


def test_download_404(server, tmp_path):
    _www, base = server
    with pytest.raises(DownloadError, match="404"):
        download_deck(f"{base}/missing.apkg", str(tmp_path))


# -- helpers ---------------------------------------------------------------------


def test_safe_filename_neutralises_hostile_names():
    url = "http://x/decks/n5.apkg"
    assert safe_filename(url) == "n5.apkg"
    assert safe_filename(url, name="../../etc/passwd") == "passwd.apkg"
    assert safe_filename(url, name="..\\..\\evil") == "evil.apkg"
    assert safe_filename(url, name="...") == "deck.apkg"
    assert safe_filename("http://x/", name="") == "deck.apkg"
    assert safe_filename("http://x/a%20b.apkg") == "a b.apkg"
    assert safe_filename(url, name="My Deck.apkg") == "My Deck.apkg"


def test_format_size():
    assert format_size(None) == ""
    assert format_size(500) == "1 kB"
    assert format_size(200 * 1024) == "200 kB"
    assert format_size(5 * 1024 * 1024) == "5.0 MB"
