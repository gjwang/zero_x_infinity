# 🔍 Pipeline Bug Analysis: Cancel/Reduce Status Not Persisted

**日期**: 2025-12-31  
**分析者**: QA/Developer Agent  
**相关缺陷**: CAN-001, RED-002, RED-003  

---

## 📋 问题总结

Cancel 和 Reduce 执行成功后，订单从订单簿正确移除 (`in_book=False`)，但 **订单状态未更新到 TDengine**，导致查询 API 仍返回 `status=NEW`。

---

## 🔬 根本原因分析

### 正常订单流程 (Order)
```
ValidAction::Order
  → MatchingEngine::process_order()
  → 创建 MEResult { order, trades, ... }
  → me_result_queue.push(me_result)  ✅
  → SettlementService 消费 me_result_queue
  → batch_insert_me_results() 写入 TDengine  ✅
```

### Cancel 流程 (有 Bug)
```
ValidAction::Cancel
  → book.remove_order_by_id()  ✅ (订单簿移除)
  → cancelled_order.status = OrderStatus::CANCELED  ✅ (内存状态更新)
  → balance_update_queue.push(unlock)  ✅ (解锁余额)
  → push_event_queue.push(OrderUpdate)  ✅ (WebSocket 推送)
  → ❌ 没有发送 MEResult 到 me_result_queue
  → ❌ TDengine 中的状态永远是旧的 NEW
```

### Reduce 流程 (有 Bug)
```
ValidAction::Reduce
  → MatchingEngine::reduce_order()  ✅ (订单簿更新)
  → balance_update_queue.push(unlock)  ✅ (解锁余额)
  → ❌ 没有发送 OrderUpdate push
  → ❌ 没有发送 MEResult 到 me_result_queue
  → ❌ TDengine 中的状态不会更新
```

---

## 📍 问题代码位置

**文件**: `src/pipeline_services.rs`

### Cancel 处理 (Line 881-942)
```rust
ValidAction::Cancel { order_id, user_id, ingested_at_ns } => {
    if let Some(mut cancelled_order) = self.book.remove_order_by_id(order_id) {
        cancelled_order.status = OrderStatus::CANCELED;
        // ... unlock balance ✅
        // ... push WebSocket ✅
        // ❌ 缺少: 发送 MEResult 到 me_result_queue
    }
}
```

### Reduce 处理 (Line 943-1003)
```rust
ValidAction::Reduce { order_id, user_id, reduce_qty, ingested_at_ns } => {
    if MatchingEngine::reduce_order(&mut self.book, order_id, reduce_qty).is_some() {
        // ... unlock balance ✅
        // ❌ 缺少: push WebSocket OrderUpdate
        // ❌ 缺少: 发送 MEResult 到 me_result_queue (如果减至零)
    }
}
```

---

## ✅ 修复方案

### 方案 A: 发送 MEResult (完整方案)

Cancel 和 Reduce 成功后，构造并发送 `MEResult` 到 `me_result_queue`：

```rust
// Cancel 修复
ValidAction::Cancel { order_id, user_id, ingested_at_ns } => {
    if let Some(mut cancelled_order) = self.book.remove_order_by_id(order_id) {
        cancelled_order.status = OrderStatus::CANCELED;
        
        // ... 现有的 unlock 和 push 逻辑 ...
        
        // 🔧 新增: 发送 MEResult 持久化状态变更
        let me_result = crate::messages::MEResult {
            order: cancelled_order.clone(),
            trades: vec![],
            maker_updates: vec![],
            final_status: OrderStatus::CANCELED,
            symbol_id: cancelled_order.symbol_id,
        };
        let _ = self.queues.me_result_queue.push(me_result);
    }
}
```

### 方案 B: 直接调用 update_order_status (轻量方案)

对于 Cancel/Reduce，可以直接调用 `persistence::orders::update_order_status()`，
但这需要 async 支持，在 MatchingService 的同步循环中不太合适。

### 推荐: 方案 A

发送 MEResult 是最符合现有架构的方式：
- 利用已有的 Settlement 异步批量处理
- 保持 MatchingService 完全同步
- 统一所有订单状态变更的持久化路径

---

## 📊 影响范围

| 操作 | 订单簿 | WebSocket | TDengine |
|------|--------|-----------|----------|
| PlaceOrder | ✅ | ✅ | ✅ |
| Cancel | ✅ 移除 | ✅ 推送 | ❌ 未更新 |
| Reduce | ✅ 更新 | ❌ 未推送 | ❌ 未更新 |
| Reduce→0 | ✅ 移除 | ❌ 未推送 | ❌ 未更新 |

---

## 🎯 修复优先级

1. **P0**: Cancel 状态持久化 (CAN-001 直接影响用户体验)
2. **P1**: Reduce 状态持久化 (RED-002/003)
3. **P2**: Reduce WebSocket 推送 (可选,非阻塞)
