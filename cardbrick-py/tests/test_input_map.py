"""Semantic input mapping: defaults, persistence, event translation."""

import json
import os

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
os.environ.setdefault("SDL_AUDIODRIVER", "dummy")

import pygame
import pytest

from cardbrick.input_map import (DEFAULT_BUTTONS, InputMap,
                                 InputTranslator, STUDY_ACTIONS)


@pytest.fixture(scope="module", autouse=True)
def pygame_init():
    pygame.init()
    yield
    pygame.quit()


def joy_button(index, down=True):
    kind = pygame.JOYBUTTONDOWN if down else pygame.JOYBUTTONUP
    return pygame.event.Event(kind, button=index, joy=0, instance_id=0)


def test_default_mapping_loads_without_file(tmp_path):
    imap = InputMap(str(tmp_path / "nope.json"))
    assert imap.buttons == DEFAULT_BUTTONS
    assert imap.source == "defaults"


def test_custom_mapping_saves_and_loads(tmp_path):
    path = str(tmp_path / "input_mapping.json")
    imap = InputMap(path)
    imap.set_button(9, "start")
    imap.set_button(8, "select")
    imap.save()

    reloaded = InputMap(path)
    assert reloaded.buttons[9] == "start"
    assert reloaded.buttons[8] == "select"
    assert reloaded.source == path
    # One button per semantic slot: old start binding was dropped.
    assert list(reloaded.buttons.values()).count("start") == 1


def test_invalid_mapping_file_falls_back_to_defaults(tmp_path):
    path = tmp_path / "input_mapping.json"
    path.write_text("{ not json !!!")
    imap = InputMap(str(path))
    assert imap.buttons == DEFAULT_BUTTONS


def test_unknown_semantics_in_file_are_ignored(tmp_path):
    path = tmp_path / "input_mapping.json"
    path.write_text(json.dumps(
        {"buttons": {"0": "south_button", "1": "warp_drive"}}))
    imap = InputMap(str(path))
    assert imap.buttons == {0: "south_button"}


def test_translate_mapped_button(tmp_path):
    translator = InputTranslator(InputMap(str(tmp_path / "m.json")))
    assert translator.translate(joy_button(0)) == "east_button"
    assert translator.translate(joy_button(1)) == "south_button"
    assert translator.translate(joy_button(7)) == "start"


def test_keyboard_a_and_enter_are_confirm_semantics(tmp_path):
    translator = InputTranslator(InputMap(str(tmp_path / "m.json")))
    a = pygame.event.Event(pygame.KEYDOWN, key=pygame.K_a)
    enter = pygame.event.Event(pygame.KEYDOWN, key=pygame.K_RETURN)
    b = pygame.event.Event(pygame.KEYDOWN, key=pygame.K_b)

    assert translator.translate(a) == "east_button"
    assert translator.translate(enter) == "east_button"
    assert translator.translate(b) == "south_button"


def test_unknown_joystick_event_does_not_crash(tmp_path):
    translator = InputTranslator(InputMap(str(tmp_path / "m.json")))
    assert translator.translate(joy_button(42)) == "unmapped"
    weird = pygame.event.Event(pygame.JOYBALLMOTION, ball=0, rel=(1, 1))
    assert translator.translate(weird) is None


def test_hat_translates_to_dpad(tmp_path):
    translator = InputTranslator(InputMap(str(tmp_path / "m.json")))
    hat = pygame.event.Event(pygame.JOYHATMOTION, hat=0, value=(0, 1))
    assert translator.translate(hat) == "dpad_up"
    hat = pygame.event.Event(pygame.JOYHATMOTION, hat=0, value=(-1, 0))
    assert translator.translate(hat) == "dpad_left"
    centre = pygame.event.Event(pygame.JOYHATMOTION, hat=0, value=(0, 0))
    assert translator.translate(centre) is None


def test_axis_emits_once_per_threshold_crossing(tmp_path):
    translator = InputTranslator(InputMap(str(tmp_path / "m.json")))

    def axis(index, value):
        return pygame.event.Event(pygame.JOYAXISMOTION, axis=index,
                                  value=value)
    assert translator.translate(axis(1, 0.9)) == "dpad_down"
    assert translator.translate(axis(1, 0.95)) is None   # still held
    assert translator.translate(axis(1, 0.0)) is None    # released
    assert translator.translate(axis(1, -0.9)) == "dpad_up"
    assert translator.translate(axis(3, 0.9)) is None    # not a stick axis


def test_force_exit_requires_both_held_two_seconds(tmp_path):
    fake_time = [0.0]
    translator = InputTranslator(InputMap(str(tmp_path / "m.json")),
                                 now_fn=lambda: fake_time[0])
    translator.translate(joy_button(6))   # select down
    translator.translate(joy_button(7))   # start down
    assert not translator.force_exit_held()
    fake_time[0] = 2.5
    assert translator.force_exit_held()
    translator.translate(joy_button(7, down=False))  # start released
    assert not translator.force_exit_held()


def test_every_semantic_has_a_study_action():
    from cardbrick.input_map import SEMANTIC_ACTIONS
    for semantic in SEMANTIC_ACTIONS:
        assert semantic in STUDY_ACTIONS
