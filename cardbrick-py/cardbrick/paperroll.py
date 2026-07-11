"""Endless spool of tractor-feed paper — the surface behind the UI.

The app's unifying metaphor is a printer with an unlimited paper roll:
the 640x480 canvas is a fixed viewport onto the roll, and content is
*printed*. Each new line of content is a line feed (the roll advances
one line height and the line lands on the fixed print position), and
each new card is a page feed (the roll races past a dashed perforation
to fresh paper). Undo rewinds the roll backwards past the perforation.

PaperRoll owns the committed items (pre-rendered surfaces, perforation
marks, blank gaps), a queue of items still waiting to be printed, and
the eased scroll offset. Screens push items instead of blitting text
directly and call draw() every frame; the golden rule is enforced by
the caller: any input while ``busy`` should call finish() first, so a
button-mashing child never waits on an animation.

The geometry is one-dimensional and display-free on purpose (items may
carry surf=None), so feed/rewind/prune logic is unit-testable headless.
"""

from collections import deque

# Fraction of the remaining distance covered per tick (at 30 FPS this
# settles a one-line feed in ~4 ticks and a page feed in ~8).
EASE = 0.35
# Offsets closer than this to the target snap and count as settled.
SNAP_PX = 0.6
# A queued item is released once the roll is within this many pixels of
# its target — this is what staggers a block of text into visibly
# sequential line feeds (~3 ticks each at 30 FPS) instead of one jump.
RELEASE_PX = 14.0
# Committed items further than this above the viewport are dropped; the
# "unlimited" spool never grows unbounded. Generous enough that a
# one-page rewind (undo) always has real paper to roll back over.
KEEP_ABOVE_PX = 1600
# Vertical size of a perforation item (dashes drawn at its centre).
PERF_H = 18
# Blank paper fed before/after the perforation on a page feed.
PAGE_GAP = 36
# Per-frame reveal speed for the tiny printer head. At 30 FPS this
# makes a line appear in about 2-3 ticks: visible, but never precious.
REVEAL_STEP = 0.45
ATTACH_DROP_FRAMES = 9
ATTACH_IMPACT_FRAMES = 7


class _Item:
    __slots__ = (
        "kind", "surf", "h", "x", "event", "reveal", "reveal_progress",
        "attach_phase", "attach_progress", "angle",
    )

    def __init__(self, kind, surf, h, x=None, event=None, reveal=False):
        self.kind = kind  # "surf" | "perf" | "tear-perf" | "gap"
        self.surf = surf
        self.h = h
        self.x = x  # None = centred in the content span
        self.event = event
        self.reveal = bool(reveal and surf is not None)
        self.reveal_progress = 1.0
        self.attach_phase = "waiting_for_reel" if kind == "attachment" else None
        self.attach_progress = 0.0
        self.angle = 0.0


class PaperRoll:
    def __init__(self, print_y, reduced_motion=False, on_feed=None):
        self.print_y = print_y  # screen row the write point rests on
        self.reduced_motion = reduced_motion
        self.on_feed = on_feed
        self._items = []
        self._queue = deque()
        self._base = 0.0  # roll coordinate of _items[0]'s top edge
        self._total = 0.0  # roll coordinate of the write point
        self.offset = -float(print_y)  # roll coordinate of screen y=0
        self._target = self.offset
        self._view_target = self.offset
        self._active_reveal = None
        self._active_attachment = None
        # Roll coordinate + committed-item index where each page's
        # content begins (right after its perforation group).
        self._pages = []

    @property
    def page_count(self):
        """Pages committed so far (queued page starts not included)."""
        return len(self._pages) + sum(
            1 for item in self._queue if item.kind == "page-start"
        )

    # -- feeding -----------------------------------------------------------------

    def feed(self, surf, h=None, x=None, reveal=True):
        """Queue one printed line (or image, stamp, ...) for a line feed."""
        self._queue.append(
            _Item(
                "surf",
                surf,
                surf.get_height() if h is None else h,
                x,
                reveal=reveal,
            )
        )
        self.scroll_to_print_position()
        self._maybe_snap()

    def feed_gap(self, h):
        self._queue.append(_Item("gap", None, h))
        self.scroll_to_print_position()
        self._maybe_snap()

    def feed_attachment(self, surf, h=None, x=None, angle=0.0,
                        impact_event="staple"):
        """Reserve reel space, then drop and fasten a physical attachment."""
        item = _Item(
            "attachment", surf,
            surf.get_height() if h is None else h,
            x, event=impact_event, reveal=False,
        )
        item.angle = float(angle)
        self._queue.append(item)
        self.scroll_to_print_position()
        self._maybe_snap()

    def feed_perf(self):
        """A lone perforation (tear line) without starting a new page."""
        self._queue.append(_Item("perf", None, PERF_H))
        self.scroll_to_print_position()
        self._maybe_snap()

    def feed_page(self, perf=True, tear=False):
        """Advance to fresh paper for a new page (card).

        The perforation group is queued like any other content, so the
        tear line visibly rides up the screen. perf=False starts a page
        without the tear — used for the very first page on a roll.
        """
        if perf:
            self._queue.append(_Item("gap", None, PAGE_GAP))
            kind = "tear-perf" if tear else "perf"
            self._queue.append(_Item(kind, None, PERF_H, event="page"))
            self._queue.append(_Item("gap", None, PAGE_GAP))
        self._queue.append(_Item("page-start", None, 0))
        self.scroll_to_print_position()
        self._maybe_snap()

    def rewind_page(self):
        """Roll backwards to where the *previous* page's content began,
        discarding everything printed since — the current page, its
        perforation, and the previous page's content (the caller
        reprints the restored card there). Returns False when there is
        no previous page still on the spool to rewind to."""
        if len(self._pages) < 2:
            return False
        self._queue.clear()
        coord, idx = self._pages[-2]
        if idx < 0:  # already pruned off the top of the spool
            return False
        del self._items[idx:]
        del self._pages[-2:]
        self._total = coord
        self._target = self._total - self.print_y
        self.scroll_to_print_position()
        self._maybe_snap()
        return True

    # -- animation ---------------------------------------------------------------

    @property
    def busy(self):
        return (
            bool(self._queue)
            or abs(self._view_target - self.offset) > SNAP_PX
            or self._active_reveal is not None
            or self._active_attachment is not None
        )

    @property
    def printing_busy(self):
        """True for automatic printing/reveal motion, not manual scroll."""
        return (
            bool(self._queue)
            or self._active_reveal is not None
            or self._active_attachment is not None
            or (
                abs(self._view_target - self._target) <= SNAP_PX
                and abs(self._target - self.offset) > SNAP_PX
            )
        )

    @property
    def at_print_position(self):
        return (
            abs(self._target - self.offset) <= SNAP_PX
            and abs(self._view_target - self._target) <= SNAP_PX
        )

    def scroll(self, delta):
        """Move the viewport over already-printed paper.

        Negative deltas look upward to older content; positive deltas
        return toward the live print position. The print target itself
        is not changed, so feeding can resume from the same write point.
        """
        lo = self._scroll_min()
        hi = self._target
        if lo > hi:
            lo = hi
        self._view_target = min(max(self._view_target + delta, lo), hi)
        if self.reduced_motion:
            self.offset = self._view_target

    def scroll_to_print_position(self):
        self._view_target = self._target
        if self.reduced_motion:
            self.offset = self._view_target

    def finish(self):
        """Snap to the fully-printed state (input interruption or
        reduced motion): commit the whole queue, land on the target."""
        while self._queue:
            self._commit(self._queue.popleft())
        for item in self._items:
            item.reveal_progress = 1.0
        self._active_reveal = None
        if self._active_attachment is not None:
            self._active_attachment.attach_phase = "settled"
            self._active_attachment.attach_progress = 1.0
        self._active_attachment = None
        self._view_target = self._target
        self.offset = self._target
        self._prune()

    def update(self):
        """Advance the animation one tick. Returns True while moving."""
        if self.reduced_motion:
            was_busy = self.busy
            self.finish()
            return was_busy
        if self._active_attachment is not None:
            self._advance_attachment()
        elif self._active_reveal is not None:
            self._advance_reveal()
        elif self._queue and abs(self._target - self.offset) <= RELEASE_PX:
            # Blank paper and page bookkeeping ride along for free: a
            # release commits up to one *printed* item (line, image,
            # perforation) so only real content paces the feed.
            while self._queue:
                item = self._queue.popleft()
                self._commit(item, notify=True)
                if item.kind not in ("gap", "page-start"):
                    break
        self.offset += (self._view_target - self.offset) * EASE
        if abs(self._view_target - self.offset) <= SNAP_PX:
            self.offset = self._view_target
        self._prune()
        return self.busy

    def _commit(self, item, notify=False):
        if item.kind == "page-start":
            self._pages.append((self._total, len(self._items)))
        else:
            self._items.append(item)
            self._total += item.h
            if item.reveal:
                item.reveal_progress = 0.0
                self._active_reveal = item
            elif item.kind == "attachment":
                item.attach_phase = "waiting_for_reel"
                item.attach_progress = 0.0
                self._active_attachment = item
        self._target = self._total - self.print_y
        self._view_target = self._target
        if notify and self.on_feed:
            if item.kind in ("surf", "attachment"):
                self.on_feed("line")
            elif item.event:
                self.on_feed(item.event)

    def _advance_reveal(self):
        item = self._active_reveal
        if item is None:
            return
        item.reveal_progress = min(item.reveal_progress + REVEAL_STEP, 1.0)
        if item.reveal_progress >= 1.0:
            self._active_reveal = None

    def _advance_attachment(self):
        item = self._active_attachment
        if item is None:
            return
        if item.attach_phase == "waiting_for_reel":
            if abs(self._target - self.offset) <= SNAP_PX:
                item.attach_phase = "dropping"
                item.attach_progress = 0.0
        elif item.attach_phase == "dropping":
            item.attach_progress = min(
                item.attach_progress + 1.0 / ATTACH_DROP_FRAMES, 1.0
            )
            if item.attach_progress >= 1.0:
                item.attach_phase = "impact"
                item.attach_progress = 0.0
                if self.on_feed and item.event:
                    self.on_feed(item.event)
        elif item.attach_phase == "impact":
            item.attach_progress = min(
                item.attach_progress + 1.0 / ATTACH_IMPACT_FRAMES, 1.0
            )
            if item.attach_progress >= 1.0:
                item.attach_phase = "settled"
                self._active_attachment = None

    def _maybe_snap(self):
        if self.reduced_motion:
            self.finish()

    def _prune(self):
        dropped = 0
        while self._items and (
            self._base + self._items[0].h < self.offset - KEEP_ABOVE_PX
        ):
            self._base += self._items[0].h
            del self._items[0]
            dropped += 1
        if dropped:
            self._pages = [(coord, idx - dropped) for coord, idx in self._pages]
        self._view_target = min(max(self._view_target, self._scroll_min()), self._target)

    def _scroll_min(self):
        return self._base - 12

    # -- drawing -----------------------------------------------------------------

    def draw(self, screen, x0, x1, y_max):
        """Blit the visible slice of the roll into the content span
        x0..x1, down to y_max (the top of the chassis bar)."""
        import pygame  # deferred: geometry stays importable headless

        y = self._base - self.offset
        for item in self._items:
            top, bottom = y, y + item.h
            y = bottom
            if bottom < 0 or top > y_max:
                continue
            if item.kind == "perf":
                mid = int(top + item.h / 2)
                for xx in range(x0 + 6, x1 - 10, 16):
                    pygame.draw.line(screen, self.perf_color, (xx, mid), (xx + 8, mid), 2)
            elif item.kind == "tear-perf":
                mid = int(top + item.h / 2)
                points = []
                offsets = (0, -4, 3, -2, 4, -3, 2, -1)
                for i, xx in enumerate(range(x0 + 6, x1 - 8, 14)):
                    points.append((xx, mid + offsets[i % len(offsets)]))
                if len(points) >= 2:
                    pygame.draw.lines(screen, self.perf_color, False, points, 2)
                    for i, (px, py) in enumerate(points[1:-1:3], start=1):
                        direction = -1 if i % 2 else 1
                        pygame.draw.line(
                            screen,
                            self.perf_color,
                            (px, py),
                            (px + 6, py + 5 * direction),
                            1,
                        )
            elif item.surf is not None:
                x = item.x if item.x is not None else x0 + (x1 - x0 - item.surf.get_width()) // 2
                draw_top = top
                draw_surf = item.surf
                if item.kind == "attachment":
                    draw_top, scale_x, scale_y = self._attachment_pose(item, top)
                    if scale_x != 1.0 or scale_y != 1.0:
                        scaled_w = max(1, int(item.surf.get_width() * scale_x))
                        scaled_h = max(1, int(item.surf.get_height() * scale_y))
                        draw_surf = pygame.transform.smoothscale(
                            item.surf, (scaled_w, scaled_h)
                        )
                        # Scale about the attachment's centre so the press
                        # feels perpendicular to the paper, not side-anchored.
                        x -= (scaled_w - item.surf.get_width()) // 2
                if item.reveal and item.reveal_progress < 1.0:
                    visible_w = int(item.surf.get_width() * item.reveal_progress)
                    if visible_w > 0:
                        area = pygame.Rect(
                            0, 0, visible_w, item.surf.get_height()
                        )
                        screen.blit(item.surf, (x, int(top)), area)
                    self._draw_head(screen, x + visible_w, int(top), item)
                else:
                    screen.blit(draw_surf, (x, int(draw_top)))
                    if item.kind == "attachment" and item.attach_phase in ("impact", "settled"):
                        self._draw_staple(screen, x, int(draw_top), item)

    def _attachment_pose(self, item, final_top):
        """Return top, x-scale, y-scale for the drop/press/rebound pose."""
        if item.attach_phase == "waiting_for_reel":
            return -item.surf.get_height() - 8, 1.0, 1.0
        if item.attach_phase == "dropping":
            p = item.attach_progress
            # Accelerating fall with a tiny overshoot at the end.
            start = -item.surf.get_height() - 8
            y = start + (final_top - start) * (p * p)
            if p > 0.82:
                y += 4.0 * ((p - 0.82) / 0.18)
            return y, 1.0, 1.0
        if item.attach_phase == "impact":
            p = item.attach_progress
            if p >= 1.0:
                return final_top, 1.0, 1.0
            if p < 0.30:
                # Stapler drives the print into the page: down, slightly
                # wider, and flatter to imply pressure toward the screen.
                q = p / 0.30
                return final_top + 4.0 * q, 1.0 + 0.012 * q, 1.0 - 0.035 * q
            if p < 0.62:
                # Release pressure and pop just above the paper plane.
                q = (p - 0.30) / 0.32
                return final_top + 4.0 - 7.0 * q, 1.012 - 0.020 * q, 0.965 + 0.050 * q
            # Damped return from the raised pose to the exact resting geometry.
            q = (p - 0.62) / 0.38
            return final_top - 3.0 * (1.0 - q), 0.992 + 0.008 * q, 1.015 - 0.015 * q
        return final_top, 1.0, 1.0

    def _draw_staple(self, screen, x, y, item):
        import pygame

        sx, sy = int(x + 17), int(y + 13)
        metal = (112, 111, 108)
        tilt = max(-2, min(2, round(item.angle)))
        pygame.draw.line(
            screen, metal, (sx - 5, sy - tilt), (sx + 5, sy + tilt), 2
        )
        if item.attach_phase == "impact":
            fade = 1.0 - item.attach_progress
            reach = max(2, int(8 * fade))
            ink = (196, 138, 44)
            for dx, dy in ((-1, -1), (1, -1), (-1, 1), (1, 1)):
                pygame.draw.line(
                    screen, ink, (sx + dx * 7, sy + dy * 5),
                    (sx + dx * (7 + reach), sy + dy * (5 + reach)), 1,
                )

    def _draw_head(self, screen, x, y, item):
        import pygame

        x = max(x, 0)
        y = max(y - 3, 0)
        head_w, head_h = 13, 7
        rect = pygame.Rect(int(x) - head_w // 2, y, head_w, head_h)
        pygame.draw.rect(screen, self.head_color, rect, border_radius=2)
        pin_x = int(x)
        pin_y = y + head_h
        pygame.draw.line(
            screen,
            self.head_color,
            (pin_x, pin_y),
            (pin_x, min(pin_y + min(item.h, 10), y + item.h + 3)),
            2,
        )

    # Set by the app once at construction; kept as an attribute so this
    # module never imports the palette (and stays headless-testable).
    perf_color = (214, 207, 193)
    head_color = (43, 45, 58)
