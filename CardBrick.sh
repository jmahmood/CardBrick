#!/bin/sh
# Enforce a conservative FPS cap on device to reduce CPU/battery.
export CARDBRICK_FORCE_FPS_CAP=1
export CARDBRICK_TARGET_FPS=${CARDBRICK_TARGET_FPS:-20}
RUST_LOGS=debug RUST_BACKTRACE=1 /storage/applications/CardBrick/cardbrick  >> /var/log/cardbrick.log 2>&1
