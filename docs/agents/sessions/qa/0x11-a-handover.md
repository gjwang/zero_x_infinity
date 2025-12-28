# QA Handover: Phase 0x11-a Sentinel Service

| **Phase** | 0x11-a: Real Chain Integration |
|-----------|--------------------------------|
| **Date** | 2025-12-28 |
| **Developer** | AI Agent |
| **Branch** | `0x11-a-real-chain` |
| **Status** | 🟢 Ready for QA Review |

---

## 📋 Feature Summary

The **Sentinel Service** monitors blockchain nodes (BTC/ETH) for deposits to user addresses, records them in the database, and tracks confirmations until finalized.

### Core Components

| Component | File | Description |
|-----------|------|-------------|
| ChainScanner | `src/sentinel/scanner.rs` | Unified blockchain interface trait |
| BtcScanner | `src/sentinel/btc.rs` | Bitcoin RPC scanner with real `bitcoincore-rpc` |
| EthScanner | `src/sentinel/eth.rs` | Ethereum scanner (mock mode) |
| SentinelWorker | `src/sentinel/worker.rs` | Main scanning orchestration |
| ConfirmationMonitor | `src/sentinel/confirmation.rs` | Deposit state machine |
| DepositPipeline | `src/sentinel/pipeline.rs` | Balance crediting |

---

## 🧪 One-Click Test

```bash
# Run the E2E test (requires Docker + PostgreSQL)
./scripts/test_sentinel_e2e.sh
```

### Test Steps Performed:
1. ✅ Start/verify bitcoind regtest container
2. ✅ Apply database migration (chain_cursor table)
3. ✅ Generate 5 test blocks
4. ✅ Run Sentinel scanner
5. ✅ Verify chain_cursor persistence
6. ✅ Verify block scanning logs

### Expected Output:
```
======================================== 
  ✅ All Sentinel E2E Tests PASSED
========================================
Test Summary:
  - BTC RPC connection: OK
  - Block scanning: 5 blocks
  - Cursor height: <N>
  - Database persistence: OK
```

---

## 🔧 Manual Testing

### Prerequisites
```bash
# 1. Start bitcoind regtest
docker run -d --name bitcoind -p 18443:18443 ruimarinho/bitcoin-core:24 \
    -regtest -rpcuser=admin -rpcpassword=admin \
    -rpcbind=0.0.0.0 -rpcallowip=0.0.0.0/0

# 2. PostgreSQL running on port 5433 (from docker-compose)

# 3. Apply migration
PGPASSWORD=trading123 psql -U trading -d exchange_info_db -h 127.0.0.1 -p 5433 \
    -f migrations/20251228180000_chain_cursor.sql
```

### Run Sentinel
```bash
cargo run -- --sentinel -e dev
```

### Generate Blocks (in another terminal)
```bash
# Create wallet
docker exec bitcoind bitcoin-cli -regtest -rpcuser=admin -rpcpassword=admin \
    createwallet "test"

# Get address
docker exec bitcoind bitcoin-cli -regtest -rpcuser=admin -rpcpassword=admin \
    -rpcwallet=test getnewaddress

# Generate 10 blocks
docker exec bitcoind bitcoin-cli -regtest -rpcuser=admin -rpcpassword=admin \
    generatetoaddress 10 <ADDRESS>
```

### Verify
```bash
# Check chain_cursor
PGPASSWORD=trading123 psql -U trading -d exchange_info_db -h 127.0.0.1 -p 5433 \
    -c "SELECT * FROM chain_cursor;"

# Expected:
#  chain_id | last_scanned_height | last_scanned_hash | updated_at
# ----------+---------------------+-------------------+------------
#  BTC      |                  10 | <hash>            | <timestamp>
```

---

## ✅ Developer Verification Results

| Test | Status | Notes |
|------|--------|-------|
| Unit Tests (20) | ✅ Pass | `cargo test sentinel::` |
| Full Test Suite (320) | ✅ Pass | `cargo test --lib` |
| Clippy | ✅ Clean | No warnings |
| BTC RPC Connection | ✅ Pass | Connected to bitcoind regtest |
| Block Scanning | ✅ Pass | Scanned 10 blocks |
| DB Persistence | ✅ Pass | chain_cursor updated |
| Re-run (idempotency) | ✅ Pass | Cursor resumes correctly |

---

## 📁 Files Changed

### New Files (11)
- `src/sentinel/mod.rs`
- `src/sentinel/scanner.rs`
- `src/sentinel/btc.rs`
- `src/sentinel/eth.rs`
- `src/sentinel/worker.rs`
- `src/sentinel/config.rs`
- `src/sentinel/error.rs`
- `src/sentinel/confirmation.rs`
- `src/sentinel/pipeline.rs`
- `config/sentinel_config.yaml`
- `config/chains/btc_regtest.yaml`
- `config/chains/eth_anvil.yaml`

### Modified Files (3)
- `Cargo.toml` - Added `bitcoincore-rpc` dependency
- `src/lib.rs` - Added sentinel module
- `src/main.rs` - Added `--sentinel` CLI mode

### Database Migration (1)
- `migrations/20251228180000_chain_cursor.sql`

### Test Script (2)
- `scripts/test_sentinel.sh` - Unit test verification
- `scripts/test_sentinel_e2e.sh` - Full E2E test

---

## ⚠️ Known Limitations

1. **ETH Scanner**: Currently mock mode only (no `alloy` RPC integration yet)
2. **Continuous Mode**: Demo runs single scan; production needs `worker.run()` loop
3. **Deposit Detection**: Requires addresses in `user_addresses` table

---

## 🔗 Related Documentation

- Design: `docs/src/0x11-a-real-chain.md`
- Architect Review: `docs/agents/sessions/architect/0x11-a-critical-review.md`
- Developer Session: `docs/agents/sessions/developer/current-task.md`

---

## 📞 Handover Checklist

- [x] All unit tests pass
- [x] E2E test script provided
- [x] Database migration included
- [x] Configuration files documented
- [x] Manual test instructions provided
- [x] Known limitations documented

**Ready for QA verification!** 🚀
