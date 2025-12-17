#!/bin/bash
# Step 2: Generate baseline (golden files)
set -e

echo "=== Step 2: Generate Baseline ==="
cd "$(dirname "$0")/.."

echo "Building release..."
cargo build --release 2>&1 | grep -v "^$" | tail -5

echo ""
echo "Running with --ubscore --baseline..."
cargo run --release -- --ubscore --baseline

echo ""
echo "Baseline files:"
ls -la baseline/

echo ""
echo "✅ Step 2 Complete"
