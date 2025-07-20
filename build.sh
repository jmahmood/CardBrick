#!/usr/bin/env bash

cross build --release --target aarch64-unknown-linux-gnu

# rsync -rtDvz ~/CardBrick/output/* root@10.0.0.27:/mnt/mmc/Roms/PORTS/CardBrick

rsync -avz ~/CardBrick/assets/icon-large.png root@10.0.0.159:/storage/applications/CardBrick
rsync -avz ~/CardBrick/assets/gameinfo.xml root@10.0.0.159:/storage/applications/
rsync -avz ~/CardBrick/assets/cardbrick.png root@10.0.0.159:/storage/applications/CardBrick
rsync -avz ~/CardBrick/assets/decks root@10.0.0.159:/storage/applications/CardBrick/decks
scp ~/CardBrick/target/aarch64-unknown-linux-gnu/release/cardbrick root@10.0.0.159:/storage/applications/CardBrick/
scp ~/CardBrick/CardBrick.sh root@10.0.0.159:/storage/applications

