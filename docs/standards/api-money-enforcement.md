# API Money Enforcement | API 层资金类型强制规范

> **目标**：确保所有 API Handler 都通过统一的转换层处理金额数据，禁止各处私自转换。
>
> **适用范围**：Request（入）和 Response（出）双向。

---

## 1. 问题陈述

Gateway 有多个 API Handler，每个都需要：
- **入向**：接收 JSON 中的金额字符串（如 `"1.5"`），转换为内部 `ScaledAmount`
- **出向**：将内部 `ScaledAmount` 格式化为 JSON 字符串返回给客户端

**核心挑战**：如何确保**所有** Handler 都通过 `SymbolManager` 转换，而不是各自写一套转换逻辑？

---

## 2. 方案对比

### 方案 A：DTO + 显式验证层

**机制**：Handler 接收原始 DTO，手动调用验证函数。

```rust
// Request
async fn place_order(Json(req): Json<PlaceOrderRequest>) -> Result<...> {
    // 每个 Handler 都要记得调用 validate()
    let validated = symbol_mgr.validate_order(&req)?;
    // ...
}

// Response
async fn get_balance(...) -> Json<BalanceResponse> {
    let raw = service.get_balance(...)?;
    // 每个 Handler 都要记得调用 format()
    Json(symbol_mgr.format_balance_response(&raw))
}
```

| 优点 | 缺点 |
|------|------|
| 简单直接 | **依赖开发者自觉**，容易遗漏 |
| 无需额外类型 | 转换逻辑分散在各 Handler |

---

### 方案 B：Service 层封装

**机制**：Handler 只能调用 Service 方法，Service 内部做转换。

```rust
// Handler 只传递原始 DTO
async fn place_order(Json(req): Json<PlaceOrderRequest>) -> Result<...> {
    order_service.place(req).await  // Service 内部调用 SymbolManager
}

async fn get_balance(...) -> Result<Json<BalanceResponse>> {
    Ok(Json(balance_service.get_formatted(...).await?))  // Service 返回已格式化数据
}
```

| 优点 | 缺点 |
|------|------|
| 业务逻辑集中 | Service 仍需记得调用 `SymbolManager` |
| Handler 简洁 | 如果 Service 遗漏，问题仍会发生 |

---

### 方案 C：Axum Extractor + IntoResponse 模式 ⭐ 推荐

**机制**：在 Axum 框架层强制转换。

#### Request 端：自定义 Extractor

```rust
/// 已验证的订单请求，Handler 直接拿到 ScaledAmount
pub struct ValidatedOrder {
    pub symbol_id: SymbolId,
    pub quantity: ScaledAmount,
    pub price: ScaledAmount,
}

#[async_trait]
impl<S> FromRequest<S> for ValidatedOrder
where
    S: Send + Sync,
{
    type Rejection = ApiError;
    
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(raw): Json<RawOrderRequest> = Json::from_request(req, state).await?;
        let symbol_mgr = state.symbol_manager();
        
        Ok(ValidatedOrder {
            symbol_id: raw.symbol_id,
            quantity: symbol_mgr.parse_qty(raw.symbol_id, &raw.quantity)?,
            price: symbol_mgr.parse_price(raw.symbol_id, &raw.price)?,
        })
    }
}

// Handler 直接拿到已验证的类型，无法绕过
async fn place_order(order: ValidatedOrder) -> Result<impl IntoResponse> {
    // order.quantity 已经是 ScaledAmount，不可能是未转换的 String
}
```

#### Response 端：自定义 IntoResponse

```rust
/// 已格式化的余额响应，自动调用 SymbolManager 格式化
pub struct FormattedBalanceResponse {
    pub balances: Vec<(AssetId, ScaledAmount)>,
    pub symbol_mgr: Arc<SymbolManager>,
}

impl IntoResponse for FormattedBalanceResponse {
    fn into_response(self) -> Response {
        let formatted: Vec<BalanceDto> = self.balances.iter()
            .map(|(asset, amount)| BalanceDto {
                asset: asset.to_string(),
                amount: self.symbol_mgr.format_asset_amount(*asset, *amount),
            })
            .collect();
        Json(formatted).into_response()
    }
}

// Handler 返回内部类型，格式化在 IntoResponse 中自动完成
async fn get_balances(State(state): State<AppState>) -> FormattedBalanceResponse {
    let balances = state.service.get_balances().await;
    FormattedBalanceResponse { balances, symbol_mgr: state.symbol_mgr.clone() }
}
```

| 优点 | 缺点 |
|------|------|
| **框架层强制**，Handler 拿不到原始 String | 需要为每类请求定义 Extractor |
| 编译期保证 | 需要在 Extractor 中获取 `SymbolManager` |
| 转换逻辑完全集中 | 初期实现成本略高 |

---

### 方案 D：类型驱动设计（最严格）

**机制**：定义"未验证"的金额类型，只能通过 SymbolManager 转换。

```rust
/// 未验证的金额，不能直接使用
pub struct UnvalidatedAmount(String);

impl UnvalidatedAmount {
    // 没有 .parse() 方法
    // 没有 Deref<Target=String>
    // 唯一的出路是传给 SymbolManager
}

impl SymbolManager {
    pub fn parse(&self, asset: AssetId, amount: UnvalidatedAmount) -> Result<ScaledAmount>;
}

// DTO 使用未验证类型
#[derive(Deserialize)]
pub struct PlaceOrderRequest {
    pub quantity: UnvalidatedAmount,  // 无法直接 .parse()
}
```

| 优点 | 缺点 |
|------|------|
| 类型系统完全封锁 | 引入更多类型 |
| 即使忘记调用也无法编译 | Serde 自定义反序列化略复杂 |

---

## 3. 推荐方案：StrictDecimal + Extractor

### 3.1 核心设计：分层验证

```
Client (JSON String "1.5")
    ↓ Serde: StrictDecimal 自定义反序列化
API DTO (StrictDecimal) ← 格式已验证
    ↓ Extractor: SymbolManager.decimal_to_scaled()
Handler (ScaledAmount) ← 精度已验证
```

**关键洞察**：
- **Serde 层负责格式验证**：利用 `rust_decimal` 的解析能力，拒绝非法格式
- **SymbolManager 负责精度验证**：检查小数位是否符合资产精度
- **业务代码只需验证范围**：数字格式和精度都已保证

---

### 3.2 StrictDecimal 实现

```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

/// 严格格式的 Decimal，在反序列化时进行格式验证
#[derive(Debug, Clone, Copy)]
pub struct StrictDecimal(Decimal);

impl StrictDecimal {
    pub fn inner(&self) -> Decimal {
        self.0
    }
}

impl<'de> Deserialize<'de> for StrictDecimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        
        // 严格格式检查：拒绝 .5, 5., 空字符串等
        if s.is_empty() {
            return Err(serde::de::Error::custom("Amount cannot be empty"));
        }
        if s.starts_with('.') {
            return Err(serde::de::Error::custom("Invalid format: use 0.5 not .5"));
        }
        if s.ends_with('.') {
            return Err(serde::de::Error::custom("Invalid format: use 5.0 not 5."));
        }
        
        // 使用 Decimal 库解析
        let d = Decimal::from_str(&s)
            .map_err(|e| serde::de::Error::custom(format!("Invalid decimal: {}", e)))?;
        
        // 拒绝负数（金额必须非负）
        if d.is_sign_negative() {
            return Err(serde::de::Error::custom("Amount cannot be negative"));
        }
        
        Ok(StrictDecimal(d))
    }
}
```

---

### 3.3 DTO 使用示例

```rust
#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub symbol: String,
    pub quantity: StrictDecimal,  // 格式已验证
    pub price: StrictDecimal,     // 格式已验证
}
```

---

### 3.4 SymbolManager 扩展

```rust
impl SymbolManager {
    /// 将已验证的 Decimal 转换为 ScaledAmount
    /// 只需验证精度，格式已在 Serde 层验证
    pub fn decimal_to_scaled(
        &self,
        symbol: SymbolId,
        decimal: Decimal,
    ) -> Result<ScaledAmount, MoneyError> {
        let decimals = self.get_symbol_decimals(symbol)?;
        
        // 检查精度是否超限
        if decimal.scale() > decimals {
            return Err(MoneyError::PrecisionExceeded {
                provided: decimal.scale(),
                max: decimals,
            });
        }
        
        // 转换为 u64
        let scaled = decimal * Decimal::from(10u64.pow(decimals));
        let raw = scaled.to_u64()
            .ok_or(MoneyError::Overflow)?;
        
        Ok(ScaledAmount::from_raw(raw))
    }
}
```

---

### 3.5 Extractor 整合

```rust
pub struct ValidatedOrder {
    pub symbol_id: SymbolId,
    pub quantity: ScaledAmount,
    pub price: ScaledAmount,
}

#[async_trait]
impl<S> FromRequest<S> for ValidatedOrder
where
    S: Send + Sync,
{
    type Rejection = ApiError;
    
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(raw): Json<PlaceOrderRequest> = Json::from_request(req, state).await?;
        let symbol_mgr = state.symbol_manager();
        let symbol_id = symbol_mgr.get_symbol_id(&raw.symbol)?;
        
        Ok(ValidatedOrder {
            symbol_id,
            // StrictDecimal 已验证格式，这里只验证精度
            quantity: symbol_mgr.decimal_to_scaled(symbol_id, raw.quantity.inner())?,
            price: symbol_mgr.decimal_to_scaled(symbol_id, raw.price.inner())?,
        })
    }
}
```

---

### 3.6 设计优势总结

| 层级 | 职责 | 验证内容 |
|------|------|----------|
| **Serde (StrictDecimal)** | 格式验证 | 拒绝 `.5`, `5.`, 负数, 非数字 |
| **SymbolManager** | 精度验证 | 检查小数位是否超限 |
| **业务代码** | 范围验证 | 检查金额是否在合理范围 |

**关键收益**：
1. **利用库能力**：`rust_decimal` 提供成熟的数字解析
2. **早期失败**：格式错误在反序列化阶段就拦截
3. **关注点分离**：每层只负责一种验证
4. **编译期保证**：Handler 拿到的是 `ScaledAmount`，无法出错

---

## 4. CI 自动化检查：机制强制，不靠自觉

> **核心原则**：我们要从**机制和流程**上规范，而不是依赖开发者的"自觉"。

### 4.1 审计脚本：`scripts/audit_api_types.sh`

```bash
#!/bin/bash
set -e

echo "🔍 Auditing API type safety..."

# 1. 检查 DTO 中是否存在 u64/i64 金额字段
# 金额字段名通常包含: amount, quantity, price, balance, volume
AMOUNT_PATTERNS="amount|quantity|price|balance|volume|size|qty"

if grep -rn "pub\s\+\(${AMOUNT_PATTERNS}\)\s*:\s*u64" --include="*.rs" src/gateway/; then
    echo "❌ FAIL: Found u64 amount field in API DTO"
    echo "   → Should use String type instead"
    exit 1
fi

if grep -rn "pub\s\+\(${AMOUNT_PATTERNS}\)\s*:\s*i64" --include="*.rs" src/gateway/; then
    echo "❌ FAIL: Found i64 amount field in API DTO"
    echo "   → Should use String type instead"
    exit 1
fi

# 2. 检查 Handler 中是否直接 parse 金额
if grep -rn "\.parse::<u64>\(\)" --include="*.rs" src/gateway/; then
    echo "❌ FAIL: Found direct u64 parsing in gateway"
    echo "   → Should use SymbolManager.parse_qty() instead"
    exit 1
fi

# 3. 检查是否直接使用 format!() 格式化金额
if grep -rn 'format!\s*(\s*"{}"\s*,\s*\w*amount' --include="*.rs" src/gateway/; then
    echo "⚠️ WARNING: Possible direct amount formatting found"
    echo "   → Consider using SymbolManager.format_*() instead"
fi

# 4. 检查 Decimal 是否绕过 SymbolManager
if grep -rn "Decimal::from_str" --include="*.rs" src/gateway/ | grep -v "// safe:"; then
    echo "⚠️ WARNING: Direct Decimal parsing found in gateway"
    echo "   → Should use SymbolManager for conversions"
fi

echo "✅ API type safety audit passed!"
```

---

### 4.2 检查规则详解

| 检查项 | 目标 | 检测模式 |
|--------|------|----------|
| **DTO 字段类型** | 金额字段必须是 `String` | `pub (amount|qty|..): u64` |
| **直接解析** | 禁止在 Handler 中 `.parse::<u64>()` | `.parse::<u64>()` in `src/gateway/` |
| **直接格式化** | 禁止 `format!("{}", amount)` | `format!(...amount...)` in `src/gateway/` |
| **绕过转换层** | 禁止直接使用 `Decimal::from_str` | `Decimal::from_str` in `src/gateway/` |

---

### 4.3 CI 集成

**GitHub Actions 配置**：

```yaml
# .github/workflows/ci.yml
- name: Audit API Type Safety
  run: |
    chmod +x scripts/audit_api_types.sh
    ./scripts/audit_api_types.sh
```

**本地 Pre-commit Hook**：

```bash
# .git/hooks/pre-commit
#!/bin/bash
./scripts/audit_api_types.sh || exit 1
```

---

### 4.4 豁免机制

对于确实需要绕过检查的特殊场景（如测试代码、内部工具），可以使用注释标记：

```rust
// safe: 这是测试代码，允许直接解析
let amount = "100".parse::<u64>().unwrap();
```

审计脚本应排除带有 `// safe:` 注释的行。

---

## 4.5 双向类型封锁：金融系统最佳实践 ⭐

> **核心原则**：金融系统的 API 边界是安全的最后一道防线。
> 任何金额数据跨越这条边界时，必须经过**强制类型转换**，不允许任何"逃逸"。

### 4.5.1 架构概览

```
                    ┌────────────────────────────────────────────┐
                    │              API Boundary                   │
                    │  (所有金额必须经过类型转换，无例外)          │
                    └────────────────────────────────────────────┘
                                        │
           ┌────────────────────────────┼────────────────────────────┐
           │                            │                            │
    ┌──────▼──────┐              ┌──────▼──────┐              ┌──────▼──────┐
    │   INPUT     │              │   OUTPUT    │              │  INTERNAL   │
    │ StrictDecimal│              │DisplayAmount│              │ ScaledAmount│
    │ (Deserialize)│              │ (Serialize) │              │   (u64)     │
    └──────┬──────┘              └──────▲──────┘              └─────────────┘
           │                            │
           │    SymbolManager           │    SymbolManager
           │    .parse_qty()            │    .format_amount()
           │                            │
           └────────────────────────────┘
```

### 4.5.2 三层类型系统

#### Layer 1: API Input Types (反序列化)

```rust
/// 严格输入金额 - 只能通过 Serde 反序列化创建
/// 
/// 职责：格式验证
/// - 拒绝 .5 (应为 0.5)
/// - 拒绝 5. (应为 5.0)
/// - 拒绝负数
/// - 拒绝空字符串
#[derive(Debug, Clone, Copy)]
pub struct StrictDecimal(Decimal);
// ✅ 已实现
```

#### Layer 2: API Output Types (序列化)

```rust
/// 严格输出金额 - 只能通过 SymbolManager 创建
/// 
/// 设计原则：
/// 1. 没有公开构造函数
/// 2. 只能通过 SymbolManager.format_*() 创建
/// 3. 序列化始终为 String (保证精度)
#[derive(Debug, Clone)]
pub struct DisplayAmount(String);

impl DisplayAmount {
    /// 私有构造 - 只有 SymbolManager 可以调用
    pub(crate) fn new(s: String) -> Self {
        Self(s)
    }
}

impl Serialize for DisplayAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}
// ⏳ 待实现
```

#### Layer 3: Internal Types (计算/存储)

```rust
/// 内部缩放金额 - 用于所有计算和存储
/// 
/// 设计原则：
/// 1. 不实现 Serialize/Deserialize
/// 2. 不能直接出现在 DTO 中
/// 3. 所有算术都是精确的整数运算
#[derive(Debug, Clone, Copy)]
pub struct ScaledAmount(u64);
// ✅ 已实现
```

### 4.5.3 SymbolManager 双向转换

```rust
impl SymbolManager {
    // ========== INPUT: Client → Internal ==========
    
    /// 解析数量 (StrictDecimal → ScaledAmount)
    pub fn parse_qty(&self, symbol_id: u32, input: StrictDecimal) 
        -> Result<ScaledAmount, MoneyError>;
    
    /// 解析价格 (StrictDecimal → u64)
    pub fn parse_price(&self, symbol_id: u32, input: StrictDecimal) 
        -> Result<u64, MoneyError>;

    // ========== OUTPUT: Internal → Client ==========
    
    /// 格式化数量 (ScaledAmount → DisplayAmount)
    pub fn format_qty(&self, symbol_id: u32, amount: ScaledAmount) -> DisplayAmount {
        let symbol = self.get_symbol_info_by_id(symbol_id).expect("Symbol not found");
        let formatted = money::format_amount(*amount, symbol.base_decimals, symbol.qty_display);
        DisplayAmount::new(formatted)
    }
    
    /// 格式化价格 (u64 → DisplayAmount)
    pub fn format_price(&self, symbol_id: u32, price: u64) -> DisplayAmount {
        let symbol = self.get_symbol_info_by_id(symbol_id).expect("Symbol not found");
        let formatted = money::format_amount(price, symbol.price_decimal, symbol.price_display);
        DisplayAmount::new(formatted)
    }
    
    /// 格式化资产金额 (ScaledAmount → DisplayAmount)
    pub fn format_asset_amount(&self, asset_id: u32, amount: ScaledAmount) -> DisplayAmount {
        let asset = self.assets.get(&asset_id).expect("Asset not found");
        let formatted = money::format_amount_full(*amount, asset.decimals);
        DisplayAmount::new(formatted)
    }
}
```

### 4.5.4 Response DTO 设计规范

```rust
/// ✅ 正确：所有金额字段使用 DisplayAmount
#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub asset: String,
    pub free: DisplayAmount,       // ✅ 强制类型
    pub locked: DisplayAmount,     // ✅ 强制类型
}

/// ❌ 错误：暴露内部表示或使用不安全类型
#[derive(Debug, Serialize)]
pub struct BadBalanceResponse {
    pub asset: String,
    pub free: u64,                 // ❌ 暴露内部表示
    pub locked: f64,               // ❌ 精度问题
    pub pending: Decimal,          // ❌ 可能格式不一致
}
```

### 4.5.5 CI 审计规则扩展

```bash
# Rule 4: Response DTO 中禁止使用裸 Decimal/f64/u64 (金额字段)
echo "Rule 4: Checking Response DTO types..."

# 金额字段模式
AMOUNT_FIELDS="free|locked|available|balance|amount|qty|quantity|price|volume|fee"

# 检查 f64 (金融系统绝对禁止)
if grep -rn "pub\s\+\(${AMOUNT_FIELDS}\)\s*:\s*f64" --include="*.rs" src/gateway/; then
    echo "❌ FAIL: Found f64 amount field (forbidden in financial systems)"
    VIOLATIONS=$((VIOLATIONS + 1))
fi

# 检查裸 Decimal (应使用 DisplayAmount)
if grep -rn "pub\s\+\(${AMOUNT_FIELDS}\)\s*:\s*Decimal\s*[,}]" --include="*.rs" src/gateway/ \
    | grep -v "StrictDecimal" | grep -v "DisplayAmount"; then
    echo "⚠️ WARNING: Found raw Decimal in Response DTO"
    echo "   → Consider using DisplayAmount for responses"
fi
```

### 4.5.6 类型流转总结

| 方向 | 类型流转 | 转换函数 |
|------|----------|----------|
| **Input** | `JSON "1.5"` → `StrictDecimal` → `ScaledAmount(u64)` | `SymbolManager.parse_*()` |
| **Output** | `ScaledAmount(u64)` → `DisplayAmount` → `JSON "1.5"` | `SymbolManager.format_*()` |
| **禁止** | `ScaledAmount` 直接序列化 | ❌ 编译失败 |
| **禁止** | `f64` 在任何 DTO 中 | ❌ CI 审计失败 |

### 4.5.7 为什么如此严格？

> **金融系统的零容忍原则**:
> 
> 1. **精度可控性**: 
>    - 内部存储使用最高精度（如 BTC 10^-8）
>    - UI 显示使用 `display_decimals` 截断（如仅显示 4 位小数 0.0001）
>    - 截断是**显式且可控**的，由 `SymbolManager.format_*()` 统一处理
>    - 客户端永远不会看到超过 `display_decimals` 的小数位
> 
> 2. **可审计性**: 任何金额转换都有明确的类型边界，便于追踪
> 
> 3. **防御深度**: 即使开发者忘记验证，类型系统也会阻止不安全操作
> 
> 4. **合规要求**: 金融监管通常要求明确的数据转换审计点

#### 精度层次说明

| 精度类型 | 用途 | 示例 (BTC) |
|----------|------|------------|
| **链上精度** | 区块链原生精度 | 8 位 (satoshi) |
| **系统精度** | 内部存储/计算 | 8 位 (系统配置) |
| **显示精度** (`display_decimals`) | UI 展示 | 4 位 (0.0001) |

> [!IMPORTANT]
> **截断 vs 四舍五入**：显示时始终使用**截断**（向下取整），永远不会显示用户实际不拥有的金额。
> 例如：用户余额 `0.00015678 BTC`，显示为 `0.0001 BTC`（截断后 4 位）。

---

## 5. 实施路线图

| 阶段 | 任务 | 状态 |
|------|------|------|
| **Phase 1a** | 实现 `StrictDecimal` 类型 (Serde 层格式验证) | ✅ 已完成 |
| **Phase 1b** | 为核心订单 API 实现 `ValidatedOrder` Extractor | ⏳ 待实现 |
| **Phase 2a** | 实现 `DisplayAmount` 类型 (Response 输出封装) | ✅ 已完成 |
| **Phase 2b** | 迁移 Response DTO 使用 `DisplayAmount` | ✅ 已完成 |
| **Phase 3** | 为所有金额相关 API 统一改造 | ⏳ 待实现 |
| **Phase 4** | 实现 `audit_api_types.sh` 并集成 CI | ✅ 已完成 |
| **Phase 5** | 扩展审计脚本检查 Response DTO 类型 | ✅ 已完成 |
| **Phase 6** | CI 集成审计脚本 | ✅ 已完成 |

---

## 6. 实施记录 (2025-12-31)

### 已完成

#### Phase 1a: StrictDecimal 类型

在 `src/gateway/types.rs` 添加了 `StrictDecimal` 类型：

```rust
/// 严格格式的 Decimal，在反序列化时进行格式验证
/// - 拒绝 .5 (应为 0.5)
/// - 拒绝 5. (应为 5.0)
/// - 拒绝负数
/// - 拒绝空字符串
pub struct StrictDecimal(Decimal);
```

**已更新的 DTO:**
- `ClientOrder.price` → `Option<StrictDecimal>`
- `ClientOrder.qty` → `StrictDecimal`
- `ReduceOrderRequest.reduce_qty` → `StrictDecimal`
- `MoveOrderRequest.new_price` → `StrictDecimal`

#### Phase 4: 审计脚本

创建 `scripts/audit_api_types.sh`：
- 检测 u64/i64 金额字段
- 检测直接 `.parse::<u64>()` 调用
- 检测绕过 StrictDecimal 的 `Decimal::from_str`

#### Phase 2a: DisplayAmount 类型 (2025-12-31)

在 `src/gateway/types.rs` 添加了 `DisplayAmount` 类型：

```rust
/// 严格输出金额 - 只能通过 SymbolManager 创建
/// - 没有公开构造函数 (pub(crate))
/// - 始终序列化为 JSON String
/// - 通过 SymbolManager.display_*() 创建
pub struct DisplayAmount(String);
```

**SymbolManager 新增方法:**
- `display_qty()` — 格式化数量
- `display_price()` — 格式化价格
- `display_price_u64()` — 格式化 u64 价格
- `display_asset_amount()` — 格式化资产余额

#### Phase 5: 扩展审计脚本 (2025-12-31)

扩展 `scripts/audit_api_types.sh` 添加新规则：

- **Rule 4**: 检测 `f64` 字段 (金融系统禁止)
- **Rule 5**: 检测 Response DTO 中的裸 `Decimal` (信息性警告)

### 验证

```bash
# 完整测试套件
cargo test gateway::types  # 28 通过

# 审计脚本 (5 条规则)
./scripts/audit_api_types.sh  # ✅ PASSED

# 全量测试
cargo test  # 390+ 通过
```

#### Phase 2b: BalanceInfo 迁移 (2025-12-31)

迁移 `BalanceInfo` 使用 `DisplayAmount` 类型：

**修改的文件:**
- `src/funding/service.rs` — `BalanceInfo.available/frozen` 从 `String` 改为 `DisplayAmount`
- `src/gateway/handlers.rs` — 更新 `BalanceInfo` 构造点使用 `DisplayAmount::new()`

**验证:**
```bash
cargo build                    # ✅ PASSED
cargo test gateway::types      # ✅ 28 passed
./scripts/audit_api_types.sh   # ✅ 5 rules PASSED
cargo test                     # ✅ 390+ passed
```

---

## 7. 参考

- [Money Type Safety Standard](./money-type-safety.md) — 资金类型安全规范
- [0x02 浮点数的诅咒](../src/0x02-the-curse-of-float.md) — 浮点数问题详解

