import os

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

import cardbrick.app as app_module  # noqa: E402
from cardbrick.app import (CardBrickApp, SYNC_DEVICE_NAMES,  # noqa: E402
                           _friendly_sync_name)
from cardbrick.paths import AppPaths  # noqa: E402
from cardbrick.sync import SyncState  # noqa: E402


class _Clock:
    def tick(self, _fps):
        pass


def _bare_app(tmp_path, actions):
    app = object.__new__(CardBrickApp)
    app.paths = AppPaths(str(tmp_path))
    app.paths.ensure_directories()
    app.language = "en"
    iterator = iter(actions)
    app.poll = lambda: next(iterator)
    app.present = lambda: None
    app.clock = _Clock()
    app._draw_parent_sync = lambda *_args: None
    app._draw_parent_sync_name = lambda *_args: None
    app._draw_parent_sync_restore = lambda *_args: None
    app._draw_sync_progress = lambda *_args: None
    return app


def test_fixed_device_names_have_expected_server_identities():
    assert SYNC_DEVICE_NAMES == (
        ("Jawaad", "jawaad"),
        ("Yumiko", "yumiko"),
        ("Maria", "maria"),
        ("Nadia", "nadia"),
        ("Maysa", "maysa"),
        ("Zak", "zak"),
    )
    for label, identity in SYNC_DEVICE_NAMES:
        assert _friendly_sync_name(identity) == label


def test_name_screen_replaces_custom_name(tmp_path):
    state = SyncState(str(tmp_path))
    state.configure(name="custom-device")
    app = _bare_app(tmp_path, ["dpad_down", "east_button"])

    assert app.screen_parent_sync_name() == "PARENT_SYNC"
    assert SyncState(str(tmp_path)).data["device_name"] == "yumiko"


def test_name_screen_cancel_preserves_name(tmp_path):
    state = SyncState(str(tmp_path))
    state.configure(name="maria")
    app = _bare_app(tmp_path, ["south_button"])

    assert app.screen_parent_sync_name() == "PARENT_SYNC"
    assert SyncState(str(tmp_path)).data["device_name"] == "maria"


def test_name_screen_starts_on_current_fixed_name(tmp_path):
    state = SyncState(str(tmp_path))
    state.configure(name="nadia")
    app = _bare_app(tmp_path, [None, "south_button"])
    drawn = []
    app._draw_parent_sync_name = lambda *args: drawn.append(args)

    assert app.screen_parent_sync_name() == "PARENT_SYNC"
    assert drawn[0][1] == 3
    assert drawn[0][2] == "nadia"


def test_sync_action_routes_unconfigured_device_to_name_screen(tmp_path):
    app = _bare_app(tmp_path, ["dpad_down", "east_button"])

    assert app.screen_parent_sync() == "PARENT_SYNC_NAME"
    assert SyncState(str(tmp_path)).data["device_name"] is None


def test_restore_action_routes_unconfigured_device_to_name_screen(tmp_path):
    app = _bare_app(
        tmp_path,
        ["dpad_down", "dpad_down", "dpad_down", "east_button"],
    )

    assert app.screen_parent_sync() == "PARENT_SYNC_NAME"


def test_restore_latest_requires_two_confirmations(tmp_path, monkeypatch):
    state = SyncState(str(tmp_path))
    state.configure(name="maysa")
    backup = {
        "filename": "maysa-latest.tar.gz",
        "created_at": "2026-07-11T12:00:00+00:00",
        "size": 1234,
        "sha256": "a" * 64,
    }
    monkeypatch.setattr(app_module, "list_backups", lambda _data_dir: [backup])
    monkeypatch.setattr(
        app_module,
        "download_backup",
        lambda _paths, selected, progress=None:
        dict(selected, archive=str(tmp_path / selected["filename"])),
    )
    app = _bare_app(tmp_path, ["east_button", "east_button"])

    assert app.screen_parent_sync_restore() == "QUIT"
    assert app.pending_restore_archive == str(tmp_path / backup["filename"])
