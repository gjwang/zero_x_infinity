# QA → Architect Handover: exchange_info 完整性

**Date**: 2025-12-31  
**From**: QA Agent  
**Phase**: 0x14-c Money Safety

---

## Summary

基于 Binance API 对比，发现 `exchange_info` 缺少关键字段，影响订单验证。

## 🔴 P0 缺失 (需 Architect 决策)

| 缺失项 | Binance | 影响 |
|--------|---------|------|
| `filters[]` | PRICE_FILTER, LOT_SIZE, NOTIONAL | 无法验证订单范围 |
| `order_types[]` | LIMIT, MARKET | 客户端不知道支持哪些类型 |

## 测试状态

| Test | Result |
|------|--------|
| FILTER-001/002 (基础结构) | ✅ PASS |
| FILTER-003 (filters 存在) | ❌ FAIL |
| FILTER-004 (order_types) | ❌ FAIL |

## 交付物

| 文件 | 位置 |
|------|------|
| 标准文档 | `docs/src/standards/exchange-info-completeness.md` |
| 测试脚本 | `scripts/tests/0x14c_money_safety/test_symbol_filters.py` |
| 验证脚本 | `scripts/tests/0x14c_money_safety/test_filter_validation.py` |

## 需要 Architect 确认

1. 是否采用 Binance-style `filters[]` 结构?
2. 优先级确认: P0 先做 LOT_SIZE + NOTIONAL?
3. 数据源: filters 存 DB (JSONB) 还是代码配置?

---

**Branch**: `0x14-c-money-safety`  
**Commits**: 3 commits pushed
