# Pipeline MT TDengine Persistence Gap

> [!WARNING]
> **严重问题**: Pipeline MT 模式不会将数据持久化到 TDengine

## 问题描述

| 模式 | 数据输入 | TDengine 持久化 | 输出 |
|------|----------|-----------------|------|
| `--pipeline-mt` | fixtures/orders.csv | ❌ **无** | output/t2_*.csv |
| `--gateway` | HTTP API | ✅ | TDengine 表 |

## 影响

1. **Settlement 对比测试无法进行**
   - Pipeline MT 生成的 balance 数据只在 CSV 中
   - TDengine 没有对应数据可供对比

2. **数据流不一致**
   - 两种模式产生的数据无法交叉验证
   - 无法验证 Pipeline 计算结果与 TDengine 持久化结果一致性

## 建议方案

### 方案 A: 为 Pipeline MT 添加 TDengine 持久化 (推荐)

```rust
// pipeline_mt.rs
// 添加 TDengineClient 初始化和持久化调用
```

好处:
- 统一数据流
- 可以直接对比 Pipeline 输出与 TDengine 数据
- 支持完整的 settlement 验证

### 方案 B: 创建 CSV-to-TDengine 导入工具

```bash
# scripts/import_balances.py
# 将 Pipeline CSV 输出导入 TDengine 用于对比
```

### 方案 C: 分离测试策略

- Pipeline 正确性: 使用 CSV 对比 (已有)
- Gateway 持久化: 使用 API 验证 (已实现)

## 当前状态

目前采用 **方案 C** 作为临时解决方案:
- `scripts/test_gateway_persistence.sh` - 验证 Gateway API 持久化
- Pipeline MT 正确性通过 CSV 对比验证

## 优先级

**P1** - 应在下一个 sprint 解决
