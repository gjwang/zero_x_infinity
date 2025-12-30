# 🐛 QA Defect Report: 0x14-b Order Commands

**报告日期**: 2025-12-31  
**报告人**: QA Engineer  
**Phase**: 0x14-b Order Commands  
**测试提交**: 775c121  

---

## 📋 摘要

40 个功能性测试用例执行完成，发现 **9 个失败**，其中 **1 个 P0 级别问题**。

| 优先级 | 数量 | 说明 |
| :--- | :--- | :--- |
| 🔴 P0 | 1 | MoveOrder 优先级丢失验证失败 |
| 🟡 P1 | 6 | 错误处理/验证问题 |
| 🟢 P2 | 2 | 状态同步问题 (可能异步设计预期) |

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

## 🟡 P1 - 需要修复/澄清

### DEF-002: MoveOrder 对不存在订单不报错

**测试用例**: MOV-004  
**问题**: `move_order(9999999999, "50000.00")` 返回成功 (code=0)  
**预期**: 返回错误 (订单不存在)

---

### DEF-003: MoveOrder 对已成交订单不报错

**测试用例**: MOV-005  
**问题**: 对 FILLED 状态订单执行 Move 返回成功  
**预期**: 返回错误 (订单已成交，无法移动)

---

### DEF-004: ReduceOrder 超量减少不报错

**测试用例**: RED-003  
**问题**: 订单数量 0.001，Reduce 0.002 返回成功  
**预期**: 返回错误 (减少数量超过原数量)

---

### DEF-005: ReduceOrder 对不存在订单不报错

**测试用例**: RED-004  
**问题**: `reduce_order(9999999999, "0.001")` 返回成功  
**预期**: 返回错误 (订单不存在)

---

### DEF-006: Cancel 对不存在订单不报错

**测试用例**: CAN-002  
**问题**: `cancel_order(9999999999)` 返回成功 (CANCEL_PENDING)  
**预期**: 返回错误 (订单不存在)

---

### DEF-007: Cancel 对已成交订单不报错

**测试用例**: CAN-004  
**问题**: 对 FILLED 订单执行 Cancel 返回成功  
**预期**: 返回错误 (订单已成交，无法取消)

---

## 🟢 P2 - 低优先级/需澄清

### DEF-008: Cancel 后状态显示 NEW 而非 CANCELED

**测试用例**: CAN-001  
**问题**: Cancel 成功后，get_order_status 返回 NEW  
**可能原因**: 异步处理，查询时 Cancel 尚未生效  
**建议**: 增加 wait_for_terminal_state 或确认异步设计预期

---

### DEF-009: ReduceOrder 至零后状态显示 NEW

**测试用例**: RED-002  
**问题**: Reduce 全部数量后，状态为 NEW 而非 CANCELED  
**预期**: 减至零等同于取消，状态应为 CANCELED

---

## 📌 问题澄清请求

对于 P1 级别的错误处理问题，请 Developer 确认：

1. **异步模式下的错误返回策略是什么？**
   - Gateway 是否先返回 `ACCEPTED` 再异步处理？
   - 如果订单不存在，是在哪一层校验拒绝？

2. **是否有专门的错误码规范？**
   - 如 `ORDER_NOT_FOUND`, `ORDER_ALREADY_FILLED` 等

---

## ✅ 验证通过的功能

以下核心功能已通过测试验证：

- [x] IOC 订单处理后**绝不**入簿 (9/9 测试通过)
- [x] GTC 订单未完全成交**必须**入簿
- [x] ReduceOrder 保留时间优先级 (RED-001 通过)
- [x] Market Order 正常成交
- [x] 边界条件正确拒绝 (零/负值参数)
- [x] 订单簿状态一致性 (depth API 正确)
