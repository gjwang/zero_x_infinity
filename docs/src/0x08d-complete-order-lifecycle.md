# 0x08d 完整订单生命周期事件

> **核心目标**：实现完整的订单生命周期事件，创建新测试集验证。

---

## 本章目标

### 1. 完整的订单状态

根据 0x08a 文档，订单持久化后的所有可能状态：

```
订单已持久化
       │
       ├──▶ 成交 (Filled)
       ├──▶ 部分成交 (PartiallyFilled)
       ├──▶ 挂单中 (New)
       ├──▶ 用户取消 (Cancelled)
       ├──▶ 系统过期 (Expired)
       └──▶ 余额不足被拒绝 (Rejected)
```

### 2. OrderStatus 需要扩展

当前：
```rust
pub enum OrderStatus {
    New,             // ✅
    PartiallyFilled, // ✅
    Filled,          // ✅
    Cancelled,       // ✅
}
```

需要添加：
```rust
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,    // NEW: 余额不足等原因被拒绝
    Expired,     // NEW: 系统过期（如 GTD 订单）
}
```

### 3. 订单事件与余额事件对照

| 订单状态 | OrderEvent | BalanceEvent | 当前状态 |
|----------|------------|--------------|----------|
| New (挂单) | Accepted | Lock | ✅ |
| **Rejected** | Rejected | **无**（未锁定） | ⏳ 需要记录 |
| PartiallyFilled | PartialFilled | Settle | ✅ |
| Filled | Filled | Settle + Unlock(剩余) | ✅/⏳ |
| **Cancelled** | Cancelled | **Unlock** | ⏳ |
| **Expired** | Expired? | **Unlock** | ⏳ |

### 4. BalanceEvent 完整类型

```rust
pub enum BalanceEventType {
    Deposit,   // ✅ 已实现 - 充值
    Withdraw,  // ⏳ 待实现 - 提现
    Lock,      // ✅ 已实现 - 下单锁定
    Unlock,    // ⏳ 待实现 - 取消/过期释放
    Settle,    // ✅ 已实现 - 成交结算
}
```

---

## 实现计划

### Phase 1: 扩展 OrderStatus

```rust
// src/models.rs
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,    // 余额不足/验证失败
    Expired,     // 系统过期
}
```

### Phase 2: 扩展 OrderEvent (可选)

当前 `OrderEvent` 已包含 `Rejected`，可能需要添加 `Expired`：

```rust
pub enum OrderEvent {
    Accepted { ... },
    Rejected { ... },     // ✅ 已有
    PartialFilled { ... },
    Filled { ... },
    Cancelled { ... },
    Expired { ... },      // NEW
}
```

### Phase 3: 实现 Unlock 事件

触发场景：
1. **用户取消** - CancelOrder 请求
2. **系统过期** - GTD 订单超时
3. **完全成交后剩余** - 因价格保护未完全使用锁定金额

```rust
impl UBSCore {
    pub fn cancel_order(&mut self, order_id: u64) -> Result<BalanceEvent, Error> {
        // 1. 从 OrderBook 移除
        let order = self.book.remove(order_id)?;
        
        // 2. 计算未使用的锁定金额
        let unlock_amount = order.remaining_cost();
        
        // 3. Unlock
        self.unlock(order.user_id, asset_id, unlock_amount)?;
        
        // 4. 生成事件
        Ok(BalanceEvent::unlock(order.user_id, asset_id, order_id, ...))
    }
}
```

### Phase 4: 扩展 CSV 格式支持

```csv
# orders.csv - 新增 action 列
order_id,user_id,action,side,price,qty
1,100,place,buy,85000.00,1.5
2,101,place,sell,85100.00,2.0
3,100,cancel,,,             # Cancel order 1
```

### Phase 5: 创建新测试集

```
fixtures/
  ├── (原文件保持不变)
  ├── orders.csv              # 原测试数据
  ├── balances_init.csv
  └── test_with_lifecycle/    # 新测试集
      ├── orders.csv          # 包含 place + cancel 订单
      └── balances_init.csv

baseline/
  ├── (原文件保持不变)
  └── test_with_lifecycle/    # 新 baseline
      ├── t2_events.csv
      ├── t2_order_events.csv  # NEW: 订单事件日志
      └── ...
```

### Phase 6: 新增订单事件日志

```csv
# t2_order_events.csv
seq_id,order_id,user_id,event_type,reason,timestamp
1,1,100,accepted,,1234567890
2,2,101,accepted,,1234567891
3,1,100,partial_filled,,1234567892
4,3,102,rejected,insufficient_balance,1234567893
5,1,100,cancelled,,1234567894
```

### Phase 7: 验证

1. **资金守恒**
   ```
   for each user-asset:
     Lock_total = Settle_total + Unlock_total
   ```

2. **订单状态一致性**
   ```
   for each order:
     if Accepted → must have Lock event
     if Rejected → must NOT have Lock event
     if Cancelled → must have Unlock event
     if Filled → must have Settle event(s)
   ```

---

## 验收标准

- [ ] OrderStatus 添加 Rejected, Expired
- [ ] OrderEvent 添加 Expired（如需要）
- [ ] Unlock 事件实现
- [ ] CSV 格式支持 cancel action
- [ ] 订单事件日志 (t2_order_events.csv)
- [ ] 新测试集创建
- [ ] 资金守恒验证通过
- [ ] 原测试集仍然通过

---

## 文件变更

| 文件 | 变更 |
|------|------|
| `src/models.rs` | 添加 Rejected, Expired 到 OrderStatus |
| `src/messages.rs` | 添加 Expired 到 OrderEvent（可选）|
| `src/ubscore.rs` | 添加 cancel_order() |
| `src/orderbook.rs` | 添加 remove_order() |
| `src/ledger.rs` | 添加 write_order_event() |
| `src/csv_io.rs` | 支持 action 列解析 |
| `scripts/generate_orders_with_lifecycle.py` | 生成带取消的订单 |
| `scripts/verify_balance_events.py` | 添加资金守恒检查 |
