import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
PORT = ROOT / "deploy" / "portmaster"
SOURCE = PORT / "port" / "cardbrick"
LAUNCHER = SOURCE / "CardBrick.sh"
DEPLOYER = PORT / "scripts" / "deploy_ssh.sh"


def test_metadata_is_aarch64_only_and_ready_to_run():
    data = json.loads((SOURCE / "cardbrick.port.json").read_text())
    assert data["version"] == 2
    assert data["name"] == "cardbrick.zip"
    assert data["attr"]["arch"] == ["aarch64"]
    assert data["attr"]["rtr"] is True
    assert data["attr"]["runtime"] is None


def test_launcher_uses_portmaster_contract():
    text = LAUNCHER.read_text()
    for required in (
        'source "$controlfolder/control.txt"',
        "get_controls",
        "DEVICE_ARCH",
        "CFW_GLIBC",
        "$ESUDO",
        "$GPTOKEYB",
        "$sdl_controllerconfig",
        "pm_platform_helper",
        "pm_finish",
        'CARD_BRICK_DATA_DIR="$CONFDIR"',
        'glibc version could not be determined',
        'display width could not be determined',
        'PortMaster control.txt could not be found',
        "-path '*[Pp]ort[Mm]aster*'",
        "/userdata/system/.local/share/PortMaster",
        "PortMaster control=",
        "PortMaster directory=",
        "seed deck installed",
        "CARDBRICK_EXTRA_MEDIA_DIRS",
        'PORTMASTER_CONTROL_DIR="$controlfolder"',
        'controlfolder="$PORTMASTER_CONTROL_DIR"',
        'source "$PORTMASTER_CONTROL_DIR/device_info.txt"',
        'uname -m',
        'getconf GNU_LIBC_VERSION',
    ):
        assert required in text
    assert text.index("get_controls") < text.index("set -u")


def test_launcher_does_not_force_cfw_specific_sdl_or_paths():
    text = LAUNCHER.read_text().lower()
    assert "/userdata/roms/cardbrick" not in text
    assert "/mnt/mmc" not in text
    assert "/opt/muos" not in text
    assert "export sdl_videodriver=" not in text
    assert "export sdl_audiodriver=" not in text
    assert "command -v pm_message" in text
    assert "command -v pm_finish" in text
    assert "command -v pm_platform_helper" in text


def test_portmaster_package_has_no_squashfs_dependency():
    for path in PORT.rglob("*"):
        assert path.suffix != ".squashfs"
    assert "squashfs" not in LAUNCHER.read_text().lower()


def test_runtime_build_is_bullseye_and_glibc_checked():
    compose = (PORT / "runtime-build" / "docker-compose.yml").read_text()
    builder = (PORT / "runtime-build" / "build-runtime.sh").read_text()
    assert "debian:bullseye" in compose
    assert "--enable-shared" in builder
    assert "GLIBC_2" in builder
    assert "-gt 31" in builder
    assert "PYTHON_SHA256" in builder
    assert "PYGAME_COMMIT" in builder
    assert "Runtime dependency closure" in builder
    assert "libstdc++.so.6" in builder


def test_package_builder_flattens_archive_items():
    builder = (PORT / "scripts" / "build_package.sh").read_text()
    makefile = (PORT / "Makefile").read_text()
    assert 'zip -qr "$PACKAGE_ZIP"' in builder
    assert "CardBrick.sh cardbrick cardbrick.port.json" in builder
    assert "Materialize runtime library SONAME links" in builder
    assert "xattr -cr" in builder
    assert 'find "./$launcher" "./$port_dir"' in builder
    assert "CARDBRICK_DECKS" in builder
    assert "docker run --rm --platform linux/arm64" in builder
    assert "-v \"$file:/deck/input:ro\"" in builder
    assert 'CARDBRICK_DECKS="$(DECKS)"' in makefile


def test_ssh_deployer_detects_ports_and_preserves_conf():
    text = DEPLOYER.read_text()
    assert 'grep -m 1 -E' in text
    assert 'PortMaster control.txt does not define a ports directory' in text
    assert '"$ports/$port_name/conf"' in text
    assert 'COPYFILE_DISABLE=1 tar --format=ustar --no-xattrs -cf -' in text
    assert "--smoke" in text
    assert "--parallel" in text
    assert "--replace" in text
    assert "restoring the previous CardBrick install" in text
    assert "write_manifest \"$TEMP_TREE\"" in text
    assert 'find "./$launcher" "./$port_dir"' in text
    assert "pass --ports /path/to/ports" in text
    assert "COPYFILE_DISABLE=1 tar" in text
    assert "tar -xof -" in text
    assert text.index('/userdata/roms/ports/PortMaster') < \
        text.index('/userdata/system/.local/share/PortMaster') < \
        text.index('find / -maxdepth 6')
    assert 'directory="userdata/roms"' in text
    assert 'echo "PortMaster control: $controlfolder/control.txt" >&2' in text
