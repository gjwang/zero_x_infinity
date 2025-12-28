#!/bin/bash
# Phase 0x11-a: Sentinel E2E Test
# One-click test for the complete Sentinel blockchain scanning flow
#
# Prerequisites:
# - Docker running (for bitcoind)
# - PostgreSQL running on port 5433
#
# This script:
# 1. Starts bitcoind regtest container (if not running)
# 2. Applies database migration
# 3. Generates test blocks
# 4. Runs Sentinel scanner
# 5. Verifies chain_cursor table
#
# // turbo-all

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RESET='\033[0m'

# Database config (from config/dev.yaml)
DB_HOST="127.0.0.1"
DB_PORT="5433"
DB_USER="trading"
DB_PASS="trading123"
DB_NAME="exchange_info_db"

# BTC config
BTC_USER="admin"
BTC_PASS="admin"
BTC_PORT="18443"

echo "========================================"
echo "  Phase 0x11-a: Sentinel E2E Test"
echo "========================================"

# Step 1: Check Docker
echo ""
echo -e "${YELLOW}Step 1: Check Docker containers${RESET}"

# Check bitcoind - use docker ps --filter to be more accurate
if docker ps --format '{{.Names}}' | grep -q "^bitcoind$"; then
    echo -e "${GREEN}✓ bitcoind container running${RESET}"
else
    echo -e "${BLUE}Starting bitcoind regtest container...${RESET}"
    # Remove old container if exists
    docker rm -f bitcoind 2>/dev/null || true
    docker run -d --name bitcoind -p 18443:18443 ruimarinho/bitcoin-core:24 \
        -printtoconsole -regtest=1 \
        -rpcbind=0.0.0.0 -rpcallowip=0.0.0.0/0 \
        -rpcuser=$BTC_USER -rpcpassword=$BTC_PASS
    sleep 3
    echo -e "${GREEN}✓ bitcoind started${RESET}"
fi

if docker ps --format '{{.Names}}' | grep -q "postgres"; then
    echo -e "${GREEN}✓ PostgreSQL container running${RESET}"
else
    echo -e "${RED}✗ PostgreSQL not running on port $DB_PORT${RESET}"
    echo "  Please start PostgreSQL: docker run -d -p 5433:5432 -e POSTGRES_DB=$DB_NAME -e POSTGRES_USER=$DB_USER -e POSTGRES_PASSWORD=$DB_PASS postgres"
    exit 1
fi

# Step 2: Apply migration
echo -e "\n${YELLOW}Step 2: Apply database migration${RESET}"
PGPASSWORD=$DB_PASS psql -U $DB_USER -d $DB_NAME -h $DB_HOST -p $DB_PORT \
    -c "CREATE TABLE IF NOT EXISTS chain_cursor (
        chain_id VARCHAR(16) PRIMARY KEY,
        last_scanned_height BIGINT NOT NULL DEFAULT 0,
        last_scanned_hash VARCHAR(128) NOT NULL DEFAULT '',
        updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    );" 2>/dev/null || true
echo -e "${GREEN}✓ chain_cursor table ready${RESET}"

# Reset cursor for clean test
PGPASSWORD=$DB_PASS psql -U $DB_USER -d $DB_NAME -h $DB_HOST -p $DB_PORT \
    -c "DELETE FROM chain_cursor WHERE chain_id = 'BTC';" 2>/dev/null || true
echo -e "${GREEN}✓ BTC cursor reset${RESET}"

# Step 3: Create wallet and generate blocks
echo -e "\n${YELLOW}Step 3: Generate test blocks on regtest${RESET}"

# Create wallet if not exists
docker exec bitcoind bitcoin-cli -regtest -rpcuser=$BTC_USER -rpcpassword=$BTC_PASS \
    createwallet "sentinel_test" 2>/dev/null || true

# Get address
DEPOSIT_ADDR=$(docker exec bitcoind bitcoin-cli -regtest -rpcuser=$BTC_USER -rpcpassword=$BTC_PASS \
    -rpcwallet=sentinel_test getnewaddress "test")
echo "  Deposit address: $DEPOSIT_ADDR"

# Generate 5 blocks
BLOCK_HASHES=$(docker exec bitcoind bitcoin-cli -regtest -rpcuser=$BTC_USER -rpcpassword=$BTC_PASS \
    generatetoaddress 5 "$DEPOSIT_ADDR")
echo -e "${GREEN}✓ Generated 5 blocks${RESET}"

# Get current height
CURRENT_HEIGHT=$(docker exec bitcoind bitcoin-cli -regtest -rpcuser=$BTC_USER -rpcpassword=$BTC_PASS \
    getblockcount)
echo "  Current block height: $CURRENT_HEIGHT"

# Step 4: Run Sentinel (in background, since it's now a continuous loop)
echo -e "\n${YELLOW}Step 4: Run Sentinel scanner (continuous mode)${RESET}"
cd "$PROJECT_DIR"
cargo run -- --sentinel -e dev > /tmp/sentinel_output.log 2>&1 &
SENTINEL_PID=$!
echo "  Sentinel started with PID $SENTINEL_PID. Waiting 10s for scans..."
sleep 10
kill $SENTINEL_PID || true
wait $SENTINEL_PID 2>/dev/null || true
echo -e "${GREEN}✓ Sentinel cycle completed${RESET}"

# Step 5: Verify cursor
echo -e "\n${YELLOW}Step 5: Verify chain_cursor${RESET}"
CURSOR_HEIGHT=$(PGPASSWORD=$DB_PASS psql -U $DB_USER -d $DB_NAME -h $DB_HOST -p $DB_PORT -t \
    -c "SELECT last_scanned_height FROM chain_cursor WHERE chain_id = 'BTC';")
CURSOR_HEIGHT=$(echo "$CURSOR_HEIGHT" | tr -d ' ')

if [ -n "$CURSOR_HEIGHT" ] && [ "$CURSOR_HEIGHT" -ge 1 ]; then
    echo -e "${GREEN}✓ chain_cursor.last_scanned_height = $CURSOR_HEIGHT${RESET}"
else
    echo -e "${RED}✗ chain_cursor not updated (height: $CURSOR_HEIGHT)${RESET}"
    exit 1
fi

# Step 6: Verify Sentinel output
echo -e "\n${YELLOW}Step 6: Verify Sentinel logs${RESET}"
if grep -q "Connected to Bitcoin node" /tmp/sentinel_output.log; then
    echo -e "${GREEN}✓ BTC RPC connection verified${RESET}"
else
    echo -e "${RED}✗ BTC RPC connection failed${RESET}"
    exit 1
fi

if grep -q "BTC scanned block" /tmp/sentinel_output.log; then
    SCANNED_COUNT=$(grep -c "BTC scanned block" /tmp/sentinel_output.log)
    echo -e "${GREEN}✓ Scanned $SCANNED_COUNT blocks${RESET}"
else
    echo -e "${RED}✗ No blocks scanned${RESET}"
    exit 1
fi

# Summary
echo -e "\n========================================"
echo -e "${GREEN}  ✅ All Sentinel E2E Tests PASSED${RESET}"
echo -e "========================================"
echo ""
echo "Test Summary:"
echo "  - BTC RPC connection: OK"
echo "  - Block scanning: ${SCANNED_COUNT:-0} blocks"
echo "  - Cursor height: $CURSOR_HEIGHT"
echo "  - Database persistence: OK"
echo ""
echo "To run Sentinel continuously:"
echo "  cargo run -- --sentinel -e dev"
