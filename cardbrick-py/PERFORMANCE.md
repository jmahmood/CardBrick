# Battery / idle-power work: success criteria

CardBrick drains the battery faster than SNES/GBA emulation on the same
handheld. Diagnosis (2026-07-11): almost every screen repaints the full
display in software at 30 FPS forever, even when nothing changes. Only
the review screen gates drawing behind a dirty flag ([app.py, `_review_loop`]).

This file is the contract for the fix. Each phase must clear its gates
**before** the next phase starts; if a gate fails, we stop and re-diagnose
instead of piling on more code.

## Metrics

Measured by `scripts/perf_probe.py` and by `top` on the device (the number
that actually matters).

Probe modes — CardBrick renders entirely in software (CPU) into pygame
surfaces, so even the default dummy-driver mode executes the full rendering
pipeline except the final present-to-display:

- default: SDL dummy driver. Deterministic, runs headless anywhere (CI).
- `--real-display`: the platform's real video driver, so flip/present cost is
  included. On the handheld this is the actual KMSDRM path — run the probe
  there for the authoritative per-frame numbers.
- `--no-gate`: disables `FrameGate` on the current build, reproducing the
  pre-fix always-redraw behavior. This makes a true A/B possible on the same
  code, same device, same display path:

```sh
python scripts/perf_probe.py --real-display --no-gate   # "before"
python scripts/perf_probe.py --real-display             # "after"
```

### Running the probe on the device (Knulli)

The bundled Python lives in a squashfs image that must be loop-mounted
first; `CardBrick.sh` does all of that, and its tool passthrough runs any
`.py` script on the bundled interpreter. Quit CardBrick first (one display
client at a time), then over SSH:

```sh
cd /userdata/roms/ports
./CardBrick.sh scripts/perf_probe.py --real-display --no-gate   # before
./CardBrick.sh scripts/perf_probe.py --real-display             # after
```

Ship the probe + updated launcher from the dev machine first:
`scripts/push_python.sh` (copies app code and `cardbrick-py/scripts/`), and
once per launcher change:
`scp deploy/knulli/CardBrick.sh root@<device>:/userdata/roms/ports/`.

If the launcher predates the tool passthrough, the manual equivalent is:

```sh
GAMEDIR=/userdata/roms/ports/CardBrick
MNT=/tmp/cardbrick-probe; mkdir -p "$MNT"
mount -o loop,ro "$(ls -1t "$GAMEDIR"/runtime/pygame-ce_*.squashfs | head -n1)" "$MNT"
export PYTHONHOME="$MNT"
PY_LIB="$(ls -d "$MNT"/lib/python3.* | head -n1)"
export PYTHONPATH="$PY_LIB:$PY_LIB/site-packages"
export LD_LIBRARY_PATH="$MNT/lib"
export PYTHONPYCACHEPREFIX=/tmp/cardbrick-pycache
export SDL_VIDEODRIVER=mali SDL_AUDIODRIVER=alsa   # kmsdrm if mali fails
cd "$GAMEDIR/cardbrick-py"
"$MNT/bin/python3" scripts/perf_probe.py --real-display --no-gate
"$MNT/bin/python3" scripts/perf_probe.py --real-display
cd /; umount "$MNT"
```

- **draws/s (idle)** — `present()` calls per second on a screen once its
  paper-roll animation has settled and no input arrives.
- **draws/s (animating)** — `present()` calls per second while the roll is
  feeding (the printer animation must NOT get slower).
- **wakeups/s (idle)** — loop iterations per second while idle (Phase 2 target).
- **CPU s / 10 s idle** — process CPU time consumed over a 10-second idle
  window (user+sys, `getrusage`). The dummy driver makes this an underestimate
  of on-device cost, so it is used as a *relative* gate vs. the baseline table
  below, not an absolute one.

## Gate A — automated, desktop (must pass per phase)

| # | Criterion | Baseline | Target |
|---|-----------|----------|--------|
| A1 | idle draws/s on every measured screen | see table | **≤ 2** (1 Hz heartbeat + slack) |
| A2 | peak draws/s during the child-start ticket feed (busiest 1 s window) | ~28 | **≥ 20** (animation not starved) |
| A3 | CPU s over the 10 s idle window, per screen | see table | **≥ 70% reduction** vs baseline |
| A4 | full test suite (`.venv/bin/python -m pytest`) | green | stays green |
| A5 | *(Phase 2)* steady-state idle wakeups/s (probe: last 3 s of window) | 30 | **≤ 12** (10 Hz band; ≤ 5 in the deep band, unit-tested) |
| A6 | *(Phase 2)* input latency: probe keypress mid-idle → next repaint | ~33 ms | **≤ 150 ms** worst case while idle; full 30 Hz within 2 s of any activity |

> A5/A6 were revised 2026-07-11 (were: ≤ 5 wakeups/s via a blocking
> `pygame.event.wait`, ≤ 50 ms). SDL implements true blocking waits only on
> desktop backends (X11/Wayland/Cocoa/Windows); on the handheld's kmsdrm/mali
> backend `SDL_WaitEventTimeout` falls back to an internal ~1 kHz poll loop
> that costs MORE CPU than ticking at 30 Hz. Phase 2 therefore uses an idle
> polling ladder instead: 30 Hz right after activity, 10 Hz after 2 s idle,
> 4 Hz after 30 s (`IDLE_*` constants in app.py). The latency trade-off is
> bounded: ≤ ~100 ms once the screen has sat static for 2 s+, imperceptible
> next to the paper-feed easing; instant feel is preserved while interacting.

## Gate B — on-device, run by hand (defines "it actually worked")

Baseline B numbers must be captured **before** merging Phase 1, using the same
commands, so before/after is apples-to-apples.

- **B1**: `top` CPU% of the CardBrick python process, sitting idle on the
  child-start screen for 1 min: target **< 5%** (capture baseline first;
  expected 30–100% today).
- **B2**: CardBrick CPU% during an active review sprint **≤** RetroArch
  running an SNES game on the same device.
- **B3**: the user-facing check — a normal study week no longer stands out
  against GBA/SNES sessions in battery drain. Subjective, but B1/B2 predict it.

On the device (ssh in, CardBrick running):

```sh
top -b -n 3 -d 5 | grep -i -E "python|cardbrick"
```

## Decision rules

1. Gate A passes but B1 barely moves → **stop writing code**. The model
   ("continuous software rendering is the drain") is wrong or incomplete;
   re-profile on the device before Phase 2/3.
2. A2 fails (animation starves) → the frame gate is wrong, fix before merging;
   the printer feel is a product feature, not a casualty.
3. A6 fails → the event-wait design is wrong; revert Phase 2, keep Phase 1.

## Phases

1. **Dirty-flag rendering everywhere** — generalize the review screen's
   `needs_draw` + 1 Hz heartbeat to all screen loops (`FrameGate`).
2. **Idle polling ladder** — free-running `clock.tick(FPS)` on the no-draw
   path becomes `_idle_tick`: 30 Hz → 10 Hz (2 s idle) → 4 Hz (30 s idle),
   full rate pinned while any button is held (hold-to-undo sampling). A
   blocking `event.wait` was rejected — see the A5/A6 revision note.
3. **Cheaper animated frames** — cache `_paper` background and footer surfaces.
   Only worth doing if B-numbers say animation frames still matter.

**GPU offload: rejected (2026-07-11).** The device log confirms
`scaling: none (display matches logical size)` in the real fullscreen app
(2026-07-08 boot), so per drawn frame the display path is a 1:1 blit + flip —
there is no software scale for a GPU renderer (`pygame.SCALED` /
`_sdl2.video`) to absorb, and post-Phase-1/2 the app idles at ~1 frame/s
anyway. Waking the GPU power domain for that would cost battery, not save
it. Revisit only if a future device logs `aspect-fit`/`integer` scaling.

## Results log

| Date | Change | child-start idle draws/s | parent-menu idle draws/s | review idle draws/s | child-start CPU s (13 s window) | wakeups/s |
|------|--------|--------------------------|--------------------------|---------------------|--------------------------------|-----------|
| 2026-07-11 | baseline (desktop dummy driver) | 27.5 | 27.3 | 0.9 | 0.32 | 27.5 |
| 2026-07-11 | Phase 1: `FrameGate` on all screen loops | **0.9** | **0.9** | 0.9 | **0.085** (−73%) | 27.4 (Phase 2) |
| 2026-07-11 | Phase 2: idle polling ladder (desktop dummy) | 1.0 | 1.0 | 1.0 | 0.055 | **10.7** steady; latency 7–50 ms |

**On-device A/B, RG35XX-family (Knulli, mali, `--real-display`), Phase 1
build:** gate off vs on — child-start idle CPU **60.0% → 14.7%**,
parent-menu **62.2% → 12.1%**, review 13.2% → 13.3% (unchanged, as
expected: it was already gated). The remaining ~13% floor is the 30 Hz
poll loop itself — REVIEW showed the same floor before and after, which is
what Phase 2's ladder attacks (predicted idle: ~4–5% in the 10 Hz band,
~2% deep). Gate B1 (< 5%) is judged after Phase 2 is deployed.

Device note: the probe runs windowed, and the OS forces the window to
720×480 while the logical canvas is 640×480 — so probe frames include a
software scale the real `--fullscreen` app skips (`_resolve_logical_size`
snaps to the native panel size; check the app log's `scaling:` line).
Probe CPU therefore slightly overstates per-frame cost; deltas are valid.

**On-device Phase 2 (2026-07-11): A-gates pass, B1 NOT met — decision rule
1 triggered.** Steady wakeups fell 30 → 11/s and latency was fine, but idle
CPU stayed flat (14.9 / 12.3 / 15.8%). Conclusion: the ~13% floor is NOT
the UI loop — cutting its wakeups 3× changed nothing. A constant background
cost owns it. Prime suspect: the SDL/ALSA audio path — `pygame.init()`
opens the audio device and runs a mixing callback (~86/s at defaults) for
the whole process lifetime, even in silence. The desktop probe never saw
this because it defaults `SDL_AUDIODRIVER=dummy`, while the device launcher
exports `alsa`. The probe now measures a "process floor" (CPU over a pure
5 s sleep, audio device open vs after `mixer.quit()`) to attribute the
floor conclusively before any fix is written.

**Floor attributed (2026-07-11, on-device):** process floor with the audio
device open **8.6%** CPU; after `mixer.quit()` **0.0%**. The silent ALSA
mixing callback owned most of the ~13% idle floor; the remainder matches
the loop's predicted ~4–6%. Fix shipped in `audio.py` + `app.py`:

- Mixer opens with a 2048-sample buffer (`MIXER_SETTINGS`) — ~4× fewer
  mixing callbacks while audio is actually playing (~46 ms effect latency,
  masked by the paper-feed easing). `pygame.mixer.pre_init` gets the same
  settings so the implicit `pygame.init()` open is equally cheap.
- The mixer backend releases the audio device after 10 s of silence
  (`AUDIO_IDLE_CLOSE_S`, called from the idle path) and reopens
  transparently on the next play; non-mixer backends never keep a device
  open at all. Unit-tested in `tests/test_audio.py`.

Validate on-device with `--with-audio` (uses the production mixer backend
instead of the probe's default `CARDBRICK_AUDIO=none`):

```sh
./CardBrick.sh scripts/perf_probe.py --real-display --with-audio
```

Expected: floor rows ≈ 0% both, screen idle CPU ~4–6% (loop cost only,
Gate B1 territory), ~2–3% once the deep idle band engages.

**On-device `--with-audio` result (2026-07-11):** parent-menu **3.1%** —
steady-state idle with the mixer closed; that is the B1 number and it
passes. Child-start 13.2% and review 14.0% are probe-window artifacts of
correct behavior: both screens play paper-feed SFX during their entry
animation, which opens the mixer, and `AUDIO_IDLE_CLOSE_S` (10 s) keeps it
open for most of the 13 s window; they converge to parent-menu's ~3% once
the device has been quiet 10 s (the close/reopen cycle is unit-tested).
Implication: the larger buffer did NOT visibly reduce the open-device cost
(13.2 − 3.1 ≈ 10%, same as the 8.6% floor before) — the device's ALSA
stack likely clamps the period size or pays a continuous resample. Known
remaining cost, accepted: it applies only while sounds have played in the
last 10 s, i.e. during active use, where 14% still beats an SNES core by a
wide margin (B2 pass). Optional future experiment: open at 48 kHz to skip
a possible 44.1→48 kHz resample.

**Resample confirmed and fixed (2026-07-11):** `/proc/asound` hw_params on
the device shows the DAC native at **48000 Hz** (S32, period 1024 — ALSA
clamps periods, explaining why the bigger buffer didn't help). We were
opening at 44100, forcing a continuous software resample. `MIXER_SETTINGS`
now opens at 48000; SDL_mixer converts media files once at load instead.
Expected: open-device cost (the ~10% seen while sounds are active) drops
substantially; re-measure with `--real-display --with-audio` and compare
child-start/review against parent-menu's ~3%.

**48 kHz confirmed on-device (2026-07-11, final numbers):** open-but-silent
audio device **8.6% → 2.8%** CPU (the 44.1→48 kHz resample was the bulk);
screens with sounds recently played 13–14% → **10.0%** window average;
parent-menu steady idle **3.2%**.

## Verdict — all measurable gates met

- Idle (the battery complaint): **60% → ~3% CPU**, a ~20× cut. B1 met.
- Active with audio in use: ~10% window avg (open device now 2.8%),
  converging to ~3% after 10 s of silence. Far below emulator load. B2 met.
- What fixed it, in order of impact: dirty-flag rendering (Phase 1, 60→13%),
  48 kHz mixer + idle audio-device close (13→3% idle), idle polling ladder
  (loop floor ~4–6% → ~3%, plus deep-idle headroom).
- Remaining: B3 — a normal study week against the GBA/SNES baseline.

Real-display A/B on the dev Mac (`--real-display`, same build, gate off vs
on): parent-menu idle 29.5 → 1.0 draws/s, CPU 0.92 s → 0.15 s per 13 s window
(−84%); child-start CPU 0.59 s → 0.28 s (remainder is the ticket-feed
animation, which is meant to stay). Direction and magnitude match the
dummy-driver numbers; the device A/B (same commands, on the handheld) is
still the authoritative one.

Phase 1 notes: peak draws/s during the ticket feed stayed at 28 (A2 pass);
all 321 tests pass (A4). Screens intentionally left free-running: REVIEW
(already had its own gate), the finite slide transitions (always animating),
and INPUT_DIAG/CALIBRATE (live raw-event viewers, rarely open — candidates
for Phase 2 cleanup). `screen_parent_sync` also stopped re-reading its sync
state file from disk every frame.
