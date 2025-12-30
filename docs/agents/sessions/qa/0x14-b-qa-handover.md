# 0x14-b Order Commands: QA Handover

## 概述

匹配引擎新增三个功能：
- **TimeInForce::IOC** - 立即成交或取消
- **ReduceOrder** - 减少订单数量（保留优先级）
- **MoveOrder** - 移动订单价格（失去优先级）

---

## 🚀 一键测试

```bash
./scripts/run_0x14b_order_commands.sh
```

### 预期输出
```
🎉 Phase 0x14-b: Order Commands - ALL TESTS PASSED
```

---

## 测试覆盖

| 功能 | 测试数 | 测试方法 |
|------|--------|----------|
| IOC 全部成交 | 1 | `test_ioc_full_match` |
| IOC 部分成交后过期 | 1 | `test_ioc_partial_fill_expire` |
| IOC 无成交立即过期 | 1 | `test_ioc_no_match_expire` |
| IOC 永不进入订单簿 | 2 | `test_ioc_never_rests_in_book`, `test_ioc_partial_fill_never_rests` |
| ReduceOrder 保留优先级 | 1 | `test_reduce_order_preserves_priority` |
| ReduceOrder 减到零移除 | 1 | `test_reduce_order_to_zero_removes` |
| ReduceOrder 不存在返回 None | 1 | `test_reduce_order_nonexistent` |
| MoveOrder 改变价格 | 1 | `test_move_order_changes_price` |
| MoveOrder 失去优先级 | 1 | `test_move_order_loses_priority` |
| MoveOrder 不存在返回 None | 1 | `test_move_order_nonexistent` |
| **总计** | **11** | 新增测试 |

---

## 修改的文件

| 文件 | 修改 |
|------|------|
| `src/models.rs` | 添加 `TimeInForce` 枚举 |
| `src/engine.rs` | IOC 逻辑 + `reduce_order()` + `move_order()` |
| `src/orderbook.rs` | 添加 `get_order_mut()` |

---

## 验收标准

✅ `./scripts/run_0x14b_order_commands.sh` 执行结果: ALL TESTS PASSED
