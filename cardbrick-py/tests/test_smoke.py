"""The --smoke-test command itself."""

import os

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

from conftest import seed_card
from cardbrick.paths import AppPaths
from cardbrick.scheduler import ReviewScheduler
from cardbrick.service import ReviewService
from cardbrick.smoke import FAIL, run_smoke_test
from cardbrick.storage import Storage


def _statuses(result):
    return {name: status for status, name, _ in result.checks}


def test_smoke_passes_with_valid_setup(tmp_path):
    paths = AppPaths(str(tmp_path / "data")).ensure_directories()
    storage = Storage(paths.db_path)
    service = ReviewService(storage, ReviewScheduler())
    seed_card(storage, service, 1, tags="greetings")
    storage.ensure_default_profile("Testkid")
    storage.close()

    result = run_smoke_test(paths)
    assert result.ok, result.summary()
    statuses = _statuses(result)
    assert statuses["database open + migrations"] == "PASS"
    assert statuses["cards present"] == "PASS"
    assert statuses["child profile"] == "PASS"
    assert statuses["scheduler queue query"] == "PASS"
    assert "PASSED" in result.summary()


def test_smoke_warns_on_empty_database(tmp_path):
    paths = AppPaths(str(tmp_path / "data")).ensure_directories()
    Storage(paths.db_path).close()

    result = run_smoke_test(paths)
    assert result.ok  # warnings, not failures
    statuses = _statuses(result)
    assert statuses["cards present"] == "WARN"
    assert statuses["child profile"] == "WARN"


def test_smoke_reports_useful_failure_on_broken_db(tmp_path):
    paths = AppPaths(str(tmp_path / "data")).ensure_directories()
    os.makedirs(paths.db_path)  # a directory where the DB should be

    result = run_smoke_test(paths)
    assert not result.ok
    statuses = _statuses(result)
    assert statuses["database open + migrations"] == FAIL
    assert "FAILED" in result.summary()


def test_font_less_pygame_fails_font_check_only(tmp_path, monkeypatch):
    """A pygame built without SDL_ttf (e.g. classic pygame compiled
    from source on Python 3.14) must fail the FONT check with an
    actionable message — and must not mask the checks after it."""
    import cardbrick.app as app_module

    def broken_font_support():
        raise app_module.FontSupportError(
            "This pygame build has no font support")

    monkeypatch.setattr(app_module, "ensure_font_support",
                        broken_font_support)
    paths = AppPaths(str(tmp_path / "data")).ensure_directories()
    Storage(paths.db_path).close()

    result = run_smoke_test(paths)
    statuses = _statuses(result)
    assert statuses["font support"] == FAIL
    detail = next(d for s, n, d in result.checks if n == "font support")
    assert "pygame" in detail and "font" in detail
    # Later hardware checks still ran despite the font failure.
    assert "joystick detection" in statuses
    assert "audio backend" in statuses
    assert not result.ok


def test_ensure_font_support_diagnoses_missing_sdl_ttf(monkeypatch):
    import pygame
    import pytest
    from cardbrick.app import FontSupportError, ensure_font_support

    def exploding_init():
        raise ImportError("cannot import name 'Font' from partially "
                          "initialized module 'pygame.font'")

    monkeypatch.setattr(pygame.font, "init", exploding_init)
    with pytest.raises(FontSupportError, match="pygame-ce"):
        ensure_font_support()


def test_smoke_fails_on_unwritable_data_dir(tmp_path):
    if os.geteuid() == 0:
        # Root ignores permission bits; the check cannot be simulated.
        import pytest
        pytest.skip("running as root")
    root = tmp_path / "locked"
    root.mkdir()
    root.chmod(0o500)
    result = run_smoke_test(AppPaths(str(root / "data")))
    assert not result.ok
