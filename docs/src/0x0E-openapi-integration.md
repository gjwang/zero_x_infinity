# 0x0E OpenAPI Integration

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.0D-persistence...v0.0E-openapi)

---

## 1. Overview

### 1.1 Connecting the Dots: From Crash Recovery to Developer Experience

In **0x0D**, we built the WAL & Snapshot persistence layer, ensuring the exchange can recover from crashes without losing a single order. Now our core trading engine is **resilient**.

But resilience alone doesn't make a usable product. 

Consider this scenario: A frontend developer wants to integrate with our API. They ask:
- *"What endpoints are available?"*
- *"What's the request/response format?"*
- *"How do I authenticate?"*

Without documentation, they have to read Rust source code. That's not acceptable.

This is the topic of this chapter: **OpenAPI Integration**.

> **Design Philosophy**: Good documentation is not a luxury—it's infrastructure. A well-documented API:
> - Reduces support burden (developers can self-serve)
> - Enables SDK auto-generation (Python, TypeScript, etc.)
> - Improves security (clear auth instructions reduce mistakes)
> - Accelerates frontend development (no guessing)

### 1.2 Goal

Integrate **OpenAPI 3.0** documentation that:
1. Auto-generates from Rust code (single source of truth)
2. Serves interactive docs at `/docs` (Swagger UI)
3. Enables client SDK generation

### 1.3 Key Concepts

| Term | Definition |
|------|------------|
| **OpenAPI** | Industry-standard API specification format (formerly Swagger) |
| **utoipa** | Rust crate for compile-time OpenAPI generation |
| **Swagger UI** | Interactive API documentation interface |
| **Code-First** | Generate spec from code, not YAML files |

### 1.4 Architecture Overview

```
┌─────────── OpenAPI Integration Flow ────────────┐
│                                                  │
│  Rust Handlers ──▶ #[utoipa::path] ──▶ OpenAPI   │
│       │                                   │      │
│       │                                   ▼      │
│       │                            Swagger UI    │
│       │                            (/docs)       │
│       │                                   │      │
│       ▼                                   ▼      │
│  Type-Safe API ◀─────────────────▶ openapi.json │
│                                          │      │
│                                          ▼      │
│                                    SDK Clients  │
│                                  (Python, TS)   │
└─────────────────────────────────────────────────┘
```

---

## 2. Implementation

### 2.1 Adding Dependencies

**Cargo.toml**:
```diff
[dependencies]
+ utoipa = { version = "5.3", features = ["axum_extras", "chrono", "uuid"] }
+ utoipa-swagger-ui = { version = "8.0", features = ["axum"] }
```

### 2.2 Creating OpenAPI Module

Create `src/gateway/openapi.rs`:

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zero X Infinity Exchange API",
        version = "1.0.0",
        description = "High-performance crypto exchange API (1.3M orders/sec)"
    ),
    paths(
        handlers::health_check,
        handlers::get_depth,
        handlers::get_klines,
        // ... all API handlers
    ),
    components(schemas(
        types::ApiResponse<()>,
        types::DepthApiData,
        // ... all response types
    ))
)]
pub struct ApiDoc;
```

### 2.3 Annotating Handlers

Add `#[utoipa::path]` to each handler:

```diff
+ #[utoipa::path(
+     get,
+     path = "/api/v1/public/depth",
+     params(
+         ("symbol" = String, Query, description = "Trading pair"),
+         ("limit" = Option<u32>, Query, description = "Depth levels")
+     ),
+     responses(
+         (status = 200, description = "Order book depth", body = ApiResponse<DepthApiData>)
+     ),
+     tag = "Market Data"
+ )]
  pub async fn get_depth(
      State(state): State<Arc<AppState>>,
      Query(params): Query<HashMap<String, String>>,
  ) -> impl IntoResponse {
      // ... existing implementation ...
  }
```

### 2.4 Adding Schema Derivations

Add `ToSchema` to response types:

```diff
+ use utoipa::ToSchema;

- #[derive(Serialize, Deserialize)]
+ #[derive(Serialize, Deserialize, ToSchema)]
  pub struct DepthApiData {
+     #[schema(example = "BTC_USDT")]
      pub symbol: String,
+     #[schema(example = json!([["85000.00", "0.5"]]))]
      pub bids: Vec<[String; 2]>,
+     #[schema(example = json!([["85001.00", "0.3"]]))]
      pub asks: Vec<[String; 2]>,
  }
```

### 2.5 Integrating Swagger UI

In `src/gateway/mod.rs`:

```diff
+ use utoipa_swagger_ui::SwaggerUi;
+ use crate::gateway::openapi::ApiDoc;

  let app = Router::new()
      .route("/api/v1/health", get(handlers::health_check))
      .nest("/api/v1/public", public_routes)
      .nest("/api/v1/private", private_routes)
+     .merge(
+         SwaggerUi::new("/docs")
+             .url("/api-docs/openapi.json", ApiDoc::openapi())
+     )
      .with_state(state);
```

---

## 3. API Endpoints

### 3.1 Public Endpoints (No Auth)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/health` | GET | Health check |
| `/api/v1/public/depth` | GET | Order book depth |
| `/api/v1/public/klines` | GET | K-line data |
| `/api/v1/public/assets` | GET | Asset list |
| `/api/v1/public/symbols` | GET | Trading pairs |
| `/api/v1/public/exchange_info` | GET | Exchange metadata |

### 3.2 Private Endpoints (Ed25519 Auth)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/private/order` | POST | Create order |
| `/api/v1/private/cancel` | POST | Cancel order |
| `/api/v1/private/orders` | GET | Query orders |
| `/api/v1/private/trades` | GET | Trade history |
| `/api/v1/private/balances` | GET | Balance query |
| `/api/v1/private/balances/all` | GET | All balances |
| `/api/v1/private/transfer` | POST | Internal transfer |
| `/api/v1/private/transfer/{id}` | GET | Transfer status |

---

## 4. SDK Generation

### 4.1 Python SDK

Auto-generated Python client with Ed25519 signing:

```python
from zero_x_infinity_sdk import ZeroXInfinityClient

client = ZeroXInfinityClient(
    api_key="your_api_key",
    secret_key_bytes=secret_key  # Ed25519 private key
)

# Create order
order = client.create_order(
    symbol="BTC_USDT",
    side="BUY",
    price="85000.00",
    qty="0.001"
)
```

### 4.2 TypeScript SDK

```typescript
import { ZeroXInfinityClient } from './zero_x_infinity_sdk';

const client = new ZeroXInfinityClient(apiKey, secretKey);
const depth = await client.getDepth('BTC_USDT');
```

---

## 5. Verification

### 5.1 Access Swagger UI

```bash
cargo run --release -- --gateway --port 8080
# Open: http://localhost:8080/docs
```

### 5.2 Test Results

| Test Category | Tests | Result |
|---------------|-------|--------|
| Unit Tests | 293 | ✅ All pass |
| Public Endpoints | 6 | ✅ All pass |
| Private Endpoints | 9 | ✅ All pass |
| E2E Total | 17 | ✅ All pass |

---

## 6. Summary

In this chapter, we added OpenAPI documentation to our trading engine:

| Achievement | Result |
|-------------|--------|
| **Swagger UI** | Available at `/docs` |
| **OpenAPI Spec** | 15 endpoints documented |
| **Python SDK** | Auto-generated with Ed25519 |
| **TypeScript SDK** | Type-safe client |
| **Zero Breaking Changes** | All existing tests pass |

**Next Chapter**: With resilience (0x0D) and documentation (0x0E) complete, the foundation is solid. The next logical step is **0x0F: Deposit & Withdraw**—connecting to blockchain for real crypto funding.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.0D-persistence...v0.0E-openapi)

---

## 1. 概述

### 1.1 承前启后：从崩溃恢复到开发者体验

在 **0x0D** 章节中，我们构建了 WAL 和快照持久化层，确保交易所能够在崩溃后恢复，不丢失任何订单。现在我们的核心交易引擎已经具备了**鲁棒性**。

但仅有鲁棒性不足以成为可用的产品。

考虑这个场景：一个前端开发者想要集成我们的 API。他们会问：
- *"有哪些可用的端点？"*
- *"请求/响应格式是什么？"*
- *"如何进行身份验证？"*

如果没有文档，他们就得去读 Rust 源代码。这是不可接受的。

这就是本章的主题：**OpenAPI 集成**。

> **设计理念**：好的文档不是奢侈品——它是基础设施。一个文档完善的 API：
> - 减少支持负担（开发者可以自助）
> - 支持 SDK 自动生成（Python、TypeScript 等）
> - 提升安全性（清晰的认证说明减少错误）
> - 加速前端开发（无需猜测）

### 1.2 目标

集成 **OpenAPI 3.0** 文档：
1. 从 Rust 代码自动生成（单一事实来源）
2. 在 `/docs` 提供交互式文档（Swagger UI）
3. 支持客户端 SDK 生成

### 1.3 核心概念

| 术语 | 定义 |
|------|------|
| **OpenAPI** | 行业标准的 API 规范格式（前身是 Swagger） |
| **utoipa** | Rust 编译时 OpenAPI 生成库 |
| **Swagger UI** | 交互式 API 文档界面 |
| **代码优先** | 从代码生成规范，而非 YAML 文件 |

### 1.4 架构总览

```
┌─────────── OpenAPI 集成流程 ────────────┐
│                                          │
│  Rust Handlers ──▶ #[utoipa::path] ──▶ OpenAPI
│       │                                   │
│       │                                   ▼
│       │                            Swagger UI
│       │                            (/docs)
│       ▼                                   │
│  类型安全 API ◀────────────────▶ openapi.json
│                                          │
│                                          ▼
│                                    SDK 客户端
│                                  (Python, TS)
└──────────────────────────────────────────┘
```

---

## 2. 实现

### 2.1 添加依赖

**Cargo.toml**:
```diff
[dependencies]
+ utoipa = { version = "5.3", features = ["axum_extras", "chrono", "uuid"] }
+ utoipa-swagger-ui = { version = "8.0", features = ["axum"] }
```

### 2.2 创建 OpenAPI 模块

创建 `src/gateway/openapi.rs`：

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zero X Infinity Exchange API",
        version = "1.0.0",
        description = "高性能加密货币交易所 API (1.3M 订单/秒)"
    ),
    paths(
        handlers::health_check,
        handlers::get_depth,
        handlers::get_klines,
        // ... 所有 API handlers
    ),
    components(schemas(
        types::ApiResponse<()>,
        types::DepthApiData,
        // ... 所有响应类型
    ))
)]
pub struct ApiDoc;
```

### 2.3 注解 Handlers

为每个 handler 添加 `#[utoipa::path]`：

```diff
+ #[utoipa::path(
+     get,
+     path = "/api/v1/public/depth",
+     params(
+         ("symbol" = String, Query, description = "交易对"),
+         ("limit" = Option<u32>, Query, description = "深度层数")
+     ),
+     responses(
+         (status = 200, description = "订单簿深度", body = ApiResponse<DepthApiData>)
+     ),
+     tag = "行情数据"
+ )]
  pub async fn get_depth(
      State(state): State<Arc<AppState>>,
      Query(params): Query<HashMap<String, String>>,
  ) -> impl IntoResponse {
      // ... 现有实现 ...
  }
```

### 2.4 添加 Schema 派生

为响应类型添加 `ToSchema`：

```diff
+ use utoipa::ToSchema;

- #[derive(Serialize, Deserialize)]
+ #[derive(Serialize, Deserialize, ToSchema)]
  pub struct DepthApiData {
+     #[schema(example = "BTC_USDT")]
      pub symbol: String,
+     #[schema(example = json!([["85000.00", "0.5"]]))]
      pub bids: Vec<[String; 2]>,
+     #[schema(example = json!([["85001.00", "0.3"]]))]
      pub asks: Vec<[String; 2]>,
  }
```

### 2.5 集成 Swagger UI

在 `src/gateway/mod.rs` 中：

```diff
+ use utoipa_swagger_ui::SwaggerUi;
+ use crate::gateway::openapi::ApiDoc;

  let app = Router::new()
      .route("/api/v1/health", get(handlers::health_check))
      .nest("/api/v1/public", public_routes)
      .nest("/api/v1/private", private_routes)
+     .merge(
+         SwaggerUi::new("/docs")
+             .url("/api-docs/openapi.json", ApiDoc::openapi())
+     )
      .with_state(state);
```

---

## 3. API 端点

### 3.1 公开端点（无需认证）

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/v1/health` | GET | 健康检查 |
| `/api/v1/public/depth` | GET | 订单簿深度 |
| `/api/v1/public/klines` | GET | K 线数据 |
| `/api/v1/public/assets` | GET | 资产列表 |
| `/api/v1/public/symbols` | GET | 交易对 |
| `/api/v1/public/exchange_info` | GET | 交易所信息 |

### 3.2 私有端点（Ed25519 认证）

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/v1/private/order` | POST | 创建订单 |
| `/api/v1/private/cancel` | POST | 取消订单 |
| `/api/v1/private/orders` | GET | 查询订单 |
| `/api/v1/private/trades` | GET | 成交历史 |
| `/api/v1/private/balances` | GET | 余额查询 |
| `/api/v1/private/balances/all` | GET | 所有余额 |
| `/api/v1/private/transfer` | POST | 内部划转 |
| `/api/v1/private/transfer/{id}` | GET | 划转状态 |

---

## 4. SDK 生成

### 4.1 Python SDK

自动生成的 Python 客户端（含 Ed25519 签名）：

```python
from zero_x_infinity_sdk import ZeroXInfinityClient

client = ZeroXInfinityClient(
    api_key="your_api_key",
    secret_key_bytes=secret_key  # Ed25519 私钥
)

# 创建订单
order = client.create_order(
    symbol="BTC_USDT",
    side="BUY",
    price="85000.00",
    qty="0.001"
)
```

### 4.2 TypeScript SDK

```typescript
import { ZeroXInfinityClient } from './zero_x_infinity_sdk';

const client = new ZeroXInfinityClient(apiKey, secretKey);
const depth = await client.getDepth('BTC_USDT');
```

---

## 5. 验证

### 5.1 访问 Swagger UI

```bash
cargo run --release -- --gateway --port 8080
# 打开: http://localhost:8080/docs
```

### 5.2 测试结果

| 测试类别 | 数量 | 结果 |
|----------|------|------|
| 单元测试 | 293 | ✅ 全部通过 |
| 公开端点 | 6 | ✅ 全部通过 |
| 私有端点 | 9 | ✅ 全部通过 |
| E2E 总计 | 17 | ✅ 全部通过 |

---

## 6. 总结

本章我们为交易引擎添加了 OpenAPI 文档：

| 成就 | 结果 |
|------|------|
| **Swagger UI** | 可通过 `/docs` 访问 |
| **OpenAPI 规范** | 15 个端点已文档化 |
| **Python SDK** | 自动生成（含 Ed25519） |
| **TypeScript SDK** | 类型安全的客户端 |
| **零破坏性变更** | 所有现有测试通过 |

**下一章**：随着鲁棒性（0x0D）和文档化（0x0E）的完成，基础已经稳固。下一个合理的步骤是 **0x0F: 充值与提现** —— 连接区块链实现真正的加密货币资金。

<br>
<div align="right"><a href="#-chinese">↑ 返回顶部</a></div>
<br>
