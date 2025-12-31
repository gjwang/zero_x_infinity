# 0x14-c Money Type Safety: API 层金额强制执行

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

| Status | 🚧 **IN PROGRESS** |
| :--- | :--- |
| **Context** | Phase V: Extreme Optimization (Step 3) |
| **Goal** | Complete all pending tasks from money-type-safety.md - unified money handling enforcement |
| **Scope** | CI Audit, API Layer Enforcement, Internal Legacy Migration |

---

### Executive Summary: Why This Matters

> **本次重构是一次战略性的技术债务清理，为系统的长期可靠性奠定不可动摇的基石。**

#### 🎯 战略目标

1. **100% 账本正确性基础**
   
   金融系统的核心承诺是：**每一分钱都可追溯、可验证、可审计**。当缩放逻辑散落在 20+ 个文件中时，任何单点故障都可能破坏这一承诺。本次重构将所有金额转换收敛到 `money.rs`，使我们能够：
   - 对唯一的转换路径进行**穷尽式测试**
   - 在账本不平时**精确定位根因**（不再有"可能是精度误差"的借口）
   - 实现**资金恒等定理**的强制验证：`总入金 ≡ 总余额 + 总出金`

2. **技术债务一次性清零**
   
   过去为了快速迭代，我们在多处使用了 `10u64.pow(8)` 手工缩放。这些代码像**隐藏的定时炸弹**，随时可能因复制粘贴错误、精度不一致或资产配置变更而引爆。本次重构系统性地扫描并修复所有遗留点，将技术债务归零。

3. **可持续迭代的坚实保障**
   
   引入 CI 审计脚本后，**新代码无法绕过类型安全机制**。这意味着：
   - 新增资产时，无需逐文件检查精度处理
   - Code Review 时，`10u64.pow` 出现即为红旗
   - 团队成员（包括 AI Agent）自动遵循统一规范

4. **从"正确性未知"到"正确性可证明"**
   
   重构前：我们**相信**系统是正确的，但无法**证明**。
   重构后：通过类型系统 + CI 审计 + 穷尽测试，正确性成为**可验证的属性**，而非信仰。

#### 📊 预期成果

| 维度 | Before | After |
|------|--------|-------|
| 金额转换入口 | 20+ 分散点 | 1 个集中模块 |
| 精度硬编码 | 6+ 文件 | 0（全部从 SymbolManager 获取）|
| CI 防护 | ❌ 无 | ✅ `audit_money_safety.sh` |
| 账本对账 | 依赖人工核查 | 系统可验证 |

> [!IMPORTANT]
> **这不是一次普通的代码清理，而是系统从"快速原型"向"生产级金融基础设施"演进的里程碑。**


---

### 0. Task Overview (from money-type-safety.md)

| Phase | Task | Status |
|-------|------|--------|
| **Phase 0** | Newtype 定义, API 收缩, 文档治理 | ✅ 已完成 |
| **Phase 1** | `audit_money_safety.sh` 集成 CI | 🚧 本次实现 |
| **Phase 1.5** | API Money Enforcement (Extractor + IntoResponse) | 🚧 本次实现 |
| **Phase 2** | 存量代码全面扫描与迁移 | 🚧 本次实现 |
| **Phase 2.5** | Legacy 代码迁移至意图封装 API | 🚧 本次实现 |

**本阶段目标**：一次性完成所有待实现任务，实现 Money Safety 的全面落地。

---

### 1. Problem Statement

> **"Money is a domain concept, not a primitive type."**

Our exchange processes millions of dollars daily. A single precision bug could cause:
- **Account reconciliation failure**: Unable to balance books 100%
- **Silent fund loss**: Truncation/overflow goes undetected
- **Regulatory risk**: Audit trails become unreliable

#### 1.1 Current Anti-patterns

```rust
// ❌ Manual scaling everywhere - error-prone, hard to maintain
let qty: u64 = request.quantity.parse()?;
let scaled = qty * 10u64.pow(8);  // What if someone forgets this?

// ❌ Hardcoded decimals - what if different assets have different precision?
let formatted = format!("{:.8}", amount as f64 / 100_000_000.0);
```

#### 1.2 The Solution: Centralized Money Module

We already have `src/money.rs` with:
- `ScaledAmount` - Newtype wrapper preventing raw arithmetic
- `parse_decimal()` / `format_amount()` - Audited conversion functions
- `MoneyFormatter` - Batch formatting for order books

**This phase activates these tools in production code paths.**

---

### 2. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Client (JSON String)                        │
│                      "quantity": "1.5"                          │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  Layer 1: Gateway Handler (src/gateway/handlers.rs)             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ money::parse_qty(&req.quantity, symbol_id, &mgr)?       │   │
│  │ → Returns ScaledAmount or MoneyError                    │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  Layer 2: Money Module (src/money.rs)                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ - Precision validation (reject if too many decimals)   │   │
│  │ - Overflow protection (checked arithmetic)              │   │
│  │ - Zero rejection (for quantities)                       │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  Layer 3: SymbolManager (src/symbol_manager.rs)                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ - Provides decimals per asset/symbol                    │   │
│  │ - Single source of truth for precision configuration   │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                 Internal: ScaledAmount(u64)                     │
│                        150_000_000                              │
└─────────────────────────────────────────────────────────────────┘
```

---

### 3. Implementation Plan

#### 3.1 Phase 1: CI Audit Script (P0)

**Purpose**: Prevent regression by detecting manual scaling outside `money.rs`.

```bash
# scripts/audit_money_safety.sh
#!/bin/bash
set -e

echo "🔍 Auditing money safety..."

# Allowed locations (whitelist)
ALLOWED_FILES="money.rs|symbol_manager.rs"

# 1. Check for manual scaling
VIOLATIONS=$(grep -rn "10u64.pow" --include="*.rs" src/ | grep -v -E "$ALLOWED_FILES" || true)
if [ -n "$VIOLATIONS" ]; then
    echo "❌ FAIL: Found 10u64.pow outside allowed files:"
    echo "$VIOLATIONS"
    exit 1
fi

echo "✅ Money safety audit passed!"
```

---

#### 3.2 Phase 1.5: API Money Enforcement (P0)

**Target**: Gateway/API layer type enforcement

| File | Current | Target |
|------|---------|--------|
| `src/gateway/handlers.rs` | Manual parse | `money::parse_qty/price()` |
| `src/gateway/types.rs` | `String` fields | `StrictDecimal` type |

---

#### 3.3 Phase 2: Legacy Code Scan & Migration

**Scan Results**: Files containing `10u64.pow()` outside `money.rs`:

| File | Line(s) | Context | Action |
|------|---------|---------|--------|
| `src/symbol_manager.rs` | 25 | `qty_unit()` helper | ✅ Keep (core infrastructure) |
| `src/models.rs` | 363, 385, 386, 399, 413 | Test helpers | 🔧 Move to test module or use constants |
| `src/sentinel/eth.rs` | 585, 613 | Chain precision conversion | 🔧 Use `ChainAsset::decimals` |
| `src/persistence/queries.rs` | 485, 1153, 1174 | Quote qty calculation | 🔧 Use `SymbolInfo::quote_qty()` |
| `src/csv_io.rs` | 148, 152, 248 | CSV parsing | 🔧 Use `SymbolManager` |
| `src/websocket/service.rs` | 273, 274, 310, 311 | Depth/Ticker formatting | ✅ Already using `money::` module |

**Priority Order**:
1. **P0**: `persistence/queries.rs` - High traffic path
2. **P1**: `sentinel/eth.rs` - Security critical
3. **P2**: `models.rs` - Test helpers (lowest risk)
4. **P3**: `csv_io.rs` - Batch import (offline)

---

#### 3.4 Phase 2.5: Intent-based API Migration

**Goal**: Replace direct `money::` calls with `Asset` / `AssetInfo` methods.

| Old Pattern | New Pattern |
|-------------|-------------|
| `money::parse_decimal(d, asset.decimals as u32)` | `asset.parse_amount(d)` |
| `money::parse_decimal_allow_zero(d, decimals)` | `asset.parse_amount_allow_zero(d)` |
| `money::format_amount(amt, dec, disp)` | `asset.format_amount(amt)` |

**Files to migrate**:
| File | Status |
|------|--------|
| `src/funding/deposit.rs` | ✅ Already uses `money::format_amount_signed` |
| `src/funding/withdraw.rs` | ✅ Already uses `money::format_amount_signed` |
| `src/funding/service.rs` | 🔧 Migrate to `asset.format_amount()` |
| `src/market/depth_service.rs` | 🔧 Use `MoneyFormatter` |
| `src/internal_transfer/api.rs` | ✅ Uses local `format_amount` wrapper |

---

### 4. Validation

#### 4.1 Unit Tests

```bash
cargo test money::
```

#### 4.2 Integration Tests

```bash
# Must pass before merge
./scripts/audit_money_safety.sh

# Full test suite
cargo test
```

#### 4.3 Manual Verification

| Test Case | Input | Expected Result |
|-----------|-------|-----------------|
| Valid quantity | `"1.5"` | `150_000_000` (8 decimals) |
| Precision exceeded | `"1.123456789"` (9 decimals) | `400 PRECISION_EXCEEDED` |
| Zero quantity | `"0"` | `400 ZERO_NOT_ALLOWED` |
| Negative | `"-1.0"` | `400 INVALID_AMOUNT` |
| Overflow | `"999999999999999999999"` | `400 AMOUNT_OVERFLOW` |

---

### 5. Success Criteria

- [ ] `scripts/audit_money_safety.sh` passes in CI
- [ ] All `10u64.pow()` outside whitelist removed or justified
- [ ] Gateway handlers use `money::parse_qty/price`
- [ ] Funding handlers use `Asset::parse_amount*`
- [ ] All 370+ tests pass

---

<div id="-chinese"></div>

## 🇨🇳 中文

| 状态 | 🚧 **进行中** |
| :--- | :--- |
| **上下文** | Phase V: 极致优化 (第三步) |
| **目标** | 在 API 边界强制类型安全的金额处理，防止精度/溢出 Bug |
| **范围** | Gateway handlers、Funding handlers、CI 审计 |

---

### 1. 问题陈述

> **"金额是领域概念，不是原始类型。"**

我们的交易所每天处理数百万美元。一个精度 Bug 可能导致：
- **账本对不齐**：无法 100% 平账
- **静默资金损失**：截断/溢出未被检测
- **合规风险**：审计轨迹变得不可靠

#### 1.1 当前反模式

```rust
// ❌ 到处手动缩放 - 容易出错，难以维护
let qty: u64 = request.quantity.parse()?;
let scaled = qty * 10u64.pow(8);  // 如果有人忘了呢？

// ❌ 硬编码精度 - 不同资产精度不同怎么办？
let formatted = format!("{:.8}", amount as f64 / 100_000_000.0);
```

#### 1.2 解决方案：集中式 Money 模块

我们已经有 `src/money.rs`：
- `ScaledAmount` - Newtype 包装，防止裸算术运算
- `parse_decimal()` / `format_amount()` - 经过审计的转换函数
- `MoneyFormatter` - 用于深度图的批量格式化

**本阶段在生产代码路径中激活这些工具。**

---

### 2. 架构

与英文版相同，请参见上方架构图。

---

### 3. 实施计划

#### 3.1 CI 审计脚本 (P0)

**目的**：检测 `money.rs` 外的手动缩放，防止回归。

#### 3.2 Gateway Handler 迁移 (P0)

**目标**：`src/gateway/handlers.rs` - 下单 handler

使用 `money::parse_qty()` 和 `money::parse_price()` 替代手工解析。

#### 3.3 Funding Handler 迁移 (P1)

**目标**：`src/funding/deposit.rs`, `src/funding/withdraw.rs`

使用 `AssetInfo` 上的意图封装 API。

---

### 4. 验证

#### 4.1 测试命令

```bash
# 审计脚本必须通过
./scripts/audit_money_safety.sh

# 全量测试
cargo test
```

#### 4.2 手工验证用例

| 测试用例 | 输入 | 预期结果 |
|----------|------|----------|
| 有效数量 | `"1.5"` | `150_000_000` |
| 精度超限 | `"1.123456789"` | `400 PRECISION_EXCEEDED` |
| 零值数量 | `"0"` | `400 ZERO_NOT_ALLOWED` |
| 负数 | `"-1.0"` | `400 INVALID_AMOUNT` |
| 溢出 | `"999999999999999999999"` | `400 AMOUNT_OVERFLOW` |

---

### 5. 完成标准

- [ ] `scripts/audit_money_safety.sh` 在 CI 中通过
- [ ] Gateway 订单 handler 使用 `money::parse_qty/price`
- [ ] Funding handlers 使用 `Asset::parse_amount*`
- [ ] 所有 370+ 测试通过
- [ ] `money.rs` 外无手动 `10u64.pow()`

---

## References

- [Money Type Safety Standard](./standards/money-type-safety.md)
- [API Money Enforcement](./standards/api-money-enforcement.md)
- [0x14-b Order Commands](./0x14-b-order-commands.md) (Previous phase)
