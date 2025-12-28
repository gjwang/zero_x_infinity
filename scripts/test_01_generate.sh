#!/bin/bash
# Step 1: Generate test data
set -e

echo "=== Step 1: Generate Test Data ==="
cd "$(dirname "$0")/.."

ORDERS=${1:-100000}
SEED=${2:-42}

echo "Generating $ORDERS orders with seed $SEED..."
uv run scripts/generate_orders.py --orders $ORDERS --price 85000 --seed $SEED

echo ""
echo "Generated files:"
ls -la fixtures/

echo ""
echo "✅ Step 1 Complete"
