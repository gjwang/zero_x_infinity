# Architect → Developer Handover: Phase 0x14-c Money Safety

> **Branch**: `0x14-c-money-safety`
> **Design Spec**: [money-type-safety.md](../../standards/money-type-safety.md)
> **Date**: 2025-12-31

---

## 1. Objective

**落地 `src/money.rs` 类型安全基础设施到 Gateway 和 Funding 关键路径。**

当前状态：
- ✅ `ScaledAmount` / `ScaledAmountSigned` 类型已定义
- ✅ `parse_decimal` / `format_amount` 工具函数已实现
- ✅ 规范文档 `money-type-safety.md` 已就绪
- ❌ **Gateway 仍使用手工转换**
- ❌ **无 CI 审计脚本**

---

## 2. Scope (范围)

### 2.1 核心改动

| 文件 | 改动 | 优先级 |
|------|------|--------|
| `scripts/audit_money_safety.sh` | **新建** - CI 审计脚本 | P0 |
| `.github/workflows/ci.yml` | 添加审计步骤 | P0 |
| `src/gateway/handlers.rs` | 订单下单改用 `SymbolManager::parse_qty/price()` | P0 |
| `src/funding/deposit.rs` | 使用 `Asset::parse_amount()` | P1 |
| `src/funding/withdraw.rs` | 使用 `Asset::parse_amount_allow_zero()` | P1 |

### 2.2 不在范围内

- ❌ 存量代码全面扫描（Phase 2）
- ❌ StrictDecimal DTO 类型（Phase 1.5，后续由架构师设计）

---

## 3. Implementation Guide (实施指南)

### 3.1 Task D.1: CI 审计脚本

**创建 `scripts/audit_money_safety.sh`:**

```bash
#!/bin/bash
set -e

echo "🔍 Auditing money safety..."

# 1. 检查非 money.rs 中的手动缩放
if grep -rn "10u64.pow" --include="*.rs" src/ | grep -v "money.rs"; then
    echo "❌ FAIL: Found 10u64.pow outside money.rs"
    exit 1
fi

# 2. 检查 Decimal 手动幂运算
if grep -rn "Decimal::from(10).powi" --include="*.rs" src/ | grep -v "money.rs"; then
    echo "❌ FAIL: Found Decimal power operation outside money.rs"
    exit 1
fi

echo "✅ Money safety audit passed!"
```

**集成到 CI:**
```yaml
# .github/workflows/ci.yml
- name: Money Safety Audit
  run: ./scripts/audit_money_safety.sh
```

---

### 3.2 Task D.2: Gateway Order Handler 改造

**文件**: `src/gateway/handlers.rs`

**Before (当前):**
```rust
// 手工解析，容易出错
let qty: u64 = request.quantity.parse()?;
let price: u64 = request.price.parse()?;
```

**After (目标):**
```rust
use crate::money;

// 使用 SymbolManager 验证精度和格式
let qty = money::parse_qty(&request.quantity, symbol_id, &symbol_mgr)?;
let price = money::parse_price(&request.price, symbol_id, &symbol_mgr)?;
```

**错误处理映射:**
| MoneyError | HTTP Response |
|------------|---------------|
| `PrecisionOverflow` | `400 INVALID_PRECISION` |
| `InvalidAmount` | `400 INVALID_AMOUNT` |
| `ZeroNotAllowed` | `400 ZERO_NOT_ALLOWED` |
| `Overflow` | `400 AMOUNT_OVERFLOW` |

---

### 3.3 Task D.3: Funding Handlers 改造

**文件**: `src/funding/deposit.rs`, `src/funding/withdraw.rs`

参考 `AssetInfo` 上已实现的 intent-based API：

```rust
// src/exchange_info/asset/models.rs 中已有：
impl AssetInfo {
    pub fn parse_amount(&self, amount: Decimal) -> Result<ScaledAmount, MoneyError>;
    pub fn parse_amount_allow_zero(&self, amount: Decimal) -> Result<ScaledAmount, MoneyError>;
}
```

**使用示例:**
```rust
// deposit.rs
let amount_scaled = asset_info.parse_amount(request.amount)?;

// withdraw.rs - 手续费可为零
let fee_scaled = asset_info.parse_amount_allow_zero(request.fee)?;
```

---

## 4. Verification (验证)

### 4.1 单元测试
```bash
cargo test money::
cargo test gateway::handlers::
cargo test funding::
```

### 4.2 集成验证
```bash
# 审计脚本必须通过
./scripts/audit_money_safety.sh

# 全量测试
cargo test
```

### 4.3 手工验证
1. 启动 Gateway
2. 发送订单请求，验证：
   - 精度超限返回 `400 INVALID_PRECISION`
   - 零值返回 `400 ZERO_NOT_ALLOWED`
   - 正常值正确解析

---

## 5. Definition of Done (完成标准)

- [ ] `scripts/audit_money_safety.sh` 通过
- [ ] CI 集成审计步骤
- [ ] Gateway order handler 使用 `money::parse_qty/price`
- [ ] Funding handlers 使用 `Asset::parse_amount*`
- [ ] 所有测试通过
- [ ] 无新增 `10u64.pow()` 在 `money.rs` 外

---

## 6. Acceptance (验收)

完成后请：
1. 提交所有更改
2. 运行 `./scripts/audit_money_safety.sh`
3. 创建 **Dev → Arch Handover** 报告
4. 通知架构师进行 Code Review
