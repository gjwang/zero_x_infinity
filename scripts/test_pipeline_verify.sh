#!/bin/bash
# Verify Multi-Thread Pipeline Against Single-Thread Baseline
# Runs multi-thread pipeline and compares results with single-thread baseline.
# ALL results must match 100% - any difference is a failure.
#
# Usage:
#   ./scripts/test_pipeline_verify.sh                    # Default: 100k orders
#   ./scripts/test_pipeline_verify.sh --with-cancel      # 1.3M orders with cancel
#
# Prerequisites:
#   Run test_pipeline_baseline.sh first to generate baseline

set -e

SCRIPT_DIR="$(dirname "$0")"
cd "$SCRIPT_DIR/.."

# Parse arguments
if [ "$1" == "--with-cancel" ]; then
    INPUT_DIR="fixtures/test_with_cancel"
    BASELINE_DIR="baseline/pipeline_st_cancel"
    DATASET_NAME="1.3M with cancel"
else
    INPUT_DIR="fixtures"
    BASELINE_DIR="baseline/pipeline_st"
    DATASET_NAME="100k (default)"
fi

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  Verify Multi-Thread Pipeline vs Single-Thread Baseline    ║"
echo "╠════════════════════════════════════════════════════════════╣"
echo "║  Dataset: $DATASET_NAME"
echo "║  Input:   $INPUT_DIR"
echo "║  Baseline: $BASELINE_DIR"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check baseline exists
if [ ! -f "$BASELINE_DIR/metrics.txt" ]; then
    echo "❌ ERROR: Baseline not found at $BASELINE_DIR"
    echo "   Run: ./scripts/test_pipeline_baseline.sh first"
    exit 1
fi

# Load baseline metrics
source "$BASELINE_DIR/metrics.txt"
BASELINE_ACCEPTED=$accepted
BASELINE_REJECTED=$rejected
BASELINE_TRADES=$trades

echo "📊 Baseline (Single-Thread):"
echo "   Accepted: $BASELINE_ACCEPTED"
echo "   Rejected: $BASELINE_REJECTED"
echo "   Trades:   $BASELINE_TRADES"
echo ""

# Build release
echo "[1/3] Building release..."
cargo build --release 2>&1 | tail -1

# Run multi-thread pipeline
echo "[2/3] Running multi-thread pipeline..."
cargo run --release -- --pipeline-mt --input "$INPUT_DIR" 2>&1 | tee /tmp/pipeline_mt_output.txt

# Extract metrics
MT_ACCEPTED=$(grep "Accepted:" /tmp/pipeline_mt_output.txt | awk '{print $2}')
MT_REJECTED=$(grep "Rejected:" /tmp/pipeline_mt_output.txt | awk '{print $2}')
MT_TRADES=$(grep "Total Trades:" /tmp/pipeline_mt_output.txt | awk '{print $3}')

echo ""
echo "[3/3] Comparing results..."
echo ""
echo "📊 Multi-Thread Results:"
echo "   Accepted: $MT_ACCEPTED"
echo "   Rejected: $MT_REJECTED"
echo "   Trades:   $MT_TRADES"
echo ""

# Compare
FAILED=0

echo "════════════════════════════════════════════════════════════"
printf "%-15s %15s %15s %10s\n" "Metric" "Single-Thread" "Multi-Thread" "Status"
echo "────────────────────────────────────────────────────────────"

# Check Accepted
if [ "$MT_ACCEPTED" == "$BASELINE_ACCEPTED" ]; then
    printf "%-15s %15s %15s %10s\n" "Accepted" "$BASELINE_ACCEPTED" "$MT_ACCEPTED" "✅ PASS"
else
    printf "%-15s %15s %15s %10s\n" "Accepted" "$BASELINE_ACCEPTED" "$MT_ACCEPTED" "❌ FAIL"
    FAILED=1
fi

# Check Rejected
if [ "$MT_REJECTED" == "$BASELINE_REJECTED" ]; then
    printf "%-15s %15s %15s %10s\n" "Rejected" "$BASELINE_REJECTED" "$MT_REJECTED" "✅ PASS"
else
    printf "%-15s %15s %15s %10s\n" "Rejected" "$BASELINE_REJECTED" "$MT_REJECTED" "❌ FAIL"
    FAILED=1
fi

# Check Trades
if [ "$MT_TRADES" == "$BASELINE_TRADES" ]; then
    printf "%-15s %15s %15s %10s\n" "Trades" "$BASELINE_TRADES" "$MT_TRADES" "✅ PASS"
else
    printf "%-15s %15s %15s %10s\n" "Trades" "$BASELINE_TRADES" "$MT_TRADES" "❌ FAIL"
    FAILED=1
fi

echo "════════════════════════════════════════════════════════════"
echo ""

# Compare final balances
echo "Comparing final balances..."
if diff -q output/t2_balances_final.csv "$BASELINE_DIR/t2_balances_final.csv" > /dev/null 2>&1; then
    echo "   Final balances: ✅ MATCH"
else
    echo "   Final balances: ❌ DIFFER"
    FAILED=1
    # Show first few differences
    echo "   First differences:"
    diff output/t2_balances_final.csv "$BASELINE_DIR/t2_balances_final.csv" | head -10
fi

echo ""

# Final result
if [ $FAILED -eq 0 ]; then
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║              ✅ ALL TESTS PASSED                           ║"
    echo "║  Multi-thread pipeline matches single-thread exactly!      ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    exit 0
else
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║              ❌ TESTS FAILED                               ║"
    echo "║  Multi-thread pipeline differs from single-thread!         ║"
    echo "║  This indicates a bug that needs to be fixed.              ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    exit 1
fi
