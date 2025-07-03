#!/bin/sh
LOG_FILE="/mnt/SDCARD/your_app.log"
# To be safe, clear the log on each run
rm -f $LOG_FILE

echo "Starting launch script" > $LOG_FILE
# Add other echos to trace progress
echo "About to execute binary" >> $LOG_FILE

export SDL_LOG_PRIORITY=debug
export SDL_LOG_CATEGORY=events,all
SDL_DEBUG=1 /mnt/SDCARD/Tools/tg5040/CardBrick.pak/cardbrick >>"$LOG_FILE" 2>&1

echo "Binary execution finished" >> $LOG_FILE
