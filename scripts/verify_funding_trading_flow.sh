#!/bin/bash
set -e

# Zero X Infinity - Master Verification Script
# Runs all Phase 0x11 and 0x12 QA tests.

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

echo "🚀 Starting Full E2E Verification Suite..."

# 1. Phase 0x11: Funding Core (Deposit/Withdraw/Idempotency)
echo "---------------------------------------------------"
echo "📦 Phase 0x11: Funding Core Verification"
echo "---------------------------------------------------"
# This script handles its own Gateway lifecycle
bash "$PROJECT_ROOT/scripts/tests/0x11_funding/run_qa_full.sh"

EXIT_11=$?
if [ $EXIT_11 -ne 0 ]; then
    echo "❌ Phase 0x11 Verification FAILED!"
    exit 1
fi

# 2. Phase 0x12: Trading Integration (Deposit -> Transfer -> Trade -> Withdraw)
# Includes Address Validation
echo "---------------------------------------------------"
echo "🤝 Phase 0x12: Trading Integration Verification"
echo "---------------------------------------------------"
# This script handles its own Gateway lifecycle
bash "$PROJECT_ROOT/scripts/tests/0x12_integration/run_poc.sh"

EXIT_12=$?
if [ $EXIT_12 -ne 0 ]; then
    echo "❌ Phase 0x12 Verification FAILED!"
    exit 1
fi

echo "---------------------------------------------------"
echo "✅  ALL SYSTEMS GO: Full E2E Verification PASSED"
echo "---------------------------------------------------"
exit 0
