# QA Bug Report: DEF-002 Sentinel Address Loading Missing

| **Bug ID** | DEF-002-B |
| :--- | :--- |
| **Status** | 🔴 CRITICAL |
| **Reported By** | QA Engineer |
| **Date** | 2025-12-29 |
| **Phase** | 0x11-b |
| **Branch** | `0x11-b-sentinel-hardening` |

---

## Summary

Sentinel 在生产代码中**从不加载 `user_addresses`**，导致永远无法检测到存款。

## Root Cause

`BtcScanner::is_watched()` 依赖 `watched_addresses` HashSet，但 `reload_addresses()` **仅在测试中被调用**，生产代码从未调用。

```rust
// src/sentinel/btc.rs:133-135
if let Some(address) = self.extract_address(&output.script_pubkey) {
    if self.is_watched(&address) {  // ← HashSet 永远为空!
        deposits.push(...);
    }
}
```

## E2E Test Evidence

| Step | Result | Timestamp |
|------|--------|-----------|
| User Registration | ✅ User 1009 | 08:25:02 |
| Address Generation | ✅ `bcrt1q4nnszh...` | 08:25:04 |
| Address Registered in DB | ✅ INSERT OK | 08:25:04 |
| BTC Sent On-Chain | ✅ 2.0 BTC | 08:25:05 |
| Block Mined | ✅ Height 104 | 08:25:06 |
| **Sentinel Scan** | ❌ `block 104 (0 deposits)` | 08:25:07 |

## Why Unit Test Passes

`test_segwit_p2wpkh_extraction_def_002` 手动调用了 `reload_addresses()`:
```rust
// btc.rs:336 (test only)
scanner.reload_addresses(vec![...]);
```

生产代码不调用此函数。

---

## Fix Required

### Option A: Load before each scan (推荐)

在 `scan_chain_once()` 开始时加载地址:

```rust
// worker.rs: scan_chain_once() 开始处添加
async fn scan_chain_once(&self, scanner: &dyn ChainScanner) -> Result<u64, SentinelError> {
    let chain_id = scanner.chain_id();
    
    // ← NEW: Reload addresses before scanning
    let addresses: Vec<String> = sqlx::query_scalar(
        "SELECT address FROM user_addresses WHERE asset = $1"
    )
    .bind(chain_id)
    .fetch_all(&self.pool)
    .await?;
    
    scanner.reload_addresses(addresses);
    
    // ... rest of existing code
}
```

### Option B: Trait extension

扩展 `ChainScanner` trait 添加 `set_watched_addresses()` 方法。

---

## Files to Modify

| File | Change |
|------|--------|
| `src/sentinel/worker.rs` | Add address loading before scan loop |
| `src/sentinel/scanner.rs` | (Optional) Add trait method for address loading |

## Acceptance Criteria

- [ ] Sentinel loads addresses from `user_addresses` before each scan cycle
- [ ] E2E test: SegWit deposit detected → `deposit_history` updated
- [ ] E2E test: Funding balance reflects deposit after confirmations

---

## References

- [BTC Scanner Code](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/src/sentinel/btc.rs#L133-L151)
- [Worker Code](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/src/sentinel/worker.rs#L94-L166)
- [Original DEF-002 Issue](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/docs/agents/sessions/shared/arch-to-dev-0x11-b-def-002.md)
