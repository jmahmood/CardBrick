# Packaging CardBrick for Knulli / RG35XX SP (pygame-ce runtime)

This is the supported way to put CardBrick on a Knulli handheld: the
app ships unchanged, next to a self-contained ARM64 Python runtime
built with the approach from
[viniciusfs/pygame-ce-runtime](https://github.com/viniciusfs/pygame-ce-runtime)
— a portable Python 3.12 virtual environment with pygame-ce, packed
into a single SquashFS image that the launch script loop-mounts at
start and unmounts at exit. The device needs **no preinstalled Python,
no pip, no network**.

Why a source-built runtime instead of pip wheels: the manylinux
pygame-ce wheel bundles its own SDL2, which knows nothing about the
handheld's display. The runtime builder compiles pygame-ce against
SDL2 *headers* so it links `libSDL2-2.0.so.0` dynamically — on the
device that resolves to Knulli's firmware SDL2, which has the
`mali`/`kmsdrm` video drivers the RG35XX family actually needs. The
Python 3.12.8 + pygame-ce 2.5.6 pair is what upstream validated on an
RG35XX-H running Knulli (same Allwinner H700 family as the RG35XX SP).

Everything below runs from `deploy/knulli/` on a PC.

```
deploy/knulli/
├── Makefile                  # make runtime / package / validate / deploy-*
├── CardBrick.sh              # on-device launch script (goes to roms/ports/)
├── runtime-build/            # Docker ARM64 runtime builder
│   ├── build-config          # PYTHON_VERSION / PYGAME_VERSION pins
│   ├── docker-compose.yml
│   ├── build-runtime.sh      # runs inside the container
│   └── requirements.txt      # extra deps baked into the runtime (fsrs, …)
├── scripts/
│   ├── build_runtime.sh      # host wrapper for the Docker build
│   ├── build_package.sh      # stage dist/ + zip (+ optional deck seeding)
│   ├── validate_package.sh   # pre-deploy checks ([PASS]/[WARN]/[FAIL])
│   ├── deploy_sdcard.sh      # copy to a mounted SD card
│   └── deploy_ssh.sh         # push over SSH (+ on-device smoke test)
├── decks/                    # drop .apkg/.csv here to bake into the package
└── dist/                     # build output (gitignored)
```

## 0. Prerequisites (PC)

- **Docker + docker compose** — only for building the runtime image.
  On an x86 PC, ARM64 emulation must be enabled once:

  ```sh
  docker run --privileged --rm tonistiigi/binfmt --install arm64
  ```

- `zip` (for the SD-card archive), `python3` (validation),
  optionally `squashfs-tools` (deeper validation) and `rsync`
  (faster SD copies). None of these are needed on the device.

No Docker? You can point the packager at any compatible prebuilt
runtime file instead:
`RUNTIME_SQUASHFS=/path/to/pygame-ce_2.5.6_python_3.12.8.squashfs make package`.

## 1. Build the runtime (once, and after version bumps)

```sh
cd deploy/knulli
make runtime
```

This starts a Debian ARM64 container, compiles pygame-ce 2.5.6 from
source, installs CardBrick's runtime deps (`runtime-build/requirements.txt`),
and packs the venv into
`runtime-build/build/pygame-ce_2.5.6_python_3.12.8.squashfs` (plus a
`runtime-manifest.txt` recording exactly what went in). Under
emulation on an x86 host this takes a while — it is a one-time cost;
the artifact is reused by every subsequent package build.

Version pins live in `runtime-build/build-config`. Optional
`runtime-build/pre-install.sh` / `post-install.sh` hooks run inside
the container before/after the venv is assembled.

## 2. Build + validate the package

```sh
make package        # = build_package.sh + validate_package.sh
```

Produces:

- `dist/CardBrick.sh` and `dist/CardBrick/` — exactly what lands in
  `/userdata/roms/ports/` on the device (app code minus tests/caches,
  the runtime squashfs, `VERSION`, `BUILD_INFO`, and a
  `PACKAGE_MANIFEST.sha256` covering every file);
- `dist/CardBrick-knulli-v<version>.zip` — the same tree zipped, for
  manual SD-card installs.

`make validate` re-runs the checks alone: launch-script syntax,
required files, no stray caches, host-side `py_compile` of every
source, squashfs integrity and contents (python3/libpython/pygame/fsrs
present), checksum manifest, zip integrity.

During development, `make package-fast` builds an app-only package
(no runtime inside) — pair it with `deploy_ssh.sh`, which never
re-uploads a runtime the device already has.

## 2b. Bake deck(s) into the package (no on-device import)

To ship a package where the deck(s) are already installed — nothing to
do on the device — either drop files in `decks/` or pass them
explicitly:

```sh
cp ~/Downloads/Spanish101.apkg decks/
make package                                  # auto-loads decks/*.apkg, *.csv

# or, without touching decks/:
make package DECKS="~/Decks/A.apkg ~/Decks/B.apkg"
scripts/build_package.sh --deck ~/Decks/A.apkg --deck ~/Decks/B.apkg
```

What happens: `build_package.sh` runs `main.py import` for each file
**on the PC** (the import code only needs the pure-Python `fsrs`
library, not pygame or the ARM64 runtime, so it runs directly on the
host — a throwaway local venv is created automatically at
`scripts/.deckbuild-venv` the first time, if `fsrs` isn't already
importable), producing a database staged as `CardBrick/seed-data/`.
`CardBrick.sh` copies the seed database into `$CARD_BRICK_DATA_DIR`
when the device has no database yet **or the one it has contains zero
cards** (an empty db is what any earlier launch — even just a smoke
test — leaves behind; it is backed up as `.pre-seed-*` before being
replaced). Seed media stays in `CardBrick/seed-data/media/` and is used
in place as a read-only fallback, so first boot does not need to
duplicate thousands of audio/image files into saves. A database with
any cards in it is never touched, so redeploying an updated package
later can't overwrite review progress that has accumulated on the
device.
Multiple deck files are imported one after another into the same
database (imports are additive, per §5 of `README.md`).

`validate_package.sh` checks the seeded database's integrity and
prints the card count; a 0-card result usually means the `.apkg` needs
re-exporting from Anki with "Support older Anki versions" checked (see
`README.md` §5 for why).

## 3a. Deploy by SD card

Either unzip `dist/CardBrick-knulli-v*.zip` into the card's
`roms/ports/` yourself, or let the script find the card:

```sh
make deploy-sd                    # auto-detect mounted SHARE partition
make deploy-sd SD=/media/$USER/SHARE
scripts/deploy_sdcard.sh /Volumes/SHARE --clean   # wipe old app dir first
```

The script locates `roms/ports/` (SHARE root or bare roms partition),
copies `CardBrick.sh` + `CardBrick/`, merges the description/image
metadata from the repo's `assets/gameinfo.xml` into the card's
`roms/ports/gamelist.xml` (via `scripts/merge_gamelist.py`; ES-owned
play stats are preserved), and runs `sync` before telling you it is
safe to eject. On the device: if the Ports menu doesn't show
CardBrick, Start menu → Games Settings → **Update Gamelists** (or
reboot).

## 3b. Deploy over SSH

Knulli's SSH defaults: user `root`, password `linux`. Find the IP in
the device's main menu (Network). For scripted deploys install a key
once: `ssh-copy-id root@<ip>`.

```sh
make deploy-ssh HOST=root@192.168.1.42
make smoke-ssh  HOST=root@192.168.1.42   # deploy + on-device smoke test
scripts/deploy_ssh.sh --host root@192.168.1.42 --no-runtime  # code only
```

The script uploads with tar-over-ssh (no rsync needed on the device),
skips the runtime upload when the on-device checksum already matches,
and deploys incrementally: it diffs the freshly built
`PACKAGE_MANIFEST.sha256` against the copy left by the previous deploy
and only transfers new/changed files (media-heavy bundles go from
thousands of files to a handful). Pass `--full` to force a whole-bundle
re-upload, e.g. after modifying files on the device by hand. It then
verifies `PACKAGE_MANIFEST.sha256` on the device, merges the
description/image metadata from the repo's `assets/gameinfo.xml` into
the device's `roms/ports/gamelist.xml` (play stats preserved), pokes
EmulationStation to reload the games list, and with `--smoke` runs
the app's own `--smoke-test` on real hardware — one `[PASS]/[FAIL]`
line per subsystem (display, font, DB, scheduler, controller, audio).

## 4. On the device

```
/userdata/roms/ports/CardBrick.sh        # Ports-menu entry
/userdata/roms/ports/CardBrick/
├── cardbrick-py/                        # the app, read-only
├── runtime/pygame-ce_*.squashfs         # mounted at /tmp/cardbrick-runtime
├── seed-data/cardbrick.db, media/       # OPTIONAL, only if built with --deck
├── splash/splash-WxH-BPP.raw.gz         # boot splash framebuffer images
├── VERSION  BUILD_INFO  PACKAGE_MANIFEST.sha256

/userdata/saves/cardbrick/               # ALL mutable data (auto-created)
├── cardbrick.db  settings.json  input_mapping.json  media/
└── logs/launch.log + cardbrick.log      # start here when debugging
```

The launch script first paints a "Loading..." splash straight into
`/dev/fb0` (picked from `splash/` by the panel geometry in
`/sys/class/graphics/fb0`; rendered at build time by
`scripts/make_splash.py`) so the several seconds of runtime mounting and
Python startup aren't a black screen. It then mounts the newest `runtime/*.squashfs`, exports
`PYTHONHOME`/`PYTHONPATH`/`LD_LIBRARY_PATH` into it, keeps `.pyc`
files in the data dir (`PYTHONPYCACHEPREFIX` — the app tree stays
read-only), installs the seed database on first boot if present (§2b),
adds packaged seed media as a read-only fallback, runs a one-time smoke
test on first boot, and unmounts on exit. Run it
manually over SSH for diagnostics:

```sh
bash /userdata/roms/ports/CardBrick.sh --smoke-test
bash /userdata/roms/ports/CardBrick.sh --input-diagnostic
```

## 5. Troubleshooting

| Symptom | Likely cause / fix |
|---|---|
| Black screen, app in log | Video driver. Default is `mali` (upstream-validated on Knulli/H700). Try `SDL_VIDEODRIVER=kmsdrm bash CardBrick.sh`; the winner can be exported permanently by editing `CardBrick.sh`. |
| `could not mount … squashfs` in launch.log | Rare on Knulli (ports run as root). Fallbacks: install nothing — just unsquash on the PC and ship `CardBrick/runtime/extracted/` instead. |
| `ImportError`/ABI errors at pygame init | Runtime/firmware mismatch — rebuild the runtime; don't mix wheels into it. The smoke test catches this at the `pygame init` step. |
| Smoke test WARN on joystick/audio over SSH | Normal when ES owns the controller or audio is busy; verify from the Ports menu launch. |
| Ports menu doesn't list CardBrick | Update Gamelists (Start → Games Settings) or reboot. |
| Wrong/laggy buttons | Parent Mode → Controller test & setup; hold any button ~3 s to force calibration. |
| Baked-in deck(s) didn't appear | Seeding installs when the device db is missing **or has 0 cards**; a db with cards in it is never overwritten. Check `launch.log`: "seed install OK" vs "device database has N card(s) — leaving it alone". If the device db has old/unwanted cards, run `admin reset` on-device (`bash CardBrick.sh admin reset --yes` over SSH) and relaunch — the seed installs on the next start. Also confirm the package actually shipped decks: `validate_package.sh` prints "seed-data DB OK: N card(s)". |
| `validate_package.sh` reports 0 cards in seed-data | The `.apkg` needs "Support older Anki versions" checked at export (`README.md` §5) — the new zstd format is rejected. |

## Relationship to the older vendored-wheels docs

`README.md` / `package_layout.md` / `launch_cardbrick_spanish.sh` in
this folder describe the earlier, runtime-less option (firmware
python3 + `pip install --target vendor`). It still works as a
fallback when Docker is unavailable, but the squashfs runtime above is
the recommended path — it pins the interpreter, survives firmware
Python changes, and matches a combination already proven on this
hardware family. `smoke_test_checklist.md` applies to both.
