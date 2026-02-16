#!/usr/bin/env bash
# Release build script for Network Manager
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Network Manager Release Build ==="
echo "Building in: $PROJECT_DIR"

# Clean previous build
echo "Cleaning previous build..."
cargo clean

# Build release binary
echo "Building release binary with LTO..."
cargo build --release

# Verify binary
if [[ -f "target/release/network-manager" ]]; then
    echo "✓ Binary created successfully"
    ls -lh target/release/network-manager
    file target/release/network-manager
else
    echo "✗ Build failed - binary not found"
    exit 1
fi

echo "=== Release build complete ==="
