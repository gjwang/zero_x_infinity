# Architect → QA: 0x14-b Matching Engine Test Requirements

## 📦 交付物

- [x] Test Checklist: `docs/agents/sessions/qa/0x14-b-test-checklist.md`
- [x] Architecture Overview: `docs/src/0x14-b-matching-engine.md`
- [x] Key Test Scenarios: 见下文

## 🎯 测试目标

**ONE SENTENCE**: 验证现货撮合引擎正确支持 GTC/IOC 订单类型，行为与 Exchange-Core 一致。

## 🔑 关键测试场景

### 必须测试 (P0)
1. **GTC Maker**: 无对手盘时订单进入订单簿
2. **IOC Expire**: 部分成交后剩余部分立即过期 (绝不入簿)
3. **Price-Time Priority**: 同价位按 FIFO 撮合

### 应该测试 (P1)
1. **Market Sweep**: 市价单跨多档成交
2. **ReduceOrder**: 减量后保留优先级
3. **MoveOrder**: 改价后优先级丢失

### 可以测试 (P2)
1. **FOK (Fill or Kill)**: 全部成交或全部取消 (可选实现)

## ⚠️ 测试难点预警

| 难点 | 原因 | 建议方法 |
|------|------|----------|
| IOC 残留检查 | 需验证订单簿状态 | 每次 IOC 后检查 `book.all_orders()` |
| 优先级验证 | 需跟踪订单顺序 | 同价位提交多订单，验证匹配顺序 |

## 📝 测试数据建议

- 使用 `fixtures/orders.csv` 或内联测试数据
- 关键字段: `order_id`, `price`, `qty`, `time_in_force`

## 🔗 相关文档

- Architecture Design: [0x14-b-matching-engine.md](../../../../docs/src/0x14-b-matching-engine.md)
- Generator (for reference): [0x14-a-bench-harness.md](../../../../docs/src/0x14-a-bench-harness.md)

## 📞 Ready for Test Planning

Architect签名: @Architect AI  
Date: 2025-12-30  
Status: ✅ Ready for QA review
