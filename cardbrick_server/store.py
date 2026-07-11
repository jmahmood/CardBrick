"""Filesystem and SQLite metadata for the CardBrick sync server."""

import hashlib
import os
import re
import shutil
import sqlite3
from datetime import datetime, timezone
from pathlib import Path


SCHEMA = """
CREATE TABLE IF NOT EXISTS devices (
    name            TEXT PRIMARY KEY,
    first_seen      TEXT NOT NULL,
    last_seen       TEXT NOT NULL,
    app_version     TEXT,
    schema_version  INTEGER,
    last_backup     TEXT,
    last_error      TEXT
);

CREATE TABLE IF NOT EXISTS content (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    filename    TEXT NOT NULL,
    stored_name TEXT NOT NULL UNIQUE,
    sha256      TEXT NOT NULL UNIQUE,
    size        INTEGER NOT NULL,
    added_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS assignments (
    device_name TEXT NOT NULL REFERENCES devices(name) ON DELETE CASCADE,
    content_id  INTEGER NOT NULL REFERENCES content(id) ON DELETE CASCADE,
    assigned_at TEXT NOT NULL,
    installed_at TEXT,
    status      TEXT NOT NULL DEFAULT 'pending',
    last_error  TEXT,
    PRIMARY KEY (device_name, content_id)
);

CREATE TABLE IF NOT EXISTS backups (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    device_name TEXT NOT NULL REFERENCES devices(name) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    size        INTEGER NOT NULL,
    sha256      TEXT NOT NULL,
    verified    INTEGER NOT NULL DEFAULT 1,
    UNIQUE (device_name, filename)
);
"""


def utc_now():
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize_device_name(value):
    """Return a stable filesystem-safe family device name."""
    name = re.sub(r"\s+", "-", (value or "").strip().lower())
    if not name or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", name):
        raise ValueError(
            "device name must use 1-64 letters, numbers, dots, dashes, "
            "or underscores")
    return name


class Store:
    def __init__(self, root):
        self.root = Path(root).expanduser().resolve()
        self.db_path = self.root / "cardbrick-sync.db"
        self.incoming_dir = self.root / "incoming"
        self.content_dir = self.root / "content"
        self.devices_dir = self.root / "devices"
        self.backups_dir = self.root / "backups"
        self.tmp_dir = self.root / "tmp"
        for directory in (self.root, self.incoming_dir, self.content_dir,
                          self.devices_dir, self.backups_dir, self.tmp_dir):
            directory.mkdir(parents=True, exist_ok=True)
        with self.connect() as conn:
            conn.executescript(SCHEMA)

    def connect(self):
        conn = sqlite3.connect(str(self.db_path), timeout=30)
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys = ON")
        conn.execute("PRAGMA journal_mode = WAL")
        return conn

    def touch_device(self, name, app_version=None, schema_version=None):
        name = normalize_device_name(name)
        now = utc_now()
        with self.connect() as conn:
            conn.execute(
                """INSERT INTO devices
                   (name, first_seen, last_seen, app_version, schema_version)
                   VALUES (?, ?, ?, ?, ?)
                   ON CONFLICT(name) DO UPDATE SET
                     last_seen=excluded.last_seen,
                     app_version=COALESCE(excluded.app_version,
                                          devices.app_version),
                     schema_version=COALESCE(excluded.schema_version,
                                             devices.schema_version)""",
                (name, now, now, app_version, schema_version))
        (self.devices_dir / name / "assigned").mkdir(parents=True,
                                                       exist_ok=True)
        (self.backups_dir / name).mkdir(parents=True, exist_ok=True)
        return name

    def devices(self):
        with self.connect() as conn:
            return [dict(row) for row in conn.execute(
                "SELECT * FROM devices ORDER BY name")]

    def _stored_name(self, filename, digest):
        candidate = self.content_dir / filename
        if not candidate.exists():
            return filename
        if sha256_file(candidate) == digest:
            return filename
        path = Path(filename)
        suffixes = "".join(path.suffixes)
        stem = path.name[:-len(suffixes)] if suffixes else path.name
        return "%s-%s%s" % (stem, digest[:8], suffixes)

    def scan_incoming(self):
        added = []
        for source in sorted(self.incoming_dir.iterdir()):
            if not source.is_file() or source.name.startswith("."):
                continue
            digest = sha256_file(source)
            size = source.stat().st_size
            with self.connect() as conn:
                existing = conn.execute(
                    "SELECT * FROM content WHERE sha256 = ?", (digest,)
                ).fetchone()
                if existing:
                    source.unlink()
                    continue
                stored_name = self._stored_name(source.name, digest)
                destination = self.content_dir / stored_name
                os.replace(str(source), str(destination))
                cursor = conn.execute(
                    """INSERT INTO content
                       (filename, stored_name, sha256, size, added_at)
                       VALUES (?, ?, ?, ?, ?)""",
                    (source.name, stored_name, digest, size, utc_now()))
                row = conn.execute("SELECT * FROM content WHERE id = ?",
                                   (cursor.lastrowid,)).fetchone()
                added.append(dict(row))
        return added

    def content(self):
        with self.connect() as conn:
            return [dict(row) for row in conn.execute(
                "SELECT * FROM content ORDER BY added_at, id")]

    def find_content(self, identifier):
        with self.connect() as conn:
            if str(identifier).isdigit():
                row = conn.execute("SELECT * FROM content WHERE id = ?",
                                   (int(identifier),)).fetchone()
            else:
                row = conn.execute(
                    """SELECT * FROM content
                       WHERE filename = ? OR stored_name = ?
                       ORDER BY id DESC LIMIT 1""",
                    (identifier, identifier)).fetchone()
            return dict(row) if row else None

    def content_path(self, content_id):
        item = self.find_content(str(content_id))
        if not item:
            return None
        path = self.content_dir / item["stored_name"]
        return path if path.is_file() else None

    def assign(self, identifier, device_names):
        item = self.find_content(identifier)
        if not item:
            raise ValueError("content not found: %s" % identifier)
        assigned = []
        for raw_name in device_names:
            name = self.touch_device(raw_name)
            with self.connect() as conn:
                # A filename is the human-facing content identity. Assigning
                # a newer file with the same name replaces the older desired
                # version for this device instead of delivering both.
                conn.execute(
                    """DELETE FROM assignments
                       WHERE device_name=? AND content_id IN
                         (SELECT id FROM content WHERE filename=? AND id<>?)""",
                    (name, item["filename"], item["id"]))
                conn.execute(
                    """INSERT INTO assignments
                       (device_name, content_id, assigned_at, status)
                       VALUES (?, ?, ?, 'pending')
                       ON CONFLICT(device_name, content_id) DO NOTHING""",
                    (name, item["id"], utc_now()))
            link = self.devices_dir / name / "assigned" / item["filename"]
            if link.exists() or link.is_symlink():
                link.unlink()
            try:
                link.symlink_to(self.content_dir / item["stored_name"])
            except OSError:
                shutil.copy2(self.content_dir / item["stored_name"], link)
            assigned.append(name)
        return item, assigned

    def unassign(self, identifier, device_names):
        item = self.find_content(identifier)
        if not item:
            raise ValueError("content not found: %s" % identifier)
        for raw_name in device_names:
            name = normalize_device_name(raw_name)
            with self.connect() as conn:
                conn.execute(
                    "DELETE FROM assignments WHERE device_name=? AND content_id=?",
                    (name, item["id"]))
            link = self.devices_dir / name / "assigned" / item["filename"]
            if link.exists() or link.is_symlink():
                link.unlink()

    def manifest(self, device_name):
        name = self.touch_device(device_name)
        with self.connect() as conn:
            rows = conn.execute(
                """SELECT c.id, c.filename, c.sha256, c.size,
                          a.status, a.installed_at
                   FROM assignments a JOIN content c ON c.id=a.content_id
                   WHERE a.device_name=? ORDER BY c.filename, c.id""",
                (name,)).fetchall()
        return [dict(row) for row in rows]

    def assignments(self, device_name=None):
        query = """SELECT a.device_name, c.id, c.filename, c.sha256,
                          a.status, a.assigned_at, a.installed_at,
                          a.last_error
                   FROM assignments a JOIN content c ON c.id=a.content_id"""
        params = ()
        if device_name:
            query += " WHERE a.device_name=?"
            params = (normalize_device_name(device_name),)
        query += " ORDER BY a.device_name, c.filename"
        with self.connect() as conn:
            return [dict(row) for row in conn.execute(query, params)]

    def report(self, device_name, content_id, status, error=None):
        name = self.touch_device(device_name)
        installed_at = utc_now() if status == "installed" else None
        with self.connect() as conn:
            conn.execute(
                """UPDATE assignments
                   SET status=?, installed_at=COALESCE(?, installed_at),
                       last_error=?
                   WHERE device_name=? AND content_id=?""",
                (status, installed_at, error, name, int(content_id)))
            if error:
                conn.execute("UPDATE devices SET last_error=? WHERE name=?",
                             (error, name))

    def backup_destination(self, device_name, timestamp):
        name = self.touch_device(device_name)
        safe_stamp = re.sub(r"[^0-9A-Za-z_.-]", "-", timestamp)[:80]
        if not safe_stamp:
            raise ValueError("backup timestamp is empty")
        if not safe_stamp.endswith(".tar.gz"):
            safe_stamp += ".tar.gz"
        return name, self.backups_dir / name / safe_stamp

    def register_backup(self, device_name, path, digest):
        name = self.touch_device(device_name)
        size = path.stat().st_size
        now = utc_now()
        with self.connect() as conn:
            conn.execute(
                """INSERT OR REPLACE INTO backups
                   (device_name, filename, created_at, size, sha256, verified)
                   VALUES (?, ?, ?, ?, ?, 1)""",
                (name, path.name, now, size, digest))
            conn.execute(
                "UPDATE devices SET last_backup=?, last_error=NULL WHERE name=?",
                (now, name))

    def backups(self, device_name=None):
        query = "SELECT * FROM backups"
        params = ()
        if device_name:
            query += " WHERE device_name=?"
            params = (normalize_device_name(device_name),)
        query += " ORDER BY device_name, created_at DESC"
        with self.connect() as conn:
            return [dict(row) for row in conn.execute(query, params)]

    def verify_backups(self):
        results = []
        for item in self.backups():
            path = self.backups_dir / item["device_name"] / item["filename"]
            actual = sha256_file(path) if path.is_file() else None
            ok = actual == item["sha256"]
            with self.connect() as conn:
                conn.execute("UPDATE backups SET verified=? WHERE id=?",
                             (1 if ok else 0, item["id"]))
            results.append((item, ok, actual))
        return results

    def prune_backups(self, keep):
        removed = []
        for device in self.devices():
            rows = self.backups(device["name"])
            verified = [row for row in rows if row["verified"]]
            for row in verified[max(1, int(keep)):]:
                path = self.backups_dir / row["device_name"] / row["filename"]
                if path.exists():
                    path.unlink()
                with self.connect() as conn:
                    conn.execute("DELETE FROM backups WHERE id=?", (row["id"],))
                removed.append(row)
        return removed
