#!/bin/bash
set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║     0xInfinity - Cancel & UBSCore Verification             ║"
echo "╚════════════════════════════════════════════════════════════╝"

# Ensure we have the cancel test dataset
if [ ! -d "fixtures/test_with_cancel" ]; then
    echo "❌ fixtures/test_with_cancel not found!"
    exit 1
fi

echo "=== Step 1: Run UBSCore with Cancel Dataset ==="
# Use --input to specify the cancel dataset folder
cargo run --release -- --ubscore --input fixtures/test_with_cancel

echo ""
echo "=== Step 2: Verify Balance Events ==="
python3 scripts/verify_balance_events.py

echo ""
echo "=== Step 3: Verify Order Events ==="
python3 scripts/verify_order_events.py

echo ""
echo "✅ Cancel Test Completed Successfully"
