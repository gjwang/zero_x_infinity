# 🐛 QA Defect Report: 0x14-b Order Commands

**报告日期**: 2025-12-31  
**报告人**: QA Engineer  
**Phase**: 0x14-b Order Commands  
**测试提交**: 775c121  
**更新日期**: 2025-12-31 (测试方法更新)

---

## 📋 摘要

40 个功能性测试用例执行完成。

| 优先级 | 数量 | 说明 |
| :--- | :--- | :--- |
| 🔴 P0 | 1 | MoveOrder 优先级丢失验证失败 (待修复) |
| ✅ FIXED | 8 | 测试方法已更新为异步结果验证模式 |

> **2025-12-31 更新**: DEF-002~DEF-009 经确认为**设计预期行为**。Gateway 作为异步系统，对无效操作返回 ACCEPTED 后在 Pipeline 中静默处理是正确的设计。测试已修改为验证"无副作用"而非期望同步错误。

---

## 🔴 P0 - 需立即修复

### DEF-001: MoveOrder 后优先级未丢失

**测试用例**: MOV-001  
**设计规范**: [0x14-b-order-commands.md L81](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/docs/src/0x14-b-order-commands.md#L81)

> `MoveOrder`: Atomic "Cancel old ID + Place new ID". **Priority Loss** is acceptable (and expected).

**复现步骤**:
```python
# 1. Place Order A at 49000 (先下单)
A = place_order(BUY, 49000, 0.01, GTC)
time.sleep(0.3)

# 2. Place Order B at 50000 (后下单)  
B = place_order(BUY, 50000, 0.01, GTC)
time.sleep(0.5)

# 3. Move A to same price as B (50000)
move_order(A, 50000)
time.sleep(0.5)

# 4. Match with Sell 0.01 @ 50000 (只够一个订单)
place_order(SELL, 50000, 0.01, IOC)
time.sleep(1.0)

# 5. Check who matched first
status_a = get_order_status(A)  
status_b = get_order_status(B)
```

**预期结果**:
```
A = NEW/ACCEPTED (移动后排在 B 后面)
B = FILLED (先到 50000 价位，先成交)
```

**实际结果**:
```
A = FILLED (反而先成交)
B = NEW (未成交)
```

**影响范围**:
- MoveOrder 占基准测试 ~8% 的命令
- 核心撮合逻辑正确性

**建议修复方向**:
1. 确认 MoveOrder 实现是否为 Atomic Cancel+Place
2. 检查 Place 时是否使用新的 seq_id (确保排在队列末尾)
3. 验证订单簿的时间优先级队列维护

---

## ✅ 已修复 (测试方法更新)

以下缺陷经确认为**异步系统设计预期**，测试已更新为验证"无副作用"模式：

### DEF-002 → ✅ MOV-004 (已修复)
**原问题**: MoveOrder 对不存在订单不报错  
**修复**: 改为验证订单簿无变化 (无副作用)

### DEF-003 → ✅ MOV-005 (已修复)
**原问题**: MoveOrder 对已成交订单不报错  
**修复**: 改为验证订单状态仍为 FILLED，新价位无订单

### DEF-004 → ✅ RED-003 (已修复)
**原问题**: ReduceOrder 超量减少不报错  
**修复**: 改为验证订单仍在簿中，状态不变

### DEF-005 → ✅ RED-004 (已修复)
**原问题**: ReduceOrder 对不存在订单不报错  
**修复**: 改为验证订单簿无变化 (无副作用)

### DEF-006 → ✅ CAN-002 (已修复)
**原问题**: Cancel 对不存在订单返回 CANCEL_PENDING  
**修复**: 改为验证订单簿无变化 (无副作用)

### DEF-007 → ✅ CAN-004 (已修复)
**原问题**: Cancel 对已成交订单不报错  
**修复**: 改为验证订单状态仍为 FILLED

### DEF-008 → ✅ CAN-001 (已修复)
**原问题**: Cancel 后状态显示 NEW 而非 CANCELED  
**修复**: 使用 `wait_for_order_terminal()` 轮询等待异步处理完成

### DEF-009 → ✅ RED-002 (已修复)
**原问题**: ReduceOrder 至零后状态显示 NEW  
**修复**: 使用 `wait_for_order_terminal()` 轮询等待异步处理完成

---

## 📐 设计确认：异步错误处理策略

经确认，系统采用以下设计模式：

```
┌─────────────────────────────────────────────────────────────────────┐
│          LMAX Disruptor-Style Async Pipeline                        │
├─────────────────────────────────────────────────────────────────────┤
│  Client Request                                                      │
│       ↓                                                              │
│  Gateway: 仅做格式校验 → 返回 ACCEPTED/PENDING                       │
│       ↓                                                              │
│  RingBuffer Queue                                                    │
│       ↓                                                              │
│  Pipeline: 业务逻辑校验 (订单存在性、状态等)                          │
│       ↓                                                              │
│  无效操作: 静默忽略 (无副作用)                                        │
│  有效操作: 执行并推送状态更新                                         │
└─────────────────────────────────────────────────────────────────────┘
```

**测试原则**:
- ❌ 旧方式: 期望 Gateway 返回同步错误码
- ✅ 新方式: 接受 ACCEPTED 响应 → 等待异步处理 → 验证无副作用

---

## ✅ 验证通过的功能

以下核心功能已通过测试验证：

- [x] IOC 订单处理后**绝不**入簿 (9/9 测试通过)
- [x] GTC 订单未完全成交**必须**入簿
- [x] ReduceOrder 保留时间优先级 (RED-001 通过)
- [x] Market Order 正常成交
- [x] 边界条件正确拒绝 (零/负值参数)
- [x] 订单簿状态一致性 (depth API 正确)
- [x] 异步无效操作无副作用 (8/8 测试通过)
