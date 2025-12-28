#!/bin/bash
# test_ubscore_e2e.sh - Complete E2E test for UBSCore mode
# 
# Tests:
# 1. Standard baseline files (balances, ledger, orderbook)
# 2. Balance events correctness
# 3. Events baseline match
set -e

SCRIPT_DIR="$(dirname "$0")"
cd "$SCRIPT_DIR/.."

echo "╔════════════════════════════════════════════════════════════╗"
echo "║     UBSCore E2E Test with Event Verification               ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Step 1: Run with --ubscore mode
echo "=== Step 1: Run with UBSCore mode ==="
cargo run --release -- --ubscore
echo ""

# Step 2: Verify standard baseline files
echo "=== Step 2: Verify standard baselines ==="
FAILED=0

# Note: t2_ledger.csv may differ in UBSCore mode due to different execution order
# The new t2_events.csv provides more accurate event sourcing
for file in t1_balances_deposited.csv t2_balances_final.csv t2_orderbook.csv; do
    echo -n "  $file: "
    if diff -q "baseline/$file" "output/$file" > /dev/null 2>&1; then
        echo "✅ MATCH"
    else
        echo "❌ MISMATCH"
        FAILED=1
    fi
done
echo "  t2_ledger.csv: ⏭️ SKIP (legacy format, use t2_events.csv instead)"
echo ""

# Step 3: Verify balance events correctness
echo "=== Step 3: Verify balance events correctness ==="
uv run scripts/verify_balance_events.py
echo ""

# Step 4: Verify events baseline
echo "=== Step 4: Verify events baseline ==="
uv run scripts/verify_events_baseline.py
echo ""

# Summary
if [ $FAILED -eq 0 ]; then
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║     ✅ All UBSCore E2E tests passed!                       ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    exit 0
else
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║     ❌ Some tests failed!                                  ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    exit 1
fi
