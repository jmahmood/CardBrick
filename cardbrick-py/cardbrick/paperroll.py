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


class _Item:
    __slots__ = ("kind", "surf", "h", "x")

    def __init__(self, kind, surf, h, x=None):
        self.kind = kind  # "surf" | "perf" | "gap"
        self.surf = surf
        self.h = h
        self.x = x  # None = centred in the content span


class PaperRoll:
    def __init__(self, print_y, reduced_motion=False):
        self.print_y = print_y  # screen row the write point rests on
        self.reduced_motion = reduced_motion
        self._items = []
        self._queue = deque()
        self._base = 0.0  # roll coordinate of _items[0]'s top edge
        self._total = 0.0  # roll coordinate of the write point
        self.offset = -float(print_y)  # roll coordinate of screen y=0
        self._target = self.offset
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

    def feed(self, surf, h=None, x=None):
        """Queue one printed line (or image, stamp, ...) for a line feed."""
        self._queue.append(_Item("surf", surf, surf.get_height() if h is None else h, x))
        self._maybe_snap()

    def feed_gap(self, h):
        self._queue.append(_Item("gap", None, h))
        self._maybe_snap()

    def feed_perf(self):
        """A lone perforation (tear line) without starting a new page."""
        self._queue.append(_Item("perf", None, PERF_H))
        self._maybe_snap()

    def feed_page(self, perf=True):
        """Advance to fresh paper for a new page (card).

        The perforation group is queued like any other content, so the
        tear line visibly rides up the screen. perf=False starts a page
        without the tear — used for the very first page on a roll.
        """
        if perf:
            self._queue.append(_Item("gap", None, PAGE_GAP))
            self._queue.append(_Item("perf", None, PERF_H))
            self._queue.append(_Item("gap", None, PAGE_GAP))
        self._queue.append(_Item("page-start", None, 0))
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
        self._maybe_snap()
        return True

    # -- animation ---------------------------------------------------------------

    @property
    def busy(self):
        return bool(self._queue) or abs(self._target - self.offset) > SNAP_PX

    def finish(self):
        """Snap to the fully-printed state (input interruption or
        reduced motion): commit the whole queue, land on the target."""
        while self._queue:
            self._commit(self._queue.popleft())
        self.offset = self._target
        self._prune()

    def update(self):
        """Advance the animation one tick. Returns True while moving."""
        if self.reduced_motion:
            was_busy = self.busy
            self.finish()
            return was_busy
        if self._queue and abs(self._target - self.offset) <= RELEASE_PX:
            # Blank paper and page bookkeeping ride along for free: a
            # release commits up to one *printed* item (line, image,
            # perforation) so only real content paces the feed.
            while self._queue:
                item = self._queue.popleft()
                self._commit(item)
                if item.kind not in ("gap", "page-start"):
                    break
        self.offset += (self._target - self.offset) * EASE
        if abs(self._target - self.offset) <= SNAP_PX:
            self.offset = self._target
        self._prune()
        return self.busy

    def _commit(self, item):
        if item.kind == "page-start":
            self._pages.append((self._total, len(self._items)))
        else:
            self._items.append(item)
            self._total += item.h
        self._target = self._total - self.print_y

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
            elif item.surf is not None:
                x = item.x if item.x is not None else x0 + (x1 - x0 - item.surf.get_width()) // 2
                screen.blit(item.surf, (x, int(top)))

    # Set by the app once at construction; kept as an attribute so this
    # module never imports the palette (and stays headless-testable).
    perf_color = (214, 207, 193)
