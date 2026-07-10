# CardBrick PortMaster package

This directory builds the experimental `cardbrick` PortMaster package for
64-bit ARM Linux handhelds.

The package is intentionally self-contained: it includes an ARM64 Python
3.12 runtime, pygame-ce, FSRS, SQLite support, and the CardBrick-Py app. It
does not require Python or pip on the device and does not use SquashFS.

The runtime is built on Debian Bullseye and is checked so its ELF files do
not require glibc newer than 2.31. Devices with an older glibc, a non-ARM64
architecture, or a display smaller than 640x480 are rejected by the
launcher.

## Build

Docker with ARM64 emulation is required for the runtime build:

```sh
cd deploy/portmaster
make runtime
make package
```

The resulting archive is `dist/cardbrick.zip`.

## Deploy over SSH

After building the package, deploy the staged launcher and runtime directly
to a PortMaster device:

```sh
make deploy-ssh HOST=root@192.168.1.42
```

The script detects the remote PortMaster ports directory from `control.txt`,
streams the package over SSH, and preserves any existing
`cardbrick/conf/` data. During an intentional replacement it keeps the old
launcher and port directory as a rollback copy until the newly installed
manifest verifies. To run the CardBrick smoke test after copying:

```sh
make deploy-ssh HOST=root@192.168.1.42 SMOKE=1
```

Deployment refuses to replace an existing `CardBrick.sh` or `cardbrick/`
installation by default. To install this experimental port beside an
existing CardBrick install, use the parallel name:

```sh
make deploy-ssh HOST=root@192.168.1.42 PARALLEL=1
```

This installs `CardBrickPM.sh` and `cardbrickpm/` with an independent
`cardbrickpm/conf/` directory. Use `REPLACE=1` only when intentionally
updating the matching installation.

Use `--ports PATH` with `scripts/deploy_ssh.sh` if the firmware's PortMaster
directory cannot be detected automatically. The SSH account needs write
access to the ports directory; PortMaster devices commonly use `root`.

To include one or more starter decks:

```sh
make package DECKS="/path/to/deck.apkg"
```

Deck paths may contain spaces. To seed multiple decks, separate their full
paths with `|`:

```sh
make package DECKS="/path/first deck.apkg|/path/second-deck.csv"
```

The deck import runs through the bundled ARM64 runtime in Docker. The
resulting seed database is installed into the user's PortMaster `conf/`
directory on first launch, or when its existing database contains zero cards.
Existing study data is never replaced.

## Validation

```sh
make validate
```

The validator checks the PortMaster layout, launcher, metadata, bundled
runtime, Python imports, ELF architecture, glibc symbol ceiling, checksums,
and optional seed database. It also fails if a SquashFS image is present.

## Hardware support

This is an experimental `aarch64` port. It requires glibc 2.31 or newer and
a 640x480-or-larger display. SDL2 remains dynamically connected to the
firmware so the CFW's display and input drivers remain available. The port
must still be smoke-tested from the PortMaster menu on each target firmware
before claiming support for that device.
