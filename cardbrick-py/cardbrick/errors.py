"""Visible error screens for fatal startup problems.

The handheld shows no terminal, so a crash must become a readable
screen: what went wrong, where the log is, and how to exit. If even the
display cannot be initialised, the error still goes to the log/stderr.
"""

import logging

log = logging.getLogger(__name__)


def show_error_screen(title, detail, log_path=None,
                      next_action="Fix the problem and restart the app."):
    """Best-effort full-screen error. Blocks until the user exits.

    Safe to call whether or not pygame is already initialised; never
    raises (a broken error screen must not mask the original error).
    """
    log.error("FATAL: %s — %s", title, detail)
    try:
        _pygame_error_screen(title, detail, log_path, next_action)
    except Exception as exc:  # noqa: BLE001 - last resort path
        log.error("could not display error screen (%s); details above",
                  exc)


def _pygame_error_screen(title, detail, log_path, next_action):
    import pygame

    from .app import _load_font, wrap_text

    if not pygame.get_init():
        pygame.init()
    if pygame.display.get_surface() is None:
        screen = pygame.display.set_mode((640, 480))
    else:
        screen = pygame.display.get_surface()
    for i in range(pygame.joystick.get_count()):
        pygame.joystick.Joystick(i).init()

    font_big = _load_font(30)
    font = _load_font(20)
    clock = pygame.time.Clock()
    w, h = screen.get_size()

    lines = []
    for chunk in (detail, "", next_action,
                  f"Log: {log_path}" if log_path else ""):
        if chunk == "":
            lines.append("")
            continue
        lines.extend(wrap_text(font, chunk, w - 80))

    waiting = True
    while waiting:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                waiting = False
            elif event.type == pygame.KEYDOWN and event.key in (
                    pygame.K_ESCAPE, pygame.K_RETURN, pygame.K_q):
                waiting = False
            elif event.type == pygame.JOYBUTTONDOWN:
                # Any START-ish or SELECT-ish button exits; indices are
                # untrusted here, so accept any button after a moment.
                waiting = False

        screen.fill((40, 16, 16))
        surf = font_big.render(title, True, (255, 200, 200))
        screen.blit(surf, ((w - surf.get_width()) // 2, 60))
        y = 140
        for line in lines:
            if line:
                text = font.render(line, True, (230, 225, 225))
                screen.blit(text, ((w - text.get_width()) // 2, y))
            y += 28
        hint = font.render("Press any button (or Esc) to exit", True,
                           (180, 160, 160))
        screen.blit(hint, ((w - hint.get_width()) // 2, h - 60))
        pygame.display.flip()
        clock.tick(15)
    pygame.quit()
