#!/bin/bash
# Generate Single-Thread Pipeline Baseline
# This creates the reference baseline that multi-thread results are compared against.
#
# Usage: 
#   ./scripts/test_pipeline_baseline.sh                    # Default: 100k orders
#   ./scripts/test_pipeline_baseline.sh --with-cancel      # 1.3M orders with cancel
#
# Output:
#   baseline/pipeline_st/         - 100k baseline (strict comparison)
#   baseline/pipeline_st_cancel/  - 1.3M baseline (allows timing variance)

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
echo "║  Generate Single-Thread Pipeline Baseline                  ║"
echo "╠════════════════════════════════════════════════════════════╣"
echo "║  Dataset: $DATASET_NAME"
echo "║  Input:   $INPUT_DIR"
echo "║  Output:  $BASELINE_DIR"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Build release
echo "[1/3] Building release..."
cargo build --release 2>&1 | tail -1

# Run single-thread pipeline
echo "[2/3] Running single-thread pipeline..."
cargo run --release -- --pipeline --input "$INPUT_DIR" 2>&1 | tee /tmp/pipeline_st_output.txt

# Extract key metrics
ACCEPTED=$(grep "Accepted:" /tmp/pipeline_st_output.txt | awk '{print $2}')
REJECTED=$(grep "Rejected:" /tmp/pipeline_st_output.txt | awk '{print $2}')
TRADES=$(grep "Total Trades:" /tmp/pipeline_st_output.txt | awk '{print $3}')

echo ""
echo "[3/3] Saving baseline to $BASELINE_DIR..."
mkdir -p "$BASELINE_DIR"

# Copy output files to baseline
cp output/t2_balances_final.csv "$BASELINE_DIR/"
cp output/t2_summary.txt "$BASELINE_DIR/"
cp output/t2_perf.txt "$BASELINE_DIR/"
cp output/t2_ledger.csv "$BASELINE_DIR/"

# Save key metrics
cat > "$BASELINE_DIR/metrics.txt" << EOF
# Single-Thread Pipeline Baseline
# Dataset: $DATASET_NAME
# Input: $INPUT_DIR
# Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
accepted=$ACCEPTED
rejected=$REJECTED
trades=$TRADES
EOF

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║  Baseline Generated Successfully                           ║"
echo "╠════════════════════════════════════════════════════════════╣"
echo "║  Accepted: $ACCEPTED"
echo "║  Rejected: $REJECTED"
echo "║  Trades:   $TRADES"
echo "╚════════════════════════════════════════════════════════════╝"
