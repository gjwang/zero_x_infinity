#!/bin/bash
#
# ============================================================================
# Phase 0x11-b: ONE-CLICK E2E Test Runner
# ============================================================================
#
# 自动启动所有依赖服务并运行完整 E2E 测试
#
# 服务启动顺序:
#   1. PostgreSQL (检查连接)
#   2. BTC Node (regtest, 创建钱包)
#   3. Gateway HTTP Server
#   4. Sentinel Service
#   5. 运行 E2E 测试
#
# Usage:
#   ./scripts/run_0x11b_e2e_full.sh
#   ./scripts/run_0x11b_e2e_full.sh --skip-startup   # 跳过服务启动
#   ./scripts/run_0x11b_e2e_full.sh --cleanup        # 清理残留进程
#
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="$PROJECT_ROOT/logs"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
export GATEWAY_URL="${GATEWAY_URL:-http://127.0.0.1:8080}"
export BTC_RPC_URL="${BTC_RPC_URL:-http://127.0.0.1:18443}"
export BTC_RPC_USER="${BTC_RPC_USER:-admin}"
export BTC_RPC_PASS="${BTC_RPC_PASS:-admin}"
export BTC_WALLET="${BTC_WALLET:-sentinel_test}"

# Process tracking (安全: 不使用 pkill -f)
GATEWAY_PID=""
SENTINEL_PID=""

# ============================================================================
# Helper Functions
# ============================================================================

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

# 安全地通过 PID 停止进程
stop_process() {
    local pid=$1
    local name=$2
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        log_info "Stopping $name (PID: $pid)..."
        kill "$pid" 2>/dev/null || true
        sleep 2
        if kill -0 "$pid" 2>/dev/null; then
            log_warn "Force killing $name..."
            kill -9 "$pid" 2>/dev/null || true
        fi
    fi
}

# 通过端口获取 PID
get_pid_by_port() {
    lsof -i :"$1" 2>/dev/null | awk 'NR>1 {print $2}' | head -1
}

# 等待端口就绪
wait_for_port() {
    local port=$1
    local name=$2
    local timeout=${3:-30}
    
    log_info "Waiting for $name on port $port..."
    for i in $(seq 1 $timeout); do
        if lsof -i :"$port" >/dev/null 2>&1; then
            log_info "$name is ready on port $port"
            return 0
        fi
        sleep 1
    done
    log_error "$name failed to start on port $port within ${timeout}s"
    return 1
}

# Cleanup handler
cleanup() {
    log_info "Cleaning up..."
    stop_process "$GATEWAY_PID" "Gateway"
    stop_process "$SENTINEL_PID" "Sentinel"
}

trap cleanup EXIT

# ============================================================================
# Service Check Functions
# ============================================================================

check_postgres() {
    log_step "[1/6] Checking PostgreSQL..."
    if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
        log_info "✅ PostgreSQL is ready"
        return 0
    else
        log_error "❌ PostgreSQL not running. Please start it first."
        log_info "   brew services start postgresql@14"
        return 1
    fi
}

run_migrations() {
    log_step "[1.5/6] Running database migrations..."
    
    local db_name="${DATABASE_NAME:-zero_x_infinity}"
    local db_user="${DATABASE_USER:-postgres}"
    local db_host="${DATABASE_HOST:-localhost}"
    local db_port="${DATABASE_PORT:-5432}"
    local psql_cmd="psql -h $db_host -p $db_port -U $db_user"
    
    # Check if database exists, create if not
    if ! $psql_cmd -lqt 2>/dev/null | cut -d \| -f 1 | grep -qw "$db_name"; then
        log_info "Creating database '$db_name'..."
        createdb -h "$db_host" -p "$db_port" -U "$db_user" "$db_name" 2>/dev/null || true
    fi
    
    # If --clean-db flag, drop all tables first for fresh start
    if [[ "${CLEAN_DB:-}" == "true" ]]; then
        log_warn "   Dropping existing tables for clean start..."
        $psql_cmd -d "$db_name" -c "DROP TABLE IF EXISTS user_addresses, deposit_history, withdraw_history CASCADE;" -q 2>/dev/null || true
    fi
    
    # Run all migrations in order
    local migrations_dir="$PROJECT_ROOT/migrations"
    if [ -d "$migrations_dir" ]; then
        for migration in "$migrations_dir"/*.sql; do
            if [ -f "$migration" ]; then
                local migration_name=$(basename "$migration")
                log_info "   Running $migration_name..."
                $psql_cmd -d "$db_name" -f "$migration" -q 2>/dev/null || true
            fi
        done
        
        # Fix: Ensure chain_id column exists (handle legacy 'network' column)
        $psql_cmd -d "$db_name" -c "ALTER TABLE user_addresses RENAME COLUMN network TO chain_id;" -q 2>/dev/null || true
        
        log_info "✅ Migrations complete"
    else
        log_warn "No migrations directory found at $migrations_dir"
    fi
}

check_btc_node() {
    log_step "[2/6] Checking BTC Node..."
    
    local response
    response=$(curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
        --data-binary '{"jsonrpc":"1.0","id":"test","method":"getblockchaininfo","params":[]}' \
        -H 'content-type:text/plain;' "http://127.0.0.1:18443/" 2>&1)
    
    if echo "$response" | grep -q '"result"'; then
        local height=$(echo "$response" | grep -o '"blocks":[0-9]*' | cut -d: -f2)
        log_info "✅ BTC Node ready (height: ${height:-unknown})"
        return 0
    else
        log_error "❌ BTC Node not running or not accessible"
        log_info "   bitcoind -regtest -daemon"
        return 1
    fi
}

setup_btc_wallet() {
    log_step "[2.5/6] Ensuring BTC wallet exists..."
    
    # Check if wallet exists
    local wallet_check
    wallet_check=$(curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
        --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"check\",\"method\":\"listwallets\",\"params\":[]}" \
        -H 'content-type:text/plain;' "http://127.0.0.1:18443/" 2>&1)
    
    if echo "$wallet_check" | grep -q "\"$BTC_WALLET\""; then
        log_info "✅ Wallet '$BTC_WALLET' already loaded"
    else
        # Try to load existing wallet
        curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
            --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"load\",\"method\":\"loadwallet\",\"params\":[\"$BTC_WALLET\"]}" \
            -H 'content-type:text/plain;' "http://127.0.0.1:18443/" > /dev/null 2>&1
        
        # If load fails, create new wallet
        if ! echo "$wallet_check" | grep -q "\"$BTC_WALLET\""; then
            log_info "Creating wallet '$BTC_WALLET'..."
            curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
                --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"create\",\"method\":\"createwallet\",\"params\":[\"$BTC_WALLET\"]}" \
                -H 'content-type:text/plain;' "http://127.0.0.1:18443/" > /dev/null 2>&1
        fi
        log_info "✅ Wallet '$BTC_WALLET' ready"
    fi
    
    # Ensure we have at least 101 blocks for coinbase maturity
    local height
    height=$(curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
        --data-binary '{"jsonrpc":"1.0","id":"h","method":"getblockcount","params":[]}' \
        -H 'content-type:text/plain;' "http://127.0.0.1:18443/wallet/$BTC_WALLET" | grep -o '"result":[0-9]*' | cut -d: -f2)
    
    if [ "${height:-0}" -lt 101 ]; then
        log_info "Mining blocks to height 101..."
        local addr
        addr=$(curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
            --data-binary '{"jsonrpc":"1.0","id":"a","method":"getnewaddress","params":[]}' \
            -H 'content-type:text/plain;' "http://127.0.0.1:18443/wallet/$BTC_WALLET" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
        
        local to_mine=$((101 - height))
        curl -s --user "${BTC_RPC_USER}:${BTC_RPC_PASS}" \
            --data-binary "{\"jsonrpc\":\"1.0\",\"id\":\"m\",\"method\":\"generatetoaddress\",\"params\":[$to_mine, \"$addr\"]}" \
            -H 'content-type:text/plain;' "http://127.0.0.1:18443/wallet/$BTC_WALLET" > /dev/null
        log_info "✅ Mined $to_mine blocks"
    fi
}

start_gateway() {
    log_step "[3/6] Starting Gateway HTTP Server..."
    
    # Check if already running
    local existing_pid
    existing_pid=$(get_pid_by_port 8080)
    if [ -n "$existing_pid" ]; then
        log_warn "Gateway already running (PID: $existing_pid)"
        GATEWAY_PID="$existing_pid"
        return 0
    fi
    
    mkdir -p "$LOG_DIR"
    nohup cargo run -- --gateway > "$LOG_DIR/gateway_e2e.log" 2>&1 &
    GATEWAY_PID=$!
    
    wait_for_port 8080 "Gateway" 30 || return 1
    log_info "✅ Gateway started (PID: $GATEWAY_PID)"
}

start_sentinel() {
    log_step "[4/6] Starting Sentinel Service..."
    
    mkdir -p "$LOG_DIR"
    nohup cargo run -- --sentinel > "$LOG_DIR/sentinel_e2e.log" 2>&1 &
    SENTINEL_PID=$!
    
    # Sentinel doesn't have HTTP port, wait for log output
    sleep 5
    if kill -0 "$SENTINEL_PID" 2>/dev/null; then
        log_info "✅ Sentinel started (PID: $SENTINEL_PID)"
    else
        log_error "❌ Sentinel failed to start"
        return 1
    fi
}

# ============================================================================
# Main
# ============================================================================

main() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  🎯 Phase 0x11-b: ONE-CLICK E2E Test Runner                          ║${NC}"
    echo -e "${CYAN}╠══════════════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${CYAN}║                                                                      ║${NC}"
    echo -e "${CYAN}║  Services:                                                           ║${NC}"
    echo -e "${CYAN}║    [1]   PostgreSQL (check)                                          ║${NC}"
    echo -e "${CYAN}║    [1.5] Database Migrations (auto-run)                              ║${NC}"
    echo -e "${CYAN}║    [2]   BTC Node + Wallet                                           ║${NC}"
    echo -e "${CYAN}║    [3]   Gateway HTTP                                                ║${NC}"
    echo -e "${CYAN}║    [4]   Sentinel                                                    ║${NC}"
    echo -e "${CYAN}║    [5]   E2E Tests                                                   ║${NC}"
    echo -e "${CYAN}║                                                                      ║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    # Handle arguments
    case "${1:-}" in
        --cleanup)
            log_info "Cleaning up any orphan processes on port 8080..."
            local gw_pid
            gw_pid=$(get_pid_by_port 8080)
            if [ -n "$gw_pid" ]; then
                stop_process "$gw_pid" "Gateway on 8080"
            fi
            log_info "Cleanup complete"
            exit 0
            ;;
        --skip-startup)
            log_warn "Skipping service startup..."
            ;;
        --clean-db)
            log_warn "Clean DB mode: will drop and recreate all tables"
            export CLEAN_DB=true
            # Fall through to full startup
            check_postgres || exit 1
            run_migrations
            check_btc_node || exit 1
            setup_btc_wallet
            start_gateway || exit 1
            start_sentinel || exit 1
            
            # Wait for services to stabilize
            log_info "Waiting for services to stabilize..."
            sleep 5
            ;;
        *)
            # Full startup sequence
            check_postgres || exit 1
            run_migrations
            check_btc_node || exit 1
            setup_btc_wallet
            start_gateway || exit 1
            start_sentinel || exit 1
            
            # Wait for services to stabilize
            log_info "Waiting for services to stabilize..."
            sleep 5
            ;;
    esac
    
    # Run E2E tests
    log_step "[5/6] Running E2E Tests..."
    echo ""
    
    cd "$PROJECT_ROOT/scripts/tests/0x11b_sentinel"
    
    TESTS_PASSED=0
    TESTS_FAILED=0
    
    # Test 1: Single User - Complete Money Flow
    echo ""
    echo "========================================================================"
    log_info "📋 Test 1/2: Single User - Complete Money Flow E2E"
    echo "   Path: Deposit → Transfer In → Trade → Transfer Out → Withdraw"
    echo "========================================================================"
    
    if uv run python3 e2e_critical_path.py; then
        log_info "✅ Test 1 PASSED: Single User E2E"
        ((TESTS_PASSED++))
    else
        log_warn "❌ Test 1 FAILED: Single User E2E"
        ((TESTS_FAILED++))
    fi
    
    # Test 2: Two Users - Order Matching
    echo ""
    echo "========================================================================"
    log_info "📋 Test 2/2: Two Users - Order Matching E2E"
    echo "   Path: User A sells BTC ↔ User B buys BTC → Trade matched"
    echo "========================================================================"
    
    if uv run python3 e2e_two_user_matching.py; then
        log_info "✅ Test 2 PASSED: Two User Matching E2E"
        ((TESTS_PASSED++))
    else
        log_warn "❌ Test 2 FAILED: Two User Matching E2E"
        ((TESTS_FAILED++))
    fi
    
    # Summary
    echo ""
    echo "========================================================================"
    log_info "📊 E2E TEST SUMMARY"
    echo "========================================================================"
    echo "   Passed: $TESTS_PASSED / 2"
    echo "   Failed: $TESTS_FAILED / 2"
    echo ""
    
    if [ $TESTS_FAILED -eq 0 ]; then
        echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║  🎉 ALL E2E TESTS PASSED!                                            ║${NC}"
        echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
        exit 0
    else
        echo -e "${YELLOW}╔══════════════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${YELLOW}║  ⚠️  Some E2E tests failed (likely DEF-002-B pending fix)            ║${NC}"
        echo -e "${YELLOW}╚══════════════════════════════════════════════════════════════════════╝${NC}"
        echo ""
        echo "Check logs at: $LOG_DIR/"
        exit 1
    fi
}

main "$@"
