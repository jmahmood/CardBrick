"""Download .apkg decks from a server URL — standard library only.

A parent hosts decks on any plain HTTP(S) server (nginx, a NAS,
`python -m http.server` on the family PC, an S3 bucket, ...); the
device downloads them over WiFi into the import folder, where the
existing importer takes over. No new dependencies: `urllib.request`
does the transfer, `json` reads the deck list, `zipfile` sanity-checks
the result.

The configured URL may point at either:

* a single deck: any URL whose path ends in ``.apkg``, or
* a deck list (manifest): a JSON document, either
  ``{"decks": [{"name": "...", "url": "n5.apkg", "size": 12345}, ...]}``
  or a bare list of the same entry objects. ``url`` (or the alias
  ``file``) may be relative — it is resolved against the manifest's own
  URL, so a manifest can sit in the same folder as its decks. ``name``
  and ``size`` are optional; ``size`` is bytes, shown in the UI.

Downloads are hardened for an unattended appliance:

* http/https only — including across redirects (the default urllib
  opener would happily follow a redirect to ``file://``).
* Streamed to ``<name>.part`` with a byte cap, verified to be a real
  zip archive, then atomically renamed — a dropped WiFi connection or
  a captive-portal HTML page never leaves a plausible-looking .apkg.
* Filenames are reduced to a safe basename; a hostile manifest cannot
  write outside the destination folder.
"""

import json
import logging
import os
import posixpath
import re
import urllib.error
import urllib.parse
import urllib.request
import zipfile

log = logging.getLogger(__name__)

DEFAULT_TIMEOUT = 30  # seconds; per socket operation, not the whole file
DEFAULT_MAX_BYTES = 1024 * 1024 * 1024  # 1 GiB sanity cap
CHUNK_SIZE = 64 * 1024
USER_AGENT = "CardBrick"

ALLOWED_SCHEMES = ("http", "https")


class DownloadError(Exception):
    """Anything that should surface as a friendly message, not a crash."""


def _build_opener():
    """An opener that speaks *only* http/https, even after redirects.

    urllib's default opener chain includes file:// and ftp:// handlers,
    and a redirect can switch schemes — so build the chain explicitly.
    """
    opener = urllib.request.OpenerDirector()
    for handler in (urllib.request.HTTPHandler(),
                    urllib.request.HTTPSHandler(),
                    urllib.request.HTTPDefaultErrorHandler(),
                    urllib.request.HTTPRedirectHandler(),
                    urllib.request.HTTPErrorProcessor(),
                    urllib.request.UnknownHandler()):
        opener.add_handler(handler)
    return opener


_opener = _build_opener()


def _open(url, timeout):
    scheme = urllib.parse.urlsplit(url).scheme.lower()
    if scheme not in ALLOWED_SCHEMES:
        raise DownloadError(f"Only http:// and https:// URLs are "
                            f"supported (got {scheme or 'no scheme'}).")
    request = urllib.request.Request(url,
                                     headers={"User-Agent": USER_AGENT})
    try:
        return _opener.open(request, timeout=timeout)
    except urllib.error.HTTPError as exc:
        raise DownloadError(f"Server said {exc.code} {exc.reason} "
                            f"for {url}") from exc
    except urllib.error.URLError as exc:
        raise DownloadError(f"Cannot reach {url}: {exc.reason}") from exc
    except (OSError, ValueError) as exc:
        raise DownloadError(f"Cannot reach {url}: {exc}") from exc


def _is_apkg_url(url):
    path = urllib.parse.urlsplit(url).path
    return path.lower().endswith(".apkg")


def safe_filename(url, name=None):
    """A bare .apkg filename for a download, from the entry name or the
    URL path — never anything that can escape the destination folder."""
    candidate = name or ""
    if not candidate:
        path = urllib.parse.urlsplit(url).path
        candidate = urllib.parse.unquote(posixpath.basename(path))
    # Basename twice over both separator styles, then whitelist chars.
    candidate = os.path.basename(candidate.replace("\\", "/"))
    candidate = re.sub(r"[^\w.\- ()]", "_", candidate).strip(". ")
    if candidate.lower().endswith(".apkg"):
        candidate = candidate[:-len(".apkg")].strip(". ")
    if not candidate:
        candidate = "deck"
    return candidate + ".apkg"


def fetch_deck_list(server_url, timeout=DEFAULT_TIMEOUT):
    """The decks offered at server_url, normalised to
    ``[{"name", "url", "size"}, ...]``.

    A direct .apkg URL yields a single-entry list; anything else is
    fetched and parsed as a JSON manifest (see module docstring).
    """
    server_url = server_url.strip()
    if not server_url:
        raise DownloadError("No server URL configured.")
    if _is_apkg_url(server_url):
        return [{"name": safe_filename(server_url), "url": server_url,
                 "size": None}]

    with _open(server_url, timeout) as response:
        raw = response.read(4 * 1024 * 1024)  # a manifest is small
        manifest_url = response.geturl()
    try:
        data = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise DownloadError(
            f"{server_url} is not a deck list: expected JSON like "
            f'{{"decks": [{{"name": ..., "url": ...}}]}} or a direct '
            f"link to an .apkg file ({exc}).") from exc

    entries = data.get("decks") if isinstance(data, dict) else data
    if not isinstance(entries, list):
        raise DownloadError(f"{server_url}: JSON must be a list of decks "
                            f'or an object with a "decks" list.')

    decks = []
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        url = entry.get("url") or entry.get("file")
        if not url or not isinstance(url, str):
            continue
        url = urllib.parse.urljoin(manifest_url, url)
        if urllib.parse.urlsplit(url).scheme.lower() not in ALLOWED_SCHEMES:
            log.warning("deck list entry skipped (bad scheme): %r", url)
            continue
        name = entry.get("name")
        name = name.strip() if isinstance(name, str) else ""
        size = entry.get("size")
        if not isinstance(size, int) or size < 0:
            size = None
        decks.append({"name": name or safe_filename(url),
                      "url": url, "size": size})
    if not decks:
        raise DownloadError(f"The deck list at {server_url} has no "
                            f"usable decks.")
    return decks


def download_deck(url, dest_dir, timeout=DEFAULT_TIMEOUT,
                  max_bytes=DEFAULT_MAX_BYTES, progress=None, name=None):
    """Download one .apkg into dest_dir; returns the final path.

    progress, if given, is called as progress(received_bytes,
    total_bytes_or_None) roughly once per chunk. The file appears at
    its final name only after it downloaded completely and proved to
    be a real zip archive.
    """
    os.makedirs(dest_dir, exist_ok=True)
    final_path = os.path.join(dest_dir, safe_filename(url, name))
    part_path = final_path + ".part"

    with _open(url, timeout) as response:
        total = response.headers.get("Content-Length")
        try:
            total = int(total) if total is not None else None
        except ValueError:
            total = None
        if total is not None and total > max_bytes:
            raise DownloadError(
                f"Deck is {total // (1024 * 1024)} MB — larger than the "
                f"{max_bytes // (1024 * 1024)} MB limit.")

        received = 0
        try:
            with open(part_path, "wb") as out:
                while True:
                    try:
                        chunk = response.read(CHUNK_SIZE)
                    except OSError as exc:
                        raise DownloadError(
                            f"Connection lost after "
                            f"{received // 1024} kB: {exc}") from exc
                    if not chunk:
                        break
                    received += len(chunk)
                    if received > max_bytes:
                        raise DownloadError(
                            f"Download exceeded the "
                            f"{max_bytes // (1024 * 1024)} MB limit.")
                    out.write(chunk)
                    if progress:
                        progress(received, total)
            if total is not None and received < total:
                raise DownloadError(f"Connection closed early: got "
                                    f"{received} of {total} bytes.")
            if not zipfile.is_zipfile(part_path):
                raise DownloadError(
                    "The server did not send an .apkg file (got "
                    "something that is not a zip archive — a login or "
                    "error page, perhaps).")
            os.replace(part_path, final_path)
        finally:
            if os.path.exists(part_path):
                try:
                    os.remove(part_path)
                except OSError:
                    log.warning("could not remove %s", part_path)

    log.info("downloaded %s (%d bytes) -> %s", url, received, final_path)
    return final_path


def format_size(size):
    """'12.3 MB' style label for the UI; '' when size is unknown."""
    if size is None:
        return ""
    if size < 1024 * 1024:
        return f"{max(size, 1) // 1024 or 1} kB"
    return f"{size / (1024 * 1024):.1f} MB"
