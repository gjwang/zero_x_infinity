# 0x10 Web Frontend Outsourcing Specification

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 📝 RFP / Requirements Spec
> **Goal**: Develop a production-grade cryptocurrency exchange frontend.

---

## 1. Project Overview

We are looking for a professional development team to build the web frontend for **Zero X Infinity**, a high-performance cryptocurrency exchange.

**Core Requirement**: The frontend must be **fast, responsive, and visually premium** (similar to Binance/Bybit Pro implementations).

**Technology Stack**: **Open Choice** (Developer proposes stack).
- Recommended: React, Vue 3, or Svelte.
- Requirement: Must produce static assets manageable by Nginx/Docker.

---

## 2. Scope of Work

### 2.1 Core Pages

| Page | Features | Backend Status |
|------|----------|----------------|
| **Home / Landing** | Market overview, Tickers, "Start Trading" CTA. | ⚠️ Mock Data (Public API part ready) |
| **Authentication** | Login, Register, Forgot Password. | ✅ **Ready** (Phase 0x10.6 Implemented) |
| **Trading Interface** | **(Core)** K-Line Chart, OrderBook, Trade History, Order Form. | ✅ **Ready** (Full API Support) |
| **Assets / Wallet** | Balance overview, Deposit, Withdrawal, Asset History. | ⚠️ **Partial** (Read Only ready; Dep/Wdw Pending) |
| **User Center** | API Key management, Password reset, Activity log. | ✅ **Backend Ready** (UI Pending) |

### 2.2 Key Features & Requirements

#### A. Trading Interface (Critical)
- **Layout**: 3-column classic layout (Left: Orderbook, Mid: Chart, Right: Trade History/Forms).
- **Chart**: Integration with **TradingView Charting Library** (or Lightweight Charts).
- **OrderBook**: Visual depth representation, clickable price to fill order form.
- **Responsiveness**: Must work flawlessly on Desktop (1080p+) and Mobile.

#### B. Technical Constraints
1.  **NO FLOATING POINT MATH**: All precision must use **String** or **BigInt** arithmetic.
    -   Backend sends: `"123.45670000"` (String).
    -   Frontend displays: Fixed precision per asset config.
2.  **WebSocket Push**: Market data is pushed via WebSocket. Frontend must handle reconnection and heartbeat.
3.  **Ed25519 Authentication**:
    -   API requests require `X-Signature` header.
    -   Frontend must sign payload using Ed25519 private key (stored in memory/session).
    -   *Note*: If using a standard password login flow, the backend may handle session cookies, but for high-security actions or if "API-Key mode" is used, client-side signing is required. **(Clarification: MVP will use opaque Session Token returned by API, standard HTTP Only Cookie or Bearer Token. Ed25519 is for API Clients, but Web UI can use session wrapper.)**

---

## 3. Deliverables

1.  **Source Code**: Full git repository history.
2.  **Docker Support**: `Dockerfile` for multi-stage build (Node build -> Nginx alpine).
3.  **Documentation**:
    -   `README.md`: Build & Run instructions.
    -   `CONFIG.md`: Environment variable reference.
4.  **Mock Server**: Simple mock logic or fixtures for UI testing without full backend.

---

## 4. Resources provided

- **API Documentation**: [Swagger UI / OpenAPI Spec](#) (See Section 6.1)
- **WebSocket Protocol**: [Docs](../src/0x09-c-websocket-push.md)
- **UI/UX References**: Binance, Kraken Pro.

---

## 5. API Inventory (Current Available)

The following APIs are implemented and available for frontend integration.

### 5.1 Public Market Data
Base URL: `/api/v1/public`

| Endpoint | Method | Description | Status |
|----------|--------|-------------|--------|
| `/exchange_info` | GET | Server time, limits | ✅ Ready |
| `/assets` | GET | List supported assets | ✅ Ready |
| `/symbols` | GET | List trading pairs | ✅ Ready |
| `/depth` | GET | Order book depth | ✅ Ready |
| `/klines` | GET | OHLCV candles | ✅ Ready |
| `/trades` | GET | Public trade history | ✅ Ready |

### 5.2 Private Trading (Requires Signature)
Base URL: `/api/v1/private`

| Endpoint | Method | Description | Status |
|----------|--------|-------------|--------|
| `/order` | POST | Place limit/market order | ✅ Ready |
| `/cancel` | POST | Cancel order | ✅ Ready |
| `/orders` | GET | List open/history orders | ✅ Ready |
| `/order/{id}` | GET | Get single order details | ✅ Ready |
| `/trades` | GET | User trade history | ✅ Ready |
| `/balances` | GET | Get specific asset balance | ✅ Ready |
| `/balances/all` | GET | Get all asset balances | ✅ Ready |

### 5.3 WebSocket Real-time Stream
Endpoint: `ws://host:port/ws`

| Channel | Type | Description | Status |
|---------|------|-------------|--------|
| `order.update` | Private | Order status change | ✅ Ready (Authenticated) |
| `trade` | Private | User trade execution | ✅ Ready (Authenticated) |
| `balance.update` | Private | Balance change | ✅ Ready (Authenticated) |
| `market.depth` | Public | Orderbook updates | ✅ Ready |
| `market.ticker` | Public | 24h Ticker updates | ✅ Ready |
| `market.trade` | Public | Public trade stream | ✅ Ready |

### 5.4 Authentication & User
| Feature | Description | Status |
|---------|-------------|--------|
| **Sign-up/Login** | User registration & JWT | ✅ Ready (Implemented) |
| **User Profile** | KYC, Password reset | ⚠️ Partial (Password Reset Ready) |
| **API Keys** | Manage API keys | ✅ Ready (Implemented) |

---

## 6. Development Resources

### 6.1 How to Access API Documentation
The backend provides auto-generated OpenAPI 3.0 documentation.

**Step 1: Start the Backend (Mock Mode)**
```bash
# Clone repository
git clone https://github.com/gjwang/zero_x_infinity
cd zero_x_infinity

# Run Gateway (requires Rust installed)
cargo run --release -- --gateway --port 8080
```

**Step 2: Access Documentation**
- **Interactive Swagger UI**: [http://localhost:8080/docs](http://localhost:8080/docs)
- **Raw OpenAPI JSON**: [http://localhost:8080/api-docs/openapi.json](http://localhost:8080/api-docs/openapi.json)

**Step 3: Generate Client SDK**
You can use `openapi-generator-cli` to generate a robust client:
```bash
npx @openapitools/openapi-generator-cli generate \
  -i http://localhost:8080/api-docs/openapi.json \
  -g typescript-axios \
  -o ./src/api
```

---

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 📝 外包需求文档 (RFP)
> **目标**: 开发一套生产级的加密货币交易所 Web 前端。

---

## 1. 项目概览

我们需要一个专业团队为 **Zero X Infinity** 高性能交易所开发 Web 前端。

**核心要求**: 界面必须 **快速、响应式且具备高级感**（对标 Binance/Bybit 专业版体验）。

**技术栈**: **不限** (由开发方提案)。
- 推荐: React, Vue 3, 或 Svelte。
- 要求: 最终产物必须是静态文件，可由 Nginx/Docker 托管。

---

## 2. 工作范围

### 2.1 核心页面

| 页面 | 功能点 | 后端状态 |
|------|________|----------|
| **首页** | 市场概览, 推荐币种, "开始交易"引导 | ⚠️ Mock 数据 (部分公有API就绪) |
| **认证模块** | 登录, 注册, 找回密码 | ✅ **后端就绪** (Phase 0x10.6 已完成) |
| **交易界面** | **(核心)** K线图, 盘口, 最新成交, 下单面板 | ✅ **完全就绪** (API 齐备) |
| **资产/钱包** | 资产总览, 充值, 提现, 资金流水 | ⚠️ **部分就绪** (仅只读余额; 充提待定) |
| **用户中心** | API Key 管理, 密码修改, 活动日志 | ✅ **后端就绪** (UI 待开发) |

### 2.2 关键特性与要求

#### A. 交易界面 (关键)
- **布局**: 经典三栏布局 (左: 盘口, 中: K线, 右: 成交/下单)。
- **图表**: 集成 **TradingView Charting Library** (或 Lightweight Charts)。
- **盘口**: 带有视觉深度的买卖盘列表，点击价格可填入下单框。
- **响应式**: 必须完美适配桌面端 (1080p+) 和移动端浏览器。

#### B. 技术限制
1.  **严禁浮点数运算**: 所有金额/价格必须使用 **String** 或 **BigInt** 处理。
    -   后端下发: `"123.45670000"` (字符串)。
    -   前端显示: 根据配置的精度进行截断/补零。
2.  **WebSocket 推送**: 行情数据通过 WS 推送。前端需处理断线重连和心跳。
3.  **Ed25519 签名 (如需)**:
    -   *注*: Web 端通常使用 Session Cookie/Token 模式。如涉及客户端签名功能，需支持 Ed25519 算法。

---

## 3. 交付物

1.  **源代码**: 完整的 Git 提交记录。
2.  **Docker 支持**: `Dockerfile` (多阶段构建: Node build -> Nginx alpine)。
3.  **文档**:
    -   `README.md`: 构建与运行指南。
    -   `CONFIG.md`: 环境变量说明。
4.  **Mock 服务**: 用于 UI 独立开发的 Mock 数据或逻辑。

---

## 4. 提供资源

- **API 文档**: [Swagger UI / OpenAPI Spec](#) (见第 6.1 节)
- **WebSocket 协议**: [文档](../src/0x09-c-websocket-push.md)
- **UI/UX 参考**: Binance, Kraken Pro.

---

## 5. API 清单 (当前可用)

以下 API 已实现并可用于前端集成。

### 5.1 公开行情数据
基础 URL: `/api/v1/public`

| 端点 | 方法 | 描述 | 状态 |
|------|------|------|------|
| `/exchange_info` | GET | 服务器时间, 限制 | ✅ 就绪 |
| `/assets` | GET | 资产列表 | ✅ 就绪 |
| `/symbols` | GET | 交易对列表 | ✅ 就绪 |
| `/depth` | GET | 订单簿深度 | ✅ 就绪 |
| `/klines` | GET | K线数据 | ✅ 就绪 |
| `/trades` | GET | 公开成交历史 | ✅ 就绪 |

### 5.2 私有交易 (需签名)
基础 URL: `/api/v1/private`

| 端点 | 方法 | 描述 | 状态 |
|------|------|------|------|
| `/order` | POST | 下单 (限价/市价) | ✅ 就绪 |
| `/cancel` | POST | 撤单 | ✅ 就绪 |
| `/orders` | GET | 查询订单 (当前/历史) | ✅ 就绪 |
| `/order/{id}` | GET | 查询单条订单 | ✅ 就绪 |
| `/trades` | GET | 用户成交历史 | ✅ 就绪 |
| `/balances` | GET | 查询特定资产余额 | ✅ 就绪 |
| `/balances/all` | GET | 查询所有余额 | ✅ 就绪 |

### 5.3 WebSocket 实时流
端点: `ws://host:port/ws`

| 频道 | 类型 | 描述 | 状态 |
|------|------|------|------|
| `order.update` | 私有 | 订单状态变更 | ✅ 就绪 (需鉴权) |
| `trade` | 私有 | 用户成交通知 | ✅ 就绪 (需鉴权) |
| `balance.update` | 私有 | 余额变更 | ✅ 就绪 (需鉴权) |
| `market.depth` | 公开 | 盘口深度更新 | ✅ 就绪 |
| `market.ticker` | 公开 | 24h Ticker更新 | ✅ 就绪 |
| `market.trade` | 公开 | 公开成交流 | ✅ 就绪 |

### 5.4 认证与用户
| 功能 | 描述 | 状态 |
|------|------|------|
| **注册/登录** | 用户注册 & JWT | ✅ 就绪 (已实现) |
| **用户资料** | KYC, 密码重置 | ⚠️ 部分就绪 (支持改密) |
| **API Key** | 管理 API Key | ✅ 就绪 (已实现) |

---

## 6. 开发资源

### 6.1 如何获取 API 文档
后端提供自动生成的 OpenAPI 3.0 文档。

**步骤 1: 启动后端 (Mock 模式)**
```bash
# 克隆仓库
git clone https://github.com/gjwang/zero_x_infinity
cd zero_x_infinity

# 运行网关 (需要安装 Rust)
cargo run --release -- --gateway --port 8080
```

**步骤 2: 访问文档**
- **交互式 Swagger UI**: [http://localhost:8080/docs](http://localhost:8080/docs)
- **原始 OpenAPI JSON**: [http://localhost:8080/api-docs/openapi.json](http://localhost:8080/api-docs/openapi.json)

**步骤 3: 生成客户端 SDK**
你可以使用 `openapi-generator-cli` 生成健壮的客户端代码：
```bash
npx @openapitools/openapi-generator-cli generate \
  -i http://localhost:8080/api-docs/openapi.json \
  -g typescript-axios \
  -o ./src/api
```
