# 0x14-b Matching Engine QA Test Checklist

> **Status**: 📋 READY FOR QA  
> **Author**: Architect Team  
> **Date**: 2025-12-30  
> **Scope**: IOC + ReduceOrder + MoveOrder (FokBudget 跳过)

---

## 测试概述

**设计目标**: 实现支持 GTC/IOC 的现货撮合引擎及 Reduce/Move 指令。

**关键质量属性**:
| 属性 | 目标 | 测试方法 |
|------|------|----------|
| Correctness | IOC 残留检查 100% | 状态验证 |
| Performance | < 5µs/order | 微观基准测试 |
| Reliability | 无 panic | Edge case 覆盖 |

---

## Phase 1: TimeInForce (TIF) Tests [P0]

### Test 1.1: GTC (Good Till Cancel) - 已有功能验证

#### Test 1.1.1: GTC Maker (No Match)
- [ ] **Precondition**: 空订单簿
- [ ] **Action**: 提交 Buy 100 @ 100 (GTC)
- [ ] **Expected**: 订单进入订单簿，best_bid = 100

#### Test 1.1.2: GTC Partial Fill
- [ ] **Precondition**: 订单簿有 Sell 60 @ 100
- [ ] **Action**: 提交 Buy 100 @ 100 (GTC)
- [ ] **Expected**: 成交 60，剩余 40 进入订单簿

### Test 1.2: IOC (Immediate or Cancel) [P0 - 新增]

#### Test 1.2.1: IOC Full Match
- [ ] **Precondition**: 订单簿有 Sell 100 @ 100
- [ ] **Action**: 提交 Buy 100 @ 100 (IOC)
- [ ] **Expected**: 成交 100，订单状态 = FILLED

#### Test 1.2.2: IOC Partial Fill + Expire
- [ ] **Precondition**: 订单簿有 Sell 60 @ 100
- [ ] **Action**: 提交 Buy 100 @ 100 (IOC)
- [ ] **Expected**: 
  - 成交 60
  - 剩余 40 过期 (状态 = EXPIRED)
  - **Critical**: 订单簿中 **绝不** 包含该 IOC 订单

**验证方法**:
```bash
cargo test test_ioc_partial_fill
```

#### Test 1.2.3: IOC No Match → Immediate Expire
- [ ] **Precondition**: 空订单簿 或 无匹配价格
- [ ] **Action**: 提交 Buy 100 @ 100 (IOC)
- [ ] **Expected**: 订单立即过期，无成交，状态 = EXPIRED

---

## Phase 2: Command Tests [P1]

### Test 2.1: ReduceOrder

#### Test 2.1.1: Reduce Quantity (保留优先级)
- [ ] **Precondition**: 订单簿有 Order ID=1, qty=100 @ price=100
- [ ] **Action**: ReduceOrder(ID=1, reduceBy=30)
- [ ] **Expected**: 
  - 订单 qty=70
  - 订单仍在原价格档位
  - 优先级保留 (不是 cancel+place)

#### Test 2.1.2: Reduce to Zero → Remove
- [ ] **Precondition**: 订单簿有 Order ID=1, qty=100
- [ ] **Action**: ReduceOrder(ID=1, reduceBy=100)
- [ ] **Expected**: 订单从簿中移除

### Test 2.2: MoveOrder

#### Test 2.2.1: Move Price (优先级丢失)
- [ ] **Precondition**: 订单簿有 Order ID=1 @ 100
- [ ] **Action**: MoveOrder(ID=1, newPrice=101)
- [ ] **Expected**: 
  - 订单价格=101
  - 优先级丢失 (相当于 cancel+place)

#### Test 2.2.2: Move Non-existent Order → No-op
- [ ] **Precondition**: 订单簿无 Order ID=999
- [ ] **Action**: MoveOrder(ID=999, newPrice=101)
- [ ] **Expected**: 返回错误或 no-op

---

## 明确跳过的测试

| 功能 | 原因 |
|------|------|
| **FokBudget** | Generator 定义但从未生成，不需测试 |

---

## 验收标准总结

### 功能验收
- [ ] 所有 Phase 1/2 测试通过
- [ ] IOC 残留检查 100% (never in book)

### 性能验收
- [ ] `bench_process_order` < 5µs

### 覆盖率要求
- [ ] IOC 逻辑 100% 覆盖
- [ ] Reduce/Move 边界覆盖

---

## 预计测试工作量

| Phase | 工作量 | 优先级 |
|-------|--------|--------|
| Phase 1 (IOC) | 1 day | P0 |
| Phase 2 (Commands) | 0.5 day | P1 |

**Total**: 1.5 days
