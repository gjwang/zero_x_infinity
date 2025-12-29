# Handover: Architect -> Developer (Phase 0x11-b)

**Date**: 2025-12-29
**Phase**: 0x11-b (Sentinel Hardening & ETH Support)
**Context**: Phase 0x11-a delivered the Sentinel architecture and BTC MVP, but left critical gaps. This phase turns "MVP" into "Production Ready" for both major chains.

## 1. Objectives

| Priority | Task | Description |
| :--- | :--- | :--- |
| **P0 (Blocker)** | **Fix DEF-002 (BTC P2WPKH)** | Sentinel must detect SegWit (`bcrt1...`) deposits. |
| **P1 (Gap)** | **Implement ETH Sentinel** | Implement `EthScanner` to poll `eth_getLogs` for ERC20 `Transfer` events. |

---

## 2. Technical Specification: DEF-002 (BTC Fix)

**Problem**: `src/sentinel/btc.rs/extract_address` fails on P2WPKH scripts in Regtest.
**Solution**:
1.  **Reproduce**: Write a test case in `src/sentinel/btc.rs` (using `test_segwit_address_extraction_def_002` logic previously identified).
2.  **Fix**: Inspect `Address::from_script` usage. Ensure `Network` is correctly passed and `rust-bitcoin` version compatibility is checked. If native parsing fails, manually extract the 20-byte witness program hash for `OP_0 <20-bytes>`.

---

## 3. Technical Specification: ETH Sentinel (New Feature)

**Context**: `src/sentinel/eth.rs` exists but is not fully implemented for event logs.
**Requirements**:
1.  **Polling**: Use `eth_blockNumber` to track tip.
2.  **Scanning**: Use `eth_getLogs` with:
    *   `fromBlock` / `toBlock`
    *   `topics`: `[TransferKeccak]`
3.  **Filtering**:
    *   Match `log.address` (Contract Address) against supported Assets (e.g., USDT Contract).
    *   Match `topic[2]` (To Address) against `user_addresses`.
    *   *Note*: `topic[1]` is From, `topic[2]` is To.
4.  **Parsing**: Convert `data` field (Amount) to `Decimal` with correct precision (18 or 6).

---

## 4. Acceptance Criteria
- [ ] **BTC**: Unit test passes for P2WPKH addresses. Real `bitcoind` Regtest deposit is detected in E2E.
- [ ] **ETH**: `EthScanner` compiles and passes unit tests for Log Parsing.
- [ ] **E2E**: `TC-B02` (ETH Deposit) passes (Mock or Anvil).

## 5. Next Steps for Developer
1.  Switch to **Developer** role.
2.  Fix DEF-002 first (High Risk / Low Effort).
3.  Implement `EthScanner` (Medium Effort).
