# 0x0A-b API 安全鉴权 (API Authentication)

> **📅 状态**: 架构设计中  
> **分支**: `0x0A-b-api-auth`  
> **日期**: 2025-12-22

---

## 1. 概述

为 Gateway API 实现安全的请求鉴权机制，保护交易接口免受未授权访问。

### 1.1 设计目标

| 目标 | 描述 |
|------|------|
| **安全性** | 防止请求伪造、重放攻击 |
| **性能** | 验证延迟 < 1ms，不成为瓶颈 |
| **可扩展** | 支持多种鉴权方式演进 |
| **易用性** | 开发者友好的 SDK 集成 |

### 1.2 安全威胁模型

```
┌─────────────────────────────────────────────────┐
│                  威胁模型                        │
├─────────────────────────────────────────────────┤
│ 1. 请求伪造 - 攻击者伪造合法请求                  │
│ 2. 重放攻击 - 截获并重新发送有效请求              │
│ 3. 中间人攻击 - 篡改传输中的请求                 │
│ 4. API Key 泄露 - Key 被盗用                    │
│ 5. 暴力破解 - 猜测 API Key                      │
└─────────────────────────────────────────────────┘
```

---

## 2. 鉴权方案对比

### 2.1 方案评估

| 方案 | 安全性 | 性能 | 复杂度 | Secret 泄露风险 |
|------|--------|------|--------|-----------------|
| HMAC-SHA256 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 中 | 🔴 服务端存储 secret |
| **Ed25519 签名** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | 中 | 🟢 服务端仅存公钥 |
| JWT Token | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 低 | 🔴 Token 可被重放 |
| OAuth 2.0 | ⭐⭐⭐⭐ | ⭐⭐⭐ | 高 | 🟡 依赖 provider |

### 2.2 选型决策

**选择 Ed25519 非对称签名**，理由：

| 优势 | 描述 |
|------|------|
| **服务端无 secret** | 仅存储公钥，即使数据库泄露也无法伪造签名 |
| **不可抵赖性** | 只有持有私钥的用户才能签名 |
| **高安全性** | 256-bit 安全强度，抗量子计算 |
| **快速验证** | ~50μs per signature |
| **小签名体积** | 64 bytes (vs RSA 256+ bytes) |

### 2.3 Ed25519 vs HMAC-SHA256

```
HMAC-SHA256 (对称):
  Client: sign(secret, payload) → signature
  Server: verify(secret, payload, signature)
  风险: secret 存储在服务端，泄露后攻击者可伪造任意请求

Ed25519 (非对称):
  Client: sign(private_key, payload) → signature
  Server: verify(public_key, payload, signature)
  优势: 服务端仅存公钥，私钥永不离开客户端
```

### 2.4 性能对比分析

| 指标 | HMAC-SHA256 | Ed25519 | 说明 |
|------|-------------|---------|------|
| **签名生成** | ~1-2 μs | ~50-60 μs | HMAC 更快 |
| **签名验证** | ~1-2 μs | ~100-150 μs | HMAC 快 50-100x |
| **签名长度** | 32 bytes | 64 bytes | Ed25519 稍大 |
| **安全性** | 对称 | 非对称 | **Ed25519 更安全** |
| **服务端存储** | 存 secret hash | 仅存公钥 | **Ed25519 无泄露风险** |

**Benchmark 参考 (x86-64)**:
```
HMAC-SHA256:  ~1,000,000 签名验证/秒
Ed25519:      ~7,000-15,000 签名验证/秒
```

**结论**: Ed25519 的 ~100μs 验证延迟在 1ms 级别的 HTTP 请求中**完全可接受**，且安全优势显著 — 即使数据库泄露，攻击者也无法伪造签名。

---

## 3. Ed25519 签名算法设计

### 3.1 密钥对生成

```
私钥 (Private Key): 32 bytes, 客户端保存, 绝不传输
公钥 (Public Key):  32 bytes, 服务端存储, 可公开
签名 (Signature):   64 bytes
```

### 3.2 请求签名格式

```
payload = api_key + ts_nonce + method + path + body
signature = Ed25519.sign(private_key, payload)
```

> **安全设计**: 
> - API Key 包含在签名 payload 中，防止签名重用于其他 Key
> - `ts_nonce` 单调递增，防止重放攻击

**请求头**: (单 Header, HTTP 标准)
```http
POST /api/v1/order HTTP/1.1
Authorization: ZXINF v1.AK_7F3D8E2A1B5C9F04.1703260800001.3kH9sLmNpQrStUvWxYzAbCdEfGhIjKlMnOpQrStUvWxYzAbCdEfGhIjKlMnOpQrStUvWxYzAbC
```

**Authorization 格式**:
```
Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>
```

| 字段 | 描述 | 编码 | 长度 |
|------|------|------|------|
| `v1` | 协议版本号 | - | 固定 2 |
| `api_key` | API Key (大写) | HEX | `AK_` + 16 = 19 chars (64-bit) |
| `ts_nonce` | 单调递增时间戳 | 数字 | 13+ digits (Unix ms) |
| `signature` | **Base62** 编码签名 | 0-9A-Za-z | ~86 chars |

> **ts_nonce 设计**: 
> - 基于 Unix 毫秒时间戳，便于调试
> - **必须严格单调递增**: `new_ts = max(now_ms, last_ts + 1)`
> - 服务端仅存每用户最后一个值，O(1) 存储
> - 防止时钟回拨导致请求失败

### 3.3 服务端验证流程

```rust
// 1. 解析 Authorization header
let auth = headers.get("Authorization")?.strip_prefix("ZXINF ")?;
let parts: Vec<&str> = auth.split('.').collect();
if parts.len() != 4 { return Err("Invalid auth format") }
let (version, api_key, ts_nonce, signature) = 
    (parts[0], parts[1], parts[2], parts[3]);

// 2. 验证版本号
if version != "v1" { return Err("Unsupported auth version") }

// 3. 验证 API Key 格式
if !api_key.starts_with("AK_") || api_key.len() != 19 {
    return Err("Invalid API Key format")  // AK_ + 16 hex = 19 chars (64-bit)
}

// 4. 验证 ts_nonce (单调递增, 原子操作)
let ts: i64 = ts_nonce.parse()?;
// 原子 Compare-And-Swap: 仅当 new_ts > last_ts 时更新
if !ts_store.compare_and_swap_if_greater(api_key, ts) {
    return Err("ts_nonce must be monotonically increasing")
}
// 注: 生产环境使用 Redis 实现持久化和多实例共享
// redis.eval("if ARGV[1] > GET(KEY) then SET(KEY,ARGV[1]) return 1 else return 0")

// 5. 验证时间戳合理性 (可选: 30秒窗口)
if abs(now_ms() - ts) > 30_000 { 
    return Err("ts_nonce too far from server time") 
}

// 6. 查询 API Key + 公钥验证
let api_key_record = db.get_api_key(api_key)?;
if api_key_record.key_data.len() != 32 {
    return Err("Invalid public key format")  // Ed25519 公钥必须 32 bytes
}

// 7. 构建 payload
let payload = format!("{}{}{}{}{}", api_key, ts_nonce, method, path, body);

// 8. 验证 Ed25519 签名
let sig_bytes = base62_decode(signature)?;
if !ed25519_verify(&api_key_record.key_data, &payload, &sig_bytes) {
    return Err("Invalid signature")
}

// 9. 审计日志
log::info!(target: "AUTH", "api_key={} user_id={} ts_nonce={} success=true", 
    api_key, api_key_record.user_id, ts_nonce);
```

> **ts_store 实现说明**:
> - 单实例: 使用 `DashMap` 或 `RwLock<HashMap>` 实现原子 CAS
> - 多实例 (P2): 迁移到 Redis `EVAL` 脚本实现原子性和持久化

---

## 4. 数据库设计

### 4.1 api_keys_tb 表 (支持多算法)

```sql
CREATE TABLE api_keys_tb (
    key_id         SERIAL PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users_tb(user_id),
    api_key        VARCHAR(35) UNIQUE NOT NULL,  -- 预留扩展，应用层确保 AK_ + 16 hex
    key_type       SMALLINT NOT NULL DEFAULT 1,  -- 1=Ed25519, 2=HMAC-SHA256, 3=RSA
    key_data       BYTEA NOT NULL,               -- 公钥/secret_hash (取决于 key_type)
    label          VARCHAR(64),                  -- 用户自定义名称
    permissions    INT NOT NULL DEFAULT 1,       -- 权限位掩码
    status         SMALLINT NOT NULL DEFAULT 1,  -- 0=disabled, 1=active
    ip_whitelist   INET[],                       -- IP 白名单（类型安全）
    created_at     TIMESTAMPTZ DEFAULT NOW(),
    expires_at     TIMESTAMPTZ DEFAULT NOW() + INTERVAL '1 year',  -- 默认1年有效期
    last_used_at   TIMESTAMPTZ,
    
    CONSTRAINT chk_key_data_len CHECK (
        (key_type = 1 AND length(key_data) = 32) OR  -- Ed25519: 32 bytes
        (key_type = 2 AND length(key_data) = 32) OR  -- HMAC: 32 bytes  
        (key_type = 3)                                -- RSA: 可变
    )
);

CREATE INDEX idx_api_keys_user ON api_keys_tb(user_id);
CREATE INDEX idx_api_keys_status ON api_keys_tb(status);
```

### 4.2 key_type 定义

| key_type | 算法 | key_data 内容 | 长度 |
|----------|------|---------------|------|
| 1 | **Ed25519** (推荐) | 公钥 (public_key) | 32 bytes |
| 2 | HMAC-SHA256 | SHA256(secret) | 32 bytes |
| 3 | RSA | PEM 公钥 | 可变 |

> **设计说明**: `key_data` 字段统一存储密钥材料，具体内容由 `key_type` 决定。
> - Ed25519/RSA: 存储公钥
> - HMAC: 存储 SHA256(secret)，验证时同样 hash 后比较

### 4.3 权限位定义

```
permissions bitmask:
  0x01 = READ      # 查询订单、余额、行情
  0x02 = TRADE     # 下单、撤单
  0x04 = WITHDRAW  # 提现
  0x08 = TRANSFER  # 划转
```

### 4.4 API Key 生成规则

```
api_key = "AK_" + random_hex(8).upper()   # 19 chars (AK_ + 16 HEX = 64-bit)

Ed25519:
  private_key = random_bytes(32)   # 客户端保存
  public_key  = ed25519_derive(private_key)  # 服务端存储
  key_data    = public_key  # 32 bytes (必须验证长度)

HMAC-SHA256:
  secret_key  = random_bytes(32)   # 客户端保存
  key_data    = SHA256(secret_key) # 服务端存储 (32 bytes)
```

### 4.5 为什么 API Key 选择 64-bit？

**API Key 是标识符，不是密钥。** 真正的安全由 Ed25519 签名保证。

| 熵 | 可能值 | 暴力破解 (10亿次/秒) | 适用场景 |
|----|--------|----------------------|----------|
| 64-bit | 1.8×10^19 | **584 年** | ✅ 标识符足够 |
| 96-bit | 7.9×10^28 | 2.5×10^12 年 | 过量 |
| 128-bit | 3.4×10^38 | 1.1×10^22 年 | 密钥级 |

**选择 64-bit 的理由：**
1. **碰撞概率极低** - 即使 100万 用户，碰撞概率 < 10^-8
2. **暴力猜测不可行** - 584 年穷举时间
3. **可读性更好** - 19 chars vs 35 chars
4. **安全由签名保证** - 即使猜到 API Key，没有私钥也无法签名
5. **网络传输效率更高** - 每请求节省 16 bytes，高频场景累积可观
6. **服务器自行生成** - 作为数据库检索标识符，无需客户端参与

> **行业参考**: Stripe API Key ~24 chars, AWS Access Key ID 20 chars

> ⚠️ **安全提醒**: 私钥/secret 仅在创建时返回一次，不可恢复。

---

## 5. 代码架构

### 5.1 模块结构

```
src/auth/
├── mod.rs              # 模块导出
├── api_key.rs          # ApiKey 模型 + Repository
├── signature.rs        # Ed25519 签名验证 (待扩展: HMAC/RSA)
├── middleware.rs       # Axum 鉴权中间件
└── error.rs            # 鉴权错误类型
```

### 5.2 请求处理流程

```
┌────────────┐    ┌──────────────┐    ┌──────────────┐    ┌─────────────┐
│   Client   │───▶│  Middleware  │───▶│   Handler    │───▶│   Response  │
└────────────┘    └──────────────┘    └──────────────┘    └─────────────┘
                        │
                        ▼
              ┌─────────────────────┐
              │ 1. 提取 Headers      │
              │ 2. 验证 Timestamp    │
              │ 3. 查询 ApiKey       │
              │ 4. 验证 Signature    │
              │ 5. 检查 Permissions  │
              │ 6. 注入 user_id     │
              └─────────────────────┘
```

### 5.3 错误响应

```json
{
  "code": 401,
  "message": "Invalid signature",
  "error": "AUTH_SIGNATURE_INVALID"
}
```

| Error Code | HTTP | 描述 |
|------------|------|------|
| `AUTH_KEY_MISSING` | 401 | 缺少 X-API-Key |
| `AUTH_KEY_INVALID` | 401 | API Key 不存在或已禁用 |
| `AUTH_TIMESTAMP_EXPIRED` | 401 | 时间戳过期 |
| `AUTH_SIGNATURE_INVALID` | 401 | 签名验证失败 |
| `AUTH_PERMISSION_DENIED` | 403 | 权限不足 |

---

## 6. 路由保护策略

### 6.1 端点分类

#### 6.1.1 公开接口 (Public) - 无需鉴权

| 类别 | 端点 | 说明 |
|------|------|------|
| **行情** | `GET /api/v1/public/exchange_info` | 交易对信息 |
| **行情** | `GET /api/v1/public/depth` | 深度数据 |
| **行情** | `GET /api/v1/public/klines` | K 线数据 |
| **行情** | `GET /api/v1/public/ticker` | 最新价格 |

#### 6.1.2 私有接口 (Private) - 需要签名鉴权

| 类别 | 端点 | 权限 |
|------|------|------|
| **账户** | `GET /api/v1/private/account` | READ |
| **账户** | `GET /api/v1/private/balance` | READ |
| **交易** | `GET /api/v1/private/orders` | READ |
| **交易** | `POST /api/v1/private/order` | TRADE |
| **交易** | `DELETE /api/v1/private/order` | TRADE |
| **资金** | `POST /api/v1/private/withdraw` | WITHDRAW |
| **资金** | `POST /api/v1/private/transfer` | TRANSFER |

### 6.2 中间件应用

```rust
// 公开路由 (无需鉴权)
let public_routes = Router::new()
    .route("/exchange_info", get(exchange_info))
    .route("/depth", get(depth))
    .route("/klines", get(klines))
    .route("/ticker", get(ticker));

// 私有路由 (需要签名鉴权)
let private_routes = Router::new()
    .route("/account", get(account))
    .route("/balance", get(balance))
    .route("/orders", get(orders))
    .route("/order", post(create_order).delete(cancel_order))
    .route("/withdraw", post(withdraw))
    .route("/transfer", post(transfer))
    .layer(from_fn(auth_middleware));

// 组合路由
let app = Router::new()
    .nest("/api/v1/public", public_routes)
    .nest("/api/v1/private", private_routes);
```

---

## 7. 性能考虑

### 7.1 缓存策略

```
ApiKey 查询优化:
1. 内存缓存 (LRU, TTL=5min)
2. 缓存 key: api_key 转 u64 (高效比较)
3. 缓存 value: {user_id, key_type, key_data, permissions, status}

API Key u64 转换:
  api_key = "AK_" + hex(16)    # 64-bit 正好对应 u64
  cache_key = parse_hex_to_u64(api_key[3..])  # 跳过 "AK_" 前缀
```

> **约束**: API Key 生成时必须确保 HEX 值 ≤ u64::MAX (18446744073709551615)

### 7.2 性能目标

| 指标 | 目标 | 实现方式 |
|------|------|----------|
| 签名验证 | < 50μs | Ed25519 |
| DB 查询 | < 1ms | 连接池 + 索引 |
| 总延迟 | < 2ms | 缓存命中 |

---

## 8. 实现计划

### 8.1 开发清单

#### Phase 1: 数据库层

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 1.1 | 创建 migration | `migrations/xxx_create_api_keys.sql` | 表创建成功 |
| 1.2 | 添加 `last_ts_nonce` 列 | 同上 | 用于持久化最后 ts_nonce |

#### Phase 2: 核心模块

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 2.1 | ApiKey 模型 | `src/auth/api_key.rs` | 涵盖所有字段 |
| 2.2 | ApiKey Repository | `src/auth/repository.rs` | CRUD + 缓存 |
| 2.3 | Base62 编解码 | `src/auth/base62.rs` | 编解码正确 |
| 2.4 | Ed25519 签名验证 | `src/auth/signature.rs` | 验证通过/失败 |
| 2.5 | ts_store (原子 CAS) | `src/auth/ts_store.rs` | 单调递增检查 |
| 2.6 | 鉴权中间件 | `src/auth/middleware.rs` | 9步验证流程 |

#### Phase 3: 路由集成

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 3.1 | 拆分 public/private 路由 | `src/routes/mod.rs` | 路径正确 |
| 3.2 | 应用鉴权中间件 | `src/main.rs` | private 路由受保护 |

#### Phase 4: 测试验证

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 4.1 | 单元测试 | `src/auth/tests.rs` | 覆盖主要场景 |
| 4.2 | 集成测试脚本 | `scripts/test_auth.py` | 签名生成 + 请求 |
| 4.3 | 失败场景测试 | 同上 | 错误码覆盖 |

### 8.2 关键数据结构

```rust
// API Key 记录
pub struct ApiKeyRecord {
    pub key_id: i32,
    pub user_id: i64,
    pub api_key: String,           // AK_ + 16 hex (19 chars)
    pub key_type: i16,             // 1=Ed25519
    pub key_data: Vec<u8>,         // 公钥 32 bytes
    pub permissions: i32,
    pub status: i16,
}

// ts_store 原子操作
pub trait TsStore {
    fn compare_and_swap_if_greater(&self, api_key: &str, ts: i64) -> bool;
}
```

### 8.3 验证 Checklist

- [ ] 请求格式: `Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>`
- [ ] api_key 格式: `AK_` + 16 HEX uppercase = 19 chars
- [ ] ts_nonce: 单调递增 (ms 时间戳)
- [ ] signature: Base62 编码 (~86 chars)
- [ ] payload: `api_key + ts_nonce + method + path + body`
- [ ] 公开路由: `/api/v1/public/*` 无鉴权
- [ ] 私有路由: `/api/v1/private/*` 需签名

---

## 9. SDK 示例

### 9.1 Python (Ed25519)

```python
from nacl.signing import SigningKey
import base64
import time
import secrets
import requests

api_key = "AK_7F3D8E2A1B5C9F04"  # 64-bit uppercase (19 chars)
# 私钥 (32 bytes) - 仅客户端保存
private_key_bytes = bytes.fromhex("your_private_key_hex")
signing_key = SigningKey(private_key_bytes)

# 记录上一次 ts_nonce，保证单调递增
last_ts_nonce = 0

# Base62 编码函数
def base62_encode(data: bytes) -> str:
    ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
    num = int.from_bytes(data, 'big')
    if num == 0:
        return ALPHABET[0]
    result = []
    while num:
        num, rem = divmod(num, 62)
        result.append(ALPHABET[rem])
    return ''.join(reversed(result))

def get_ts_nonce() -> str:
    """生成单调递增的 ts_nonce (基于时间戳)"""
    global last_ts_nonce
    now = int(time.time() * 1000)
    ts_nonce = max(now, last_ts_nonce + 1)  # 保证单调递增
    last_ts_nonce = ts_nonce
    return str(ts_nonce)

def sign_request(method: str, path: str, body: str = ""):
    ts_nonce = get_ts_nonce()
    payload = f"{api_key}{ts_nonce}{method}{path}{body}"
    signature = signing_key.sign(payload.encode()).signature
    sig_b62 = base62_encode(signature)
    # 组装 Authorization header (v1 版本，4 部分)
    auth_token = f"v1.{api_key}.{ts_nonce}.{sig_b62}"
    return auth_token

auth_token = sign_request("GET", "/api/v1/orders")
response = requests.get(
    "http://localhost:8080/api/v1/orders",
    headers={"Authorization": f"ZXINF {auth_token}"}
)
print(response.json())
```

> **依赖**: `pip install pynacl requests`

### 9.2 Curl (with openssl)

```bash
API_KEY="AK_7F3D8E2A1B5C9F04"

# 生成单调递增的 ts_nonce (基于时间戳)
# 注意: 生产环境需要持久化 LAST_TS 以保证重启后仍然递增
LAST_TS_FILE="/tmp/.zxinf_last_ts"
NOW=$(date +%s%3N)  # Unix 毫秒
LAST_TS=$(cat "$LAST_TS_FILE" 2>/dev/null || echo 0)
TS_NONCE=$((NOW > LAST_TS ? NOW : LAST_TS + 1))
echo "$TS_NONCE" > "$LAST_TS_FILE"

METHOD="GET"
PATH="/api/v1/orders"
PAYLOAD="${API_KEY}${TS_NONCE}${METHOD}${PATH}"

# 生成签名 (Ed25519 需要 openssl 3.0+)
# SIGNATURE=$(echo -n "$PAYLOAD" | openssl pkeyutl -sign -inkey private.pem | base62_encode)

# 组装 Authorization header (v1 版本, 4 部分)
AUTH_TOKEN="v1.${API_KEY}.${TS_NONCE}.${SIGNATURE}"

curl -H "Authorization: ZXINF ${AUTH_TOKEN}" \
     http://localhost:8080/api/v1/orders
```

---

## 10. 设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 签名算法 | Ed25519 (首实现) | 服务端不存 secret，最高安全性 |
| **传输格式** | 单 `Authorization` Header | HTTP 标准 (RFC 7235)，Proxy 兼容 |
| 多算法支持 | key_type 字段 | 未来可扩展 HMAC/RSA |
| 密钥存储 | key_data (BYTEA) | 统一存储公钥/hash |
| 时间戳精度 | 毫秒 | 与 Binance 兼容 |
| 重放窗口 | 30秒 | 平衡安全与时钟偏差 |
| Key 格式 | `AK_` 前缀 (64-bit) | 易于识别，u64 缓存高效 |

---

## 11. 未来优化: 混合鉴权 (Hybrid Auth)

> **优先级**: P2  
> **目标**: 兼顾安全性与性能

### 11.1 方案概述

类似 TLS 握手，使用 Ed25519 协商临时 HMAC 密钥：

```
┌─────────────────────────────────────────────────────────────────┐
│                    混合鉴权流程                                  │
├─────────────────────────────────────────────────────────────────┤
│  Session Start (慢, 安全)                                        │
│    Client → Server: Ed25519 签名请求                            │
│    Server → Client: 临时 session_key (内存, 不持久化)            │
│                                                                 │
│  后续请求 (快)                                                   │
│    Client: HMAC-SHA256(session_key, payload)                    │
│    Server: 验证 ~1μs                                            │
│                                                                 │
│  Session End                                                    │
│    session_key 丢弃, 下次会话重新协商                            │
└─────────────────────────────────────────────────────────────────┘
```

### 11.2 关键设计

| 特性 | 设计 |
|------|------|
| Session Key | 内存存储，**不持久化** |
| 有效期 | 单次会话，断开即失效 |
| 密钥刷新 | 每次新连接重新协商 |
| 安全保障 | Ed25519 确保初始身份验证安全 |

### 11.3 性能预期

| 阶段 | 算法 | 延迟 | 频率 |
|------|------|------|------|
| 会话建立 | Ed25519 | ~100μs | 1次/连接 |
| API 请求 | HMAC-SHA256 | ~1μs | N次/连接 |

**适用场景**: WebSocket 长连接、高频 API 调用

---

## 12. 安全审核记录

> **审核日期**: 2025-12-22  
> **审核结论**: ✅ 设计安全合理，可开始实现

### 12.1 安全评估

| 安全层 | 机制 | 评估 | 说明 |
|--------|------|------|------|
| 身份认证 | Ed25519 签名 | ⭐⭐⭐⭐⭐ | 非对称，服务端无 secret |
| 防伪造 | 私钥签名 | ⭐⭐⭐⭐⭐ | 只有私钥持有者可签名 |
| 防重放 | ts_nonce 单调递增 | ⭐⭐⭐⭐⭐ | 原子 CAS，O(1) 存储 |
| 防签名跨用 | API Key 在 payload 中 | ⭐⭐⭐⭐⭐ | 签名绑定特定 Key |
| 时钟容错 | `max(now, last+1)` | ⭐⭐⭐⭐⭐ | 客户端防时钟回拨 |
| 协议升级 | v1 版本号 | ⭐⭐⭐⭐⭐ | 向后兼容预留 |
| 传输效率 | 4 字段 / Base62 | ⭐⭐⭐⭐ | 无特殊字符，紧凑 |

### 12.2 已处理风险

| # | 风险 | 处理方式 |
|---|------|----------|
| 1 | ts_store 并发安全 | ✅ 原子 `compare_and_swap_if_greater()` |
| 2 | 服务重启持久化 | ✅ 标注 Redis P2 |
| 3 | 多实例部署 | ✅ 标注 Redis `EVAL` P2 |

### 12.3 P2 未来工作

| 项目 | 优先级 | 说明 |
|------|--------|------|
| Rate Limiting | P2 | Gateway 层限流 |
| 失败审计日志 | P2 | 记录验证失败尝试 |
| API Key 缓存失效 | P2 | 禁用后实时通知 |
| Redis ts_store | P2 | 多实例共享和持久化 |

---

**设计审核通过，可开始实现。**
