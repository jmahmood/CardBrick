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
