#!/usr/bin/env python3
"""Standard-library end-to-end tests for the simple LAN sync service."""

import hashlib
import json
import os
import sys
import tarfile
import tempfile
import threading
import types
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "cardbrick-py"))

# The sync transport itself is standard-library-only.  The production app
# bundles py-fsrs, but this host test only needs scheduler.py's date helpers
# while importing pattern content, so provide its import-time names.
if "fsrs" not in sys.modules:
    fake_fsrs = types.ModuleType("fsrs")
    fake_fsrs.Card = object
    fake_fsrs.Scheduler = object
    fake_fsrs.Rating = types.SimpleNamespace(Again=1, Hard=2, Good=3, Easy=4)
    fake_fsrs.State = types.SimpleNamespace(Review=2)
    sys.modules["fsrs"] = fake_fsrs

from cardbrick.paths import AppPaths  # noqa: E402
from cardbrick.storage import Storage  # noqa: E402
from cardbrick.sync import (SyncError, SyncState, create_backup,  # noqa: E402
                            restore_backup, sync_once)
from cardbrick_server.server import make_server  # noqa: E402
from cardbrick_server.store import Store  # noqa: E402


class FakeScheduler:
    """Minimal importer scheduler; sync behavior does not depend on FSRS."""

    def initial_state(self, card_id):
        return {
            "card_id": card_id,
            "due": "2026-07-10T00:00:00+00:00",
            "stability": None,
            "difficulty": None,
            "elapsed_days": 0,
            "scheduled_days": 0,
            "reps": 0,
            "lapses": 0,
            "state": 0,
            "fsrs_json": "{}",
        }


def json_request(method, url, value=None):
    data = json.dumps(value).encode() if value is not None else None
    request = Request(url, data=data, method=method,
                      headers={"Content-Type": "application/json"})
    with urlopen(request, timeout=5) as response:
        return json.loads(response.read().decode())


class RunningServer:
    def __init__(self, root):
        self.server = make_server(root, "127.0.0.1", 0)
        self.thread = threading.Thread(target=self.server.serve_forever,
                                       daemon=True)

    def __enter__(self):
        self.thread.start()
        host, port = self.server.server_address
        self.url = "http://%s:%s" % (host, port)
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


class ServerTests(unittest.TestCase):
    def test_checkin_manifest_download_and_atomic_backup(self):
        with tempfile.TemporaryDirectory() as root:
            store = Store(root)
            source = store.incoming_dir / "lesson.json"
            source.write_text('{"format":"cardbrick-pattern-pack"}')
            item = store.scan_incoming()[0]
            store.assign(str(item["id"]), ["Maya"])

            with RunningServer(root) as running:
                checkin = json_request(
                    "POST", running.url + "/devices/maya/check-in",
                    {"app_version": "1.2.3", "schema_version": 11})
                self.assertEqual(checkin["device"], "maya")
                manifest = json_request(
                    "GET", running.url + "/devices/maya/manifest")
                self.assertEqual(manifest["content"][0]["filename"],
                                 "lesson.json")
                with urlopen(running.url + "/files/%s" % item["id"]) as response:
                    self.assertEqual(response.read(), source_content(item, store))

                payload = b"ordinary tar gzip bytes for test"
                digest = hashlib.sha256(payload).hexdigest()
                request = Request(
                    running.url + "/devices/maya/backups/20260710T120000Z",
                    data=payload, method="PUT",
                    headers={"X-CardBrick-SHA256": digest,
                             "Content-Type": "application/gzip"})
                with urlopen(request) as response:
                    result = json.loads(response.read().decode())
                self.assertEqual(result["sha256"], digest)
                backups = store.backups("maya")
                self.assertEqual(len(backups), 1)
                stored = store.backups_dir / "maya" / backups[0]["filename"]
                self.assertEqual(stored.read_bytes(), payload)
                self.assertFalse(list(store.tmp_dir.glob("*.partial")))

    def test_bad_backup_checksum_is_not_published(self):
        with tempfile.TemporaryDirectory() as root, RunningServer(root) as running:
            request = Request(
                running.url + "/devices/maya/backups/bad",
                data=b"broken", method="PUT",
                headers={"X-CardBrick-SHA256": "0" * 64})
            with self.assertRaises(HTTPError) as caught:
                urlopen(request)
            self.assertEqual(caught.exception.code, 400)
            store = Store(root)
            self.assertEqual(store.backups("maya"), [])
            self.assertFalse(list(store.tmp_dir.glob("*.partial")))


def source_content(item, store):
    return (store.content_dir / item["stored_name"]).read_bytes()


class DeviceTests(unittest.TestCase):
    def _new_device(self, root):
        paths = AppPaths(str(Path(root) / "device"))
        paths.ensure_directories()
        storage = Storage(paths.db_path)
        return paths, storage

    def test_old_mdns_default_migrates_but_custom_servers_do_not(self):
        with tempfile.TemporaryDirectory() as root:
            data_dir = Path(root) / "device"
            data_dir.mkdir()
            state_path = data_dir / "sync.json"
            state_path.write_text(json.dumps({
                "device_name": "maysa",
                "server": "http://raspberrypi.local:6429",
            }))
            migrated = SyncState(str(data_dir))
            self.assertEqual(migrated.data["server"],
                             "http://10.0.0.30:6429")
            migrated.configure()
            self.assertEqual(
                json.loads(state_path.read_text())["server"],
                "http://10.0.0.30:6429")

            state_path.write_text(json.dumps({
                "device_name": "maysa",
                "server": "http://10.0.0.99:6429",
            }))
            custom = SyncState(str(data_dir))
            self.assertEqual(custom.data["server"],
                             "http://10.0.0.99:6429")

    def test_backup_is_readable_and_restore_preserves_sync_identity(self):
        with tempfile.TemporaryDirectory() as root:
            paths, storage = self._new_device(root)
            storage.ensure_default_profile("Maya")
            Path(paths.settings_path).write_text('{"language":"en"}')
            Path(paths.input_map_path).write_text('{"buttons":{}}')
            (Path(paths.media_dir) / "hola.mp3").write_bytes(b"audio")
            (Path(paths.import_dir) / "source.apkg").write_bytes(b"deck")
            Path(paths.log_path).write_text("diagnostic log")
            state = SyncState(paths.data_dir)
            state.configure(name="maya", server="http://old-server:6429")
            temp, archive, digest, manifest = create_backup(
                paths, storage, "1.0", "maya")
            storage.close()
            try:
                self.assertEqual(len(digest), 64)
                self.assertEqual(manifest["device"], "maya")
                with tarfile.open(archive, "r:gz") as backup:
                    names = backup.getnames()
                    self.assertIn("cardbrick/cardbrick.db", names)
                    self.assertIn("cardbrick/manifest.json", names)
                    self.assertIn("cardbrick/settings.json", names)
                    self.assertIn("cardbrick/input_mapping.json", names)
                    self.assertIn("cardbrick/media/hola.mp3", names)
                    self.assertIn("cardbrick/import/source.apkg", names)
                    self.assertIn("cardbrick/logs/cardbrick.log", names)
                # Change the current identity after the backup; restore must
                # retain the current device identity, not the archived one.
                state.configure(name="replacement",
                                server="http://new-server:6429")
                state.data["installed"] = {"newer": {"sha256": "not-in-backup"}}
                state.save()
                rollback = restore_backup(archive, paths.data_dir)
                restored_state = SyncState(paths.data_dir)
                self.assertEqual(restored_state.data["device_name"],
                                 "replacement")
                self.assertEqual(restored_state.data["server"],
                                 "http://new-server:6429")
                self.assertNotIn("newer", restored_state.data["installed"])
                self.assertIsNone(
                    restored_state.data["last_backup_fingerprint"])
                self.assertTrue(os.path.isdir(rollback))
                restored = Storage(paths.db_path)
                self.assertEqual(restored.ensure_default_profile()["name"],
                                 "Maya")
                restored.close()
            finally:
                import shutil
                shutil.rmtree(temp, ignore_errors=True)

    def test_end_to_end_backup_assignment_and_idempotent_import(self):
        with tempfile.TemporaryDirectory() as root:
            server_root = Path(root) / "server"
            store = Store(server_root)
            pattern = {
                "format": "cardbrick-pattern-pack",
                "deck": "Server Drills",
                "items": [{
                    "pattern_id": "SYNC-001",
                    "kind": "production",
                    "prompt_en": "Say hello",
                    "sibling_es": "Hola, Ana.",
                    "skeleton_es": "Hola, ___.",
                    "answer_es": "Hola, Luis.",
                    "tags": "sync-test"
                }]
            }
            incoming = store.incoming_dir / "server-drills.json"
            incoming.write_text(json.dumps(pattern), encoding="utf-8")
            item = store.scan_incoming()[0]
            store.assign(str(item["id"]), ["maya"])

            paths, storage = self._new_device(root)
            scheduler = FakeScheduler()
            with RunningServer(server_root) as running:
                previous_failure = SyncState(paths.data_dir)
                previous_failure.configure(name="maya", server=running.url)
                previous_failure.data["last_error"] = "old network error"
                previous_failure.save()
                backup_stages = []
                backup = sync_once(
                    storage, scheduler, paths, "1.0", name="maya",
                    server=running.url, backup_only=True, force_backup=True,
                    progress=lambda phase, _message: backup_stages.append(phase))
                self.assertTrue(backup["backed_up"])
                self.assertEqual(backup["installed"], 0)
                self.assertEqual(storage.conn.execute(
                    "SELECT COUNT(*) FROM cards").fetchone()[0], 0)
                self.assertEqual(
                    backup_stages,
                    ["connecting", "examining", "creating-backup",
                     "uploading-backup", "complete"],
                )
                self.assertIsNone(
                    SyncState(paths.data_dir).data["last_error"])

                stages = []
                first = sync_once(storage, scheduler, paths, "1.0",
                                  name="maya", server=running.url,
                                  progress=lambda phase, _message:
                                  stages.append(phase))
                self.assertTrue(first["backed_up"])
                self.assertEqual(first["installed"], 1)
                self.assertIn("checking-content", stages)
                self.assertIn("downloading", stages)
                self.assertIn("installing", stages)
                self.assertEqual(stages[-1], "complete")
                self.assertEqual(storage.conn.execute(
                    "SELECT COUNT(*) FROM cards").fetchone()[0], 1)
                self.assertEqual(store.assignments("maya")[0]["status"],
                                 "installed")
                self.assertTrue(store.backups("maya"))

                second = sync_once(storage, scheduler, paths, "1.0")
                self.assertEqual(second["installed"], 0)
                self.assertEqual(storage.conn.execute(
                    "SELECT COUNT(*) FROM cards").fetchone()[0], 1)

                storage.conn.execute(
                    "UPDATE review_state SET reps=7 WHERE card_id IN "
                    "(SELECT id FROM cards)")
                storage.conn.commit()
                pattern["items"][0]["answer_es"] = "Hola, Marta."
                incoming.write_text(json.dumps(pattern), encoding="utf-8")
                updated = store.scan_incoming()[0]
                store.assign(str(updated["id"]), ["maya"])
                third = sync_once(storage, scheduler, paths, "1.0")
                self.assertEqual(third["installed"], 1)
                row = storage.conn.execute(
                    """SELECT c.back, r.reps FROM cards c
                       JOIN review_state r ON r.card_id=c.id""").fetchone()
                self.assertEqual(row["back"], "Hola, Marta.")
                self.assertEqual(row["reps"], 7)
                self.assertEqual(len(store.manifest("maya")), 1)
            storage.close()

    def test_failure_is_saved_without_replacing_success_timestamps(self):
        with tempfile.TemporaryDirectory() as root:
            paths, storage = self._new_device(root)
            state = SyncState(paths.data_dir)
            state.configure(name="maya", server="http://127.0.0.1:1")
            state.data["last_sync_at"] = "2026-07-10T12:00:00+00:00"
            state.data["last_backup_at"] = "2026-07-10T11:00:00+00:00"
            state.save()

            with self.assertRaises(SyncError):
                sync_once(storage, FakeScheduler(), paths, "1.0")

            failed = SyncState(paths.data_dir).data
            self.assertTrue(failed["last_error"])
            self.assertEqual(failed["last_sync_at"],
                             "2026-07-10T12:00:00+00:00")
            self.assertEqual(failed["last_backup_at"],
                             "2026-07-10T11:00:00+00:00")
            storage.close()


if __name__ == "__main__":
    unittest.main()
