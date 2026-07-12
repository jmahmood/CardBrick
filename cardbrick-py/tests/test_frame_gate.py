"""FrameGate + the idle polling ladder (PERFORMANCE.md Phases 1-2).

These guard the battery fix: screens must not repaint while static
(beyond the ~1 Hz heartbeat) and idle loops must descend to slower
polling instead of spinning at 30 FPS forever.
"""

import os

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

import pygame  # noqa: E402

from cardbrick.app import (  # noqa: E402
    FPS,
    IDLE_DEEP_AFTER_S,
    IDLE_DEEP_WAIT_MS,
    IDLE_SLOW_AFTER_S,
    IDLE_WAIT_MS,
    CardBrickApp,
    FrameGate,
)


def test_first_tick_always_draws():
    assert FrameGate().tick(dirty=False)


def test_static_screen_draws_only_on_heartbeat(monkeypatch):
    now = [100.0]
    monkeypatch.setattr("cardbrick.app.time.monotonic", lambda: now[0])
    gate = FrameGate(heartbeat_s=1.0)
    assert gate.tick()  # first draw
    start = now[0]
    for i in range(1, 20):  # static frames just short of the heartbeat
        now[0] = start + i * 0.05
        assert not gate.tick()
    now[0] = start + 1.05
    assert gate.tick()  # heartbeat lands at ~1 s


def test_dirty_always_draws_and_resets_idle(monkeypatch):
    now = [100.0]
    monkeypatch.setattr("cardbrick.app.time.monotonic", lambda: now[0])
    gate = FrameGate()
    gate.tick()
    now[0] += 10.0
    assert gate.idle_for() == 10.0
    assert gate.tick(dirty=True)
    assert gate.idle_for() == 0.0


class _RecordingClock:
    def __init__(self):
        self.ticks = []

    def tick(self, fps):
        self.ticks.append(fps)


class _SilentAudio:
    def __init__(self):
        self.release_calls = 0

    def release_if_idle(self, idle_s=None):
        self.release_calls += 1


def _bare_app():
    app = object.__new__(CardBrickApp)
    app._held_inputs = {}
    app.clock = _RecordingClock()
    app.audio = _SilentAudio()
    return app


def test_idle_ladder_descends(monkeypatch):
    now = [100.0]
    monkeypatch.setattr("cardbrick.app.time.monotonic", lambda: now[0])
    waits = []
    monkeypatch.setattr(pygame.time, "wait", waits.append)
    app = _bare_app()
    gate = FrameGate()
    gate.tick(dirty=True)  # activity at t=0

    app._idle_tick(gate)  # fresh activity: full rate
    assert app.clock.ticks == [FPS] and not waits

    now[0] += IDLE_SLOW_AFTER_S + 0.1
    app._idle_tick(gate)
    assert waits == [IDLE_WAIT_MS]

    now[0] += IDLE_DEEP_AFTER_S
    app._idle_tick(gate)
    assert waits == [IDLE_WAIT_MS, IDLE_DEEP_WAIT_MS]

    # New input climbs straight back to full rate.
    gate.tick(dirty=True)
    app._idle_tick(gate)
    assert app.clock.ticks == [FPS, FPS]


def test_held_button_pins_full_rate(monkeypatch):
    """Hold-to-undo needs 30 Hz sampling while a button is down."""
    now = [100.0]
    monkeypatch.setattr("cardbrick.app.time.monotonic", lambda: now[0])
    waits = []
    monkeypatch.setattr(pygame.time, "wait", waits.append)
    app = _bare_app()
    gate = FrameGate()
    gate.tick(dirty=True)
    now[0] += IDLE_DEEP_AFTER_S + 1.0

    app._held_inputs[("key", pygame.K_UP)] = "dpad_up"
    app._idle_tick(gate)
    assert app.clock.ticks == [FPS] and not waits
