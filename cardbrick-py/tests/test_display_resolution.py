"""Handheld display resolution selection."""

from cardbrick.app import _resolve_logical_size


def test_fullscreen_720x480_uses_native_logical_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(720, 480), fullscreen=True
    ) == (720, 480)


def test_fullscreen_640x480_keeps_native_logical_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(640, 480), fullscreen=True
    ) == (640, 480)


def test_windowed_mode_keeps_configured_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(720, 480), fullscreen=False
    ) == (640, 480)


def test_custom_logical_size_is_not_overridden():
    assert _resolve_logical_size(
        (800, 480), display_size=(720, 480), fullscreen=True
    ) == (800, 480)
