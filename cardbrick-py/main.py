#!/usr/bin/env python3
"""CardBrick-Py entry point.

Usage:
    python main.py study [--fullscreen]     CardBrick-style study appliance
                                            (child/parent flow; the default)
    python main.py import <deck.apkg>       Import an Anki package
    python main.py review [--deck NAME]     Legacy prototype reviewer
    python main.py decks                    List decks and due counts
    python main.py profile [...]            View/edit the child profile

Deployment flags (usable with or without a subcommand):
    --smoke-test          Non-interactive sanity check; exit 0 on pass
    --input-diagnostic    Controller event viewer + calibration
    --desktop / --knulli  Force platform mode (else auto-detected)
    --data-dir PATH       Writable data root (else CARD_BRICK_DATA_DIR /
                          CARDBRICK_DATA env, Knulli userdata, or ./data)
"""

import argparse
import json
import logging
import sys

from cardbrick import __version__
from cardbrick.bootlog import log_environment, setup_logging
from cardbrick.importer import ApkgError, import_apkg
from cardbrick.paths import AppPaths
from cardbrick.scheduler import ReviewScheduler, iso, now_utc
from cardbrick.storage import Storage

log = logging.getLogger("cardbrick.main")


def build_parser():
    parser = argparse.ArgumentParser(prog="cardbrick", description=__doc__)
    parser.add_argument("--data-dir", default=None,
                        help="Writable data root (default: "
                             "CARD_BRICK_DATA_DIR env, Knulli userdata, "
                             "or ./data)")
    parser.add_argument("--desktop", action="store_true",
                        help="Force desktop mode")
    parser.add_argument("--knulli", action="store_true",
                        help="Force Knulli/handheld mode")
    parser.add_argument("--smoke-test", action="store_true",
                        help="Run non-interactive deployment checks and "
                             "exit")
    parser.add_argument("--input-diagnostic", action="store_true",
                        help="Open the controller test/calibration screen")
    parser.add_argument("--verbose", action="store_true",
                        help="Debug-level logging")
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
    return parser


def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command is None:
        args.command, args.fullscreen = "study", False

    cli_mode = "knulli" if args.knulli else "desktop" if args.desktop \
        else None
    paths = AppPaths.resolve(cli_data_dir=args.data_dir, cli_mode=cli_mode)
    try:
        paths.ensure_directories()
    except OSError as exc:
        print(f"FATAL: cannot create data directory {paths.data_dir}: "
              f"{exc}", file=sys.stderr)
        return 1
    setup_logging(paths.log_path, verbose=args.verbose)
    log_environment(paths, __version__)

    if args.smoke_test:
        from cardbrick.smoke import run_smoke_test
        return 0 if run_smoke_test(paths).ok else 1

    try:
        storage = Storage(paths.db_path)
    except Exception as exc:  # noqa: BLE001 - anything here is fatal
        from cardbrick.errors import show_error_screen
        show_error_screen(
            "Cannot open database", f"{paths.db_path}: {exc}",
            log_path=paths.log_path,
            next_action="If the file is corrupt, restore a .backup-* "
                        "copy from the same folder or move the file "
                        "away and re-import your decks.")
        return 1

    scheduler = ReviewScheduler()
    try:
        if args.input_diagnostic:
            return _run_app(storage, scheduler, paths,
                            fullscreen=False, initial_state="INPUT_DIAG")
        if args.command == "import":
            stats = import_apkg(args.apkg, storage, scheduler,
                                paths.media_dir)
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
            audio = AudioPlayer(paths.media_dir)
            app = ReviewApp(storage, scheduler, audio,
                            deck=args.deck, fullscreen=args.fullscreen)
            app.run()
        else:  # study
            return _run_app(storage, scheduler, paths,
                            fullscreen=args.fullscreen or None)
    except ApkgError as exc:
        print(f"Import failed: {exc}", file=sys.stderr)
        return 1
    finally:
        storage.close()
    return 0


def _run_app(storage, scheduler, paths, fullscreen=None,
             initial_state=None):
    """Boot the appliance UI; fatal errors become visible screens."""
    from cardbrick.app import CardBrickApp, DisplayInitError
    from cardbrick.audio import AudioPlayer
    from cardbrick.errors import show_error_screen
    from cardbrick.service import ReviewService
    from cardbrick.settings import AppSettings

    settings = AppSettings(paths.settings_path)
    service = ReviewService(storage, scheduler)
    audio = AudioPlayer(paths.media_dir)
    try:
        app = CardBrickApp(storage, service, audio, settings, paths=paths,
                           fullscreen=fullscreen,
                           initial_state=initial_state)
    except DisplayInitError as exc:
        # No display: an error screen can't render either; log + stderr.
        print(f"FATAL: display init failed: {exc}\n"
              f"Check SDL_VIDEODRIVER (see {paths.log_path})",
              file=sys.stderr)
        return 1
    except Exception as exc:  # noqa: BLE001
        log.exception("startup failed")
        show_error_screen("CardBrick could not start", str(exc),
                          log_path=paths.log_path)
        return 1

    try:
        app.run()
    except Exception as exc:  # noqa: BLE001
        log.exception("unhandled error during session")
        show_error_screen("CardBrick hit an error", str(exc),
                          log_path=paths.log_path,
                          next_action="Your completed reviews are saved. "
                                      "Restart the app to continue.")
        return 1
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
            categories = None
        else:
            categories = [c.strip() for c in args.categories.split(",")
                          if c.strip()]
        # update_profile serializes lists; None means "all categories"
        storage.update_profile(profile["id"],
                               active_categories=categories)
    if updates:
        storage.update_profile(profile["id"], **updates)
    profile = storage.get_profile(profile["id"])
    print(json.dumps(profile, indent=2))
    tags = storage.all_tags()
    if tags:
        print("Available categories: " + ", ".join(tags))


if __name__ == "__main__":
    sys.exit(main())
