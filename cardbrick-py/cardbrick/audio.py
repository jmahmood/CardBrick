"""Audio playback via pygame.mixer.

Fails soft: if the device has no working audio output (common when
testing over SSH or in CI), playback becomes a no-op instead of
crashing the reviewer.
"""

import os

import pygame


class AudioPlayer:
    def __init__(self, media_dir):
        self.media_dir = media_dir
        try:
            pygame.mixer.init()
            self.enabled = True
        except pygame.error:
            self.enabled = False

    def play(self, filename):
        """Play a media file by its imported name. Returns True if played."""
        if not self.enabled or not filename:
            return False
        path = os.path.join(self.media_dir, os.path.basename(filename))
        if not os.path.exists(path):
            return False
        try:
            pygame.mixer.music.stop()
            pygame.mixer.music.load(path)
            pygame.mixer.music.play()
            return True
        except pygame.error:
            return False

    def stop(self):
        if self.enabled:
            pygame.mixer.music.stop()
