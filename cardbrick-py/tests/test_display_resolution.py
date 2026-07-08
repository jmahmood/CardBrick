"""Handheld display resolution selection."""

from cardbrick.app import _compute_scaled_canvas_size, _resolve_logical_size


def test_fullscreen_720x480_uses_native_logical_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(720, 480), fullscreen=True
    ) == (720, 480)


def test_fullscreen_640x480_keeps_native_logical_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(640, 480), fullscreen=True
    ) == (640, 480)


def test_fullscreen_720x720_uses_native_logical_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(720, 720), fullscreen=True
    ) == (720, 720)


def test_fullscreen_1024x768_keeps_640x480_logical_size():
    # The TrimUI Brick isn't a "native size" panel — its 640x480 canvas
    # is scaled up to fill it instead (see the scaling tests below).
    assert _resolve_logical_size(
        (640, 480), display_size=(1024, 768), fullscreen=True
    ) == (640, 480)


def test_windowed_mode_keeps_configured_size():
    assert _resolve_logical_size(
        (640, 480), display_size=(720, 480), fullscreen=False
    ) == (640, 480)


def test_custom_logical_size_is_not_overridden():
    assert _resolve_logical_size(
        (800, 480), display_size=(720, 480), fullscreen=True
    ) == (800, 480)


def test_integer_scaling_1024x768_display_fills_exactly():
    # 1024x768 is an exact 1.6x of the 640x480 canvas and shares its
    # 4:3 aspect ratio, so it should fill the panel rather than floor
    # to a 1x integer factor and leave it tiny with huge borders.
    assert _compute_scaled_canvas_size(
        (1024, 768), (640, 480), integer_scaling=True
    ) == ((1024, 768), "exact-fill")


def test_integer_scaling_exact_multiple_stays_integer_mode():
    assert _compute_scaled_canvas_size(
        (1280, 960), (640, 480), integer_scaling=True
    ) == ((1280, 960), "integer")


def test_integer_scaling_mismatched_aspect_falls_back_to_floor():
    # 1920x1080 is 16:9, not a clean multiple of the 4:3 canvas, so it
    # should still floor to an integer factor and letterbox.
    assert _compute_scaled_canvas_size(
        (1920, 1080), (640, 480), integer_scaling=True
    ) == ((1280, 960), "integer")


def test_non_integer_scaling_uses_aspect_fit():
    assert _compute_scaled_canvas_size(
        (1024, 768), (640, 480), integer_scaling=False
    ) == ((1024, 768), "aspect-fit")
