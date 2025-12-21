#!/bin/bash
# scripts/generate_baseline.sh - Generate baseline data for regression testing
#
# Usage: ./scripts/generate_baseline.sh [100k|1.3m]

set -e

DATASET=$1
if [ -z "$DATASET" ]; then
    echo "Usage: $0 [100k|1.3m]"
    exit 1
fi

FORCE=false
if [ "$2" == "--force" ] || [ "$2" == "-f" ]; then
    FORCE=true
fi

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$PROJECT_DIR/baseline/$DATASET"

if [ -d "$TARGET_DIR" ] && [ "$FORCE" = false ]; then
    echo -e "Error: Baseline for $DATASET already exists at $TARGET_DIR"
    echo -e "Use --force or -f to overwrite if you are SURE the new output is the new ground truth."
    exit 1
fi

TEMP_DIR="/tmp/baseline_gen"

mkdir -p "$TARGET_DIR"
mkdir -p "$TEMP_DIR"

echo "Generating baseline for $DATASET..."

# 1. Start clean environment (script handles TDengine start if needed)
# Using test_persistence.sh logic for 100k or manually running for highbal
if [ "$DATASET" == "100k" ]; then
    echo "Running 100K (Single-Thread) to capture Ground Truth..."
    ./target/release/zero_x_infinity --mode pipeline_st --dataset fixtures/orders.csv
    cp output/t2_orderbook.csv "$TARGET_DIR/orders_final.csv"
    cp output/t2_ledger_events.csv "$TARGET_DIR/trades.csv"
    cp output/t2_balances_final.csv "$TARGET_DIR/balances_final.csv"
elif [ "$DATASET" == "1.3m" ]; then
    echo "Running 1.3M (Single-Thread) to capture Ground Truth..."
    ./target/release/zero_x_infinity --mode pipeline_st --dataset fixtures/test_with_cancel_highbal
    cp output/t2_orderbook.csv "$TARGET_DIR/orders_final.csv"
    cp output/t2_ledger_events.csv "$TARGET_DIR/trades.csv"
    cp output/t2_balances_final.csv "$TARGET_DIR/balances_final.csv"
else
    echo "Unknown dataset: $DATASET"
    exit 1
fi

# Create README in baseline folder if not exists
if [ ! -f "$PROJECT_DIR/baseline/README.md" ]; then
    cat > "$PROJECT_DIR/baseline/README.md" <<EOF
# Regression Testing Baselines

These files represent the known correct state of the engine after processing specific datasets.
They are used to verify that future changes do not break correctness or consistency.

## Datasets

- **100k**: Standard 100,000 order dataset (fixtures/orders.csv)
- **1.3m**: High-frequency dataset with high balance and 300,000 cancels (fixtures/test_with_cancel_highbal)

## Content

- **orders_final.csv**: Final state of all orders (status, filled_qty, etc.)
- **trades.csv**: All trade events generated during matching.
- **balances_final.csv**: Final available and frozen balances for all users.
EOF
fi

echo "Baseline for $DATASET generated in $TARGET_DIR"
