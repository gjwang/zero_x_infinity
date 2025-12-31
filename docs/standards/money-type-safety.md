# Money Type Safety Standard | 资金类型安全规范

> **Version**: 1.3 | **Last Updated**: 2025-12-31
>
> 本文件定义了本项目处理资金（余额、订单金额、成交价格）的**治理方案**。
> 重点是：**如何在代码层面禁止不符合规范的操作**。
> 任何违反本规范的代码**不得合并**。

---

## Part I: 背景与设计决策

### 1.1 核心风险

**金额是领域概念，不是原始类型。**

在任何金融系统中，"钱"都不应被视为一个裸露的整数。它是一个携带精度语义的**领域对象**——1 BTC 内部表示为 `100_000_000` 聪，这个 `10^8` 的缩放因子是资产的**内在属性**，而非程序员的临时决定。

当开发者在代码中随意写下 `amount * 10u64.pow(8)` 时，他实际上在**破坏这层抽象**，将领域逻辑泄漏到业务代码的每一个角落。这会导致：

| 风险类型 | 后果 |
|----------|------|
| **账本无法对齐** | 任何微小误差都会破坏"资金恒等定理"，导致无法 100% 精确对账。我们无法区分"正常误差"还是"真正的 Bug"。 |
| **语义错误** | 错误地将 BTC 金额与 USDT 金额直接相加。 |
| **溢出攻击** | 恶意构造的超大数值导致系统崩溃或资金错算。 |
| **维护噩梦** | 转换逻辑复杂，到处重复写必然到处犯错。 |

### 1.2 为什么选择 `u64` + 内部缩放？

> **前置阅读**: 关于浮点数的问题，请参阅 [0x02 浮点数的诅咒](../src/0x02-the-curse-of-float.md)，此处不再重复。

**核心结论**：
- `f64` 无法满足**跨平台确定性**（不同 CPU/编译器结果可能不同）。
- `Decimal` 无法满足**极致性能**（比 `u64` 慢 10x+）。
- **`u64` 是唯一能同时满足"区块链级验证强度"和"高频撮合性能"的方案。**

但 `u64` 需要**内部缩放**，这引入了复杂性。因此我们必须：
1. 将缩放算法封装在 `money.rs` 中。
2. **严禁**在其他地方手工进行缩放运算。

---

### 1.3 内部缩放方案：如何实现大额处理？

**核心机制**：我们为每种资产定义**系统精度**（通常 8 位），而非使用链上原生精度（如 ETH 的 18 位）。

| 资产 | 链上精度 | 系统精度 | `u64` 最大可处理金额 |
|------|----------|----------|----------------------|
| BTC  | 8 位     | 8 位     | **1844 亿 BTC** (远超总供应量) |
| ETH  | 18 位    | **8 位** | **1844 亿 ETH** ✅ |
| USDT | 6 位     | 6 位     | **18.4 万亿 USDT** ✅ |

> [!IMPORTANT]
> **精度权衡**：使用 8 位系统精度意味着 ETH 最小单位是 `0.00000001 ETH` (10 gwei)，而非链上的 `1 wei`。
> 对于交易所场景，这完全足够——没有人会交易 1 wei 的 ETH。

**这就是为什么"缩放"必须封装**：
- 不同资产有不同的**链上精度**和**系统精度**。
- 入金时：链上精度 → 系统精度（可能截断极小尾数）。
- 出金时：系统精度 → 链上精度（补零）。
- 这套转换逻辑复杂，必须集中管理，严禁各处手写。

> [!TIP]
> **`u128` 的替代方案**：如果不追求极致性能，使用 `u128` 可以直接采用统一的 18 位精度，避免不同资产间的精度转换问题。但这会牺牲约 10-20% 的撮合性能。

---

## Part II: 解决方案与决策 (Solutions & Decisions)

### 2.1 类型安全：Newtype 守卫 (The Newtype Guardian)

**问题**: `u64` 是原生类型，开发者可以轻易写出 `amount * 10u64.pow(8)`。

**方案**: 引入**不透明**的包装类型 `ScaledAmount(u64)`:
- 内部字段 `u64` 是 **private** 的，无法直接访问。
- 所有构造必须通过 `money.rs` 提供的审计过的 Constructor。
- 如果有人想"私自计算"，他必须先解包（`to_raw()`），这种"不自然"的操作在 Code Review 中一眼可见。

```rust
// 🛡️ 核心类型定义
pub struct ScaledAmount(u64);        // 无符号：余额、订单数量
pub struct ScaledAmountSigned(i64);  // 有符号：盈亏、差额
```

**已实现**:
- [x] `ScaledAmount` / `ScaledAmountSigned` 定义
- [x] `checked_add` / `checked_sub` 安全算术
- [x] `Deref<Target = u64>` 允许比较，但禁止直接算术

---

### 2.2 访问控制：入口收缩 (Visibility Chokepoint)

**问题**: 如果底层函数 `parse_amount(str, decimals)` 是 `pub` 的，开发者会倾向于直接使用它。

**方案**: 将 Layer 1 工具函数收缩为 `pub(crate)`：

| 可见性 | 函数 | 用途 |
|--------|------|------|
| `pub(crate)` | `parse_amount`, `format_amount` | 仅限 `money.rs` 和核心模块内部使用 |
| `pub` | `SymbolManager::parse_qty()`, `SymbolManager::format_price()` | **外部唯一入口** |

**效果**: 在代码自动补全时，开发者首先看到的是 `SymbolManager` 上的高层方法。

**已实现**:
- [x] `parse_amount` / `format_amount` 改为 `pub(crate)`

---

### 2.3 分层架构 (Layered Architecture)

| 层级 | 组件 | 职责 | 可见性 |
|------|------|------|--------|
| **Layer 1 (Core)** | `money.rs` | 原子类型定义与底层缩放 | `pub(crate)` |
| **Layer 2 (Domain)** | `SymbolManager` | 感知资产/交易对精度，提供意图 API | `pub` |
| **Layer 3 (Integration)** | `MoneyFormatter` | 高性能批量格式化（深度图、Ticker） | `pub` |

> [!TIP]
> **扩展性**: `MoneyFormatter` 目前服务于深度图。随着 Kline/Ticker 复杂化，此模式可推广至所有行情展示。

---

## Part III: 内外边界与显示策略 (Internal/External Boundary & Display)

### 3.0 核心规范：内部实现绝不暴露

> [!CAUTION]
> **内部的 `u64` 表示是实现细节，绝对不能暴露给客户端。**

**强制规范**：
1.  **统一转换层**：内部系统与外部 Client 之间，必须经过**统一的转换层**。
2.  **API 层使用 Decimal**：DTO 中的金额字段使用 `StrictDecimal`（自定义类型），利用 `rust_decimal` 的格式验证能力。
3.  **分层验证**：
    - **Serde 层**：格式验证（拒绝 `.5`、非数字等）→ 得到 `Decimal`
    - **SymbolManager 层**：精度/范围验证 → 得到 `ScaledAmount`
4.  **精度来源唯一**：资产精度从 `SymbolManager` 获取，严禁硬编码。

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────────┐
│   Client    │ ──→ │  Serde 层    │ ──→ │SymbolManager │ ──→ │  Internal   │
│  (String)   │     │ (Decimal)    │     │ (验证精度)   │     │   (u64)     │
└─────────────┘     └──────────────┘     └──────────────┘     └─────────────┘
     "1.5"       格式验证     Decimal(1.5)   精度验证    ScaledAmount(150_000_000)
```

**设计优势**：
- **利用库能力**：`rust_decimal` 提供成熟的数字解析
- **早期失败**：格式错误在反序列化阶段就拦截
- **关注点分离**：格式验证 vs 精度验证 分开处理
- **业务代码简化**：Handler 拿到的 `Decimal` 已是合法数字，只需验证范围

---

### 3.1 截断是唯一合法的舍入策略

**决策**：所有转换、计算过程中的精度损失，**一律使用截断（Truncation）**，不允许四舍五入。

**原因**：
- **一致性**：与整数除法的行为一致（向零截断）。
- **可预测性**：任何人在任何平台重算，结果完全一致。
- **安全性**：宁愿少显示，也不能让用户认为自己拥有实际不存在的余额。

| 场景 | 策略 | 示例 |
|------|------|------|
| 入金转换 | 截断 | 链上 `1.23456789012345678 ETH` → 系统 `1.23456789 ETH` |
| 余额显示 | 截断 | 内部 `123456789` → 显示 `"1.2345"` (4位显示精度) |
| 成交计算 | 截断 | 避免凭空产生资金 |

---

### 3.2 严格解析：拒绝模糊输入

**决策**: 拒绝 `.5` 和 `5.` 等简写，强制要求 `0.5` 和 `5.0`。

**原因**：处理金额数据，**严谨和安全是第一位的**。模糊的输入格式可能导致：
- 手抖或脚本错误输入不完整数字
- 不同解析器对歧义格式有不同解读
- 隐蔽的精度丢失

**行动项**:
- [ ] 在 OpenAPI 文档和错误信息中明确提示此规范

---

## Part IV: 如何在代码层面强制执行？

> **核心问题**：如何禁止开发者到处私自转换？

### 4.1 第一道防线：类型系统 (编译期)

**Newtype 封装**：`ScaledAmount(u64)` 的内部字段是 **private** 的。

```rust
pub struct ScaledAmount(u64);  // u64 不可直接访问

impl ScaledAmount {
    pub(crate) fn from_raw(v: u64) -> Self { Self(v) }  // 仅 crate 内部可构造
    pub fn to_raw(self) -> u64 { self.0 }                // 显式"逃逸"
}
```

**效果**：
- ❌ `ScaledAmount::from_raw(100)` — 外部模块无法调用
- ❌ `amount.0` — 无法直接访问内部字段
- ❌ `amount + 100u64` — 类型不匹配，编译失败
- ✅ `*amount > 0` — 通过 `Deref` 允许比较

---

### 4.2 第二道防线：可见性控制 (API 入口收缩)

**层级隔离**：

| 函数 | 可见性 | 谁可以调用 |
|------|--------|------------|
| `parse_amount()` | `pub(crate)` | 仅 `money.rs` 和核心模块 |
| `format_amount()` | `pub(crate)` | 仅 `money.rs` 和核心模块 |
| `SymbolManager::parse_qty()` | `pub` | **任何模块（唯一合法入口）** |
| `SymbolManager::format_price()` | `pub` | **任何模块（唯一合法入口）** |

**效果**：
- Gateway Handler 的代码自动补全中，**只能看到** `SymbolManager` 的方法。
- 如果开发者想调用底层 `parse_amount()`，会发现它**不在作用域内**。

---

### 4.3 第三道防线：API 层数据类型 (DTO 设计)

**强制规范**：API 请求/响应中的金额字段，**必须使用 `String` 类型**。

```rust
// ✅ 正确: 使用 String，由 Handler 调用 SymbolManager 转换
#[derive(Deserialize)]
pub struct PlaceOrderRequest {
    pub quantity: String,  // "1.5"
    pub price: String,     // "50000.00"
}

// ❌ 错误: 直接使用 u64，暴露内部实现
#[derive(Deserialize)]
pub struct PlaceOrderRequest {
    pub quantity: u64,     // 客户端如何知道要传 150_000_000？
}
```

**Serde 不会自动转换**：如果客户端传 `"quantity": 1.5`（JSON number），`String` 类型会反序列化失败，强制客户端传 `"1.5"`（JSON string）。

---

### 4.4 第四道防线：CI 自动化审计

**审计脚本**: `scripts/audit_money_safety.sh`

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

# 3. 检查硬编码精度 (可选，需要更精细的规则)
# grep -rn "decimals.*=.*8" --include="*.rs" src/ | grep -v "symbol_manager.rs"

echo "✅ Money safety audit passed!"
```

**集成**：
- `.github/workflows/ci.yml` — 每次 PR 自动运行
- `.git/hooks/pre-commit` — 本地提交前拦截

---

### 4.5 第五道防线：Code Review 信号

**高危操作清单** (PR 审查时重点关注)：

| 代码模式 | 风险等级 | 处理方式 |
|----------|----------|----------|
| `.to_raw()` | ⚠️ 高 | 必须注释说明原因 |
| `10u64.pow` 在 `money.rs` 外 | 🚫 禁止 | 拒绝合并 |
| `decimals: u32` 硬编码 | ⚠️ 高 | 应从 `SymbolManager` 获取 |
| API DTO 中 `u64` 金额字段 | 🚫 禁止 | 必须使用 `String` |
| `Deref` 后直接算术 (`*a + *b`) | ⚠️ 高 | 应使用 `checked_add` |

---

### 4.6 第六道防线：Agent 记忆 (AGENTS.md)

**已生效**: `AGENTS.md` 必读列表中包含本规范。所有 AI Agent 在开始工作前必须阅读，确保生成的代码符合规范。

---

## Part V: 未来升级路径 (Future Upgrade Path)

| 阶段 | 目标 | 状态 |
|------|------|------|
| **Phase 0** | Newtype 定义, API 收缩, 文档治理 | ✅ 已完成 |
| **Phase 1** | `audit_money_safety.sh` 集成 CI | ⏳ 待实现 |
| **Phase 1.5** | [API Money Enforcement](./api-money-enforcement.md)：Extractor + IntoResponse 强制转换 | ⏳ 待实现 |
| **Phase 2** | 存量代码全面扫描与迁移 | ⏳ 待执行 |
| **Phase 3** | `u64` → `u128` 升级 (支持 18 位高精度资产) | 📋 规划中 |

---

## 总结：为什么如此严苛 (Why So Heavy?)

### 核心原则 1：账本必须 100% 可对账

> 如果允许任何精度误差的存在，系统的账本就无法做到 **100% 对齐**。
> 我们无法利用"**资金恒等定理**"（总入金 = 总余额 + 总出金）来进行精确对账。
> 一旦账本不能 100% 对齐，我们就**无法分辨**一个差异是"可接受的正常误差"还是一个**隐藏的 Bug**。
> 真正的问题可能被"误差"掩盖，直到造成无法挽回的损失。

### 核心原则 2：转换逻辑必须收敛到唯一位置

> 金额转换逻辑非常复杂（精度、舍入、溢出检查）。
> 如果允许在代码库各处重复编写，**每个地方都可能犯不同的错误**。
> 将转换收敛到**唯一的、经过充分审计和测试的代码位置** (`money.rs` + `SymbolManager`)，我们可以：
> - 对这一处进行**穷尽式测试**（边界值、溢出、负数等）。
> - 确保**所有调用者**都享受同等的安全保障。
> - 在发现 Bug 时，**只需修复一处**，全局生效。

### 简单总结 (The Rules)

- **NO** `10u64.pow()` outside `money.rs`.
- **NO** raw `u64` arithmetic for amounts.
- **NO** implicit scaling.
- **YES** `SymbolManager` for all intent-based conversions.

---

## 速查表 (Quick Reference)

| 场景 | ✅ 正确做法 | ❌ 错误做法 |
|------|------------|------------|
| API DTO 字段 | `quantity: StrictDecimal` | `quantity: u64` 或 `quantity: String` |
| Decimal → ScaledAmount | `symbol_mgr.decimal_to_scaled(symbol, decimal)` | 手动计算 `decimal * 10^8` |
| ScaledAmount → String | `symbol_mgr.format_price(symbol, amount)` | `format!("{}", amount)` |
| 获取精度 | `symbol_mgr.get_decimals(asset)` | `let decimals = 8;` |
| 算术运算 | `amount.checked_add(other)?` | `*amount + *other` |
| 比较运算 | `*amount > 0` | ✅ 允许 (Deref) |
