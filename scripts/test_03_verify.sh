#!/bin/bash
# Step 3: Run test and verify against baseline
set -e

echo "=== Step 3: Run Test and Verify ==="
cd "$(dirname "$0")/.."

echo "Running test (output to output/)..."
cargo run --release

echo ""
echo "Comparing output/ vs baseline/..."
echo ""

FAILED=0

for file in t1_balances_deposited.csv t2_balances_final.csv t2_ledger.csv t2_orderbook.csv; do
    echo -n "  $file: "
    if diff -q "baseline/default/$file" "output/$file" > /dev/null 2>&1; then
        echo "✅ MATCH"
    else
        echo "❌ MISMATCH"
        FAILED=1
    fi
done

echo ""
if [ $FAILED -eq 0 ]; then
    echo "✅ All tests passed!"
    exit 0
else
    echo "❌ Some tests failed!"
    echo ""
    echo "To see differences:"
    echo "  diff -r baseline/default/ output/"
    exit 1
fi
