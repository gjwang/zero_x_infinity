# exchange_info API 完整性标准

> **基准**: Binance Spot API `/exchangeInfo`  
> **状态**: 📋 QA Gap Analysis (待 Architect 评审)

---

## 1. 目标

确保 `/api/v1/public/exchange_info` 返回 Binance 兼容的完整信息，支持：
- 客户端订单预验证
- 动态 UI 配置 (精度、限制等)
- 第三方集成

---

## 2. 当前 vs Binance 对比

### 2.1 Symbol Info

| 字段 | Binance | 我们 | 差距 |
|------|---------|------|------|
| `symbol` | ✅ | ✅ | - |
| `baseAsset` / `quoteAsset` | ✅ | ✅ | - |
| `baseAssetPrecision` | ✅ | ✅ `qty_decimals` | 命名不同 |
| `quotePrecision` | ✅ | ❌ | **缺失** |
| `status` | ✅ TRADING/HALT | ⚠️ 仅 boolean | **待升级** |
| `orderTypes[]` | ✅ | ❌ | **缺失** |
| `filters[]` | ✅ | ❌ | **P0 缺失** |

### 2.2 Symbol Filters (P0)

| Filter | 用途 | 字段 | 状态 |
|--------|------|------|------|
| **PRICE_FILTER** | 价格限制 | minPrice, maxPrice, tickSize | ❌ |
| **LOT_SIZE** | 数量限制 | minQty, maxQty, stepSize | ❌ |
| **NOTIONAL** | 金额限制 | minNotional, maxNotional | ❌ |
| MARKET_LOT_SIZE | 市价单限制 | minQty, maxQty, stepSize | ❌ |
| MAX_NUM_ORDERS | 挂单数限制 | maxNumOrders | ❌ |

### 2.3 Asset Info

| 字段 | Binance | 我们 | 差距 |
|------|---------|------|------|
| `asset` | ✅ | ✅ | - |
| `name` | ✅ | ✅ | - |
| `decimals` | - | ✅ | Binance 无此字段 |
| `withdrawFee` | ✅ | ❌ | **缺失** |
| `withdrawMin` | ✅ | ❌ | **缺失** |

---

## 3. 建议 API 响应格式

```json
{
  "symbols": [{
    "symbol": "BTC_USDT",
    "status": "TRADING",
    "baseAsset": "BTC",
    "quoteAsset": "USDT",
    "baseAssetPrecision": 8,
    "quoteAssetPrecision": 6,
    "orderTypes": ["LIMIT", "MARKET"],
    "filters": [
      {"filterType": "PRICE_FILTER", "minPrice": "0.01", "maxPrice": "1000000", "tickSize": "0.01"},
      {"filterType": "LOT_SIZE", "minQty": "0.00001", "maxQty": "9000", "stepSize": "0.00001"},
      {"filterType": "NOTIONAL", "minNotional": "5.00"}
    ]
  }]
}
```

---

## 4. 实施优先级

| 优先级 | 项目 | 影响 |
|--------|------|------|
| **P0** | PRICE_FILTER, LOT_SIZE, NOTIONAL | 订单验证必需 |
| **P1** | orderTypes[], status enum | 客户端兼容 |
| **P2** | withdrawFee, withdrawMin | 提现功能 |
| P3 | MARKET_LOT_SIZE, MAX_NUM_ORDERS | 高级限制 |

---

## 5. 测试覆盖

| 文件 | 测试内容 |
|------|----------|
| `test_symbol_filters.py` | 验证 filters/orderTypes 字段存在 |
| `test_filter_validation.py` | 验证 filter 规则执行 |

---

## 6. 参考

- [Binance Filters Doc](https://developers.binance.com/docs/binance-spot-api-docs/filters)
- 现有 API: `GET /api/v1/public/exchange_info`
