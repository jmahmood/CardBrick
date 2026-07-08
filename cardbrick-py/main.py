#!/usr/bin/env python3
"""CardBrick-Py entry point.

Usage:
    python main.py study [--fullscreen]     CardBrick-style study appliance
                                            (child/parent flow; the default)
    python main.py import <deck.apkg|URL>   Import an Anki package (a
                                            file path or an http(s) URL)
    python main.py download [URL]           Download decks from a server
                                            (URL or deck_server_url
                                            setting) into the import
                                            folder
    python main.py review [--deck NAME]     Legacy prototype reviewer
    python main.py decks                    List decks and due counts
    python main.py profile [...]            View/edit the child profile
    python main.py admin purge-decks [...]  Permanently delete imported decks

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
import shutil
import sys
from datetime import datetime

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

    p_import = sub.add_parser(
        "import", help="Import an .apkg file, or a vocab .csv "
                       "(detected by extension); .apkg may also be an "
                       "http(s) URL, downloaded before import")
    p_import.add_argument("apkg", help="Path or http(s) URL of the "
                                       ".apkg (or path of a .csv) file")
    p_import.add_argument("--media-dir",
                          help="For .csv import: folder holding the audio/"
                               "image files it references, copied into "
                               "the app's media folder")
    p_import.add_argument("--deck", dest="import_deck",
                          help="For .csv import: deck name to file the "
                               "cards under (default: Vocabulario)")

    p_download = sub.add_parser(
        "download", help="Download decks from a server URL into the "
                         "import folder (does not import them)")
    p_download.add_argument("url", nargs="?", default=None,
                            help="Deck URL or JSON deck-list URL "
                                 "(default: the deck_server_url "
                                 "setting)")
    p_download.add_argument("--deck", action="append", dest="decks",
                            metavar="NAME",
                            help="Only download this deck from the "
                                 "list (repeatable; default: all)")
    p_download.add_argument("--list", action="store_true", dest="list_only",
                            help="Show what the server offers and exit")

    p_review = sub.add_parser("review",
                              help="Legacy prototype reviewer (no limits)")
    p_review.add_argument("--deck", help="Review only this deck")
    p_review.add_argument("--fullscreen", action="store_true",
                          help="Run fullscreen (use on the handheld)")

    sub.add_parser("decks", help="List decks and due counts")

    p_profile = sub.add_parser("profile",
                               help="View or edit the child profile")
    p_profile.add_argument("--name", help="Child's name")
    p_profile.add_argument("--daily-goal", type=int,
                           help="Daily goal in distinct cards; done in "
                                "sprints of --sprint-cards")
    p_profile.add_argument("--daily-new", type=int,
                           help="New cards per day")
    p_profile.add_argument("--sprint-cards", "--session-cards", type=int,
                           dest="sprint_cards",
                           help="Max cards per sprint")
    p_profile.add_argument("--sprint-minutes", "--session-minutes",
                           type=int, dest="sprint_minutes",
                           help="Max minutes per sprint (0 = no limit)")
    p_profile.add_argument("--study-ahead-days", type=int,
                           help="How many days ahead a sprint may pull "
                                "soon-due cards from (default 1)")
    p_profile.add_argument("--study-ahead", choices=["on", "off"],
                           help="Allow filling sprints with soon-due "
                                "cards and offering bonus sprints")
    p_profile.add_argument("--categories",
                           help="Comma-separated active categories, "
                                "or 'all' for every tag")
    p_profile.add_argument("--decks",
                           help="Comma-separated active deck names, "
                                "or 'all' for every deck")
    p_profile.add_argument("--direction", choices=["normal", "reversed"],
                           help="Card direction")

    p_admin = sub.add_parser(
        "admin", help="Administrative commands (destructive — use with "
                      "care)")
    admin_sub = p_admin.add_subparsers(dest="admin_command")

    p_purge = admin_sub.add_parser(
        "purge-decks", help="Permanently delete imported decks: cards, "
                            "review history, and vocab content. Child "
                            "profiles and settings are untouched.")
    p_purge.add_argument("--deck", action="append", dest="decks",
                         metavar="NAME",
                         help="Deck to purge (repeatable). Omit to purge "
                              "ALL decks.")
    p_purge.add_argument("--yes", action="store_true",
                         help="Skip the confirmation prompt (for "
                              "scripts/automation)")
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
            if args.apkg.lower().startswith(("http://", "https://")):
                from cardbrick.deck_download import (DownloadError,
                                                     download_deck)
                try:
                    args.apkg = download_deck(args.apkg, paths.import_dir)
                except DownloadError as exc:
                    print(f"Download failed: {exc}", file=sys.stderr)
                    return 1
                print(f"Downloaded to {args.apkg}")
            if args.apkg.lower().endswith(".csv"):
                from cardbrick.vocab_csv import import_vocab_csv
                kwargs = {"source_media_dir": args.media_dir}
                if args.import_deck:
                    kwargs["deck_name"] = args.import_deck
                stats = import_vocab_csv(args.apkg, storage, scheduler,
                                         paths.media_dir, **kwargs)
            else:
                stats = import_apkg(args.apkg, storage, scheduler,
                                    paths.media_dir)
            print(stats.summary())
        elif args.command == "download":
            from cardbrick.settings import AppSettings
            settings = AppSettings(paths.settings_path)
            return _download_command(settings, paths, args)
        elif args.command == "decks":
            rows = storage.decks(iso(now_utc()))
            if not rows:
                print("No decks imported yet.")
            for row in rows:
                print(f"{row['name']}: {row['due']} due / "
                      f"{row['total']} total")
        elif args.command == "profile":
            _profile_command(storage, args)
        elif args.command == "admin":
            return _admin_command(storage, paths, args)
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
    from cardbrick.app import (CardBrickApp, DisplayInitError,
                               FontSupportError)
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
    except FontSupportError as exc:
        # No fonts: a text error screen can't render either.
        log.error("%s", exc)
        print(f"FATAL: {exc}\n(see {paths.log_path})", file=sys.stderr)
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


def _download_command(settings, paths, args):
    """Fetch the server's deck list and download into the import
    folder, where both `main.py import` and the parent-mode Import
    screen will find the files."""
    from cardbrick.deck_download import (DownloadError, download_deck,
                                         fetch_deck_list, format_size)

    url = args.url or settings.get("deck_server_url")
    if not url:
        print("No URL given and no deck_server_url setting configured.\n"
              f"Either pass a URL or add \"deck_server_url\" to "
              f"{paths.settings_path}", file=sys.stderr)
        return 1

    try:
        decks = fetch_deck_list(url)
    except DownloadError as exc:
        print(f"Download failed: {exc}", file=sys.stderr)
        return 1

    if args.list_only:
        for deck in decks:
            size = format_size(deck["size"])
            print(f"{deck['name']}{f'  ({size})' if size else ''}  "
                  f"-> {deck['url']}")
        return 0

    wanted = decks
    if args.decks:
        by_name = {d["name"]: d for d in decks}
        missing = [n for n in args.decks if n not in by_name]
        if missing:
            print(f"Not on the server: {', '.join(missing)}\n"
                  f"Available: {', '.join(by_name)}", file=sys.stderr)
            return 1
        wanted = [by_name[n] for n in args.decks]

    failures = 0
    for deck in wanted:
        print(f"Downloading {deck['name']}...")
        try:
            path = download_deck(deck["url"], paths.import_dir,
                                 name=deck["name"])
            print(f"  -> {path}")
        except DownloadError as exc:
            failures += 1
            print(f"  failed: {exc}", file=sys.stderr)
    if failures == 0:
        print(f"Done. Import with: python main.py import <file>, or use "
              f"the parent-mode Import screen.")
    return 1 if failures else 0


def _profile_command(storage, args):
    profile = storage.ensure_default_profile()
    updates = {}
    if args.name:
        updates["name"] = args.name
    if args.daily_goal is not None:
        updates["daily_goal_cards"] = args.daily_goal
    if args.daily_new is not None:
        updates["daily_new_cards"] = args.daily_new
    if args.sprint_cards is not None:
        updates["session_card_limit"] = args.sprint_cards
    if args.sprint_minutes is not None:
        updates["session_time_minutes"] = args.sprint_minutes
    if args.study_ahead_days is not None:
        updates["study_ahead_days"] = args.study_ahead_days
    if args.study_ahead:
        updates["study_ahead_enabled"] = 1 if args.study_ahead == "on" \
            else 0
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
    if args.decks:
        if args.decks.strip().lower() == "all":
            decks = None
        else:
            decks = [d.strip() for d in args.decks.split(",") if d.strip()]
        # None means "all decks", same convention as active_categories
        storage.update_profile(profile["id"], active_decks=decks)
    if updates:
        storage.update_profile(profile["id"], **updates)
    profile = storage.get_profile(profile["id"])
    print(json.dumps(profile, indent=2))
    tags = storage.all_tags()
    if tags:
        print("Available categories: " + ", ".join(tags))
    decks = storage.deck_names_list()
    if decks:
        print("Available decks: " + ", ".join(decks))


def _admin_command(storage, paths, args):
    if args.admin_command == "purge-decks":
        return _admin_purge_decks(storage, paths, args.decks, args.yes)
    print("Usage: cardbrick admin purge-decks [--deck NAME ...] [--yes]",
          file=sys.stderr)
    return 1


def _admin_purge_decks(storage, paths, deck_names, assume_yes):
    """Permanently delete cards (and their cascading review history) for
    the given decks, or every deck if none are named. Always backs up
    the database first and always asks for confirmation unless --yes
    was passed — this is the one command in the app that discards data
    the child-facing UI has no way to create a do-over for."""
    available = storage.deck_names_list()
    if not available:
        print("No decks to purge.")
        return 0

    if deck_names:
        unknown = [d for d in deck_names if d not in available]
        if unknown:
            print(f"Unknown deck(s): {', '.join(unknown)}\n"
                  f"Available decks: {', '.join(available)}",
                  file=sys.stderr)
            return 1
        target_desc = ", ".join(deck_names)
    else:
        deck_names = None  # purge_decks(None) means "every deck"
        target_desc = f"ALL {len(available)} deck(s): " + \
            ", ".join(available)

    count = storage.count_cards_in_decks(deck_names)
    if count == 0:
        print("Nothing to purge (0 cards match).")
        return 0

    print(f"This will PERMANENTLY delete {count} card(s) and all their "
          f"review history from: {target_desc}")
    print(f"Database: {paths.db_path}")
    if not assume_yes:
        answer = input("Type 'yes' to confirm: ").strip().lower()
        if answer != "yes":
            print("Aborted — nothing was deleted.")
            return 1

    backup_path = _backup_database(paths.db_path, "purge")
    deleted = storage.purge_decks(deck_names)
    log.warning("admin purge-decks: deleted %d card(s) from %s "
               "(backup: %s)", deleted, target_desc, backup_path)
    print(f"Deleted {deleted} card(s).")
    print(f"Database backed up to:\n  {backup_path}")
    return 0


def _backup_database(db_path, label):
    stamp = datetime.now().strftime("%Y%m%dT%H%M%S")
    backup_path = f"{db_path}.backup-{label}-{stamp}"
    shutil.copyfile(db_path, backup_path)
    return backup_path


if __name__ == "__main__":
    sys.exit(main())
