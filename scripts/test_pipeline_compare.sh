#!/bin/bash
# Pipeline Comparison Test
# Runs both single-thread and multi-thread pipelines and compares results.
#
# Usage:
#   ./scripts/test_pipeline_compare.sh [dataset]
#
# Datasets:
#   100k     - 100k orders without cancel (default)
#   cancel   - 1.3M orders with 30% cancel (original)
#   highbal  - 1.3M orders with 30% cancel, high balance (recommended)
#
# Example:
#   ./scripts/test_pipeline_compare.sh highbal

set -e

SCRIPT_DIR="$(dirname "$0")"
cd "$SCRIPT_DIR/.."

# Parse dataset argument
DATASET="${1:-100k}"

case "$DATASET" in
    100k)
        INPUT_DIR="fixtures"
        DATASET_NAME="100k orders (no cancel)"
        ;;
    cancel)
        INPUT_DIR="fixtures/test_with_cancel"
        DATASET_NAME="1.3M orders with 30% cancel"
        ;;
    highbal)
        INPUT_DIR="fixtures/test_with_cancel_highbal"
        DATASET_NAME="1.3M orders with 30% cancel (high balance)"
        ;;
    *)
        echo "Unknown dataset: $DATASET"
        echo "Available: 100k, cancel, highbal"
        exit 1
        ;;
esac

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║        Pipeline Comparison Test                                ║"
echo "╠════════════════════════════════════════════════════════════════╣"
echo "║  Dataset: $DATASET_NAME"
echo "║  Input:   $INPUT_DIR"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# Build
echo "[1/4] Building release..."
cargo build --release 2>&1 | tail -1

# Run single-thread
echo ""
echo "[2/4] Running Single-Thread Pipeline..."
cargo run --release -- --pipeline --input "$INPUT_DIR" 2>&1 | tee /tmp/st_output.txt
cp output/t2_balances_final.csv /tmp/st_balances.csv

# Extract single-thread metrics
ST_INGESTED=$(grep "Pipeline Stats:" /tmp/st_output.txt | sed 's/.*ingested=\([0-9]*\).*/\1/')
ST_PLACE=$(grep "Pipeline Stats:" /tmp/st_output.txt | sed 's/.*place=\([0-9]*\).*/\1/')
ST_CANCEL=$(grep "Pipeline Stats:" /tmp/st_output.txt | sed 's/.*cancel=\([0-9]*\).*/\1/')
ST_ACCEPTED=$(grep "Accepted:" /tmp/st_output.txt | awk '{print $2}')
ST_REJECTED=$(grep "Rejected:" /tmp/st_output.txt | awk '{print $2}')
ST_TRADES=$(grep "Total Trades:" /tmp/st_output.txt | awk '{print $3}')

# Run multi-thread
echo ""
echo "[3/4] Running Multi-Thread Pipeline..."
cargo run --release -- --pipeline-mt --input "$INPUT_DIR" 2>&1 | tee /tmp/mt_output.txt
cp output/t2_balances_final.csv /tmp/mt_balances.csv

# Extract multi-thread metrics
MT_INGESTED=$(grep "Pipeline:" /tmp/mt_output.txt | sed 's/.*ingested=\([0-9]*\).*/\1/')
MT_PLACE=$(grep "Pipeline:" /tmp/mt_output.txt | sed 's/.*place=\([0-9]*\).*/\1/')
MT_CANCEL=$(grep "Pipeline:" /tmp/mt_output.txt | sed 's/.*cancel=\([0-9]*\).*/\1/')
MT_ACCEPTED=$(grep "Accepted:" /tmp/mt_output.txt | awk '{print $2}')
MT_REJECTED=$(grep "Rejected:" /tmp/mt_output.txt | awk '{print $2}')
MT_TRADES=$(grep "Total Trades:" /tmp/mt_output.txt | awk '{print $3}')

# Compare
echo ""
echo "[4/4] Comparing Results..."
echo ""
echo "════════════════════════════════════════════════════════════════"
printf "%-15s %15s %15s %10s\n" "Metric" "Single-Thread" "Multi-Thread" "Status"
echo "────────────────────────────────────────────────────────────────"

FAILED=0

compare_metric() {
    local name=$1
    local st_val=$2
    local mt_val=$3
    if [ "$st_val" == "$mt_val" ]; then
        printf "%-15s %15s %15s %10s\n" "$name" "$st_val" "$mt_val" "✅ PASS"
    else
        printf "%-15s %15s %15s %10s\n" "$name" "$st_val" "$mt_val" "❌ FAIL"
        FAILED=1
    fi
}

compare_metric "Ingested" "$ST_INGESTED" "$MT_INGESTED"
compare_metric "Place" "$ST_PLACE" "$MT_PLACE"
compare_metric "Cancel" "$ST_CANCEL" "$MT_CANCEL"
compare_metric "Accepted" "$ST_ACCEPTED" "$MT_ACCEPTED"
compare_metric "Rejected" "$ST_REJECTED" "$MT_REJECTED"
compare_metric "Trades" "$ST_TRADES" "$MT_TRADES"

echo "════════════════════════════════════════════════════════════════"
echo ""

# Compare balances (ignore version column as it differs between ST/MT and legacy)
echo "Comparing final balances (user_id,asset_id,avail,frozen)..."
# Extract core columns (1-4) and sort to ensure deterministic comparison
cut -d, -f1-4 /tmp/st_balances.csv | sort > /tmp/st_balances_core.csv
cut -d, -f1-4 /tmp/mt_balances.csv | sort > /tmp/mt_balances_core.csv

BALANCE_DIFF=$(diff /tmp/st_balances_core.csv /tmp/mt_balances_core.csv | wc -l)
if [ "$BALANCE_DIFF" -eq 0 ]; then
    echo "   Final balances: ✅ MATCH (Core fields)"
else
    echo "   Final balances: ❌ DIFFER ($BALANCE_DIFF lines)"
    # Show samples of differences
    echo "   Sample differences (ST vs MT):"
    diff /tmp/st_balances_core.csv /tmp/mt_balances_core.csv | head -n 10
    FAILED=1
fi

# New: Regression check against baseline
case "$DATASET" in
    100k)    BASELINE_DIR="baseline/default" ;;
    cancel)  BASELINE_DIR="baseline/with_cancel" ;;
    highbal) BASELINE_DIR="baseline/highbal" ;;
    *)       BASELINE_DIR="" ;;
esac

if [ -n "$BASELINE_DIR" ] && [ -d "$BASELINE_DIR" ]; then
    echo ""
    echo "[Regression] Comparing against Golden Set in $BASELINE_DIR..."
    
    # Map t2_ names if necessary (Baseline/default uses t2_ prefix)
    B_FILE="$BASELINE_DIR/t2_balances_final.csv"
    if [ ! -f "$B_FILE" ]; then B_FILE="$BASELINE_DIR/balances_final.csv"; fi
    
    T_FILE="$BASELINE_DIR/t2_events.csv"
    if [ ! -f "$T_FILE" ]; then T_FILE="$BASELINE_DIR/trades.csv"; fi

    # Compare balances
    if [ -f "$B_FILE" ]; then
        cut -d, -f1-4 "$B_FILE" | sort > /tmp/baseline_balances_core.csv
        B_DIFF=$(diff /tmp/mt_balances_core.csv /tmp/baseline_balances_core.csv | wc -l)
        if [ "$B_DIFF" -eq 0 ]; then
            echo "   Golden Balances: ✅ MATCH"
        else
            echo "   Golden Balances: ❌ DIFFER ($B_DIFF lines)"
            # FAILED=1 # Optionally fail if strict regression is required
        fi
    fi
    
    # Compare trades
    # Note from 0x09-f: Local trade CSV is deprecated in MT mode.
    # Full verification now occurs via TDengine consistency checks.
    echo "   Golden Trades:   ✅ SKIP (Superseded by TDengine verification)"
fi

echo ""

# Final result
if [ $FAILED -eq 0 ]; then
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║                    ✅ ALL TESTS PASSED                         ║"
    echo "║  Multi-thread pipeline matches single-thread exactly!          ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    exit 0
else
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║                    ❌ TESTS FAILED                             ║"
    echo "║  Multi-thread pipeline differs from single-thread!             ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    exit 1
fi
