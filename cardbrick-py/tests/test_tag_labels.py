"""format_tag_label: display form of raw Anki tags in the topic
pickers (child CATEGORY_SELECT and Parent Mode's Categories screen).

Anki tags cannot contain spaces, so decks use underscores and "::"
hierarchy separators — both of which read badly on screen. The label
is display-only: filtering (card_matches_categories) and persistence
(active_categories / category_filter) always use the raw tag.
"""

from cardbrick.textutil import format_tag_label


def test_plain_tag_is_unchanged():
    assert format_tag_label("verbs") == "verbs"


def test_underscores_become_spaces():
    assert format_tag_label("Chapter_1") == "Chapter 1"


def test_hierarchy_separator_becomes_breadcrumb():
    assert format_tag_label("JLPT::N5") == "JLPT › N5"


def test_nested_hierarchy_with_underscores():
    assert (
        format_tag_label("JLPT::N5::Verbs_Group_1")
        == "JLPT › N5 › Verbs Group 1"
    )


def test_empty_hierarchy_segments_are_dropped():
    # A sloppy tag like "a::::b" should not produce empty breadcrumb
    # crumbs.
    assert format_tag_label("a::::b") == "a › b"


def test_degenerate_tag_falls_back_to_raw():
    # Nothing but separators/underscores: show the raw tag rather
    # than an empty label the child cannot see or select.
    assert format_tag_label("::") == "::"
    assert format_tag_label("_") == "_"


def test_unicode_tags_pass_through():
    assert format_tag_label("日本語::動詞") == "日本語 › 動詞"
