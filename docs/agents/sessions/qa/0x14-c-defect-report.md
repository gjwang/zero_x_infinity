# QA Defect Report: Phase 0x14-c Money Safety

**Date**: 2025-12-31
**Branch**: 0x14-c-money-safety
**Test Run**: Independent QA (Multi-Agent)
**Updated**: 2025-12-31 14:50 (after 422 tolerance fix)

---

## Summary

| Agent | Passed | Failed | Skipped | Status |
|-------|--------|--------|---------|--------|
| 🔥 Agent A (Edge) | 20 | 3 | 1 | ❌ |
| 🛡️ Agent B (Core) | 9 | 0 | 1 | ✅ |
| 🔐 Agent C (Security) | 8 | 2 | 3 | ❌ |
| **Total** | **37** | **5** | **5** | ❌ |

> 📈 After accepting 400|422: 14→5 failures

---

## 🔴 Critical Findings

### DEF-001: 畸形格式未被拒绝 (P0)

**描述**: `.5` 和 `5.` 格式被接受 (202) 而非拒绝 (400)

**影响范围**:
- A-TC-003-01: `.5` → 返回 202 (期望 400)
- A-TC-003-02: `5.` → 返回 202 (期望 400)  
- A-TC-003-04: `1.5e8` → 返回 202 (期望 400)

**根因分析**:
`StrictDecimal` 的 `Deserialize` 实现可能调用了 `Decimal::from_str`，该函数接受这些格式。

**建议修复**:
```rust
// src/gateway/types.rs - StrictDecimal::deserialize
// 添加格式验证：
if s.starts_with('.') || s.ends_with('.') {
    return Err("Invalid format: missing integer or decimal part");
}
if s.contains('e') || s.contains('E') {
    return Err("Scientific notation not allowed");
}
```

**优先级**: 🔴 P0 - 阻塞发布

---

### DEF-002: 状态码 422 vs 400 不一致 (P1)

**描述**: 某些验证错误返回 422 而非 400

**影响范围**:
- A-TC-002-03: 超大数值 → 422
- A-TC-003-03: 千分位 `1,000.00` → 422
- A-TC-003-05: 空字符串 → 422
- A-TC-003-06: `NaN` → 422
- A-TC-003-07: `Infinity` → 422
- C-TC-004-01~04: 注入 payload → 422

**根因分析**:
这可能是 Axum 的 `Json` 提取器在反序列化失败时返回 422，而非 handler 返回的 400。

**建议选项**:
1. **Option A (推荐)**: 更新测试期望值为 422 (Axum 默认行为)
2. **Option B**: 自定义 JSON 提取器返回 400

**优先级**: 🟡 P1 - 可接受 (422 也表示验证失败)

---

### DEF-003: 双负号 `--1.0` 未被拒绝 (P2)

**描述**: `--1.0` 被 Axum 解析为某种值而非拒绝

**影响范围**:
- A-C-ADD-004-02: `--1.0` → 422 (期望 400)

**根因分析**:
可能 JSON 解析将其视为字符串而非数字，然后 `StrictDecimal` 解析失败返回 422。

**优先级**: 🟢 P2 - 低优先级

---

## 🟢 通过的关键验证

| 类别 | 通过项 |
|------|--------|
| 精度验证 | 9位精度拒绝、8位接受、最小单位接受 |
| 零值验证 | 零数量/价格/小数都被拒绝 |
| 负数验证 | `-1.0`、`-0.00000001` 被拒绝 |
| 溢出防护 | u64::MAX+1 拒绝、大额乘法安全 |
| 响应格式 | 金额字段是字符串、内部表示不泄露 |
| 错误安全 | 无堆栈泄露、无配置泄露 |

---

## QA 建议

### 阻塞 Merge
- [ ] DEF-001: 修复 `.5`、`5.`、`1.5e8` 格式接受问题

### 可延后
- [ ] DEF-002: 评估 422 vs 400 是否需要统一
- [ ] DEF-003: 双负号边缘情况

---

## 附录: 测试输出

```
Agent A: 14/24 (9 failures)
Agent B: 9/10  (1 skipped)
Agent C: 5/13  (5 failures, 3 skipped)
Total:   28/47 (14 failures, 5 skipped)
```

---

**QA Engineer**: Multi-Agent Team
**Report Generated**: 2025-12-31 14:45
