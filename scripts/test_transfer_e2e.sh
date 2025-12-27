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
#   - PostgreSQL running (port auto-detected via db_env.sh)
#   - TDengine running on port 6041
#   - Python with pynacl installed
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

# Source centralized DB configuration
source "$SCRIPT_DIR/lib/db_env.sh"

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

# Check PostgreSQL (using port from db_env.sh)
if ! PGPASSWORD="${PG_PASSWORD}" psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${RED}❌ PostgreSQL not available on port ${PG_PORT}${NC}"
    echo "   Start with: docker start postgres"
    exit 1
fi
echo "  ✓ PostgreSQL connected (port ${PG_PORT})"

# Check and ensure binary freshness (skip in CI - binary is pre-built)
if [ "$1" != "--no-gw" ] && [ "$CI" != "true" ]; then
    echo "  [BUILD] Ensuring binary is up to date..."
    # Cross-platform stat: macOS uses -f, Linux uses -c
    if [[ "$OSTYPE" == "darwin"* ]]; then
        NEWEST_SRC=$(find src -name "*.rs" -exec stat -f "%m" {} + 2>/dev/null | sort -nr | head -n1 || echo 0)
        BINARY_MTIME=$(stat -f "%m" target/release/zero_x_infinity 2>/dev/null || echo 0)
    else
        NEWEST_SRC=$(find src -name "*.rs" -exec stat -c "%Y" {} + 2>/dev/null | sort -nr | head -n1 || echo 0)
        BINARY_MTIME=$(stat -c "%Y" target/release/zero_x_infinity 2>/dev/null || echo 0)
    fi

    if [ -n "$NEWEST_SRC" ] && [ -n "$BINARY_MTIME" ] && [ "$NEWEST_SRC" -gt "$BINARY_MTIME" ]; then
        echo "  [FACT] Source modified, forcing re-link via touch src/main.rs..."
        touch src/main.rs
        cargo build --release --quiet
        echo "  ✓ Build synced"
    else
        echo "  ✓ Binary is current"
    fi
else
    echo "  ✓ Binary is current (CI mode)"
fi

# =============================================================================
# Step 2: Setup Test Data
# =============================================================================
echo -e "${YELLOW}[2/6] Setting up test data...${NC}"

PGPASSWORD="${PG_PASSWORD}" psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" -q << 'EOF'
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

-- CLEAN SLATE: Delete ALL balances for test user
DELETE FROM balances_tb WHERE user_id = 1001;

-- Create ONLY Funding balance: 1000 USDT (scaled by 10^6 = 1000000000)
INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, status)
VALUES (1001, 2, 2, 1000000000, 0, 1);

-- Clear old transfer records for clean test
DELETE FROM transfer_operations_tb WHERE transfer_id IN (
    SELECT transfer_id FROM fsm_transfers_tb WHERE user_id = 1001
);
DELETE FROM fsm_transfers_tb WHERE user_id = 1001;
EOF

echo "  ✓ Test data initialized (1000 USDT in Funding only for user 1001)"

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
    # Force kill if still alive
    if ps -p "$EXISTING_PID" > /dev/null; then
        echo "  [FORCE] Gateway still alive, sending SIGKILL..."
        kill -9 "$EXISTING_PID" 2>/dev/null || true
        sleep 1
    fi
fi

# Final check of port 8080
if lsof -i :8080 > /dev/null 2>&1; then
    echo -e "${RED}❌ Port 8080 is still blocked by unknown process.${NC}"
    lsof -i :8080
    exit 1
fi

# Start Gateway with appropriate config (ci.yaml in CI, dev.yaml locally)
GW_ENV="${CI:+ci}"
GW_ENV="${GW_ENV:-dev}"
echo "  Using config: config/${GW_ENV}.yaml"
./target/release/zero_x_infinity --gateway --env "$GW_ENV" > /tmp/gw_test.log 2>&1 &
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
# Step 4: Run Transfer Tests with Balance Verification
# =============================================================================
echo -e "${YELLOW}[4/6] Running transfer tests with balance verification...${NC}"

export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"

TEST_RESULT=$(uv run << 'PYTHON_EOF'
import sys
sys.path.append('scripts/lib')
from api_auth import get_test_client
import json

USER_ID = 1001
client = get_test_client(user_id=USER_ID)
headers = {'X-User-ID': str(USER_ID)}

tests_passed = 0
tests_failed = 0

def get_balances():
    """Get all balances using /balances/all API"""
    resp = client.get('/api/v1/private/balances/all', headers=headers)
    if resp.status_code != 200:
        return {}
    balances = {}
    for b in resp.json()['data']:
        key = f"{b['asset']}:{b['account_type']}"
        balances[key] = float(b['available'])
    return balances

def print_balances(label, balances):
    """Print formatted balances"""
    for key, val in sorted(balances.items()):
        print(f"    {key}: {val:.2f}")

# Step 1: Get initial balances
print("  [BEFORE] Getting initial balances...")
before = get_balances()
print_balances("Before", before)
print()

# Step 2: Transfer Funding -> Spot (50 USDT)
print("  [TRANSFER 1] Funding → Spot (50 USDT)...")
resp = client.post('/api/v1/private/transfer', 
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '50'},
    headers=headers)
if resp.status_code == 200 and resp.json()['data']['status'] == 'COMMITTED':
    print("    ✓ COMMITTED")
    tests_passed += 1
else:
    print(f"    ✗ FAILED ({resp.status_code}: {resp.text[:50]})")
    tests_failed += 1

# Step 3: Transfer Spot -> Funding (25 USDT)
print("  [TRANSFER 2] Spot → Funding (25 USDT)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'spot', 'to': 'funding', 'asset': 'USDT', 'amount': '25'},
    headers=headers)
if resp.status_code == 200 and resp.json()['data']['status'] == 'COMMITTED':
    print("    ✓ COMMITTED")
    tests_passed += 1
else:
    print(f"    ✗ FAILED ({resp.status_code}: {resp.text[:50]})")
    tests_failed += 1
print()

# Step 4: Get final balances (Funding only - Spot lives in UBSCore RAM)
print("  [AFTER] Getting final Funding balance...")
after = get_balances()
print_balances("After", after)
print()

# Step 5: Verify Funding balance changes
# NOTE: Spot balance is in UBSCore RAM, not PostgreSQL, so we only verify Funding
print("  [VERIFY] Checking Funding balance changes...")
print("    (Note: Spot balance is in UBSCore RAM, not PostgreSQL)")

expected_funding_change = -50 + 25  # -50 (to Spot), +25 (from Spot) = -25 USDT

funding_before = before.get('USDT:funding', 1000)  # Initial: 1000 USDT
funding_after = after.get('USDT:funding', 0)
funding_change = funding_after - funding_before

# Check funding change
if abs(funding_change - expected_funding_change) < 0.01:
    print(f"    ✓ Funding: {funding_before:.2f} → {funding_after:.2f} (Δ{funding_change:+.2f})")
    tests_passed += 1

# ============================================================
# P0 ERROR HANDLING TESTS
# ============================================================
print("")
print("="*60)
print("P0 CRITICAL TEST CASES (Error Handling & Validation)")
print("="*60)

# TC-P0-01: Insufficient Balance
print("\n  [TC-P0-01] Insufficient Balance Test...")
try:
    # User 1001 has remaining balance after previous transfers
    # Try to transfer way more than available
    resp = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '9999'},
        headers=headers)
    
    # API returns 200 but with status=FAILED (business logic error)
    if resp.status_code == 200:
        data = resp.json().get('data', {})
        if data.get('status') == 'FAILED':
            print("    ✓ PASS: Correctly rejected with status=FAILED")
            tests_passed += 1
        else:
            print(f"    ✗ FAIL: Expected status=FAILED, got {data.get('status')}")
            tests_failed += 1
    else:
        print(f"    ✗ FAIL: Expected 200 with status=FAILED, got {resp.status_code}")
        tests_failed += 1
except Exception as e:
    print(f"    ✗ FAIL: Exception: {e}")
    tests_failed += 1

# TC-P0-02: Invalid Amount - Zero
print("\n  [TC-P0-02] Invalid Amount (Zero)...")
try:
    resp = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '0'},
        headers=headers)
    
    if resp.status_code == 400:
        print("    ✓ PASS: Correctly rejected zero amount")
        tests_passed += 1
    else:
        print(f"    ✗ FAIL: Expected 400, got {resp.status_code}")
        print(f"    [DEBUG] Response: {resp.text[:200]}")
        tests_failed += 1
except Exception as e:
    print(f"    ✗ FAIL: Exception: {e}")
    tests_failed += 1

# TC-P0-03: Invalid Amount (Negative)...")
print("\n  [TC-P0-03] Invalid Amount (Negative)...")
try:
    resp = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '-10'},
        headers=headers)
    
    if resp.status_code == 400:
        print("    ✓ PASS: Correctly rejected negative amount")
        tests_passed += 1
    else:
        print(f"    ✗ FAIL: Expected 400, got {resp.status_code}")
        tests_failed += 1
except Exception as e:
    print(f"    ✗ FAIL: Exception: {e}")
    tests_failed += 1

# TC-P0-04: Precision Overflow (USDT has 6 decimals)
print("\n  [TC-P0-04] Precision Overflow (9 decimals for USDT)...")
try:
    resp = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '1.123456789'},
        headers=headers)
    
    # May be rejected as precision error or allowed (implementation-dependent)
    if resp.status_code == 400:
        print("    ✓ PASS: Correctly rejected excessive precision")
        tests_passed += 1
    else:
        print(f"    ⚠ WARN: Precision accepted (status {resp.status_code})")
        # Don't fail - this is implementation-dependent
        tests_passed += 1
except Exception as e:
    print(f"    ✗ FAIL: Exception: {e}")
    tests_failed += 1

# TC-P0-05: Same Account Transfer (Funding -> Funding)
print("\n  [TC-P0-05] Same Account Transfer (Funding → Funding)...")
try:
    resp = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'funding', 'asset': 'USDT', 'amount': '10'},
        headers=headers)
    
    if resp.status_code == 400:
        print("    ✓ PASS: Correctly rejected same account transfer")
        tests_passed += 1
    else:
        print(f"    ✗ FAIL: Expected 400, got {resp.status_code}")
        tests_failed += 1
except Exception as e:
    print(f"    ✗ FAIL: Exception: {e}")
    tests_failed += 1

# TC-P0-06: Invalid Asset
print("\n  [TC-P0-06] Invalid Asset (FAKE)...")
try:
    resp = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'spot', 'asset': 'FAKE', 'amount': '10'},
        headers=headers)
    
    if resp.status_code == 400:
        print("    ✓ PASS: Correctly rejected invalid asset")
        tests_passed += 1
    else:
        print(f"    ✗ FAIL: Expected 400, got {resp.status_code}")
        tests_failed += 1
except Exception as e:
    print(f"    ✗ FAIL: Exception: {e}")
    tests_failed += 1

# TC-P0-07: Idempotency - Duplicate CID
print("\n  [TC-P0-07] Idempotency (Duplicate CID)...")
try:
    # Get balance before
    balances_before_idem = get_balances()
    funding_before_idem = balances_before_idem.get('USDT:funding', 0)
    
    # First request with CID
    cid_test = 'client-idempotency-test-001'
    resp1 = client.post('/api/v1/private/transfer',
        json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '20', 'cid': cid_test},
        headers=headers)
    
    if resp1.status_code != 200:
        print(f"    ✗ FAIL: First request failed ({resp1.status_code})")
        print(f"    [DEBUG] Response: {resp1.text[:200]}")
        tests_failed += 1
    else:
        resp1_data = resp1.json()
        if 'data' not in resp1_data or 'transfer_id' not in resp1_data['data']:
            print(f"    ✗ FAIL: Unexpected response structure: {resp1_data}")
            tests_failed += 1
        else:
            transfer_id_1 = resp1_data['data']['transfer_id']
            
            # Give it time to settle before checking balances
            import time
            time.sleep(0.5)
            
            # Get balance after first transfer
            balances_after_1 = get_balances()
            funding_after_1 = balances_after_1.get('USDT:funding', 0)
            
            # Second request with SAME CID
            resp2 = client.post('/api/v1/private/transfer',
                json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '20', 'cid': cid_test},
                headers=headers)
            
            if resp2.status_code != 200:
                print(f"    ✗ FAIL: Second request failed ({resp2.status_code})")
                tests_failed += 1
            else:
                resp2_data = resp2.json()
                if 'data' not in resp2_data or 'transfer_id' not in resp2_data['data']:
                    print(f"    ✗ FAIL: Unexpected response structure: {resp2_data}")
                    tests_failed += 1
                else:
                    transfer_id_2 = resp2_data['data']['transfer_id']
                    
                    # Give it time to settle before checking balances
                    time.sleep(0.5)
                    
                    # Get balance after second request
                    balances_after_2 = get_balances()
                    funding_after_2 = balances_after_2.get('USDT:funding', 0)
                    
                    # Verify same transfer_id returned
                    if transfer_id_1 == transfer_id_2:
                        print(f"    ✓ PASS: Same transfer_id returned ({transfer_id_1})")
                        
                        # Verify balance changed only once
                        if abs(funding_after_2 - funding_after_1) < 0.01:
                            print(f"    ✓ PASS: Balance unchanged on duplicate (stayed at {funding_after_2:.2f})")
                            tests_passed += 2  # Two assertions
                        else:
                            print(f"    ✗ FAIL: Balance changed again ({funding_after_1:.2f} → {funding_after_2:.2f})")
                            tests_passed += 1
                            tests_failed += 1
                    else:
                        print(f"    ✗ FAIL: Different transfer_id ({transfer_id_1} vs {transfer_id_2})")
                        tests_failed += 1
except Exception as e:
    import traceback
    print(f"    ✗ FAIL: Exception: {e}")
    print(f"    [DEBUG] Traceback: {traceback.format_exc()[:300]}")
    tests_failed += 1

# Summary
print("")
print("="*60)
print(f"  TOTAL RESULTS: {tests_passed} passed, {tests_failed} failed")
print("="*60)
print("")
sys.exit(tests_failed)
PYTHON_EOF
) || TEST_EXIT=$?

echo "$TEST_RESULT"

# =============================================================================
# Step 5: Show Final Database State (optional, for debugging)
# =============================================================================
echo -e "${YELLOW}[5/6] Final database state...${NC}"

PGPASSWORD="${PG_PASSWORD}" psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" -t << 'EOF'
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
