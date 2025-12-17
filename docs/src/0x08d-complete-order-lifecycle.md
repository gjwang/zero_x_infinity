# 0x08d 完整订单生命周期事件

> **核心目标**：实现完整的订单生命周期事件（包括 Cancel/Unlock），创建新测试集验证。

---

## 本章目标

### 1. 完整的订单生命周期

```
Place Order → Lock → [Match] → Settle / Cancel
                                    ↓
                               Unlock (if cancelled)
```

### 2. 完整的事件类型

| 事件 | 触发 | 当前状态 |
|------|------|----------|
| Deposit | 充值 | ✅ |
| Lock | 下单 | ✅ |
| Settle | 成交 | ✅ |
| **Unlock** | **取消/部分成交** | ⏳ |
| Withdraw | 提现 | ⏳ |

### 3. 新的测试集

```
fixtures/
  ├── test_basic/           # 原测试集 (100K orders, no cancel)
  │   ├── orders.csv
  │   └── balances_init.csv
  └── test_with_cancel/     # 新测试集 (with cancel orders)
      ├── orders.csv        # 包含 limit/cancel 订单
      └── balances_init.csv

baseline/
  ├── test_basic/           # 原 baseline
  │   ├── t2_events.csv
  │   └── ...
  └── test_with_cancel/     # 新 baseline
      ├── t2_events.csv
      └── ...
```

---

## 实现计划

### Phase 1: 扩展订单类型

#### 1.1 新增 OrderAction

```rust
pub enum OrderAction {
    Place,      // 下单
    Cancel,     // 取消
}

pub struct InputOrder {
    pub order_id: u64,
    pub user_id: u64,
    pub action: OrderAction,  // NEW
    pub side: Side,
    pub price: u64,
    pub qty: u64,
}
```

#### 1.2 修改 CSV 格式

```csv
# orders.csv
order_id,user_id,action,side,price,qty
1,100,place,buy,85000.00,1.5
2,101,place,sell,85100.00,2.0
3,100,cancel,buy,85000.00,0    # Cancel order 1
```

---

### Phase 2: 实现 Cancel 逻辑

#### 2.1 UBSCore.cancel_order()

```rust
impl UBSCore {
    pub fn cancel_order(&mut self, order_id: u64) -> Result<UnlockEvent, Error> {
        // 1. 从 OrderBook 移除订单
        let order = self.book.remove_order(order_id)?;
        
        // 2. Unlock 锁定的资金
        let unlock_amount = order.remaining_qty * order.price / qty_unit;
        self.unlock(order.user_id, asset_id, unlock_amount)?;
        
        // 3. 生成 Unlock 事件
        Ok(BalanceEvent::unlock(...))
    }
}
```

#### 2.2 OrderBook.remove_order()

```rust
impl OrderBook {
    pub fn remove_order(&mut self, order_id: u64) -> Option<InternalOrder> {
        // 需要维护 order_id → order 的索引
    }
}
```

---

### Phase 3: 创建新测试集

#### 3.1 生成带取消的订单数据

```python
# scripts/generate_orders_with_cancel.py
def generate():
    orders = []
    for i in range(10000):
        # 70% 下单
        orders.append(place_order(...))
        
        # 30% 取消之前的订单
        if random() < 0.3 and has_active_orders():
            orders.append(cancel_order(pick_random_active()))
    
    return orders
```

#### 3.2 预期事件统计

```
Events breakdown:
  Deposit: 2,000 (users × assets)
  Lock: N (placed orders)
  Unlock: M (cancelled + partially filled remainder)
  Settle: K × 4 (trades × 2 parties × 2 assets)
  
Verification:
  Lock_count = Settle_count + Unlock_count  # 资金守恒
```

---

### Phase 4: 验证

#### 4.1 验证脚本升级

```python
# verify_balance_events.py 新增检查
def check_fund_conservation():
    """验证资金守恒：Lock = Settle + Unlock"""
    for user_asset in lock_events.keys():
        lock_total = sum(lock_events[user_asset])
        settle_total = sum(settle_events[user_asset])
        unlock_total = sum(unlock_events[user_asset])
        
        assert lock_total == settle_total + unlock_total
```

---

## 验收标准

- [ ] OrderAction enum 实现
- [ ] Cancel 逻辑实现
- [ ] Unlock 事件记录
- [ ] 新测试集生成
- [ ] 新 baseline 创建
- [ ] 资金守恒验证通过
- [ ] 原测试集仍然通过

---

## 参考

- 原测试集：`fixtures/` → 保持不变
- 新测试集：`fixtures/test_with_cancel/` → 新增
