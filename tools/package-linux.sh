#!/usr/bin/env bash
set -euo pipefail

cargo install cargo-deb --locked
cargo deb --release
find target/debian -maxdepth 1 -name "*.deb" -print
