#!/usr/bin/env python3
"""Merge CardBrick's EmulationStation metadata into a gamelist.xml.

Takes the repo's assets/gameinfo.xml (desc, image, developer, ...) and a
device/SD-card gamelist.xml, and prints the merged result: CardBrick's
metadata fields are set/refreshed on the existing <game> entry while
ES-owned play stats (playcount, lastplayed, gametime) are left alone.
Creates the file or the entry when either is missing.

Note: ElementTree drops XML comments — ES-generated gamelists don't
have any, so nothing is lost in practice.

Usage: merge_gamelist.py <gameinfo.xml> <gamelist.xml (may be empty/absent)>
"""
import sys
import xml.etree.ElementTree as ET

CARDBRICK_PATHS = ("./CardBrick.sh", "CardBrick.sh")


def main() -> int:
    src_path, dst_path = sys.argv[1], sys.argv[2]
    src = ET.parse(src_path).getroot().find("game")
    if src is None:
        print(f"ERROR: no <game> entry in {src_path}", file=sys.stderr)
        return 1

    try:
        root = ET.parse(dst_path).getroot()
    except (ET.ParseError, FileNotFoundError, OSError):
        root = ET.Element("gameList")

    entry = None
    for game in root.iter("game"):
        if (game.findtext("path") or "").strip() in CARDBRICK_PATHS:
            entry = game
            break
    if entry is None:
        entry = ET.SubElement(root, "game")

    for field in src:
        cur = entry.find(field.tag)
        if cur is None:
            cur = ET.SubElement(entry, field.tag)
        cur.text = field.text

    ET.indent(root, space="\t")
    sys.stdout.write(ET.tostring(root, encoding="unicode",
                                 xml_declaration=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
