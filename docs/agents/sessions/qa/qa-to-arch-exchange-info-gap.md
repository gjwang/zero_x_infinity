# QA → Architect: Exchange Info API Test Matrix

**Date**: 2025-12-31  
**From**: QA Agent  
**To**: Architect  
**Subject**: exchange_info / symbol_info / asset_info 测试覆盖与缺失分析

---

## 1. 当前测试覆盖状态

### ✅ 已覆盖 (PASS)

| API | 字段 | 测试 | 状态 |
|-----|------|------|------|
| exchange_info | `symbols[]` 数组 | FILTER-001 | ✅ |
| symbol_info | `base_asset`, `quote_asset` | FILTER-002 | ✅ |
| symbol_info | `price_decimals` | CFG-004 | ✅ |
| symbol_info | `qty_decimals` | CFG-003 | ✅ |
| symbol_info | `is_tradable` | - | ✅ (隐式) |
| asset_info | `decimals` | CFG-005 | ✅ |
| asset_info | `can_deposit/withdraw/trade` | - | ✅ (隐式) |

### ❌ 缺失字段 (需要开发实现)

| API | 缺失字段 | Binance 对应 | 优先级 | 测试 |
|-----|---------|-------------|--------|------|
| **symbol_info** | `filters[]` | ✅ 有 | **P0** | FILTER-003 ❌ |
| **symbol_info** | `order_types[]` | ✅ 有 | **P1** | FILTER-004 ❌ |
| **symbol_info** | `status` (TRADING/HALT) | ✅ 有 | P2 | - |
| **symbol_info** | `minQty`, `maxQty`, `stepSize` | LOT_SIZE filter | **P0** | FILTER-005 ⏭️ |
| **symbol_info** | `minPrice`, `maxPrice`, `tickSize` | PRICE_FILTER | **P0** | FILTER-006 ⏭️ |
| **symbol_info** | `minNotional` | NOTIONAL filter | **P0** | FILTER-007 ⏭️ |
| **asset_info** | `min_withdraw` | withdrawMin | P2 | - |
| **asset_info** | `withdraw_fee` | withdrawFee | P2 | - |

---

## 2. 缺失 Filters 详细说明

### 2.1 LOT_SIZE (P0)

**用途**: 限制订单数量范围和步进

```json
{
  "filterType": "LOT_SIZE",
  "minQty": "0.00001",    // 最小数量
  "maxQty": "9000.00000", // 最大数量
  "stepSize": "0.00001"   // 步进
}
```

**验证规则**:
- `qty >= minQty`
- `qty <= maxQty`
- `(qty - minQty) % stepSize == 0`

### 2.2 PRICE_FILTER (P0)

**用途**: 限制价格范围和步进

```json
{
  "filterType": "PRICE_FILTER",
  "minPrice": "0.01",
  "maxPrice": "1000000.00",
  "tickSize": "0.01"
}
```

**验证规则**:
- `price >= minPrice`
- `price <= maxPrice`
- `(price - minPrice) % tickSize == 0`

### 2.3 NOTIONAL (P0)

**用途**: 限制最小订单金额

```json
{
  "filterType": "NOTIONAL",
  "minNotional": "5.00",
  "maxNotional": "10000000.00"
}
```

**验证规则**:
- `price * qty >= minNotional`

---

## 3. 新增测试文件

| 文件 | 用途 | 测试数 |
|------|------|--------|
| `test_symbol_filters.py` | 验证 exchange_info 结构 | 7 |
| `test_filter_validation.py` | 验证 filter 执行 | 6 |

---

## 4. 建议的开发优先级

### P0 - 必须 (影响订单验证)
1. 添加 `filters[]` 字段到 symbol_info
2. 添加 LOT_SIZE, PRICE_FILTER, NOTIONAL filter
3. Gateway 执行 filter 验证

### P1 - 重要 (客户端兼容)
4. 添加 `order_types[]` 字段

### P2 - 可选 (完整性)
5. 添加 `status` 枚举 (TRADING/HALT/BREAK)
6. 添加 asset 提现限制字段

---

## 5. 相关文件

- 测试: `scripts/tests/0x14c_money_safety/test_symbol_filters.py`
- 测试: `scripts/tests/0x14c_money_safety/test_filter_validation.py`
- 现有 API: `src/gateway/handlers.rs` (SymbolApiData)

---

**QA Agent**: 等待 Architect 确认优先级后，可继续补充测试用例。
