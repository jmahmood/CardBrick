"""Data-directory resolution and creation."""

import os

from cardbrick.paths import AppPaths, detect_mode, resolve_data_dir


def test_cli_data_dir_wins(tmp_path, monkeypatch):
    monkeypatch.setenv("CARD_BRICK_DATA_DIR", "/should/not/be/used")
    path, source = resolve_data_dir(str(tmp_path / "cli"), mode="desktop")
    assert path == str(tmp_path / "cli")
    assert source == "--data-dir"


def test_env_var_priority(tmp_path, monkeypatch):
    monkeypatch.setenv("CARD_BRICK_DATA_DIR", str(tmp_path / "new"))
    monkeypatch.setenv("CARDBRICK_DATA", str(tmp_path / "legacy"))
    path, source = resolve_data_dir(None, mode="desktop")
    assert path == str(tmp_path / "new")
    assert source == "CARD_BRICK_DATA_DIR"


def test_legacy_env_var_still_honoured(tmp_path, monkeypatch):
    monkeypatch.delenv("CARD_BRICK_DATA_DIR", raising=False)
    monkeypatch.setenv("CARDBRICK_DATA", str(tmp_path / "legacy"))
    path, source = resolve_data_dir(None, mode="desktop")
    assert path == str(tmp_path / "legacy")
    assert source == "CARDBRICK_DATA"


def test_knulli_mode_defaults_to_userdata(monkeypatch):
    monkeypatch.delenv("CARD_BRICK_DATA_DIR", raising=False)
    monkeypatch.delenv("CARDBRICK_DATA", raising=False)
    path, source = resolve_data_dir(None, mode="knulli")
    assert path == "/userdata/saves/cardbrick"
    assert "knulli" in source


def test_desktop_mode_defaults_to_local_data(monkeypatch):
    monkeypatch.delenv("CARD_BRICK_DATA_DIR", raising=False)
    monkeypatch.delenv("CARDBRICK_DATA", raising=False)
    path, _source = resolve_data_dir(None, mode="desktop")
    assert path.endswith(os.path.join("cardbrick-py", "data"))


def test_mode_cli_flag_beats_env(monkeypatch):
    monkeypatch.setenv("CARDBRICK_MODE", "knulli")
    assert detect_mode("desktop") == "desktop"
    assert detect_mode(None) == "knulli"


def test_paths_resolve_from_one_root(tmp_path):
    paths = AppPaths(str(tmp_path / "root"))
    assert paths.db_path == str(tmp_path / "root" / "cardbrick.db")
    assert paths.settings_path == str(tmp_path / "root" / "settings.json")
    assert paths.input_map_path == str(tmp_path / "root" /
                                       "input_mapping.json")
    assert paths.log_path == str(tmp_path / "root" / "logs" /
                                 "cardbrick.log")
    assert paths.media_dir == str(tmp_path / "root" / "media")


def test_missing_directories_created_safely(tmp_path):
    paths = AppPaths(str(tmp_path / "a" / "b" / "c"))
    paths.ensure_directories()
    for path in (paths.data_dir, paths.media_dir,
                 os.path.dirname(paths.log_path), paths.import_dir):
        assert os.path.isdir(path)
    paths.ensure_directories()  # idempotent


def test_describe_lists_every_path(tmp_path):
    info = AppPaths(str(tmp_path), mode="knulli", source="test").describe()
    assert info["mode"] == "knulli"
    for key in ("data_dir", "db_path", "media_dir", "settings_path",
                "input_map_path", "log_path"):
        assert key in info
