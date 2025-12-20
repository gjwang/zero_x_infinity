# Integration Test Scripts Guide

This document explains how to run integration tests for the Zero X Infinity matching engine.

## Prerequisites

1. **TDengine** running in Docker:
   ```bash
   docker ps | grep tdengine  # Verify running
   ```

2. **Gateway** running in release mode:
   ```bash
   ./target/release/zero_x_infinity --gateway --port 8080
   ```

3. **Test data** in `fixtures/`:
   - `orders.csv` - 100K order dataset
   - `balances_init.csv` - Initial account balances
   - `assets_config.csv` - Asset definitions
   - `symbols_config.csv` - Trading pair definitions

---

## Order Injection Script

### Location
```
scripts/inject_orders.py
```

### Basic Usage
```bash
# Run all 100K orders
python3 scripts/inject_orders.py --input fixtures/orders.csv --limit 0

# Run first 10K orders only
python3 scripts/inject_orders.py --input fixtures/orders.csv --limit 10000

# With rate limiting (500 orders/sec)
python3 scripts/inject_orders.py --input fixtures/orders.csv --limit 0 --rate-limit 500
```

### Options
| Option | Description | Default |
|--------|-------------|---------|
| `--input FILE` | Path to orders CSV | Required |
| `--limit N` | Max orders (0=all) | 0 |
| `--rate-limit N` | Orders/sec (0=unlimited) | 0 |
| `--quiet` | Suppress progress output | False |

### Retry Logic
- **max_retries**: 25 attempts
- **initial_delay**: 200ms
- **max_delay**: 5 seconds (cap)
- **Retryable errors**: URLError, socket.timeout, ConnectionRefused, ConnectionReset, BrokenPipe, TimeoutError, OSError, HTTP 503
- **Non-retryable**: Other HTTP errors, unknown exceptions

### Important Behavior
- **Sequential injection**: Orders MUST be injected in order for deterministic matching
- **Exit on failure**: Script exits on first non-recoverable error (order sequence critical)
- **KeyboardInterrupt**: Handled gracefully via `safe_sleep()`

---

## Before Running Tests

### 1. Clear TDengine Data
```bash
docker exec tdengine taos -s "DELETE FROM trading.orders; DELETE FROM trading.trades; DELETE FROM trading.balances;"
```

### 2. Restart Gateway (if needed)
```bash
# Stop existing
pkill -f "./target/release/zero_x_infinity"

# Rebuild (if code changed)
cargo build --release

# Start fresh
./target/release/zero_x_infinity --gateway --port 8080 > /tmp/gw.log 2>&1 &
```

### 3. Verify Gateway Ready
```bash
nc -z localhost 8080 && echo "Gateway: OK" || echo "Gateway: FAILED"
```

---

## Monitoring Test Progress

### Check Injection Status
```bash
ps aux | grep inject_orders | grep -v grep
```

### Check TDengine Counts
```bash
docker exec tdengine taos -s "SELECT 'orders' as t, COUNT(*) FROM trading.orders UNION ALL SELECT 'trades', COUNT(*) FROM trading.trades UNION ALL SELECT 'balances', COUNT(*) FROM trading.balances"
```

### Check Backpressure Events
```bash
grep -c "Backpressure" /tmp/gw.log
```

### Check Specific Queue Backpressure
```bash
grep "Backpressure" /tmp/gw.log | grep "queue=" | head -10
```

---

## Expected Results (100K Orders)

| Metric | Expected |
|--------|----------|
| Orders persisted | ~50K (depends on Gateway capacity) |
| Trades | ~2x orders |
| Balances | ~200K+ records |
| Success rate | 100% if no 503 failures |
| Throughput | ~200-700 orders/sec |

---

## Troubleshooting

### HTTP 503 (Order queue full)
- Gateway backpressure - reduce injection rate
- Increase `ORDER_QUEUE_CAPACITY` in `src/pipeline.rs`

### Script exits early
- Check for retry logs: `⏳ Retry X/Y: ...`
- Check final error message
- Script exits on first unrecoverable failure

### No data in TDengine
- Verify Gateway is running with TDengine enabled
- Check Gateway logs for persistence errors
- Verify `config/dev.yaml` has `persistence.enabled: true`

---

## Related Files

| File | Purpose |
|------|---------|
| `scripts/inject_orders.py` | Order injection script |
| `src/pipeline.rs` | Queue capacities, backpressure |
| `src/pipeline_services.rs` | Service implementations |
| `config/dev.yaml` | Gateway configuration |
| `fixtures/orders.csv` | Test order data |
