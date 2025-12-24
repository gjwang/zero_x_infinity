# 0x0A-c: API Authentication

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: ✅ **Implemented**
> **Branch**: `0x0A-b-api-auth`
> **Date**: 2025-12-23
> **Code Changes**: [v0.0A-a-account-system...v0.0A-b-api-auth](https://github.com/gjwang/zero_x_infinity/compare/v0.0A-a-account-system...v0.0A-b-api-auth)

## Implementation Summary

| Metric | Result |
|--------|--------|
| **Auth Module** | 8 Files |
| **Unit Tests** | 35/35 ✅ |
| **Total Tests** | 188/188 ✅ |
| **Commits** | 31 commits |

---

## 1. Overview

Implement secure request authentication for Gateway API to protect trading endpoints from unauthorized access.

### 1.1 Design Goals

| Goal | Description |
|------|-------------|
| **Security** | Prevent forgery and replay attacks |
| **Performance** | Verification latency < 1ms |
| **Scalability** | Support multiple auth methods |
| **Usability** | Developer-friendly SDK integration |

### 1.2 Threat Model

*   Request Forgery
*   Replay Attack
*   Man-in-the-Middle (MITM)
*   API Key Leakage
*   Brute Force

---

## 2. Authentication Scheme Comparison

### 2.1 Evaluation

| Scheme | Security | Performance | Complexity | Leal Risk |
|--------|----------|-------------|------------|-----------|
| HMAC-SHA256 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | Medium | 🔴 Secret on server |
| **Ed25519** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | Medium | 🟢 Public key only |
| JWT Token | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | Low | 🔴 Token replayable |
| OAuth 2.0 | ⭐⭐⭐⭐ | ⭐⭐⭐ | High | 🟡 Dependency |

### 2.2 Decision: Ed25519

**Selected Ed25519 Asymmetric Signature**.

*   **No Server Secret**: Only public key stored.
*   **Non-Repudiation**: Only private key holder can sign.
*   **High Security**: 128-bit security level (256-bit key).
*   **Fast Verification**: ~100μs.

---

## 3. Ed25519 Signature Design

### 3.1 Key Pair

*   **Private Key**: 32 bytes, stored on Client, NEVER transmitted.
*   **Public Key**: 32 bytes, stored on Server.
*   **Signature**: 64 bytes.

### 3.2 Request Signature Format

```
payload = api_key + ts_nonce + method + path + body
signature = Ed25519.sign(private_key, payload)
```

**Header Format**:
```
Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>
```

| Field | Description | Encoding |
|-------|-------------|----------|
| `api_key` | `AK_` + 16 HEX (19 chars) | plain |
| `ts_nonce` | Monotonic Timestamp (ms) | numeric |
| `signature` | 64-byte signature | **Base62** |

> **ts_nonce**: Must be strictly monotonically increasing. `new_ts = max(now_ms, last_ts + 1)`.

---

## 4. Database Design

### 4.1 api_keys_tb Table

```sql
CREATE TABLE api_keys_tb (
    key_id         SERIAL PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users_tb(user_id),
    api_key        VARCHAR(35) UNIQUE NOT NULL,
    key_type       SMALLINT NOT NULL DEFAULT 1,  -- 1=Ed25519
    key_data       BYTEA NOT NULL,               -- Public Key (32 bytes)
    permissions    INT NOT NULL DEFAULT 1,
    status         SMALLINT NOT NULL DEFAULT 1,
    ...
);
```

### 4.2 Key Types

| key_type | Algorithm | key_data |
|----------|-----------|----------|
| 1 | **Ed25519** | Public Key (32 bytes) |
| 2 | HMAC-SHA256 | SHA256(secret) |
| 3 | RSA | PEM Public Key |

---

## 5. Code Architecture

### 5.1 Module Structure

```
src/api_auth/
├── mod.rs
├── api_key.rs          # Model + Repository
├── signature.rs        # Ed25519 verification
├── middleware.rs       # Axum Middleware
└── error.rs            # Auth Errors
```

### 5.2 Request Flow

1.  Extract Headers.
2.  Verify Timestamp window.
3.  Query ApiKey (Cache/DB).
4.  Verify Ed25519 Signature.
5.  Check Permissions.
6.  Inject `user_id` into context.

---

## 6. Route Protection

### 6.1 Public Endpoints (No Auth)

*   `GET /api/v1/public/exchange_info`
*   `GET /api/v1/public/depth`
*   `GET /api/v1/public/klines`
*   `GET /api/v1/public/ticker`

### 6.2 Private Endpoints (Auth Required)

*   `GET /api/v1/private/account`
*   `POST /api/v1/private/order` (Trade Perm)
*   `POST /api/v1/private/withdraw` (Withdraw Perm)

---

## 7. Performance

*   **Signature Verification**: < 50μs (Ed25519).
*   **DB Query**: < 1ms (Cached).
*   **Total Latency Overhead**: < 2ms.

---

## 8. SDK Example (Python)

```python
from nacl.signing import SigningKey
import time

api_key = "AK_..."
private_key = bytes.fromhex("...")
signing_key = SigningKey(private_key)

def sign_request(method, path, body=""):
    ts_nonce = str(int(time.time() * 1000))
    payload = f"{api_key}{ts_nonce}{method}{path}{body}"
    signature = signing_key.sign(payload.encode()).signature
    sig_b62 = base62_encode(signature)
    return f"v1.{api_key}.{ts_nonce}.{sig_b62}"
```

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: ✅ **实现完成**
> **代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.0A-a-account-system...v0.0A-b-api-auth)

## Implementation Summary

| 指标 | 结果 |
|------|------|
| **Auth 模块** | 8 文件 |
| **单元测试** | 35/35 ✅ |
| **全部测试** | 188/188 ✅ |

---

## 1. 概述

为 Gateway API 实现安全的请求鉴权机制，保护交易接口免受未授权访问。

### 1.1 设计目标

安全、高性能、可扩展、易用。

### 1.2 安全威胁模型

请求伪造、重放攻击、中间人攻击、Key 泄露等。

---

## 2. 鉴权方案对比

### 2.2 选型决策

**选择 Ed25519 非对称签名**。

*   **服务端无 secret**：仅存储公钥。
*   **不可抵赖性**。
*   **高安全性**。
*   **快速验证** (~100μs)。

---

## 3. Ed25519 签名算法设计

### 3.1 密钥对生成

私钥客户端保存，公钥服务端存储。

### 3.2 请求签名格式

```
payload = api_key + ts_nonce + method + path + body
signature = Ed25519.sign(private_key, payload)
```

**Header**: `Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>`

---

## 4. 数据库设计

### 4.1 api_keys_tb 表

支持 `key_type` (1=Ed25519, 2=HMAC, 3=RSA)。`key_data` 存储公钥或 secret hash。

---

## 5. 代码架构

`src/api_auth/` 下包含 `api_key`, `signature`, `middleware` 等模块。

---

## 6. 路由保护策略

*   **Public**: 行情接口，无需鉴权。
*   **Private**: 交易/账户接口，需签名鉴权。

---

## 7. 性能考虑

使用 Ed25519 极速验证 (< 50μs) + 内存缓存，总延迟 < 2ms。

---

## 8. SDK 示例 (Python)

提供 Python/Curl 示例代码，展示如何生成符合规范的 Authorization header。
