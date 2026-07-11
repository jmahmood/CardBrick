"""Small outbound backup and content client for the family sync server.

There is deliberately no account/enrollment machinery.  A device is its
configured name, and all requests are ordinary HTTP requests on the LAN.
"""

import hashlib
import http.client
import json
import logging
import os
import shutil
import sqlite3
import tarfile
import tempfile
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlsplit
from urllib.request import Request, urlopen


log = logging.getLogger(__name__)
DEFAULT_SERVER = "http://10.0.0.30:6429"
LEGACY_MDNS_SERVER = "http://raspberrypi.local:6429"
STATE_FILENAME = "sync.json"


class SyncError(Exception):
    pass


def _emit_progress(callback, phase, message):
    """Report a sync stage without making the client depend on a UI."""
    if callback is not None:
        callback(phase, message)


def _now_iso():
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def _timestamp():
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")


def _sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


class SyncState:
    def __init__(self, data_dir):
        self.path = os.path.join(data_dir, STATE_FILENAME)
        self.data = {"device_name": None, "server": DEFAULT_SERVER,
                     "installed": {}, "last_backup_at": None,
                     "last_backup_fingerprint": None,
                     "last_error": None, "last_sync_at": None}
        self.load()

    def load(self):
        try:
            with open(self.path, encoding="utf-8") as source:
                loaded = json.load(source)
            if isinstance(loaded, dict):
                self.data.update(loaded)
        except FileNotFoundError:
            pass
        except (OSError, json.JSONDecodeError) as exc:
            log.warning("cannot read sync state %s: %s", self.path, exc)
        # The handheld firmwares do not reliably resolve .local names. Move
        # installations that still use our old built-in default to the
        # prepared server's static address. Explicit custom URLs are kept.
        if self.data.get("server") == LEGACY_MDNS_SERVER:
            self.data["server"] = DEFAULT_SERVER
        if not isinstance(self.data.get("installed"), dict):
            self.data["installed"] = {}
        return self

    def save(self):
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as target:
            json.dump(self.data, target, indent=2, sort_keys=True)
        os.replace(tmp, self.path)

    def configure(self, name=None, server=None):
        if name:
            normalized = name.strip().lower().replace(" ", "-")
            if not normalized or any(ch not in
                                     "abcdefghijklmnopqrstuvwxyz0123456789._-"
                                     for ch in normalized):
                raise SyncError("invalid device name: %s" % name)
            self.data["device_name"] = normalized
        if server:
            self.data["server"] = server.rstrip("/")
        elif os.environ.get("CARDBRICK_SYNC_URL"):
            self.data["server"] = os.environ["CARDBRICK_SYNC_URL"].rstrip("/")
        self.save()


def _json_request(method, url, value=None, timeout=8):
    data = None
    headers = {"Accept": "application/json"}
    if value is not None:
        data = json.dumps(value).encode("utf-8")
        headers["Content-Type"] = "application/json"
    request = Request(url, data=data, headers=headers, method=method)
    try:
        with urlopen(request, timeout=timeout) as response:
            body = response.read()
            return json.loads(body.decode("utf-8")) if body else {}
    except HTTPError as exc:
        try:
            detail = exc.read().decode("utf-8")
        except Exception:
            detail = str(exc)
        raise SyncError("server returned HTTP %s: %s" %
                        (exc.code, detail)) from exc
    except (URLError, OSError) as exc:
        raise SyncError("cannot reach sync server: %s" % exc) from exc


def _upload_file(url, path, digest, timeout=300):
    parsed = urlsplit(url)
    if parsed.scheme not in ("http", "https") or not parsed.hostname:
        raise SyncError("invalid sync server URL: %s" % url)
    connection_class = (http.client.HTTPSConnection if parsed.scheme == "https"
                        else http.client.HTTPConnection)
    connection = connection_class(parsed.hostname, parsed.port,
                                  timeout=timeout)
    target = parsed.path or "/"
    if parsed.query:
        target += "?" + parsed.query
    size = os.path.getsize(path)
    try:
        connection.putrequest("PUT", target)
        connection.putheader("Content-Length", str(size))
        connection.putheader("Content-Type", "application/gzip")
        connection.putheader("X-CardBrick-SHA256", digest)
        connection.endheaders()
        with open(path, "rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                connection.send(chunk)
        response = connection.getresponse()
        body = response.read()
        if response.status not in (200, 201):
            raise SyncError("backup upload failed: HTTP %s %s" %
                            (response.status, body.decode("utf-8", "replace")))
        return json.loads(body.decode("utf-8")) if body else {}
    except (OSError, http.client.HTTPException) as exc:
        raise SyncError("backup upload failed: %s" % exc) from exc
    finally:
        connection.close()


def data_fingerprint(paths):
    """Cheaply identify durable changes without hashing large media files."""
    digest = hashlib.sha256()
    root = Path(paths.data_dir)
    excluded_dirs = {".pycache", "sync-downloads", "sync-staging", "logs"}
    excluded_files = {STATE_FILENAME, STATE_FILENAME + ".tmp"}
    for current, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in excluded_dirs)
        for filename in sorted(files):
            if filename in excluded_files or filename.endswith(("-wal", "-shm")):
                continue
            path = Path(current) / filename
            try:
                stat = path.stat()
            except OSError:
                continue
            relative = str(path.relative_to(root))
            digest.update(relative.encode("utf-8"))
            digest.update(str(stat.st_size).encode("ascii"))
            digest.update(str(stat.st_mtime_ns).encode("ascii"))
    return digest.hexdigest()


def _copy_data_tree(paths, destination, storage, app_version):
    root = Path(paths.data_dir)
    target = Path(destination) / "cardbrick"
    target.mkdir(parents=True)

    # sqlite3.backup produces a transactionally consistent database even
    # while the study process has the source connection open.
    database = sqlite3.connect(str(target / "cardbrick.db"))
    try:
        storage.conn.backup(database)
    finally:
        database.close()

    excluded_dirs = {".pycache", "sync-downloads", "sync-staging"}
    excluded_files = {"cardbrick.db", "cardbrick.db-wal", "cardbrick.db-shm",
                      STATE_FILENAME + ".tmp"}
    for current, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in excluded_dirs]
        relative_dir = Path(current).relative_to(root)
        output_dir = target / relative_dir
        output_dir.mkdir(parents=True, exist_ok=True)
        for filename in files:
            if filename in excluded_files:
                continue
            source = Path(current) / filename
            if source.is_symlink():
                continue
            shutil.copy2(source, output_dir / filename)

    manifest = {
        "format": "cardbrick-backup",
        "format_version": 1,
        "device": None,
        "created_at": _now_iso(),
        "app_version": app_version,
        "schema_version": storage.schema_version(),
    }
    return target, manifest


def create_backup(paths, storage, app_version, device_name):
    temp_dir = tempfile.mkdtemp(prefix="cardbrick-backup-")
    try:
        target, manifest = _copy_data_tree(paths, temp_dir, storage,
                                           app_version)
        manifest["device"] = device_name
        with open(target / "manifest.json", "w", encoding="utf-8") as output:
            json.dump(manifest, output, indent=2, sort_keys=True)
        archive = os.path.join(temp_dir, "%s-%s.tar.gz" %
                               (device_name, _timestamp()))
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(target, arcname="cardbrick")
        return temp_dir, archive, _sha256(archive), manifest
    except Exception:
        shutil.rmtree(temp_dir, ignore_errors=True)
        raise


def _download(url, destination, expected_sha, timeout=120):
    partial = destination + ".partial"
    digest = hashlib.sha256()
    request = Request(url, headers={"Accept": "application/octet-stream"})
    try:
        with urlopen(request, timeout=timeout) as response, \
                open(partial, "wb") as target:
            while True:
                chunk = response.read(1024 * 1024)
                if not chunk:
                    break
                target.write(chunk)
                digest.update(chunk)
        actual = digest.hexdigest()
        if actual != expected_sha:
            raise SyncError("download checksum mismatch for %s" %
                            os.path.basename(destination))
        os.replace(partial, destination)
        return destination
    except (HTTPError, URLError, OSError) as exc:
        raise SyncError("content download failed: %s" % exc) from exc
    finally:
        if os.path.exists(partial):
            os.unlink(partial)


def _safe_extract_zip(path, destination):
    root = os.path.realpath(destination)
    with zipfile.ZipFile(path) as archive:
        for member in archive.infolist():
            target = os.path.realpath(os.path.join(root, member.filename))
            if target != root and not target.startswith(root + os.sep):
                raise SyncError("unsafe path in zip: %s" % member.filename)
        archive.extractall(root)


def _safe_extract_tar(path, destination):
    root = os.path.realpath(destination)
    with tarfile.open(path, "r:gz") as archive:
        for member in archive.getmembers():
            target = os.path.realpath(os.path.join(root, member.name))
            if target != root and not target.startswith(root + os.sep):
                raise SyncError("unsafe path in tar: %s" % member.name)
            if member.issym() or member.islnk():
                raise SyncError("links are not allowed in content archives")
        archive.extractall(root)


def _check_media_collisions(source_media_dir, media_dir):
    if not source_media_dir or not os.path.isdir(source_media_dir):
        return
    for current, _, files in os.walk(source_media_dir):
        for filename in files:
            source = os.path.join(current, filename)
            existing = os.path.join(media_dir, filename)
            if os.path.isfile(existing) and _sha256(source) != _sha256(existing):
                raise SyncError("media filename collision: %s" % filename)


def _import_entry(path, storage, scheduler, paths, media_dir=None, deck=None):
    lower = path.lower()
    if lower.endswith(".apkg"):
        from .importer import import_apkg
        return import_apkg(path, storage, scheduler, paths.media_dir)
    if lower.endswith(".csv"):
        from .vocab_csv import import_vocab_csv
        _check_media_collisions(media_dir, paths.media_dir)
        kwargs = {"source_media_dir": media_dir}
        if deck:
            kwargs["deck_name"] = deck
        return import_vocab_csv(path, storage, scheduler, paths.media_dir,
                                **kwargs)
    if lower.endswith(".json"):
        from .pattern_pack import import_pattern_pack
        return import_pattern_pack(path, storage, scheduler, deck_name=deck)
    raise SyncError("unsupported content file: %s" % os.path.basename(path))


def import_content(path, storage, scheduler, paths):
    lower = path.lower()
    if lower.endswith((".apkg", ".csv", ".json")):
        return _import_entry(path, storage, scheduler, paths)
    if not lower.endswith((".zip", ".tar.gz", ".tgz")):
        raise SyncError("unsupported assigned content: %s" %
                        os.path.basename(path))
    temp_dir = tempfile.mkdtemp(prefix="cardbrick-content-")
    try:
        if lower.endswith(".zip"):
            _safe_extract_zip(path, temp_dir)
        else:
            _safe_extract_tar(path, temp_dir)
        manifest_path = os.path.join(temp_dir, "manifest.json")
        try:
            with open(manifest_path, encoding="utf-8") as source:
                manifest = json.load(source)
        except (OSError, json.JSONDecodeError) as exc:
            raise SyncError("content archive needs a valid manifest.json: %s" %
                            exc) from exc
        entrypoint = os.path.realpath(os.path.join(
            temp_dir, manifest.get("entrypoint", "")))
        root = os.path.realpath(temp_dir)
        if not entrypoint.startswith(root + os.sep) or not os.path.isfile(entrypoint):
            raise SyncError("content archive entrypoint is missing or unsafe")
        media = manifest.get("media_dir")
        media_dir = os.path.realpath(os.path.join(temp_dir, media)) if media else None
        if media_dir and not media_dir.startswith(root + os.sep):
            raise SyncError("content archive media_dir is unsafe")
        return _import_entry(entrypoint, storage, scheduler, paths,
                             media_dir=media_dir, deck=manifest.get("deck"))
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)


def _report(server, name, content_id, status, error=None):
    try:
        _json_request("POST", "%s/devices/%s/report" %
                      (server, quote(name)),
                      {"content_id": content_id, "status": status,
                       "error": error})
    except SyncError as exc:
        log.warning("could not report sync result: %s", exc)


def sync_once(storage, scheduler, paths, app_version, name=None, server=None,
              backup_only=False, content_only=False, force_backup=False,
              progress=None):
    state = SyncState(paths.data_dir)
    try:
        state.configure(name=name, server=server)
        name = state.data.get("device_name")
        server = state.data.get("server") or DEFAULT_SERVER
        if not name:
            raise SyncError(
                "sync is not configured; choose a device name first")

        _emit_progress(progress, "connecting", "Connecting to the server…")
        _json_request("POST", "%s/devices/%s/check-in" %
                      (server, quote(name)),
                      {"app_version": app_version,
                       "schema_version": storage.schema_version()})

        _emit_progress(progress, "examining", "Checking local data…")
        fingerprint = data_fingerprint(paths)
        did_backup = False
        if not content_only and (force_backup or fingerprint !=
                                 state.data.get("last_backup_fingerprint")):
            _emit_progress(progress, "creating-backup",
                           "Creating a safe backup…")
            temp_dir, archive, digest, _ = create_backup(
                paths, storage, app_version, name)
            try:
                _emit_progress(progress, "uploading-backup",
                               "Uploading the backup…")
                url = ("%s/devices/%s/backups/%s" %
                       (server, quote(name),
                        quote(os.path.basename(archive))))
                _upload_file(url, archive, digest)
            finally:
                shutil.rmtree(temp_dir, ignore_errors=True)
            state.data["last_backup_at"] = _now_iso()
            state.data["last_backup_fingerprint"] = fingerprint
            state.data["last_error"] = None
            state.save()
            did_backup = True

        installed_count = 0
        if not backup_only:
            _emit_progress(progress, "checking-content",
                           "Checking for assigned content…")
            manifest = _json_request(
                "GET", "%s/devices/%s/manifest" % (server, quote(name)))
            download_dir = os.path.join(paths.data_dir, "sync-downloads")
            os.makedirs(download_dir, exist_ok=True)
            installed = state.data["installed"]
            for item in manifest.get("content", []):
                key = str(item["id"])
                if installed.get(key, {}).get("sha256") == item["sha256"]:
                    continue
                # A verified pre-import server backup is mandatory even when
                # the caller requested content-only sync.
                if not did_backup:
                    _emit_progress(progress, "creating-backup",
                                   "Creating a pre-install backup…")
                    temp_dir, archive, digest, _ = create_backup(
                        paths, storage, app_version, name)
                    try:
                        _emit_progress(progress, "uploading-backup",
                                       "Uploading the pre-install backup…")
                        url = ("%s/devices/%s/backups/%s" %
                               (server, quote(name),
                                quote(os.path.basename(archive))))
                        _upload_file(url, archive, digest)
                    finally:
                        shutil.rmtree(temp_dir, ignore_errors=True)
                    did_backup = True
                filename = os.path.basename(item["filename"])
                destination = os.path.join(download_dir, filename)
                try:
                    _emit_progress(progress, "downloading",
                                   "Downloading %s…" % filename)
                    _download("%s/files/%s" % (server, item["id"]),
                              destination, item["sha256"])
                    _emit_progress(progress, "installing",
                                   "Installing %s…" % filename)
                    stats = import_content(
                        destination, storage, scheduler, paths)
                    installed[key] = {"filename": filename,
                                      "sha256": item["sha256"],
                                      "installed_at": _now_iso()}
                    state.save()
                    _report(server, name, item["id"], "installed")
                    installed_count += 1
                    log.info("installed assigned content %s: %s", filename,
                             stats.summary())
                except Exception as exc:
                    _report(server, name, item["id"], "error", str(exc))
                    raise SyncError("could not install %s: %s" %
                                    (filename, exc)) from exc

        state.data["last_sync_at"] = _now_iso()
        state.data["last_error"] = None
        state.save()
        _emit_progress(progress, "complete", "Sync complete.")
        return {"device": name, "server": server, "backed_up": did_backup,
                "installed": installed_count}
    except Exception as exc:
        state.data["last_error"] = str(exc)
        try:
            state.save()
        except OSError:
            log.exception("could not save sync failure state")
        if isinstance(exc, SyncError):
            raise
        raise SyncError(str(exc)) from exc


def sync_status(data_dir):
    return SyncState(data_dir).data


def restore_backup(archive_path, data_dir):
    """Restore a local backup archive, preserving an automatic rollback."""
    archive_path = os.path.abspath(archive_path)
    data_dir = os.path.abspath(data_dir)
    temp_dir = tempfile.mkdtemp(prefix="cardbrick-restore-")
    rollback = data_dir + ".pre-restore-" + _timestamp()
    try:
        _safe_extract_tar(archive_path, temp_dir)
        restored = os.path.join(temp_dir, "cardbrick")
        database = os.path.join(restored, "cardbrick.db")
        manifest = os.path.join(restored, "manifest.json")
        if not os.path.isfile(database) or not os.path.isfile(manifest):
            raise SyncError("not a CardBrick backup archive")
        conn = sqlite3.connect("file:%s?mode=ro" % database, uri=True)
        try:
            result = conn.execute("PRAGMA integrity_check").fetchone()[0]
        finally:
            conn.close()
        if result != "ok":
            raise SyncError("restored database failed integrity check: %s" %
                            result)
        current_sync = os.path.join(data_dir, STATE_FILENAME)
        current_identity = None
        if os.path.isfile(current_sync):
            try:
                with open(current_sync, encoding="utf-8") as source:
                    current = json.load(source)
                current_identity = {
                    "device_name": current.get("device_name"),
                    "server": current.get("server"),
                }
            except (OSError, json.JSONDecodeError):
                pass
        if os.path.exists(data_dir):
            os.replace(data_dir, rollback)
        try:
            shutil.copytree(restored, data_dir)
            if current_identity is not None:
                restored_state = SyncState(data_dir)
                restored_state.data.update(current_identity)
                # The restored database may differ from the device's most
                # recent snapshot, so force a fresh verified server backup.
                restored_state.data["last_backup_fingerprint"] = None
                restored_state.save()
        except Exception:
            shutil.rmtree(data_dir, ignore_errors=True)
            if os.path.exists(rollback):
                os.replace(rollback, data_dir)
            raise
        return rollback
    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)
