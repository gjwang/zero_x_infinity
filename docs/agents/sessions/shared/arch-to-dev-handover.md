# Architect → Developer: 0x14-b Matching Engine Handover

## 📦 设计交付物

- [x] Architecture Design: `docs/src/0x14-b-matching-engine.md`
- [x] Implementation Plan: `docs/agents/sessions/architect/0x14-b-matching-engine-handover.md`
- [x] Test Checklist: `docs/agents/sessions/qa/0x14-b-test-checklist.md`

## 🎯 实施目标

**ONE SENTENCE**: 实现支持 GTC/IOC 订单类型的现货撮合引擎，以通过 Exchange-Core Benchmark 验证。

**关键指标**:
- Performance: 单次 `process_order` < 5µs (无 I/O)
- Correctness: Golden Single Pair Spot 100% 匹配
- Reliability: 无 `unwrap()` panic 风险

## 📋 实施计划概要

### Phase 1: Model Extensions (Priority P0)
- Task 1.1: `models.rs` 增加 `TimeInForce` 枚举 (~0.5 days)
- Task 1.2: `InternalOrder` 增加 `time_in_force` 字段 (~0.5 days)

### Phase 2: Matching Engine Core (Priority P0)
- Task 2.1: 创建 `src/engine/matching.rs` 模块 (~1 day)
- Task 2.2: 实现 `match_limit_order` (GTC + IOC) (~1 day)
- Task 2.3: 实现 `match_market_order` (~0.5 days)

### Phase 3: Command Support (Priority P1)
- Task 3.1: 实现 `reduce_order` (~0.5 days)
- Task 3.2: 实现 `move_order` (atomic cancel+place) (~0.5 days)

## 🔑 关键设计决策

| 决策 | 原因 | 替代方案 |
|------|------|----------|
| `TimeInForce` 枚举 | 明确区分 GTC/IOC 执行策略 | 隐式 GTC (无法支持 IOC) |
| IOC 剩余过期 | Exchange-Core 行为一致性 | 部分成交入簿 (非标准) |
| `MoveOrder` = Cancel + Place | 简化实现，优先级丢失可接受 | 原地修改 (复杂度高) |

## ⚠️ 实施注意事项

### DO (必须)
- [x] `TimeInForce::GTC` 为默认值
- [x] IOC 订单处理后 **绝不** 留存在订单簿中
- [x] 使用 `Result` 处理错误，避免 `unwrap()`

### DON'T (禁止)
- [x] 不要实现 Margin/Futures 逻辑 (推迟至 0x14-c)
- [x] 不要修改 `orderbook.rs` 核心结构 (仅扩展)
- [x] 不要在 matching loop 中使用 `println!` (性能)

## 📝 代码示例

```rust
// 预期的 API 设计
impl Engine {
    pub fn process_order(&mut self, order: InternalOrder) -> OrderResult {
        match order.time_in_force {
            TimeInForce::GTC => self.match_and_rest(order),
            TimeInForce::IOC => self.match_and_expire(order),
            TimeInForce::FOK => self.match_or_cancel(order), // Optional
        }
    }
}
```

## ✅ 验收标准

### 功能验收
- [ ] `test_gtc_maker`: GTC 订单进入订单簿
- [ ] `test_ioc_partial_fill`: IOC 部分成交后过期
- [ ] `test_market_sweep`: 市价单跨多档成交

### 性能验收
- [ ] `process_order` 平均延迟 < 5µs

### 质量验收
- [ ] `cargo clippy` 无 warning
- [ ] 单元测试覆盖核心逻辑

## 🔗 相关文档

- Architecture: [0x14-b-matching-engine.md](../../../../docs/src/0x14-b-matching-engine.md)
- Generator Spec: [0x14-a-bench-harness.md](../../../../docs/src/0x14-a-bench-harness.md)

## 📞 Ready for Development

Architect签名: @Architect AI Agent  
Date: 2025-12-30  
Status: ✅ Ready for implementation
