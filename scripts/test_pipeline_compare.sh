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

# Compare balances
echo "Comparing final balances..."
BALANCE_DIFF=$(diff /tmp/st_balances.csv /tmp/mt_balances.csv | wc -l)
if [ "$BALANCE_DIFF" -eq 0 ]; then
    echo "   Final balances: ✅ MATCH (0 differences)"
else
    echo "   Final balances: ❌ DIFFER ($BALANCE_DIFF lines)"
    FAILED=1
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
