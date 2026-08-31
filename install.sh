#!/usr/bin/env bash
set -euo pipefail

echo "Installing yfiles..."
cargo build --release
mkdir -p "$HOME/.local/bin"
cp target/release/yfiles "$HOME/.local/bin/yfiles"
echo "yfiles installed successfully to $HOME/.local/bin/yfiles"
