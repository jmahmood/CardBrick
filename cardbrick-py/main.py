#!/usr/bin/env python3
"""CardBrick-Py entry point.

Usage:
    python main.py import <deck.apkg>    Import an Anki package
    python main.py review [--deck NAME]  Start the pygame reviewer
    python main.py decks                 List decks and due counts
"""

import argparse
import os
import sys

from cardbrick.importer import ApkgError, import_apkg
from cardbrick.scheduler import ReviewScheduler, iso, now_utc
from cardbrick.storage import Storage

DEFAULT_DATA_DIR = os.environ.get(
    "CARDBRICK_DATA",
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "data"))


def main(argv=None):
    parser = argparse.ArgumentParser(prog="cardbrick", description=__doc__)
    parser.add_argument("--data-dir", default=DEFAULT_DATA_DIR,
                        help="Where the database and media live "
                             "(default: ./data)")
    sub = parser.add_subparsers(dest="command")

    p_import = sub.add_parser("import", help="Import an .apkg file")
    p_import.add_argument("apkg", help="Path to the .apkg file")

    p_review = sub.add_parser("review", help="Start the reviewer (default)")
    p_review.add_argument("--deck", help="Review only this deck")
    p_review.add_argument("--fullscreen", action="store_true",
                          help="Run fullscreen (use on the handheld)")

    sub.add_parser("decks", help="List decks and due counts")

    args = parser.parse_args(argv)
    if args.command is None:
        args.command, args.deck, args.fullscreen = "review", None, False

    db_path = os.path.join(args.data_dir, "cardbrick.db")
    media_dir = os.path.join(args.data_dir, "media")
    storage = Storage(db_path)
    scheduler = ReviewScheduler()

    try:
        if args.command == "import":
            stats = import_apkg(args.apkg, storage, scheduler, media_dir)
            print(stats.summary())
        elif args.command == "decks":
            rows = storage.decks(iso(now_utc()))
            if not rows:
                print("No decks imported yet.")
            for row in rows:
                print(f"{row['name']}: {row['due']} due / "
                      f"{row['total']} total")
        else:
            from cardbrick.audio import AudioPlayer
            from cardbrick.ui import ReviewApp
            audio = AudioPlayer(media_dir)
            app = ReviewApp(storage, scheduler, audio,
                            deck=args.deck, fullscreen=args.fullscreen)
            app.run()
    except ApkgError as exc:
        print(f"Import failed: {exc}", file=sys.stderr)
        return 1
    finally:
        storage.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
