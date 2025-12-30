#!/bin/bash
set -e

# =============================================================================
# 🚀 ZERO X INFINITY - MASTER SYSTEM VERIFICATION 🚀
# =============================================================================
# "The One Script To Rule Them All"
# 
# Covers:
# 1. Phase 0x11: Funding Core (Deposit, Withdraw, Idempotency)
# 2. Phase 0x12: Trading Integration (Spot, Matching, Settlement)
# 3. Phase 0x13: Transfer Operations (Internal Transfers)
# 4. API Surface: OpenAPI E2E & Admin API
#
# Usage:
#   ./scripts/verify_full_system.sh
# 
# Env Vars:
#   GATEWAY_BINARY: Path to gateway binary (default: target/release/zero_x_infinity)
#   Skip specific stages with SKIP_0x11=1, SKIP_0x12=1, etc.
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
LOG_DIR="${PROJECT_ROOT}/logs"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}======================================================${NC}"
echo -e "${BLUE}       FULL SYSTEM VERIFICATION SUITE       ${NC}"
echo -e "${BLUE}======================================================${NC}"

mkdir -p "$LOG_DIR"

# -----------------------------------------------------------------------------
# 0. Build & Prep
# -----------------------------------------------------------------------------
echo -e "\n${YELLOW}[0/5] Preparing Environment...${NC}"

# Ensure binary exists
GATEWAY_BIN="${GATEWAY_BINARY:-$PROJECT_ROOT/target/release/zero_x_infinity}"
if [ ! -f "$GATEWAY_BIN" ]; then
    echo -e "${RED}❌ Binary not found at $GATEWAY_BIN${NC}"
    echo "Please build with: cargo build --release"
    # Fallback to debug if release not found? No, strictly require build.
    if [ -f "$PROJECT_ROOT/target/debug/zero_x_infinity" ]; then
        echo -e "${YELLOW}⚠️  Release binary not found, falling back to debug...${NC}"
        GATEWAY_BIN="$PROJECT_ROOT/target/debug/zero_x_infinity"
    else
        exit 1
    fi
else
    echo "✅ Binary verified: $GATEWAY_BIN"
fi
export GATEWAY_BINARY="$GATEWAY_BIN"

# Source Utils
source "$PROJECT_ROOT/scripts/lib/test_utils.sh"
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

# -----------------------------------------------------------------------------
# 1. Phase 0x11: Funding Core
# -----------------------------------------------------------------------------
if [ -z "$SKIP_0x11" ]; then
    echo -e "\n${BLUE}[1/5] Phase 0x11: Funding Core (Deposit/Withdraw)${NC}"
    echo "---------------------------------------------------"
    
    # Ensure clean state
    "$PROJECT_ROOT/scripts/db/clean.sh" all
    
    # Run Validations
    if bash "$PROJECT_ROOT/scripts/tests/0x11_funding/run_qa_full.sh"; then
        echo -e "${GREEN}✅ Phase 0x11 Passed${NC}"
    else
        echo -e "${RED}❌ Phase 0x11 Failed${NC}"
        exit 1
    fi
else
    echo -e "\n${YELLOW}[1/5] Phase 0x11: Skipped${NC}"
fi

# -----------------------------------------------------------------------------
# 2. Phase 0x12: Trading Integration
# -----------------------------------------------------------------------------
if [ -z "$SKIP_0x12" ]; then
    echo -e "\n${BLUE}[2/5] Phase 0x12: Trading Integration${NC}"
    echo "---------------------------------------------------"
    
    # Clean state
    "$PROJECT_ROOT/scripts/db/clean.sh" all
    
    if bash "$PROJECT_ROOT/scripts/tests/0x12_integration/run_poc.sh"; then
        echo -e "${GREEN}✅ Phase 0x12 Passed${NC}"
    else
        echo -e "${RED}❌ Phase 0x12 Failed${NC}"
        exit 1
    fi
else
    echo -e "\n${YELLOW}[2/5] Phase 0x12: Skipped${NC}"
fi

# -----------------------------------------------------------------------------
# 3. Phase 0x13: Transfer Operations
# -----------------------------------------------------------------------------
if [ -z "$SKIP_0x13" ]; then
    echo -e "\n${BLUE}[3/5] Phase 0x13: Transfer Operations${NC}"
    echo "---------------------------------------------------"
    
    # Clean state
    "$PROJECT_ROOT/scripts/db/clean.sh" all
    
    # test_transfer_e2e.sh expects Gateway running? 
    # Let's check test_transfer_e2e.sh... it starts its own gateway!
    
    if bash "$PROJECT_ROOT/scripts/test_transfer_e2e.sh"; then
        echo -e "${GREEN}✅ Phase 0x13 Passed${NC}"
    else
        echo -e "${RED}❌ Phase 0x13 Failed${NC}"
        exit 1
    fi
else
    echo -e "\n${YELLOW}[3/5] Phase 0x13: Skipped${NC}"
fi

# -----------------------------------------------------------------------------
# 4. API Surface: OpenAPI & Admin
# -----------------------------------------------------------------------------
if [ -z "$SKIP_API" ]; then
    echo -e "\n${BLUE}[4/5] API Surface Verification${NC}"
    echo "---------------------------------------------------"
    
    # 4.1 Admin API
    echo ">> Running Admin API E2E..."
    "$PROJECT_ROOT/scripts/db/clean.sh" all
    if bash "$PROJECT_ROOT/scripts/run_admin_tests_standalone.sh"; then
         echo -e "${GREEN}  ✅ Admin API Passed${NC}"
    else
         echo -e "${RED}  ❌ Admin API Failed${NC}"
         exit 1
    fi
    
    # 4.2 OpenAPI E2E
    echo -e "\n>> [4.2] Running OpenAPI E2E..."
    "$PROJECT_ROOT/scripts/db/clean.sh" all
    
    # Init DB ensuring seed data exists (init.sh runs seed)
    "$PROJECT_ROOT/scripts/db/init.sh" pg

    # Start Gateway for OpenAPI
    echo "Starting Gateway for OpenAPI tests..."
    "$GATEWAY_BIN" > "$LOG_DIR/gateway_openapi.log" 2>&1 &
    GATEWAY_PID=$!
    
    # Trap to ensure cleanup if script exits early here
    trap "kill $GATEWAY_PID 2>/dev/null || true" EXIT

    if wait_for_gateway 8080; then
        echo "Gateway is up."
    else
        echo "Gateway failed to start."
        exit 1
    fi
    
    if bash "$PROJECT_ROOT/scripts/test_openapi_e2e.sh"; then
        echo -e "${GREEN}  ✅ OpenAPI E2E Passed${NC}"
    else
        echo -e "${RED}  ❌ OpenAPI E2E Failed${NC}"
        kill "$GATEWAY_PID" 2>/dev/null || true
        wait "$GATEWAY_PID" 2>/dev/null || true
        exit 1
    fi
    
    kill "$GATEWAY_PID" 2>/dev/null || true
    wait "$GATEWAY_PID" 2>/dev/null || true
    trap - EXIT # Reset trap to default
fi

echo -e "\n${GREEN}======================================================${NC}"
echo -e "${GREEN}       ALL SYSTEMS VERIFIED SUCCESSFULLY            ${NC}"
echo -e "${GREEN}======================================================${NC}"
exit 0
