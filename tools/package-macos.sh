#!/usr/bin/env bash
set -euo pipefail

cargo install cargo-bundle --locked
brew install create-dmg
cargo bundle --release
mkdir -p dist
create-dmg \
  --volname "AuvroAI" \
  --window-pos 200 120 \
  --window-size 800 400 \
  --icon-size 100 \
  --icon "AuvroAI.app" 200 190 \
  --hide-extension "AuvroAI.app" \
  --app-drop-link 600 185 \
  "dist/AuvroAI.dmg" \
  "target/release/bundle/osx/"

ls -lh dist/*.dmg
