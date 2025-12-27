#!/bin/bash
set -e

# One-Shot Verification Script
# Automates the entire lifecycle: Cleanup -> Setup -> Build -> Init DB -> Start Gateway -> Verify

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

echo "========================================================="
echo "🔄 Starting One-Shot Verification (From Scratch)"
echo "========================================================="

# 1. Cleanup
echo ""
echo "🧹 [1/6] Cleaning up old processes..."
# ⚠️ CRITICAL: Do NOT use pkill -f "zero_x_infinity" as it kills the IDE language server
GW_PID=$(pgrep -f "./target/release/zero_x_infinity" | head -1)
if [ -n "$GW_PID" ]; then
    echo "   Killing existing Gateway (PID: $GW_PID)..."
    kill "$GW_PID" || true
fi
# Kill any lingering test runners
pkill -f "test_qa_adversarial.py" || true
sleep 1

# 2. Setup Env
echo ""
echo "🛠️ [2/6] Verifying Environment..."
./scripts/setup-dev.sh

# 3. Database Init
echo ""
echo "📦 [3/6] Initializing Databases..."
# We need to ensure we can run these. Assuming standard dev env ports.
./scripts/db/init.sh pg
./scripts/db/init.sh td
sleep 2

# 4. Build
echo ""
echo "🏗️ [4/6] Building Gateway (Release)..."
cargo build --release

# 5. Start Gateway
echo ""
echo "🚀 [5/6] Starting Gateway..."
# Source environment variables for the gateway
source scripts/lib/db_env.sh
# Run in background, verify with curl loop
./target/release/zero_x_infinity --gateway --port 8080 > gateway_oneshot.log 2>&1 &
GATEWAY_PID=$!

echo "   Waiting for Gateway health check..."
STARTED=false
for i in {1..30}; do
    if curl -s http://localhost:8080/api/v1/health > /dev/null; then
        echo "   ✅ Gateway is UP"
        STARTED=true
        break
    fi
    sleep 1
done

if [ "$STARTED" = false ]; then
    echo "   ❌ Gateway failed to start. Logs:"
    cat gateway_oneshot.log
    kill $GATEWAY_PID || true
    exit 1
fi

# 6. Verify
echo ""
echo "🧪 [6/6] Running Verification Suite..."
# Using the standard release verification script
# Note: verify_0x10_release.sh internally uses `uv run` for its Python steps
./scripts/verify_0x10_release.sh
VERIFY_EXIT_CODE=$?

# 7. Teardown
echo ""
echo "🛑 [7/6] Teardown..."
kill $GATEWAY_PID || true

if [ $VERIFY_EXIT_CODE -eq 0 ]; then
    echo ""
    echo "========================================================="
    echo "✅ ONE SHOT VERIFICATION PASSED"
    echo "========================================================="
    exit 0
else
    echo ""
    echo "========================================================="
    echo "❌ ONE SHOT VERIFICATION FAILED"
    echo "========================================================="
    exit $VERIFY_EXIT_CODE
fi
