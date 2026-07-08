# Pre-Deployment Smoke Checklist — RG35XX SP / Knulli

Work through this in order. Steps 1–6 happen on the PC; the rest on
the device. Every automated step also writes to
`<data>/logs/cardbrick.log`.

## On the PC

- [ ] 1. `python3 -m pytest tests/` in `cardbrick-py/` — all green.
- [ ] 2. `python3 main.py --smoke-test` — `SMOKE TEST PASSED`
      (audio/joystick WARNs are fine on a desktop).
- [ ] 3. `python3 main.py study` — full loop works with keyboard:
      start → reveal (arrows) → rate (1/2/3/4) → summary → quit (Esc).
- [ ] 4. Import the real Spanish deck:
      `python3 main.py --data-dir /tmp/carddata import Deck.apkg`
      — check the import summary for skipped-card reasons; spot-check
      accents (á ñ ¿ ¡) and audio playback in a study session.
- [ ] 5. Configure the profile (name, limits, categories) with
      `python3 main.py --data-dir /tmp/carddata profile ...`.
- [ ] 6. Build the bundle per `package_layout.md` (vendored ARM64
      wheels matching the device's Python minor version).

## On the device (SSH or file manager)

- [ ] 7. Copy the bundle to `/userdata/roms/ports/CardBrickSpanish/`
      and the prepared data folder to `/userdata/saves/cardbrick/`.
- [ ] 8. `sh launch_cardbrick_spanish.sh` once from SSH; then read
      `/userdata/saves/cardbrick/logs/launch.log`:
      smoke test PASSED? correct SDL video driver? joystick detected?
- [ ] 9. If the screen stays black: set `SDL_VIDEODRIVER=kmsdrm` (then
      `mali`, `wayland`, `x11`) in the launch script and retry; the log
      shows what SDL actually used.

## On the device (in the app)

- [ ] 10. Parent Mode → **Controller test & setup**: press every
      button, confirm raw indices appear; run calibration (hold any
      button 3 s), map all buttons, confirm `input_mapping.json`
      exists in the data folder afterwards.
- [ ] 11. Study one real session: audio plays automatically, L1
      replays, D-pad reveals, four face buttons rate, R1 buries,
      START menu can undo/suspend.
- [ ] 12. SELECT finishes/quits; START opens the menu or parent-mode
      actions.
- [ ] 13. Pull the battery (or hard power-off) mid-session, reboot,
      relaunch: the app opens, the interrupted session is closed, and
      already-answered cards did not come back.
- [ ] 14. Reboot the device and confirm settings, mapping, and review
      progress survived.
- [ ] 15. Let the daily cap complete: "All done for today!" screen
      appears rather than an endless backlog.

## Rollback

- Bad migration → replace `cardbrick.db` with the newest
  `cardbrick.db.backup-*` in the same folder.
- Bad settings/mapping → delete `settings.json` /
  `input_mapping.json`; the app regenerates defaults.
