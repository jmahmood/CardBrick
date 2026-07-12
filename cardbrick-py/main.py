#!/usr/bin/env python3
"""CardBrick-Py entry point.

Usage:
    python main.py study [--fullscreen]     CardBrick-style study appliance
                                            (child/parent flow; the default)
    python main.py import <deck.apkg>       Import an Anki package
    python main.py decks                    List decks and due counts
    python main.py profile [...]            View/edit the child profile
    python main.py sync --name DEVICE       Configure and run LAN sync
    python main.py sync-status              Show local sync configuration
    python main.py sync-restore FILE        Restore a server backup archive
    python main.py admin purge-decks [...]  Permanently delete imported decks
    python main.py admin reset [--yes]      Wipe ALL study context (decks,
                                            progress, profiles, media) for
                                            a from-scratch start

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
import os
import shutil
import sys
from datetime import datetime

from cardbrick import __version__
from cardbrick.bootlog import log_environment, setup_logging
from cardbrick.importer import ApkgError, import_apkg
from cardbrick.paths import AppPaths
from cardbrick.pattern_pack import PatternPackError
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
        "import", help="Import an .apkg file, a vocab .csv, or a "
                       "pattern-pack .json (detected by extension)")
    p_import.add_argument("apkg", help="Path to the .apkg, .csv or "
                                       ".json file")
    p_import.add_argument("--media-dir",
                          help="For .csv import: folder holding the audio/"
                               "image files it references, copied into "
                               "the app's media folder")
    p_import.add_argument("--deck", dest="import_deck",
                          help="For .csv/.json import: deck name to file "
                               "the cards under (default: the pack's own "
                               "deck, or Vocabulario for .csv)")

    sub.add_parser("decks", help="List decks and due counts")

    p_profile = sub.add_parser("profile",
                               help="View or edit the child profile")
    p_profile.add_argument("--name", help="Child's name")
    p_profile.add_argument("--daily-goal", type=int,
                           help="Daily goal in distinct cards; done in "
                                "sprints of --sprint-cards")
    p_profile.add_argument("--daily-new", type=int,
                           help="Fixed cap on new cards per day "
                                "(0 = auto: the daily goal paces new "
                                "intake)")
    p_profile.add_argument("--sprint-cards", "--session-cards", type=int,
                           dest="sprint_cards",
                           help="Max cards per sprint")
    p_profile.add_argument("--sprint-minutes", "--session-minutes",
                           type=int, dest="sprint_minutes",
                           help="Max minutes per sprint (0 = no limit)")
    p_profile.add_argument("--drill-sprint-cards", type=int,
                           help="Max cards per pattern-drill sprint "
                                "(default 6 — production cards take "
                                "minutes, not seconds)")
    p_profile.add_argument("--drill-sprint-minutes", type=int,
                           help="Max minutes per pattern-drill sprint "
                                "(default 5, 0 = no limit)")
    p_profile.add_argument("--drill-daily-new", type=int,
                           help="Fixed drip of new drill patterns per "
                                "day (default 6, 0 = none)")
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

    p_reset = admin_sub.add_parser(
        "reset", help="Start from a clean slate: delete the whole "
                      "database (decks, review history, sessions, "
                      "profiles) and imported media. Device settings "
                      "and controller calibration are kept. The "
                      "database is backed up first.")
    p_reset.add_argument("--yes", action="store_true",
                         help="Skip the confirmation prompt (for "
                              "scripts/automation)")

    p_sync = sub.add_parser(
        "sync", help="Back up this device and install assigned content")
    p_sync.add_argument("--name", help="Set this device's family name")
    p_sync.add_argument("--server", help="Server URL (default: "
                                          "http://10.0.0.30:6429)")
    p_sync.add_argument("--backup-only", action="store_true")
    p_sync.add_argument("--content-only", action="store_true")
    p_sync.add_argument("--force-backup", action="store_true")

    sub.add_parser("sync-status", help="Show local sync configuration")
    p_restore = sub.add_parser(
        "sync-restore", help="Restore a local CardBrick backup .tar.gz")
    p_restore.add_argument("archive")
    p_restore.add_argument("--yes", action="store_true",
                           help="Skip the destructive confirmation")
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

    # Restore must happen before logging or SQLite opens files under the data
    # root.  The current sync identity is preserved by restore_backup().
    if args.command == "sync-restore":
        if not args.yes:
            answer = input("Replace current CardBrick data from this backup? "
                           "Type 'yes': ").strip().lower()
            if answer != "yes":
                print("Restore cancelled.")
                return 1
        from cardbrick.sync import restore_backup
        try:
            rollback = restore_backup(args.archive, paths.data_dir)
            print("Restore complete. Previous data retained at:\n  %s" %
                  rollback)
            return 0
        except Exception as exc:
            print("Restore failed: %s" % exc, file=sys.stderr)
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
            if args.apkg.lower().endswith(".csv"):
                from cardbrick.vocab_csv import import_vocab_csv
                kwargs = {"source_media_dir": args.media_dir}
                if args.import_deck:
                    kwargs["deck_name"] = args.import_deck
                stats = import_vocab_csv(args.apkg, storage, scheduler,
                                         paths.media_dir, **kwargs)
            elif args.apkg.lower().endswith(".json"):
                from cardbrick.pattern_pack import import_pattern_pack
                stats = import_pattern_pack(args.apkg, storage, scheduler,
                                            deck_name=args.import_deck)
            else:
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
        elif args.command == "sync":
            from cardbrick.sync import sync_once
            if args.backup_only and args.content_only:
                print("--backup-only and --content-only cannot be combined",
                      file=sys.stderr)
                return 1
            try:
                result = sync_once(
                    storage, scheduler, paths, __version__, name=args.name,
                    server=args.server, backup_only=args.backup_only,
                    content_only=args.content_only,
                    force_backup=args.force_backup)
            except Exception as exc:  # sync must never fail with a traceback
                log.warning("sync failed: %s", exc)
                print("Sync unavailable: %s" % exc, file=sys.stderr)
                return 1
            print(json.dumps(result, indent=2, sort_keys=True))
        elif args.command == "sync-status":
            from cardbrick.sync import sync_status
            print(json.dumps(sync_status(paths.data_dir), indent=2,
                             sort_keys=True))
        elif args.command == "admin":
            return _admin_command(storage, paths, args)
        else:  # study
            return _run_app(storage, scheduler, paths,
                            fullscreen=args.fullscreen or None)
    except (ApkgError, PatternPackError) as exc:
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
        if app.pending_restore_archive:
            # The database must be closed before restore_backup atomically
            # replaces the complete data directory. Storage.close() is
            # idempotent because main() also closes it in its finally block.
            storage.close()
            from cardbrick.sync import restore_backup
            try:
                rollback = restore_backup(
                    app.pending_restore_archive, paths.data_dir)
                log.warning("in-app restore complete; previous data at %s",
                            rollback)
                print("Restore complete. Previous data retained at:\n  %s" %
                      rollback)
                return 0
            except Exception as exc:  # noqa: BLE001
                log.exception("in-app restore failed")
                show_error_screen(
                    "Restore failed",
                    str(exc),
                    log_path=paths.log_path,
                    next_action="The previous CardBrick data was kept. "
                                "Restart the app to continue.",
                )
                return 1
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
    if args.daily_goal is not None:
        updates["daily_goal_cards"] = args.daily_goal
    if args.daily_new is not None:
        updates["daily_new_cards"] = args.daily_new
    if args.sprint_cards is not None:
        updates["session_card_limit"] = args.sprint_cards
    if args.sprint_minutes is not None:
        updates["session_time_minutes"] = args.sprint_minutes
    if args.drill_sprint_cards is not None:
        updates["drill_sprint_cards"] = args.drill_sprint_cards
    if args.drill_sprint_minutes is not None:
        updates["drill_sprint_minutes"] = args.drill_sprint_minutes
    if args.drill_daily_new is not None:
        updates["drill_daily_new"] = args.drill_daily_new
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
    if args.admin_command == "reset":
        return _admin_reset(storage, paths, args.yes)
    print("Usage: cardbrick admin purge-decks [--deck NAME ...] [--yes]\n"
          "       cardbrick admin reset [--yes]",
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


def _admin_reset(storage, paths, assume_yes):
    """Wipe all study context for a from-scratch start.

    Deletes the database (decks, review history, sessions, profiles)
    after backing it up, and removes imported media. settings.json and
    input_mapping.json survive — display and controller calibration are
    device facts, not study context, and redoing the on-device
    controller setup for every reset would be pointless friction. The
    next app start recreates an empty database with a default profile.
    """
    print("This will PERMANENTLY delete ALL study context:")
    print(f"  - decks, review history, sessions, and child profiles:\n"
          f"      {paths.db_path}")
    print(f"  - imported media:\n      {paths.media_dir}")
    print("Device settings and controller calibration are kept.")
    if not assume_yes:
        answer = input("Type 'yes' to confirm: ").strip().lower()
        if answer != "yes":
            print("Aborted — nothing was deleted.")
            return 1

    storage.close()  # release the file before touching it
    backup_path = None
    if os.path.exists(paths.db_path):
        backup_path = _backup_database(paths.db_path, "reset")
        os.remove(paths.db_path)
    for suffix in ("-wal", "-shm"):  # stale WAL pages must not survive
        leftover = paths.db_path + suffix
        if os.path.exists(leftover):
            os.remove(leftover)

    media_files = 0
    if os.path.isdir(paths.media_dir):
        for name in os.listdir(paths.media_dir):
            path = os.path.join(paths.media_dir, name)
            if os.path.isfile(path):
                os.remove(path)
                media_files += 1

    log.warning("admin reset: database deleted (backup: %s), %d media "
                "file(s) removed", backup_path, media_files)
    print("Clean slate ready.")
    if backup_path:
        print(f"Database backed up to:\n  {backup_path}")
    print(f"Removed {media_files} media file(s).")
    print("The next start creates a fresh database and default profile; "
          "re-import decks with: python main.py import <file.apkg>")
    return 0


def _backup_database(db_path, label):
    stamp = datetime.now().strftime("%Y%m%dT%H%M%S")
    backup_path = f"{db_path}.backup-{label}-{stamp}"
    shutil.copyfile(db_path, backup_path)
    return backup_path


if __name__ == "__main__":
    sys.exit(main())
