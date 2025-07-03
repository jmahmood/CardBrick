#!/usr/bin/env bash

rm -rf ~/CardBrick/output
mkdir ~/CardBrick/output
cp ~/CardBrick/launch.sh ~/CardBrick/output
# cp -R ~/CardBrick/assets ~/CardBrick/output
# cp -R ~/CardBrick/decks ~/CardBrick/output
docker build --platform linux/arm64 --progress=plain --output ~/CardBrick/output -f ~/CardBrick/toolchain/Dockerfile.simplified .
rsync -rtDvz ~/CardBrick/output/* root@10.0.0.210:/mnt/SDCARD/Tools/tg5040/CardBrick.pak
