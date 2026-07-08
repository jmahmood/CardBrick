"""PaperRoll geometry/animation logic — fully headless.

Items are fed with surf=None and an explicit height, so no pygame
display (or even pygame import) is needed: everything here is the
one-dimensional spool arithmetic behind the printer UI.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from cardbrick.paperroll import PAGE_GAP, PERF_H, PaperRoll  # noqa: E402


def settle(roll, max_ticks=500):
    for _ in range(max_ticks):
        if not roll.busy:
            return
        roll.update()
    raise AssertionError("roll never settled")


def make_roll(**kwargs):
    return PaperRoll(print_y=100, **kwargs)


def test_starts_idle_at_blank_paper():
    roll = make_roll()
    assert not roll.busy
    assert roll.offset == -100.0
    assert roll.page_count == 0


def test_feeds_settle_with_write_point_on_print_position():
    roll = make_roll()
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed(None, h=30)
    assert roll.busy
    settle(roll)
    # 60px of content; the write point (total) rests on print_y.
    assert roll.offset == 60 - 100
    assert roll.page_count == 1


def test_line_feeds_are_staggered_not_batched():
    roll = make_roll()
    roll.feed_page(perf=False)
    for _ in range(3):
        roll.feed(None, h=30)
    committed_counts = set()
    for _ in range(200):
        committed_counts.add(len(roll._items))
        if not roll.busy:
            break
        roll.update()
    # Every intermediate count appears: lines landed one at a time.
    assert {0, 1, 2, 3} <= committed_counts


def test_finish_snaps_instantly():
    roll = make_roll()
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed(None, h=50)
    roll.finish()
    assert not roll.busy
    assert roll.offset == 80 - 100


def test_reduced_motion_never_animates():
    roll = make_roll(reduced_motion=True)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    assert not roll.busy
    assert roll.offset == 30 - 100


def test_page_feed_inserts_perforation_group():
    roll = make_roll(reduced_motion=True)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed_page()
    assert roll.page_count == 2
    assert roll.offset == 30 + PAGE_GAP + PERF_H + PAGE_GAP - 100
    assert any(item.kind == "perf" for item in roll._items)


def test_torn_page_feed_uses_jagged_perforation_kind():
    roll = make_roll(reduced_motion=True)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed_page(tear=True)

    assert roll.page_count == 2
    assert any(item.kind == "tear-perf" for item in roll._items)
    assert not any(item.kind == "perf" and item.event == "page"
                   for item in roll._items)


def test_feed_perf_remains_clean_lone_perforation():
    roll = make_roll(reduced_motion=True)
    roll.feed_perf()

    assert [(item.kind, item.event) for item in roll._items] == [
        ("perf", None)
    ]


def test_feed_events_fire_for_animated_content_and_page_feed():
    events = []
    roll = make_roll(on_feed=events.append)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    settle(roll)
    roll.feed_page()
    settle(roll)

    assert events == ["line", "page"]


def test_finish_does_not_burst_feed_events():
    events = []
    roll = make_roll(on_feed=events.append)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed_page()

    roll.finish()

    assert events == []


def test_reduced_motion_does_not_emit_feed_events():
    events = []
    roll = make_roll(reduced_motion=True, on_feed=events.append)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed_page()

    assert events == []


def test_rewind_page_rolls_back_to_previous_page_start():
    roll = make_roll(reduced_motion=True)
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed(None, h=30)
    roll.feed_page()
    roll.feed(None, h=40)
    assert roll.rewind_page()
    # Back to the blank start of the first page: everything printed
    # since (both cards) is gone, ready to reprint.
    assert roll.offset == 0 - 100
    assert roll.page_count == 0
    # The caller re-registers the page and reprints the restored card.
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    assert roll.offset == 30 - 100
    assert roll.page_count == 1


def test_rewind_needs_a_previous_page():
    roll = make_roll(reduced_motion=True)
    assert not roll.rewind_page()
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    assert not roll.rewind_page()


def test_rewind_animates_backwards():
    roll = make_roll()
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.feed_page()
    roll.feed(None, h=40)
    settle(roll)
    before = roll.offset
    roll.rewind_page()
    roll.update()
    assert roll.offset < before  # moving backwards
    settle(roll)
    assert roll.offset == 0 - 100


def test_prune_bounds_memory_and_keeps_recent_pages_rewindable():
    roll = make_roll(reduced_motion=True)
    roll.feed_page(perf=False)
    for _ in range(200):
        roll.feed(None, h=40)
        roll.feed_page()
        roll.feed(None, h=40)
    # Far more was printed than is kept on the spool...
    assert len(roll._items) < 100
    # ...but the roll can still rewind to the previous page.
    assert roll.rewind_page()


def test_interrupt_mid_feed_then_more_feeds():
    roll = make_roll()
    roll.feed_page(perf=False)
    roll.feed(None, h=30)
    roll.update()
    roll.finish()  # button mashed mid-animation
    assert not roll.busy
    roll.feed(None, h=30)
    settle(roll)
    assert roll.offset == 60 - 100
