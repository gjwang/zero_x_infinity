# QA Audit Report: Money Type Safety Compliance

**Date**: 2025-12-31  
**Auditor**: QA Agent  
**Standards Reviewed**:
- `docs/standards/money-type-safety.md`
- `docs/standards/api-money-enforcement.md`

---

## 1. Audit Results Summary

| Audit Script | Result |
|--------------|--------|
| `audit_money_safety.sh` | ✅ PASSED |
| `audit_api_types.sh` | ✅ PASSED |

---

## 2. Passed Checks (无违规)

| Rule | Description | Status |
|------|-------------|--------|
| No `10u64.pow` outside money.rs | ✅ PASS |
| No `u64/i64` amount fields in DTO | ✅ PASS |
| No direct `.parse::<u64>()` in gateway | ✅ PASS |
| No `f64` in DTO fields | ✅ PASS |
| No `.to_raw()` calls | ✅ PASS |
| No Deref arithmetic (`*a + *b`) | ✅ PASS |

---

## 3. Informational Warnings (待迁移)

### 3.1 Direct `money::` Calls (~30 处)

**Status**: Phase 4 Migration Pending

**位置**:
- `csv_io.rs` - benchmark 数据生成
- `websocket/service.rs` - 行情推送
- `internal_transfer/api.rs` - 内部转账
- `exchange_info/asset/models.rs` - 资产信息

**风险**: 低 (这些是核心模块，非业务代码)

---

## 4. 潜在违规 (需 Review)

### 4.1 `as f64` Usage in Money-Related Code

| File | Line | Code | Risk |
|------|------|------|------|
| `sentinel/btc.rs` | 138 | `output.value.to_sat() as f64 / 100_000_000.0` | ⚠️ **高** |
| `csv_io.rs` | 156 | `max_qty_display as f64 / qty_scale as f64` | ⚠️ 中 |
| `csv_io.rs` | 306 | `(price_float * quote_multiplier as f64).round() as u64` | ⚠️ 中 |
| `bench/order_generator.rs` | 多处 | benchmark 价格计算 | ℹ️ 低 |

**分析**:
- `sentinel/btc.rs:138` - **高风险**: 这是实际的 BTC 金额转换，使用 f64 可能导致精度问题
- `csv_io.rs` - 中风险: benchmark 数据生成代码，非生产路径
- `bench/` - 低风险: 纯 benchmark 代码

---

## 5. 建议修复

### 5.1 高优先级: sentinel/btc.rs

```rust
// ❌ 当前代码 (有精度风险)
output.value.to_sat() as f64 / 100_000_000.0

// ✅ 建议修改 (使用 Decimal 或直接 u64)
// 方案 A: 使用 Decimal
Decimal::from(output.value.to_sat()) / Decimal::from(100_000_000)
// 方案 B: 直接使用 satoshi (应该在 SymbolManager 转换)
output.value.to_sat()  // 返回 u64，后续用 SymbolManager 格式化
```

### 5.2 低优先级: csv_io.rs / bench/

这些是 benchmark 代码，可以保留 f64，但建议添加注释说明:

```rust
// safe: benchmark code, f64 precision acceptable for test data generation
```

---

## 6. 结论

| Category | Count |
|----------|-------|
| 严重违规 | 0 |
| 潜在风险 | 1 (`sentinel/btc.rs`) |
| 待迁移 | ~30 (informational) |
| 无问题 | 大部分代码 ✅ |

**Overall**: ✅ **通过** (无严重违规，1处潜在风险建议修复)

---

**QA Agent**: 建议 Architect 评审 `sentinel/btc.rs:138` 的 f64 使用
