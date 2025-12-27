#!/bin/bash
# verify_0x10_release.sh
# Unified E2E Test Runner for Phase 0x10.5 Release
#
# Runs:
# 1. Regression: OpenAPI E2E
# 2. Functional: Public Trades (REST) & WebSocket (Ticker/Depth/Trade)
# 3. Security: Adversarial QA

echo "========================================================"
echo "🚀 Starting Phase 0x10.5 Release Verification Suite"
echo "========================================================"

# 0. Check Gateway Health
echo ">> Checking Gateway availability..."
if ! curl -s http://localhost:8080/api/v1/health > /dev/null; then
    echo "❌ Gateway is NOT running on port 8080."
    echo "   Please run: source scripts/lib/db_env.sh && cargo run --release -- --gateway --port 8080"
    exit 1
fi
echo "✅ Gateway is UP"

# 1. Regression Tests
echo ""
echo "--------------------------------------------------------"
echo "🔍 1. Regression Tests (OpenAPI E2E)"
echo "--------------------------------------------------------"
if python3 scripts/test_openapi_e2e.py; then
    echo "✅ Regression Tests PASSED"
else
    echo "❌ Regression Tests FAILED"
    exit 1
fi

# 2. Functional Tests (Using Developer's Robust Scripts)
echo ""
echo "--------------------------------------------------------"
echo "✨ 2. Functional Tests (0x10.5 Backend Gaps)"
echo "--------------------------------------------------------"

echo ">> 2.1 Public Trades (REST)"
if ./scripts/test_public_trades_e2e.sh; then
    echo "✅ REST API PASSED"
else
    echo "❌ REST API FAILED"
    exit 1
fi

echo ">> 2.2 WebSocket Channels"
# Run them as a group
if python3 scripts/test_websocket_ticker_e2e.py && \
   python3 scripts/test_websocket_depth_e2e.py && \
   python3 scripts/test_websocket_public_e2e.py; then
   echo "✅ WebSocket Channels PASSED"
else
   echo "❌ WebSocket Channels FAILED"
   exit 1
fi

# 3. Security Tests
echo ""
echo "--------------------------------------------------------"
echo "🛡️ 3. Security Tests (Adversarial QA)"
echo "--------------------------------------------------------"
if python3 scripts/test_qa_adversarial.py; then
    echo "✅ Security Tests PASSED"
else
    echo "❌ Security Tests FAILED (CRITICAL VULNERABILITY DETECTED)"
    echo "   Action: REJECT RELEASE"
    echo "   See 0x10-qa-rejection-report.md"
    exit 1
fi

echo ""
echo "========================================================"
echo "✅ ALL SYSTEMS GO - Release Verified for 0x10.5"
echo "========================================================"
exit 0
