#!/bin/bash
# =============================================================================
# Internal Transfer E2E Test Script
# Phase 0x0B-a: Funding <-> Spot Transfer Verification
# =============================================================================
#
# Usage:
#   ./scripts/test_transfer_e2e.sh           # Run with auto-start Gateway
#   ./scripts/test_transfer_e2e.sh --no-gw   # Run against existing Gateway
#
# Prerequisites:
#   - PostgreSQL running on port 5433
#   - TDengine running on port 6041
#   - Python with pynacl installed
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=============================================="
echo "Internal Transfer E2E Test (Phase 0x0B-a)"
echo "=============================================="
echo ""

# =============================================================================
# Step 1: Check Prerequisites
# =============================================================================
echo -e "${YELLOW}[1/6] Checking prerequisites...${NC}"

# Check PostgreSQL
if ! PGPASSWORD=trading123 psql -h localhost -p 5433 -U trading -d exchange_info_db -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${RED}❌ PostgreSQL not available on port 5433${NC}"
    echo "   Start with: docker start postgres"
    exit 1
fi
echo "  ✓ PostgreSQL connected"

# Check release build
if [ ! -f target/release/zero_x_infinity ]; then
    echo "  Building release binary..."
    cargo build --release --quiet
fi
echo "  ✓ Release binary ready"

# =============================================================================
# Step 2: Setup Test Data
# =============================================================================
echo -e "${YELLOW}[2/6] Setting up test data...${NC}"

PGPASSWORD=trading123 psql -h localhost -p 5433 -U trading -d exchange_info_db -q << 'EOF'
-- Enable internal transfer for USDT (add 0x10 = 16 to flags)
UPDATE assets_tb SET asset_flags = asset_flags | 16 WHERE asset_id = 2;

-- Ensure balances table exists
CREATE TABLE IF NOT EXISTS balances_tb (
    id SERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    asset_id INT NOT NULL,
    account_type INT NOT NULL DEFAULT 1,
    available DECIMAL(30, 8) NOT NULL DEFAULT 0,
    frozen DECIMAL(30, 8) NOT NULL DEFAULT 0,
    version INT NOT NULL DEFAULT 1,
    status INT NOT NULL DEFAULT 1,
    UNIQUE (user_id, asset_id, account_type)
);

-- Create test balance: 1000 USDT in Funding for user 1001
INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, status)
VALUES (1001, 2, 2, 1000000000, 0, 1)
ON CONFLICT (user_id, asset_id, account_type) DO UPDATE SET available = 1000000000;

-- Clear old transfer records for clean test
DELETE FROM fsm_transfers_tb WHERE user_id = 1001;
DELETE FROM transfer_operations_tb WHERE req_id IN (
    SELECT req_id FROM fsm_transfers_tb WHERE user_id = 1001
);
EOF

echo "  ✓ Test data initialized (1000 USDT in Funding for user 1001)"

# =============================================================================
# Step 3: Start Gateway (always restart to load updated asset_flags)
# =============================================================================
echo -e "${YELLOW}[3/6] Starting Gateway...${NC}"

# Kill existing Gateway (using correct method per agent-testing-notes.md)
EXISTING_PID=$(pgrep -f "./target/release/zero_x_infinity" 2>/dev/null | head -1 || true)
if [ -n "$EXISTING_PID" ]; then
    echo "  Stopping existing Gateway (PID: $EXISTING_PID)"
    kill "$EXISTING_PID" 2>/dev/null || true
    sleep 2
fi

# Start Gateway
./target/release/zero_x_infinity --gateway --env dev > /tmp/gw_test.log 2>&1 &
GW_PID=$!
echo "  Gateway started (PID: $GW_PID)"

# Wait for Gateway to be ready
for i in {1..15}; do
    if curl -s --max-time 1 http://localhost:8080/api/v1/health > /dev/null 2>&1; then
        echo "  ✓ Gateway ready"
        break
    fi
    if [ $i -eq 15 ]; then
        echo -e "${RED}❌ Gateway failed to start${NC}"
        cat /tmp/gw_test.log | tail -20
        exit 1
    fi
    sleep 1
done

# =============================================================================
# Step 4: Run Transfer Tests
# =============================================================================
echo -e "${YELLOW}[4/6] Running transfer tests...${NC}"

export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"

TEST_RESULT=$(python3 << 'PYTHON_EOF'
import sys
sys.path.append('scripts/lib')
from api_auth import get_test_client

USER_ID = 1001
client = get_test_client(user_id=USER_ID)

tests_passed = 0
tests_failed = 0

# Test 1: Funding -> Spot
resp = client.post('/api/v1/private/transfer', 
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '50'},
    headers={'X-User-ID': str(USER_ID)})
if resp.status_code == 200 and resp.json()['data']['status'] == 'COMMITTED':
    print("  ✓ TEST 1: Funding → Spot (50 USDT) - COMMITTED")
    tests_passed += 1
else:
    print(f"  ✗ TEST 1: Funding → Spot - FAILED ({resp.status_code})")
    tests_failed += 1

# Test 2: Spot -> Funding  
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'spot', 'to': 'funding', 'asset': 'USDT', 'amount': '25'},
    headers={'X-User-ID': str(USER_ID)})
if resp.status_code == 200 and resp.json()['data']['status'] == 'COMMITTED':
    print("  ✓ TEST 2: Spot → Funding (25 USDT) - COMMITTED")
    tests_passed += 1
else:
    print(f"  ✗ TEST 2: Spot → Funding - FAILED ({resp.status_code})")
    tests_failed += 1

# Summary
print(f"\n  Results: {tests_passed} passed, {tests_failed} failed")
sys.exit(tests_failed)
PYTHON_EOF
) || TEST_EXIT=$?

echo "$TEST_RESULT"

# =============================================================================
# Step 5: Verify Database State
# =============================================================================
echo -e "${YELLOW}[5/6] Verifying database state...${NC}"

PGPASSWORD=trading123 psql -h localhost -p 5433 -U trading -d exchange_info_db -t << 'EOF'
SELECT 
    CASE account_type WHEN 1 THEN 'Spot' WHEN 2 THEN 'Funding' END as account,
    (available / 1000000)::text || ' USDT' as balance
FROM balances_tb 
WHERE user_id = 1001 AND asset_id = 2
ORDER BY account_type;
EOF

# =============================================================================
# Step 6: Cleanup
# =============================================================================
echo -e "${YELLOW}[6/6] Cleanup...${NC}"

if [ -n "$GW_PID" ]; then
    echo "  Stopping Gateway (PID: $GW_PID)"
    kill "$GW_PID" 2>/dev/null || true
fi

# Final result
echo ""
if [ "${TEST_EXIT:-0}" -eq 0 ]; then
    echo -e "${GREEN}=============================================="
    echo "✅ All E2E Transfer Tests PASSED"
    echo "==============================================${NC}"
    exit 0
else
    echo -e "${RED}=============================================="
    echo "❌ Some E2E Transfer Tests FAILED"
    echo "==============================================${NC}"
    exit 1
fi
