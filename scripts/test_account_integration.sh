#!/bin/bash
# Integration test script for Phase 0x0A Account System
# Tests Gateway endpoints for assets and symbols

set -e  # Exit on error

echo "=== Phase 0x0A Integration Test ==="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
GATEWAY_PORT=8080
GATEWAY_URL="http://localhost:${GATEWAY_PORT}"
WAIT_TIME=5

# Step 1: Start PostgreSQL
echo -e "${YELLOW}[1/6]${NC} Starting PostgreSQL..."
docker-compose up -d postgres
sleep 3

# Check PostgreSQL is ready
echo "Waiting for PostgreSQL to be ready..."
for i in {1..10}; do
    if docker exec postgres pg_isready -U trading > /dev/null 2>&1; then
        echo -e "${GREEN}✅ PostgreSQL is ready${NC}"
        break
    fi
    if [ $i -eq 10 ]; then
        echo -e "${RED}❌ PostgreSQL failed to start${NC}"
        exit 1
    fi
    sleep 1
done

# Step 2: Build the project
echo -e "${YELLOW}[2/6]${NC} Building project..."
cargo build --release
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Build successful${NC}"

# Step 3: Start Gateway in background
echo -e "${YELLOW}[3/6]${NC} Starting Gateway..."
cargo run --release -- --gateway --env dev > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!
echo "Gateway PID: $GATEWAY_PID"

# Wait for Gateway to start
echo "Waiting for Gateway to start..."
sleep $WAIT_TIME

# Check if Gateway is running
if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo -e "${RED}❌ Gateway failed to start${NC}"
    cat /tmp/gateway.log
    exit 1
fi

# Check health endpoint
for i in {1..10}; do
    if curl -s "${GATEWAY_URL}/api/v1/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Gateway is ready${NC}"
        break
    fi
    if [ $i -eq 10 ]; then
        echo -e "${RED}❌ Gateway health check failed${NC}"
        kill $GATEWAY_PID 2>/dev/null || true
        cat /tmp/gateway.log
        exit 1
    fi
    sleep 1
done

# Step 4: Test /api/v1/assets endpoint
echo -e "${YELLOW}[4/6]${NC} Testing /api/v1/assets endpoint..."
ASSETS_RESPONSE=$(curl -s "${GATEWAY_URL}/api/v1/assets")
echo "Response: $ASSETS_RESPONSE"

# Check if response contains expected data
if echo "$ASSETS_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Assets endpoint returned success${NC}"
else
    echo -e "${RED}❌ Assets endpoint failed${NC}"
    kill $GATEWAY_PID 2>/dev/null || true
    exit 1
fi

# Verify BTC, USDT, ETH exist
if echo "$ASSETS_RESPONSE" | jq -e '.data | map(.asset) | contains(["BTC", "USDT", "ETH"])' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Found expected assets: BTC, USDT, ETH${NC}"
else
    echo -e "${RED}❌ Missing expected assets${NC}"
    kill $GATEWAY_PID 2>/dev/null || true
    exit 1
fi

# Step 5: Test /api/v1/symbols endpoint
echo -e "${YELLOW}[5/6]${NC} Testing /api/v1/symbols endpoint..."
SYMBOLS_RESPONSE=$(curl -s "${GATEWAY_URL}/api/v1/symbols")
echo "Response: $SYMBOLS_RESPONSE"

# Check if response contains expected data
if echo "$SYMBOLS_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Symbols endpoint returned success${NC}"
else
    echo -e "${RED}❌ Symbols endpoint failed${NC}"
    kill $GATEWAY_PID 2>/dev/null || true
    exit 1
fi

# Verify BTC_USDT exists
if echo "$SYMBOLS_RESPONSE" | jq -e '.data | map(.symbol) | contains(["BTC_USDT"])' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Found expected symbol: BTC_USDT${NC}"
else
    echo -e "${RED}❌ Missing expected symbol${NC}"
    kill $GATEWAY_PID 2>/dev/null || true
    exit 1
fi

# Step 6: Cleanup
echo -e "${YELLOW}[6/6]${NC} Cleaning up..."
kill $GATEWAY_PID 2>/dev/null || true
wait $GATEWAY_PID 2>/dev/null || true
echo -e "${GREEN}✅ Gateway stopped${NC}"

# Summary
echo ""
echo "=== Integration Test Summary ==="
echo -e "${GREEN}✅ All tests passed!${NC}"
echo ""
echo "Tested endpoints:"
echo "  - GET /api/v1/assets"
echo "  - GET /api/v1/symbols"
echo ""
echo "To stop PostgreSQL:"
echo "  docker-compose down"
