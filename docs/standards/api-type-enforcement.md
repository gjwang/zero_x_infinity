# API Type Enforcement | API 层类型强制执行方案

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

## 3. 推荐方案

对于我们的场景，**方案 C (Extractor + IntoResponse)** 是最实用的：

1. **框架层拦截**：Handler 无法绕过
2. **编译期保证**：如果用错签名，编译失败
3. **双向覆盖**：Request 和 Response 都强制通过统一层
4. **集中维护**：所有转换逻辑在 Extractor/IntoResponse 中

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

## 5. 实施路线图

| 阶段 | 任务 | 状态 |
|------|------|------|
| **Phase 1** | 为核心订单 API 实现 `ValidatedOrder` Extractor | ⏳ 待实现 |
| **Phase 2** | 为余额/资产 API 实现 `FormattedBalanceResponse` | ⏳ 待实现 |
| **Phase 3** | 为所有金额相关 API 统一改造 | ⏳ 待实现 |
| **Phase 4** | 实现 `audit_api_types.sh` 并集成 CI | ⏳ 待实现 |
| **Phase 5** | 添加 pre-commit hook 本地拦截 | 📋 规划中 |

---

## 6. 参考

- [Money Type Safety Standard](./money-type-safety.md) — 资金类型安全规范
- [0x02 浮点数的诅咒](../src/0x02-the-curse-of-float.md) — 浮点数问题详解
