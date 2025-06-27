#!/usr/bin/env bash
# This script scaffolds the entire directory structure for the CardBrick project.

set -euo pipefail

echo ">>> Scaffolding CardBrick project structure..."

# Create top-level directories
mkdir -p .github/workflows
mkdir -p assets/{font,sprites}
mkdir -p scripts
mkdir -p src
mkdir -p tests
mkdir -p dist # For build artifacts like the .opk

# Create the nested source code directory structure
mkdir -p src/ui
mkdir -p src/deck
mkdir -p src/storage
mkdir -p src/audio
mkdir -p src/net

# Create placeholder files to establish the module structure
touch src/main.rs
touch src/scheduler.rs
touch src/config.rs
touch src/ui/mod.rs
touch src/ui/canvas.rs
touch src/ui/sprite.rs
touch src/ui/progress.rs
touch src/deck/mod.rs
touch src/deck/loader.rs
touch src/deck/media.rs
touch src/storage/mod.rs
touch src/storage/db.rs
touch src/storage/replay.rs
touch src/audio/mod.rs
touch src/audio/recorder.rs
touch src/audio/encoder.rs
touch src/net/mod.rs
touch src/net/http.rs
touch src/net/mdns.rs

# Create an empty readme and gitignore
touch README.md
echo "/target" > .gitignore
echo "/dist" >> .gitignore
echo "*.corrupt" >> .gitignore

echo ">>> Scaffolding complete."
echo ">>> Next steps: Populate Cargo.toml, the CI workflow, and src/main.rs"

