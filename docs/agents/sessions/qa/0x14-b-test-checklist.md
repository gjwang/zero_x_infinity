# 0x14-b Matching Engine QA Test Checklist

> **Status**: 📋 READY FOR QA  
> **Author**: Architect Team  
> **Date**: 2025-12-30

---

## 测试概述

**设计目标**: 实现支持 GTC/IOC 的现货撮合引擎，使 Rust 实现与 Exchange-Core 行为一致。

**关键质量属性**:
| 属性 | 目标 | 测试方法 |
|------|------|----------|
| Correctness | Golden Data 100% parity | 数据比对 |
| Performance | < 5µs/order | 微观基准测试 |
| Reliability | 无 panic | Edge case 覆盖 |

---

## Phase 1: TimeInForce (TIF) Tests

### Test 1.1: GTC (Good Till Cancel)

#### Test 1.1.1: GTC Maker (No Match)
- [ ] **Precondition**: 空订单簿
- [ ] **Action**: 提交 Buy 100 @ 100 (GTC)
- [ ] **Expected**: 订单进入订单簿，best_bid = 100

**验证方法**:
```bash
cargo test test_gtc_maker
```

#### Test 1.1.2: GTC Partial Fill
- [ ] **Precondition**: 订单簿有 Sell 60 @ 100
- [ ] **Action**: 提交 Buy 100 @ 100 (GTC)
- [ ] **Expected**: 成交 60，剩余 40 进入订单簿

**验证方法**:
```bash
cargo test test_gtc_partial_fill
```

### Test 1.2: IOC (Immediate or Cancel)

#### Test 1.2.1: IOC Full Match
- [ ] **Precondition**: 订单簿有 Sell 100 @ 100
- [ ] **Action**: 提交 Buy 100 @ 100 (IOC)
- [ ] **Expected**: 成交 100，订单状态 = FILLED

#### Test 1.2.2: IOC Partial Fill + Expire
- [ ] **Precondition**: 订单簿有 Sell 60 @ 100
- [ ] **Action**: 提交 Buy 100 @ 100 (IOC)
- [ ] **Expected**: 成交 60，剩余 40 过期 (状态 = EXPIRED)
- [ ] **Critical**: 订单簿中 **绝不** 包含该 IOC 订单

**验证方法**:
```bash
cargo test test_ioc_partial_fill
```

#### Test 1.2.3: IOC No Match
- [ ] **Precondition**: 空订单簿
- [ ] **Action**: 提交 Buy 100 @ 100 (IOC)
- [ ] **Expected**: 订单立即过期，无成交

---

## Phase 2: Command Tests

### Test 2.1: CancelOrder

#### Test 2.1.1: Cancel Existing Order
- [ ] **Precondition**: 订单簿有 Order ID=1
- [ ] **Action**: CancelOrder(ID=1)
- [ ] **Expected**: 订单从簿中移除

### Test 2.2: ReduceOrder

#### Test 2.2.1: Reduce Quantity
- [ ] **Precondition**: 订单簿有 Order ID=1, qty=100
- [ ] **Action**: ReduceOrder(ID=1, reduceBy=30)
- [ ] **Expected**: 订单 qty=70，保留优先级

### Test 2.3: MoveOrder

#### Test 2.3.1: Move Price
- [ ] **Precondition**: 订单簿有 Order ID=1 @ 100
- [ ] **Action**: MoveOrder(ID=1, newPrice=101)
- [ ] **Expected**: 订单价格=101，优先级丢失 (相当于 cancel+place)

---

## Phase 3: Market Order Tests

### Test 3.1: Market Sweep

#### Test 3.1.1: Consume Multiple Levels
- [ ] **Precondition**: Asks: [Sell 50 @ 100, Sell 50 @ 101]
- [ ] **Action**: Market Buy 80
- [ ] **Expected**: 成交 50@100 + 30@101，两笔 Trade

---

## 关键测试场景

### Happy Path
- [ ] GTC order rests in book
- [ ] IOC order matches and expires remainder

### Error Handling
- [ ] Cancel non-existent order → No-op
- [ ] Reduce by more than qty → Clamp or Error

### Edge Cases
- [ ] 边界值: qty=0, price=0
- [ ] 空订单簿: Market order → Expire
- [ ] 同价位多订单: FIFO 顺序

### Failure Scenarios
- [ ] 无匹配对手盘时 IOC 行为
- [ ] MoveOrder 目标价已存在订单

---

## 验收标准总结

### 功能验收
- [ ] 所有 Phase 测试通过
- [ ] IOC 残留检查 (never in book)

### 性能验收
- [ ] `bench_process_order` < 5µs

### 覆盖率要求
- [ ] 单元测试覆盖 GTC/IOC/Market
- [ ] Edge cases 覆盖

---

## 测试工具/脚本建议

| 工具 | 用途 |
|------|------|
| `cargo test engine::` | Engine 单元测试 |
| `cargo test golden_` | Golden 数据验证 |
| `cargo bench` | 性能基准 |

---

## 预计测试工作量

| Phase | 工作量 | 优先级 |
|-------|--------|--------|
| Phase 1 (TIF) | 1 day | P0 |
| Phase 2 (Commands) | 0.5 day | P1 |
| Phase 3 (Market) | 0.5 day | P1 |

**Total**: 2 days
