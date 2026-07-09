"""Text helpers: HTML stripping, audio-reference extraction, word wrapping.

The prototype does not render HTML. Anki fields are reduced to plain
text: block-level tags become newlines, everything else is stripped,
and entities are unescaped.
"""

import html
import os
import re

SOUND_RE = re.compile(r"\[sound:([^\]]+)\]")
_IMG_SRC_RE = re.compile(r'<img[^>]+src=["\']([^"\']+)["\']', re.IGNORECASE)

_BREAK_RE = re.compile(r"<\s*(?:br|/p|/div|/li|/tr)\s*/?\s*>", re.IGNORECASE)
_TAG_RE = re.compile(r"<[^>]+>")
_STYLE_RE = re.compile(r"<(style|script)[^>]*>.*?</\1>", re.IGNORECASE | re.DOTALL)


def extract_audio(text):
    """Return (text_without_sound_tags, [audio filenames])."""
    filenames = SOUND_RE.findall(text)
    return SOUND_RE.sub("", text), filenames


def extract_image_filename(html_snippet):
    """Basename of the first <img src="..."> in an HTML snippet, or None.

    Deliberately shallow — no image rendering pipeline is built here
    beyond "find the referenced file"; multiple images are not
    supported (only the first is used).
    """
    match = _IMG_SRC_RE.search(html_snippet or "")
    return os.path.basename(match.group(1)) if match else None


def clean_html(text):
    """Reduce an Anki field to displayable plain text."""
    text = _STYLE_RE.sub("", text)
    text = _BREAK_RE.sub("\n", text)
    text = _TAG_RE.sub("", text)
    text = html.unescape(text)
    # Collapse horizontal whitespace but keep intentional line breaks.
    lines = [re.sub(r"[ \t\xa0]+", " ", line).strip() for line in text.split("\n")]
    out = []
    for line in lines:
        if line:
            out.append(line)
        elif out and out[-1] != "":
            out.append("")  # keep at most one blank line in a row
    while out and out[-1] == "":
        out.pop()
    return "\n".join(out)


def format_tag_label(tag):
    """Human-friendly display form of a raw Anki tag.

    Anki tags cannot contain spaces, so decks conventionally use
    underscores ("Chapter_1") and "::" for hierarchy
    ("JLPT::N5::Verbs_Group_1"). Neither reads well on screen, so
    pickers show "JLPT › N5 › Verbs Group 1" instead. Display-only:
    filtering and persistence always use the raw tag.
    """
    parts = [part.replace("_", " ").strip() for part in tag.split("::")]
    return " › ".join(part for part in parts if part) or tag


def wrap_text(font, text, max_width):
    """Wrap text to pixel width using a pygame font.

    Splits on spaces where possible; falls back to per-character
    wrapping for long unbroken runs (e.g. Japanese, which has no
    spaces between words).
    """
    lines = []
    for paragraph in text.split("\n"):
        if not paragraph:
            lines.append("")
            continue
        current = ""
        for word in paragraph.split(" "):
            candidate = word if not current else current + " " + word
            if font.size(candidate)[0] <= max_width:
                current = candidate
                continue
            if current:
                lines.append(current)
                current = ""
            # Word alone is too wide: wrap by character.
            for ch in word:
                if current and font.size(current + ch)[0] > max_width:
                    lines.append(current)
                    current = ch
                else:
                    current += ch
        lines.append(current)
    return lines
