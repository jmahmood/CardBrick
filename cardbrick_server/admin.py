"""SSH-side administration for the CardBrick sync server."""

import argparse
import json
import os
import sys

try:
    from .store import Store
except ImportError:  # direct execution
    from store import Store


def print_rows(rows):
    if not rows:
        print("(none)")
        return
    for row in rows:
        print(json.dumps(row, sort_keys=True))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=os.environ.get(
        "CARDBRICK_SERVER_ROOT", "/srv/cardbrick"))
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("scan", help="register files placed in incoming/")
    sub.add_parser("devices", help="list devices")
    sub.add_parser("content", help="list registered content")
    assign = sub.add_parser("assign", help="assign content to devices")
    assign.add_argument("content")
    assign.add_argument("devices", nargs="+")
    unassign = sub.add_parser("unassign", help="stop assigning content")
    unassign.add_argument("content")
    unassign.add_argument("devices", nargs="+")
    assignments = sub.add_parser("assignments", help="list assignments")
    assignments.add_argument("device", nargs="?")
    backups = sub.add_parser("backups", help="list verified backups")
    backups.add_argument("device", nargs="?")
    sub.add_parser("verify-backups", help="rehash every backup")
    prune = sub.add_parser("prune-backups", help="keep newest N per device")
    prune.add_argument("--keep", type=int, default=30)
    args = parser.parse_args(argv)
    store = Store(args.root)

    try:
        if args.command == "scan":
            print_rows(store.scan_incoming())
        elif args.command == "devices":
            print_rows(store.devices())
        elif args.command == "content":
            print_rows(store.content())
        elif args.command == "assign":
            item, devices = store.assign(args.content, args.devices)
            print("assigned %s to %s" % (item["filename"],
                                          ", ".join(devices)))
        elif args.command == "unassign":
            store.unassign(args.content, args.devices)
            print("unassigned %s from %s" % (args.content,
                                               ", ".join(args.devices)))
        elif args.command == "assignments":
            print_rows(store.assignments(args.device))
        elif args.command == "backups":
            print_rows(store.backups(args.device))
        elif args.command == "verify-backups":
            failed = False
            for item, ok, actual in store.verify_backups():
                print("%s %s/%s" % ("OK" if ok else "BAD",
                                      item["device_name"], item["filename"]))
                failed = failed or not ok
            return 1 if failed else 0
        elif args.command == "prune-backups":
            print_rows(store.prune_backups(args.keep))
    except (ValueError, OSError) as exc:
        print("ERROR: %s" % exc, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
