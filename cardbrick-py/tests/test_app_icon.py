"""Application icon assets."""

from cardbrick.app import resolve_icon_path


def test_resolve_icon_path_finds_bundled_icon():
    path = resolve_icon_path()

    assert path is not None
    assert path.endswith("assets/icon.png")
