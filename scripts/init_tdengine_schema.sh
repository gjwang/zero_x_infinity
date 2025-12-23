#!/bin/bash
# Initialize TDengine schema for CI
# This creates the database with correct precision and all super tables
# MUST be run in each CI job that uses TDengine (jobs don't share containers)

set -e

echo "=== Initializing TDengine Schema ==="

# Wait for TDengine to be ready
echo "[1/4] Waiting for TDengine..."
for i in {1..30}; do
    if curl -s http://localhost:6041/rest/login/root/taosdata > /dev/null; then
        echo "✅ TDengine ready"
        break
    fi
    echo "Waiting... ($i/30)"
    sleep 2
    if [ $i -eq 30 ]; then
        echo "❌ TDengine connection timeout"
        exit 1
    fi
done

# Create database with correct precision
# CRITICAL: Precision MUST be 'us' (microseconds)
# Code uses SystemTime::now().as_micros(), wrong precision causes
# "Timestamp data out of range" errors
echo "[2/4] Creating database with PRECISION 'us'..."
curl -s -u root:taosdata -d "CREATE DATABASE IF NOT EXISTS trading KEEP 365d DURATION 10d BUFFER 256 WAL_LEVEL 2 PRECISION 'us'" \
    http://localhost:6041/rest/sql

# Use database
curl -s -u root:taosdata -d "USE trading" http://localhost:6041/rest/sql

# Create all super tables
echo "[3/4] Creating super tables..."

# Orders table
curl -s -u root:taosdata -d "CREATE STABLE IF NOT EXISTS orders (ts TIMESTAMP, order_id BIGINT UNSIGNED, user_id BIGINT UNSIGNED, side TINYINT UNSIGNED, order_type TINYINT UNSIGNED, price BIGINT UNSIGNED, qty BIGINT UNSIGNED, filled_qty BIGINT UNSIGNED, status TINYINT UNSIGNED, cid NCHAR(64)) TAGS (symbol_id INT UNSIGNED)" \
    http://localhost:6041/rest/sql

# Trades table
curl -s -u root:taosdata -d "CREATE STABLE IF NOT EXISTS trades (ts TIMESTAMP, trade_id BIGINT UNSIGNED, order_id BIGINT UNSIGNED, user_id BIGINT UNSIGNED, side TINYINT UNSIGNED, price BIGINT UNSIGNED, qty BIGINT UNSIGNED, fee BIGINT UNSIGNED, role TINYINT UNSIGNED) TAGS (symbol_id INT UNSIGNED)" \
    http://localhost:6041/rest/sql

# Balances table
curl -s -u root:taosdata -d "CREATE STABLE IF NOT EXISTS balances (ts TIMESTAMP, avail BIGINT UNSIGNED, frozen BIGINT UNSIGNED, lock_version BIGINT UNSIGNED, settle_version BIGINT UNSIGNED) TAGS (user_id BIGINT UNSIGNED, asset_id INT UNSIGNED)" \
    http://localhost:6041/rest/sql

# K-Lines table
curl -s -u root:taosdata -d "CREATE STABLE IF NOT EXISTS klines (ts TIMESTAMP, open BIGINT UNSIGNED, high BIGINT UNSIGNED, low BIGINT UNSIGNED, close BIGINT UNSIGNED, volume BIGINT UNSIGNED, quote_volume DOUBLE, trade_count INT UNSIGNED) TAGS (symbol_id INT UNSIGNED, intv NCHAR(8))" \
    http://localhost:6041/rest/sql

# Order events table
curl -s -u root:taosdata -d "CREATE STABLE IF NOT EXISTS order_events (ts TIMESTAMP, order_id BIGINT UNSIGNED, event_type TINYINT UNSIGNED, prev_status TINYINT UNSIGNED, new_status TINYINT UNSIGNED, filled_qty BIGINT UNSIGNED, remaining_qty BIGINT UNSIGNED) TAGS (symbol_id INT UNSIGNED)" \
    http://localhost:6041/rest/sql

# Verify
echo "[4/4] Verifying schema..."
result=$(curl -s -u root:taosdata -d "SHOW trading.STABLES" http://localhost:6041/rest/sql)
echo "Super tables: $result"

echo "✅ TDengine schema initialized successfully"
