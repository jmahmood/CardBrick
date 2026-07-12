#!/usr/bin/env python3
"""Idle-power probe: how much render work does each screen do?

Boots the real app headless (SDL dummy driver), parks it on a screen
with no input for a fixed window, and reports loop wakeups, draws
(``present()`` calls), and process CPU time. These are the Gate A
numbers in PERFORMANCE.md.

Usage:
    .venv/bin/python scripts/perf_probe.py [--seconds N] [--json]
        [--real-display] [--no-gate]

By default the SDL dummy driver is used: all of CardBrick's software
rendering (fills, font rendering, blits, scaling) still runs for real
on the CPU, but the final present-to-display is stubbed, so CPU
figures slightly understate a real screen. ``--real-display`` uses the
platform's actual video driver (opens a window; on the handheld, the
real KMSDRM path) so the flip cost is included. ``--no-gate`` disables
FrameGate so any build can reproduce the pre-fix always-redraw
behavior for an apples-to-apples before/after on the same code.
draws/s and wakeups/s are exact under any driver.
"""

import argparse
import json
import os
import resource
import sys
import tempfile
import threading
import time

# Must be decided before pygame is imported.
if "--real-display" not in sys.argv:
    os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")
if "--with-audio" not in sys.argv:
    os.environ.setdefault("CARDBRICK_AUDIO", "none")

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import pygame  # noqa: E402

from cardbrick.app import CardBrickApp, QuitApp  # noqa: E402
from cardbrick.audio import AudioPlayer  # noqa: E402
from cardbrick.paths import AppPaths  # noqa: E402
from cardbrick.scheduler import ReviewScheduler, iso, now_utc  # noqa: E402
from cardbrick.service import ReviewService  # noqa: E402
from cardbrick.settings import AppSettings  # noqa: E402
from cardbrick.storage import Storage  # noqa: E402

# Length of the initial slice attributed to the entry animation (the
# child-start ticket feed settles well inside this).
ANIMATION_SLICE = 3.0


# Loop wakeups are counted at poll(): every screen loop calls it
# exactly once per iteration, on both the draw and the idle path
# (idle iterations pace with pygame.time.wait, not the Clock).


def seed_cards(storage, count=8):
    scheduler = ReviewScheduler()
    now = now_utc()
    for i in range(count):
        card_id = i + 1
        state = scheduler.initial_state(card_id, now=now)
        storage.upsert_card(card_id, card_id, "Probe", f"front {i}",
                            f"back {i}", "probe", now_iso=iso(now))
        storage.init_review_state(state)
    storage.commit()


def build_app(data_dir):
    paths = AppPaths.resolve(cli_data_dir=data_dir, cli_mode="desktop")
    paths.ensure_directories()
    storage = Storage(paths.db_path)
    seed_cards(storage)
    service = ReviewService(storage, ReviewScheduler())
    audio = AudioPlayer(paths.media_dir)
    settings = AppSettings(paths.settings_path)
    app = CardBrickApp(storage, service, audio, settings, paths=paths,
                       fullscreen=False)
    return app, storage


def measure_screen(app, name, screen_fn, seconds):
    """Run one screen handler for ``seconds``, then QUIT.

    No input arrives except a single probe keypress (K_DOWN) in the
    middle of the idle window, used to measure input latency (Gate A6):
    the time from the event being posted to the next repaint.
    """
    draw_times = []
    poll_times = []
    original_present = app.present
    original_poll = app.poll

    def counting_present():
        draw_times.append(time.monotonic())
        original_present()

    def counting_poll():
        poll_times.append(time.monotonic())
        return original_poll()

    app.present, app.poll = counting_present, counting_poll

    probe_input_at = []

    def post_probe_input():
        probe_input_at.append(time.monotonic())
        # Press AND release: a lone KEYDOWN would leave the semantic
        # marked as held, which pins the idle loop at full rate and
        # would corrupt the steady-state wakeup numbers.
        pygame.event.post(pygame.event.Event(pygame.KEYDOWN,
                                             key=pygame.K_DOWN))
        pygame.event.post(pygame.event.Event(pygame.KEYUP,
                                             key=pygame.K_DOWN))

    quitter = threading.Timer(
        seconds, lambda: pygame.event.post(pygame.event.Event(pygame.QUIT)))
    latency_timer = threading.Timer(
        ANIMATION_SLICE + (seconds - ANIMATION_SLICE) / 2, post_probe_input)
    start_cpu = resource.getrusage(resource.RUSAGE_SELF)
    start = time.monotonic()
    quitter.start()
    latency_timer.start()
    try:
        screen_fn()
    except QuitApp:
        pass
    finally:
        quitter.cancel()
        latency_timer.cancel()
        app.present, app.poll = original_present, original_poll
    end = time.monotonic()
    end_cpu = resource.getrusage(resource.RUSAGE_SELF)

    wall = end - start
    idle_start = start + ANIMATION_SLICE
    # Static idle ends at the probe keypress: the frames drawn in
    # response to it are the screen working, not idle waste.
    idle_end = probe_input_at[0] if probe_input_at else end
    idle_span = max(idle_end - idle_start, 0.001)
    idle_draws = sum(1 for t in draw_times if idle_start <= t < idle_end)
    # Steady-state wakeups: the last 3 s of the window, well past the
    # probe keypress, so the idle ladder has descended again.
    steady_start = end - 3.0
    steady_wakeups = sum(1 for t in poll_times if t >= steady_start)
    # Input latency (A6): probe keypress -> next repaint.
    latency_ms = None
    if probe_input_at:
        after = [t for t in draw_times if t > probe_input_at[0]]
        if after:
            latency_ms = round((after[0] - probe_input_at[0]) * 1000.0, 1)
    # Animation health (Gate A2): the busiest 1 s window. The entry
    # animation is shorter than ANIMATION_SLICE, so an average over the
    # slice would understate the rate the roll actually animates at.
    peak = 0
    j = 0
    for i, t in enumerate(draw_times):
        while draw_times[j] < t - 1.0:
            j += 1
        peak = max(peak, i - j + 1)
    cpu = (end_cpu.ru_utime - start_cpu.ru_utime
           + end_cpu.ru_stime - start_cpu.ru_stime)
    return {
        "screen": name,
        "wall_s": round(wall, 2),
        "draws_total": len(draw_times),
        "peak_draws_per_s": peak,
        "idle_draws_per_s": round(idle_draws / idle_span, 2),
        "steady_wakeups_per_s": round(steady_wakeups / 3.0, 1),
        "input_latency_ms": latency_ms,
        "cpu_s": round(cpu, 3),
        "cpu_pct_of_wall": round(100.0 * cpu / wall, 1),
    }


def measure_floor(seconds=5.0):
    """Process CPU while NO loop runs — pure sleep.

    Anything showing up here is background load (SDL audio callback,
    driver threads), not the UI loop. This is what separates "our loop
    is expensive" from "the process has a fixed cost" on the device.
    """
    start_cpu = resource.getrusage(resource.RUSAGE_SELF)
    start = time.monotonic()
    time.sleep(seconds)
    wall = time.monotonic() - start
    end_cpu = resource.getrusage(resource.RUSAGE_SELF)
    cpu = (end_cpu.ru_utime - start_cpu.ru_utime
           + end_cpu.ru_stime - start_cpu.ru_stime)
    return round(100.0 * cpu / wall, 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, default=13.0,
                        help="window per screen (first %.0fs counted as "
                             "animation, the rest as idle)" % ANIMATION_SLICE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--real-display", action="store_true",
                        help="use the platform's real video driver "
                             "(opens a window) so flip/present cost is "
                             "included")
    parser.add_argument("--no-gate", action="store_true",
                        help="disable FrameGate: reproduces the pre-fix "
                             "always-redraw behavior on the same build")
    parser.add_argument("--with-audio", action="store_true",
                        help="use the real audio backend (default forces "
                             "CARDBRICK_AUDIO=none) so the mixer idle-close "
                             "path is exercised like production")
    args = parser.parse_args()

    if args.no_gate:
        import cardbrick.app as app_module
        app_module.FrameGate.tick = lambda self, dirty=False: True

    with tempfile.TemporaryDirectory(prefix="cardbrick-probe-") as data_dir:
        app, storage = build_app(data_dir)
        floors = {
            "mixer_initialised": bool(pygame.mixer.get_init()
                                      if getattr(pygame, "mixer", None)
                                      else False),
            "audio_open_cpu_pct": measure_floor(),
        }
        screens = [
            ("CHILD_START", app.screen_child_start),
            ("PARENT_MENU", app.screen_parent_menu),
            ("REVIEW", app.screen_review),
        ]
        results = []
        for name, fn in screens:
            results.append(measure_screen(app, name, fn, args.seconds))
        if getattr(pygame, "mixer", None):
            pygame.mixer.quit()
        floors["audio_closed_cpu_pct"] = measure_floor()
        pygame.quit()
        storage.close()

    if args.json:
        print(json.dumps({"screens": results, "floors": floors}, indent=2))
        return

    header = (f"{'screen':<14} {'peak draws/s':>12} {'idle draws/s':>12} "
              f"{'steady wakeups/s':>16} {'latency ms':>10} "
              f"{'cpu s':>8} {'cpu %':>6}")
    print(header)
    print("-" * len(header))
    for r in results:
        print(f"{r['screen']:<14} {r['peak_draws_per_s']:>12} "
              f"{r['idle_draws_per_s']:>12} {r['steady_wakeups_per_s']:>16} "
              f"{str(r['input_latency_ms']):>10} "
              f"{r['cpu_s']:>8} {r['cpu_pct_of_wall']:>6}")
    print()
    print("process floor, no UI loop running (5 s sleep each):")
    print(f"  audio device open  : {floors['audio_open_cpu_pct']:>5}% CPU"
          f"  (mixer initialised: {floors['mixer_initialised']})")
    print(f"  after mixer.quit() : {floors['audio_closed_cpu_pct']:>5}% CPU")


if __name__ == "__main__":
    main()
