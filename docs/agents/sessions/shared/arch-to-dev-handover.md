# Architect → Developer: 0x14-b Matching Engine Handover

## 📦 设计交付物

- [x] Architecture Design: `docs/src/0x14-b-order-commands.md`
- [x] Implementation Plan: 本文档
- [x] Test Checklist: `docs/agents/sessions/qa/0x14-b-test-checklist.md`

## 🎯 实施目标

**ONE SENTENCE**: 实现支持 GTC/IOC 订单类型及 Reduce/Move 指令的现货撮合引擎，以通过 Exchange-Core Benchmark 验证。

**关键指标**:
- Performance: 单次 `process_order` < 5µs (无 I/O)
- Correctness: Golden Single Pair Spot 100% 匹配
- Reliability: 无 `unwrap()` panic 风险

## 📋 实施计划 (基于 Generator 分析)

### Phase 1: TimeInForce::IOC (Priority P0)
- Task 1.1: `models.rs` 增加 `TimeInForce` 枚举 (~0.5 days)
- Task 1.2: `InternalOrder` 增加 `time_in_force` 字段 (~0.5 days)
- Task 1.3: `engine.rs::process_order()` 增加 IOC 逻辑: 剩余不入簿 (~0.5 days)

### Phase 2: ReduceOrder + MoveOrder (Priority P1)
- Task 2.1: 实现 `Engine::reduce_order(order_id, reduce_by)` (~0.5 days)
- Task 2.2: 实现 `Engine::move_order(order_id, new_price)` = atomic cancel + place (~0.5 days)

## 🔑 关键设计决策

| 决策 | 原因 | 替代方案 |
|------|------|----------|
| `TimeInForce` 枚举 | 明确区分 GTC/IOC 执行策略 | 隐式 GTC (无法支持 IOC) |
| IOC 剩余过期 | Exchange-Core 行为一致性 | 部分成交入簿 (非标准) |
| `MoveOrder` = Cancel + Place | 简化实现，优先级丢失可接受 | 原地修改 (复杂度高) |
| **FokBudget 跳过** | Generator 定义但从未生成 | - |

## ⚠️ 实施注意事项

### DO (必须)
- [x] `TimeInForce::GTC` 为默认值
- [x] IOC 订单处理后 **绝不** 留存在订单簿中
- [x] `ReduceOrder` 应保留时间优先级 (原地修改 qty)
- [x] 使用 `Result` 处理错误，避免 `unwrap()`

### DON'T (禁止)
- [x] 不要实现 `FokBudget` (Generator 未使用)
- [x] 不要实现 Margin/Futures 逻辑 (推迟至 0x14-c)
- [x] 不要在 matching loop 中使用 `println!` (性能)

## 📝 代码示例

```rust
// TimeInForce 枚举
pub enum TimeInForce {
    GTC, // Good Till Cancel (Default)
    IOC, // Immediate or Cancel
}

// 修改后的 process_order
impl MatchingEngine {
    pub fn process_order(book: &mut OrderBook, mut order: InternalOrder) -> OrderResult {
        let (trades, makers) = match order.side {
            Side::Buy => Self::match_buy(book, &mut order),
            Side::Sell => Self::match_sell(book, &mut order),
        };
        
        // IOC: 剩余不入簿
        if order.time_in_force == TimeInForce::IOC {
            if !order.is_filled() {
                order.status = OrderStatus::EXPIRED;
            }
            // DO NOT rest_order for IOC
        } else {
            // GTC: 剩余入簿
            if !order.is_filled() && order.order_type == OrderType::Limit {
                book.rest_order(order.clone());
            }
        }
        
        OrderResult { order, trades, makers }
    }
}
```

## ✅ 验收标准

### 功能验收
- [ ] `test_ioc_partial_fill`: IOC 100 qty vs 60 book → 60 filled, 40 expired
- [ ] `test_ioc_never_rests`: IOC 处理后 `book.all_orders()` 不含该订单
- [ ] `test_reduce_order`: 100 qty → reduce 30 → 70 qty 保留优先级
- [ ] `test_move_order`: Move 改价后优先级丢失

### 性能验收
- [ ] `process_order` 平均延迟 < 5µs

### 质量验收
- [ ] `cargo clippy` 无 warning
- [ ] 单元测试覆盖 IOC/Reduce/Move 逻辑

## 🔗 相关文档

- Architecture: [0x14-b-order-commands.md](../../../../docs/src/0x14-b-order-commands.md)
- Generator Spec: [0x14-a-bench-harness.md](../../../../docs/src/0x14-a-bench-harness.md)
- Generator Code: `src/bench/order_generator.rs` (L472, L504, L555)

## 📞 Ready for Development

Architect签名: @Architect AI Agent  
Date: 2025-12-30  
Status: ✅ Ready for implementation
