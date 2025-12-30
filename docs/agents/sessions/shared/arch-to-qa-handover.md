# Architect → QA: 0x14-b Matching Engine Test Requirements

## 📦 交付物

- [x] Test Checklist: `docs/agents/sessions/qa/0x14-b-test-checklist.md`
- [x] Architecture Overview: `docs/src/0x14-b-matching-engine.md`
- [x] Key Test Scenarios: 见下文

## 🎯 测试目标

**ONE SENTENCE**: 验证现货撮合引擎正确支持 IOC 订单类型及 Reduce/Move 指令。

## 🔑 关键测试场景 (基于 Generator 分析)

### 必须测试 (P0)
1. **IOC Expire**: 部分成交后剩余部分立即过期 (绝不入簿)
2. **IOC Never Rests**: 处理后 `book.all_orders()` 不含该订单

### 应该测试 (P1)
1. **ReduceOrder**: 减量后保留优先级
2. **MoveOrder**: 改价后优先级丢失 (cancel+place 语义)

### 明确跳过
1. **FokBudget**: Generator 定义但从未生成，不需测试

## ⚠️ 测试难点预警

| 难点 | 原因 | 建议方法 |
|------|------|----------|
| IOC 残留检查 | 需验证订单簿状态 | 每次 IOC 后检查 `book.all_orders()` |
| 优先级验证 | ReduceOrder 应保留，MoveOrder 应丢失 | 同价位提交多订单，验证匹配顺序 |

## 📝 测试数据建议

- Generator 行号参考:
  - IOC: L555 `generate_ioc_order()`
  - ReduceOrder: L472
  - MoveOrder: L504

## 🔗 相关文档

- Architecture Design: [0x14-b-matching-engine.md](../../../../docs/src/0x14-b-matching-engine.md)
- Generator (for reference): [0x14-a-bench-harness.md](../../../../docs/src/0x14-a-bench-harness.md)

## 📞 Ready for Test Planning

Architect签名: @Architect AI  
Date: 2025-12-30  
Status: ✅ Ready for QA review
