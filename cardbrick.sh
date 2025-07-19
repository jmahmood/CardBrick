#!/bin/sh
cd "/mnt/mmc/Roms/PORTS/cardbrick"
export LD_LIBRARY_PATH="./lib:$LD_LIBRARY_PATH"
./cardbrick
