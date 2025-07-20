#!/bin/sh
RUST_LOGS=debug RUST_BACKTRACE=1 /storage/applications/CardBrick/cardbrick  >> /var/log/cardbrick.log 2>&1
