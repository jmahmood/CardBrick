# CardBrick

CardBrick is an offline Spanish flashcard study appliance. It imports Anki
`.apkg` files as a content format and then runs from its own SQLite database.

This PortMaster package includes its own ARM64 Python runtime and is ready to
run without installing Python, pip, or network dependencies on the device.

Requirements:

- 64-bit ARM (`aarch64`)
- glibc 2.31 or newer
- 640x480 display or larger

User data is stored in the port's `conf/` directory. Import decks from Parent
Mode or place them in the CardBrick data import directory.

This package is experimental. The first real-device smoke test should be run
from the PortMaster menu, and `cardbrick/log.txt` should be retained if the
launch fails.
