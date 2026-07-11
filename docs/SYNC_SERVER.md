# CardBrick family sync server

The replacement sync service is intentionally small: one standard-library
Python HTTP process, one SQLite metadata file, and ordinary files under a
single storage directory. Devices make outbound requests; the server never
connects back to a device.

## Server installation

On Debian or Raspberry Pi OS, copy the repository to the server and run:

```sh
sudo deploy/server/install.sh
```

This installs the server code under `/opt/cardbrick`, stores all mutable data
under `/srv/cardbrick`, installs `cardbrick-server`, and starts the systemd
service on port 6429. There are no Python package dependencies.

If the prepared SSH account does not have passwordless sudo, install it as a
user service instead:

```sh
loginctl enable-linger "$USER"
deploy/server/install-user.sh
```

The rootless layout uses `~/.local/share/cardbrick-sync` for code and
`~/cardbrick-data` for the SQLite file, content, and backups.

The prepared family server uses `http://10.0.0.30:6429`, which is the
client's default. The fixed address avoids depending on mDNS support in the
handheld firmware.

For development, run without installing:

```sh
python3 -m cardbrick_server.server --root /tmp/cardbrick-server
python3 -m cardbrick_server.admin --root /tmp/cardbrick-server devices
```

## Storage layout

```text
/srv/cardbrick/
├── cardbrick-sync.db
├── incoming/              files copied here by the parent
├── content/               registered content files
├── devices/DEVICE/assigned/
├── backups/DEVICE/        ordinary .tar.gz backups
└── tmp/                    incomplete uploads only
```

The storage directory can live directly on a hard disk or mounted RAID. Back
it up with the same filesystem backup tools used for the rest of the server.

## Adding and assigning content

For a friendlier Mac/Linux workflow, install the small Click-based publisher
CLI once on your computer:

```sh
python3 -m venv ~/.venvs/cardbrick-tools
~/.venvs/cardbrick-tools/bin/python -m pip install ./tools
```

From the repository, run it with no arguments to choose a file and devices
interactively:

```sh
~/.venvs/cardbrick-tools/bin/cardbrick-publish
```

It uploads the file, runs the server scan, finds the newest registered copy
of that filename, and assigns it to the selected devices. For repeatable
commands, provide the file and device flags:

```sh
~/.venvs/cardbrick-tools/bin/cardbrick-publish \
  "Spanish.apkg" --device maysa --device zak
```

To assign it to all six handheld identities:

```sh
~/.venvs/cardbrick-tools/bin/cardbrick-publish \
  "Spanish.apkg" --all --yes
```

The defaults target the prepared rootless server as
`jawaad@raspberrypi.local`, using `~/cardbrick-data` and
`~/.local/bin/cardbrick-server`. For a system-wide installation, use its
admin command and data root explicitly:

```sh
~/.venvs/cardbrick-tools/bin/cardbrick-publish "Spanish.apkg" --all \
  --admin cardbrick-server \
  --remote-root /srv/cardbrick \
  --server-root /srv/cardbrick
```

Copy a supported file into the incoming directory, scan it, then assign it:

```sh
scp Spanish.apkg cardbrick:/srv/cardbrick/incoming/
ssh cardbrick
sudo -u cardbrick cardbrick-server scan
sudo -u cardbrick cardbrick-server content
sudo -u cardbrick cardbrick-server assign Spanish.apkg maysa zak
```

Supported direct files are `.apkg`, pattern-pack `.json`, and vocab `.csv`.
For a CSV with media, use a normal `.zip` or `.tar.gz`:

```text
vocabulary.zip
├── manifest.json
├── vocabulary.csv
└── media/
    ├── hola.mp3
    └── gato.jpg
```

`manifest.json` is plain JSON:

```json
{
  "entrypoint": "vocabulary.csv",
  "media_dir": "media",
  "deck": "Vocabulario"
}
```

Useful administration commands:

```sh
cardbrick-server devices
cardbrick-server content
cardbrick-server assignments
cardbrick-server assignments maya
cardbrick-server unassign Spanish.apkg maya
cardbrick-server backups maya
cardbrick-server verify-backups
cardbrick-server prune-backups --keep 30
```

Unassigning stops future delivery; it deliberately does not remotely delete
cards or review history from a device.

## Device setup

On the handheld, open **Parent Mode → Server sync → Device name** and
choose Jawaad, Yumiko, Maria, Nadia, Maysa, or Zak. The app stores the
lowercase form as the server identity. Then choose **Sync now**; the same
screen shows the last successful sync and backup, installed package count,
and the most recent error. **Force backup** uploads a fresh archive without
fetching assigned content.

The equivalent CLI setup remains available for troubleshooting or custom
device identities:

```sh
CardBrick.sh sync --name maysa
```

For a static server address:

```sh
CardBrick.sh sync --name maysa --server http://192.168.1.20:6429
```

The configuration is stored in the device data directory as `sync.json`.
After configuration, the Knulli, MinUI, and PortMaster launchers:

1. Sync before opening the study app.
2. Back up after the study app exits.
3. Ignore network failures so offline study always works.

Manual commands are also available:

```sh
CardBrick.sh sync
CardBrick.sh sync --backup-only --force-backup
CardBrick.sh sync --content-only
CardBrick.sh sync-status
```

## Backups and restore

Each server backup is an ordinary `.tar.gz` containing `cardbrick/` with the
consistent SQLite snapshot, settings, controller mapping, media, imports,
logs, and a readable `manifest.json`. The server writes an upload to `tmp/`,
checks its SHA-256, fsyncs it, and only then atomically publishes it under
`backups/DEVICE/`.

To restore, copy the selected archive to the device and run:

```sh
CardBrick.sh sync-restore /path/to/maya-20260710T120000Z.tar.gz
```

Restore verifies the SQLite database before replacing data and keeps the
previous data directory as a timestamped rollback. The current device name
and server address survive a restore, which permits restoring a learner onto
replacement hardware.

## Operational checks

```sh
curl http://10.0.0.30:6429/health
systemctl status cardbrick-sync-server
journalctl -u cardbrick-sync-server -f
sudo -u cardbrick cardbrick-server verify-backups
```

The server has no login or enrollment layer. It is intended only for the
trusted family LAN described above.
