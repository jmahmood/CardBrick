#!/usr/bin/env python3
"""Upload and assign CardBrick content through the family sync server.

This is intentionally an SSH/SCP client rather than another server API.  It
works on macOS and Linux anywhere the OpenSSH client and Click are available.
"""

from __future__ import annotations

import json
import shlex
import subprocess
import sys
from pathlib import Path

import click


DEVICE_NAMES = (
    ("Jawaad", "jawaad"),
    ("Yumiko", "yumiko"),
    ("Maria", "maria"),
    ("Nadia", "nadia"),
    ("Maysa", "maysa"),
    ("Zak", "zak"),
)


def _run(command, label):
    """Run a local command and turn failures into a readable CLI error."""
    try:
        completed = subprocess.run(
            command,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as exc:
        raise click.ClickException(
            "%s could not start: %s" % (label, exc)
        ) from exc
    if completed.returncode:
        detail = (completed.stderr or completed.stdout).strip()
        raise click.ClickException(
            "%s failed (exit %d)%s" % (
                label,
                completed.returncode,
                ": %s" % detail if detail else "",
            )
        )
    return completed.stdout


def _remote_command(admin, server_root, *arguments):
    """Build a shell-safe command for ssh's remote login shell."""
    command = []
    if server_root:
        command.extend(("env", shlex.quote(
            "CARDBRICK_SERVER_ROOT=%s" % server_root)))
    # Leave the default ~/.local path unquoted so the remote shell expands ~.
    command.append(admin)
    command.extend(shlex.quote(str(argument)) for argument in arguments)
    return " ".join(command)


def _remote(host, admin, server_root, *arguments):
    return _run(
        ["ssh", host, _remote_command(admin, server_root, *arguments)],
        "remote server command",
    )


def _parse_rows(output):
    rows = []
    for line in output.splitlines():
        line = line.strip()
        if not line or line == "(none)":
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise click.ClickException(
                "server returned unexpected output: %s" % line
            ) from exc
    return rows


def _choose_devices(requested, assign_all):
    if requested and assign_all:
        raise click.UsageError("use --device or --all, not both")
    if requested:
        return tuple(requested)
    if assign_all:
        return tuple(identity for _, identity in DEVICE_NAMES)

    click.echo("Choose devices to receive this content:")
    for number, (label, identity) in enumerate(DEVICE_NAMES, 1):
        click.echo("  %d) %s (%s)" % (number, label, identity))
    click.echo("  a) All devices")
    answer = click.prompt("Device numbers (comma-separated) or a", default="a")
    if answer.strip().lower() in ("a", "all"):
        return tuple(identity for _, identity in DEVICE_NAMES)

    selected = []
    for value in answer.split(","):
        value = value.strip()
        try:
            number = int(value)
            identity = DEVICE_NAMES[number - 1][1]
        except (ValueError, IndexError) as exc:
            raise click.ClickException(
                "invalid device selection: %s" % value
            ) from exc
        if identity not in selected:
            selected.append(identity)
    if not selected:
        raise click.ClickException("choose at least one device")
    return tuple(selected)


def _newest_matching_content(rows, filename):
    matches = [row for row in rows if row.get("filename") == filename]
    if not matches:
        return None
    return max(matches, key=lambda row: int(row.get("id", 0)))


@click.command()
@click.version_option(version="0.1.0", prog_name="cardbrick-publish")
@click.argument(
    "file",
    required=False,
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
)
@click.option(
    "--device",
    "requested_devices",
    multiple=True,
    metavar="NAME",
    help="Device identity to assign to; repeat for multiple devices.",
)
@click.option("--all", "assign_all", is_flag=True,
              help="Assign to Jawaad, Yumiko, Maria, Nadia, Maysa, and Zak.")
@click.option("--yes", is_flag=True,
              help="Skip the assignment confirmation prompt.")
@click.option("--host", default="jawaad@raspberrypi.local", show_default=True,
              help="SSH destination for the sync server.")
@click.option("--remote-root", default="~/cardbrick-data", show_default=True,
              help="Server data root used by SCP.")
@click.option("--admin", default="~/.local/bin/cardbrick-server",
              show_default=True,
              help="Remote cardbrick-server admin command.")
@click.option(
    "--server-root",
    default=None,
    help="Override CARDBRICK_SERVER_ROOT for the remote admin command "
         "(for example /srv/cardbrick for a system install).",
)
def main(file, requested_devices, assign_all, yes, host, remote_root, admin,
         server_root):
    """Upload FILE, scan it, and assign it to family devices.

    FILE may be an .apkg, .csv, .json, .zip, or .tar.gz content file.  If it
    is omitted, the tool prompts for a path.
    """
    if file is None:
        file = click.prompt(
            "Content file path",
            type=click.Path(exists=True, dir_okay=False, path_type=Path),
        )
    file = file.resolve()
    devices = _choose_devices(requested_devices, assign_all)

    click.echo("File:    %s" % file)
    click.echo("Devices: %s" % ", ".join(devices))
    if not yes and not click.confirm("Upload and assign this content?", default=True):
        click.echo("Cancelled.")
        return

    remote_file = "%s/incoming/%s" % (remote_root.rstrip("/"), file.name)
    click.echo("Uploading %s…" % file.name)
    _run(["scp", str(file), "%s:%s" % (host, remote_file)], "file upload")

    click.echo("Scanning incoming content…")
    _remote(host, admin, server_root, "scan")
    content_rows = _parse_rows(_remote(host, admin, server_root, "content"))
    item = _newest_matching_content(content_rows, file.name)
    if item is None:
        raise click.ClickException(
            "server scanned the file but did not register %s" % file.name
        )

    click.echo("Assigning %s (content #%s)…" % (file.name, item["id"]))
    _remote(host, admin, server_root, "assign", file.name, *devices)
    click.echo("Assigned %s to %s." % (file.name, ", ".join(devices)))


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        click.echo("\nCancelled.", err=True)
        sys.exit(130)
