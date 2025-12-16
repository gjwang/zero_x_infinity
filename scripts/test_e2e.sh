#!/bin/bash
# E2E Test: Complete test flow
# Usage: ./test_e2e.sh [--regenerate]
#   --regenerate: regenerate baseline before running test
set -e

SCRIPT_DIR="$(dirname "$0")"
cd "$SCRIPT_DIR/.."

echo "╔════════════════════════════════════════════════════════════╗"
echo "║     0xInfinity Testing Framework - E2E Test                ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check if --regenerate flag is passed
if [ "$1" == "--regenerate" ]; then
    echo ">>> Regenerating baseline..."
    echo ""
    
    # Step 1: Generate data
    bash "$SCRIPT_DIR/test_01_generate.sh"
    echo ""
    
    # Step 2: Generate baseline
    bash "$SCRIPT_DIR/test_02_baseline.sh"
    echo ""
fi

# Step 3: Run and verify
bash "$SCRIPT_DIR/test_03_verify.sh"

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    E2E Test Complete                       ║"
echo "╚════════════════════════════════════════════════════════════╝"
