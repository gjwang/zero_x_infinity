#!/bin/bash
set -e
cd "$(dirname "$0")"

echo "=========================================================="
echo "🛡️  0x11 RELEASE VERIFICATION: DEPOSIT & WITHDRAW"
echo "=========================================================="

echo ""
echo "🤖 [Agent B] Running Core Stability Tests..."
if uv run python3 test_deposit_withdraw_core.py; then
    echo "✅ Agent B: PASS"
else
    echo "❌ Agent B: FAIL"
    exit 1
fi

echo ""
echo "🏴‍☠️ [Agent A] Running Chaos & Idempotency Tests..."
if uv run python3 test_funding_idempotency.py; then
    echo "✅ Agent A: PASS"
else
    echo "❌ Agent A: FAIL"
    exit 1
fi

echo ""
echo "🔒 [Agent C] Running Security Tests..."
if uv run python3 test_funding_security.py; then
    echo "✅ Agent C: PASS"
else
    echo "❌ Agent C: FAIL"
    exit 1
fi

echo ""
echo "=========================================================="
echo "✅  ALL SYSTEMS GO: 0x11 READY FOR RELEASE"
echo "=========================================================="
