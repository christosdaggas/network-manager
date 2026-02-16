#!/usr/bin/env bash
# Lint script for Network Manager
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Network Manager Lint Check ==="

echo ""
echo "Running cargo clippy..."
cargo clippy --release -- -D warnings

echo ""
echo "Running cargo fmt check..."
cargo fmt -- --check

echo ""
echo "=== Lint check complete ==="
