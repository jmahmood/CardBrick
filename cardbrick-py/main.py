#!/usr/bin/env python3
"""CardBrick-Py entry point.

Usage:
    python main.py study [--fullscreen]     CardBrick-style study appliance
                                            (child/parent flow; the default)
    python main.py import <deck.apkg>       Import an Anki package
    python main.py review [--deck NAME]     Legacy prototype reviewer
    python main.py decks                    List decks and due counts
    python main.py profile [...]            View/edit the child profile
"""

import argparse
import json
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

    p_study = sub.add_parser(
        "study", help="CardBrick-style study appliance (default)")
    p_study.add_argument("--fullscreen", action="store_true",
                         help="Run fullscreen (use on the handheld)")

    p_import = sub.add_parser("import", help="Import an .apkg file")
    p_import.add_argument("apkg", help="Path to the .apkg file")

    p_review = sub.add_parser("review",
                              help="Legacy prototype reviewer (no limits)")
    p_review.add_argument("--deck", help="Review only this deck")
    p_review.add_argument("--fullscreen", action="store_true",
                          help="Run fullscreen (use on the handheld)")

    sub.add_parser("decks", help="List decks and due counts")

    p_profile = sub.add_parser("profile",
                               help="View or edit the child profile")
    p_profile.add_argument("--name", help="Child's name")
    p_profile.add_argument("--daily-new", type=int,
                           help="New cards per day")
    p_profile.add_argument("--daily-review", type=int,
                           help="Review cards per day")
    p_profile.add_argument("--session-cards", type=int,
                           help="Max cards per session")
    p_profile.add_argument("--session-minutes", type=int,
                           help="Max minutes per session (0 = no limit)")
    p_profile.add_argument("--categories",
                           help="Comma-separated active categories, "
                                "or 'all' for every tag")
    p_profile.add_argument("--direction", choices=["normal", "reversed"],
                           help="Card direction")

    args = parser.parse_args(argv)
    if args.command is None:
        args.command, args.fullscreen = "study", False

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
        elif args.command == "profile":
            _profile_command(storage, args)
        elif args.command == "review":
            from cardbrick.audio import AudioPlayer
            from cardbrick.ui import ReviewApp
            audio = AudioPlayer(media_dir)
            app = ReviewApp(storage, scheduler, audio,
                            deck=args.deck, fullscreen=args.fullscreen)
            app.run()
        else:  # study
            from cardbrick.app import CardBrickApp
            from cardbrick.audio import AudioPlayer
            from cardbrick.service import ReviewService
            from cardbrick.settings import AppSettings
            settings = AppSettings(os.path.join(args.data_dir,
                                                "settings.json"))
            service = ReviewService(storage, scheduler)
            audio = AudioPlayer(media_dir)
            app = CardBrickApp(storage, service, audio, settings,
                               fullscreen=args.fullscreen or None)
            app.run()
    except ApkgError as exc:
        print(f"Import failed: {exc}", file=sys.stderr)
        return 1
    finally:
        storage.close()
    return 0


def _profile_command(storage, args):
    profile = storage.ensure_default_profile()
    updates = {}
    if args.name:
        updates["name"] = args.name
    if args.daily_new is not None:
        updates["daily_new_cards"] = args.daily_new
    if args.daily_review is not None:
        updates["daily_review_cards"] = args.daily_review
    if args.session_cards is not None:
        updates["session_card_limit"] = args.session_cards
    if args.session_minutes is not None:
        updates["session_time_minutes"] = args.session_minutes
    if args.direction:
        updates["study_direction"] = args.direction
    if args.categories:
        if args.categories.strip().lower() == "all":
            updates["active_categories"] = None
        else:
            updates["active_categories"] = [
                c.strip() for c in args.categories.split(",") if c.strip()]
        # update_profile serializes lists; None means "all categories"
        storage.update_profile(profile["id"],
                               active_categories=updates.pop(
                                   "active_categories"))
    if updates:
        storage.update_profile(profile["id"], **updates)
    profile = storage.get_profile(profile["id"])
    print(json.dumps(profile, indent=2))
    tags = storage.all_tags()
    if tags:
        print("Available categories: " + ", ".join(tags))


if __name__ == "__main__":
    sys.exit(main())
