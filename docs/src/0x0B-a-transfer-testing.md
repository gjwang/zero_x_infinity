# Internal Transfer E2E Testing Guide

## 概述 / Overview

本文档描述了 Phase 0x0B-a 内部转账功能的完成工作、实现细节和端到端测试方法。

This document describes the completed work, implementation details, and end-to-end testing methodology for Phase 0x0B-a Internal Transfer feature.

---

## 本章完成工作 / Chapter Deliverables

### 架构实现 / Architecture Implementation

实现了跨系统资金划转的 2-Phase Commit FSM:

```
                     ┌─────────────────┐
                     │  TransferAPI    │  Gateway 层
                     └────────┬────────┘
                              │
                     ┌────────▼────────┐
                     │ TransferCoord.  │  FSM 协调器
                     └────────┬────────┘
                              │
           ┌──────────────────┼──────────────────┐
           │                  │                  │
  ┌────────▼────────┐ ┌───────▼───────┐ ┌───────▼───────┐
  │ FundingAdapter  │ │ TradingAdapter│ │  TransferDb   │
  │   (PostgreSQL)  │ │  (UBSCore)    │ │  (FSM State)  │
  └─────────────────┘ └───────────────┘ └───────────────┘
```

### 核心模块 / Core Modules

| 模块 / Module | 文件 / File | 功能 / Function |
|---------------|------------|-----------------|
| **TransferCoordinator** | `src/transfer/coordinator.rs` | FSM 状态机驱动<br>State machine driver |
| **FundingAdapter** | `src/transfer/adapters/funding.rs` | PostgreSQL 资金操作<br>PostgreSQL balance ops |
| **TradingAdapter** | `src/transfer/adapters/trading.rs` | UBSCore 通道通信<br>UBSCore channel comm |
| **TransferDb** | `src/transfer/db.rs` | FSM 状态持久化<br>FSM state persistence |
| **TransferChannel** | `src/transfer/channel.rs` | 跨线程通信<br>Cross-thread messaging |

### 新增 API / New APIs

| Endpoint | Method | 描述 / Description |
|----------|--------|---------------------|
| `/api/v1/private/transfer` | POST | 创建内部转账 |
| `/api/v1/private/transfer/{req_id}` | GET | 查询转账状态 |
| `/api/v1/private/balances/all` | GET | 查询所有账户余额 |

### 数据库表 / Database Tables

| 表 / Table | 用途 / Purpose |
|------------|----------------|
| `fsm_transfers_tb` | FSM 转账状态记录 |
| `transfer_operations_tb` | 幂等操作追踪 |
| `balances_tb` | 账户余额 (Funding/Spot) |

### 交付物 / Deliverables

- ✅ 完整的 FSM 实现 (Init → SourcePending → SourceDone → TargetPending → Committed)
- ✅ 双向转账验证 (Funding ↔ Spot)
- ✅ 可复用 E2E 测试脚本
- ✅ `/balances/all` 余额查询 API
- ✅ 232 个单元测试通过

---

## 测试脚本 / Test Script

### 自动化 E2E 测试 / Automated E2E Test

```bash
# 运行完整 E2E 测试 (自动启动 Gateway)
./scripts/test_transfer_e2e.sh
```

脚本位置: [`scripts/test_transfer_e2e.sh`](../../../scripts/test_transfer_e2e.sh)

### 测试流程 / Test Flow

```
[1/6] Prerequisites Check
    ✓ PostgreSQL connected (port 5433)
    ✓ Release binary ready

[2/6] Setup Test Data
    - Enable CAN_INTERNAL_TRANSFER for USDT
    - Create 1000 USDT in Funding for user 1001
    - Clear previous transfer records

[3/6] Start Gateway
    - Stop existing Gateway (pgrep + kill)
    - Start new Gateway with updated config
    - Wait for health check

[4/6] Run Transfer Tests
    - Funding → Spot (50 USDT)
    - Spot → Funding (25 USDT)
    - Verify both COMMITTED

[5/6] Verify Balance Changes
    - Check Funding: 1000 → 975 (Δ-25)
    - Use /balances/all API

[6/6] Cleanup
    - Stop Gateway
```

---

## API 测试 / API Testing

### 使用 Python 客户端 / Using Python Client

```python
import sys
sys.path.append('scripts/lib')
from api_auth import get_test_client

USER_ID = 1001
client = get_test_client(user_id=USER_ID)
headers = {'X-User-ID': str(USER_ID)}

# 1. 查询余额 / Query balances
resp = client.get('/api/v1/private/balances/all', headers=headers)
print(resp.json())

# 2. 发起转账 / Create transfer
resp = client.post('/api/v1/private/transfer',
    json_body={
        'from': 'funding',
        'to': 'spot',
        'asset': 'USDT',
        'amount': '50'
    },
    headers=headers)
print(resp.json())

# 3. 查询转账状态 / Query transfer status
req_id = resp.json()['data']['req_id']
resp = client.get(f'/api/v1/private/transfer/{req_id}', headers=headers)
print(resp.json())
```

### 使用 curl / Using curl

```bash
# 查询余额 (需要正确签名)
curl http://localhost:8080/api/v1/private/balances/all \
  -H "X-API-Key: AK_0000000000001001" \
  -H "X-Signature: ..." \
  -H "X-User-ID: 1001"
```

---

## 数据库验证 / Database Verification

### 检查余额 / Check Balances

```bash
PGPASSWORD=trading123 psql -h localhost -p 5433 -U trading -d exchange_info_db -c "
SELECT 
    CASE account_type WHEN 1 THEN 'Spot' WHEN 2 THEN 'Funding' END as account,
    (available / 1000000)::text || ' USDT' as balance
FROM balances_tb 
WHERE user_id = 1001 AND asset_id = 2
ORDER BY account_type;
"
```

### 检查 FSM 状态 / Check FSM State

```bash
PGPASSWORD=trading123 psql -h localhost -p 5433 -U trading -d exchange_info_db -c "
SELECT req_id, amount, state, created_at 
FROM fsm_transfers_tb 
WHERE user_id = 1001
ORDER BY created_at DESC LIMIT 5;
"
```

State 值含义 / State Values:
- `0`: INIT
- `10`: SOURCE_PENDING
- `20`: SOURCE_DONE
- `30`: TARGET_PENDING
- `40`: COMMITTED ✅
- `-10`: FAILED
- `-20`: COMPENSATING
- `-30`: ROLLED_BACK

---

## 已修复的 Bug / Fixed Bugs

### 1. FSM 未执行 / FSM Not Executing

**问题**: `create_transfer_fsm` 只调用 `coordinator.create()`，没有调用 `coordinator.execute()`

**修复**: 添加 execute() 调用

```rust
// src/transfer/api.rs
let req_id = coordinator.create(core_req).await?;
let state = coordinator.execute(req_id).await?; // ← Added
```

### 2. 金额解析为 0 / Amount Parsed as 0

**问题**: `Decimal.to_string().parse::<u64>()` 对 "50000000.00000000" 返回失败

**修复**: 使用 `trunc().to_i64()`

```rust
// src/transfer/db.rs
let amount_u64 = amount.trunc().to_i64().unwrap_or(0) as u64;
```

### 3. 类型不匹配 / Type Mismatch

- `status` 列: INT4 (i32), 不是 INT2
- `decimals` 列: INT2 (i16), 不是 i32

---

## 测试结果示例 / Sample Test Output

```
==============================================
Internal Transfer E2E Test (Phase 0x0B-a)
==============================================

[1/6] Checking prerequisites...
  ✓ PostgreSQL connected
  ✓ Release binary ready
[2/6] Setting up test data...
  ✓ Test data initialized (1000 USDT in Funding only for user 1001)
[3/6] Starting Gateway...
  ✓ Gateway ready
[4/6] Running transfer tests with balance verification...
  [BEFORE] Getting initial balances...
    USDT:funding: 1000.00

  [TRANSFER 1] Funding → Spot (50 USDT)...
    ✓ COMMITTED
  [TRANSFER 2] Spot → Funding (25 USDT)...
    ✓ COMMITTED

  [AFTER] Getting final Funding balance...
    USDT:funding: 975.00

  [VERIFY] Checking Funding balance changes...
    ✓ Funding: 1000.00 → 975.00 (Δ-25.00)

  Results: 3 passed, 0 failed
[5/6] Final database state...
 Funding | 975.0000000000000000 USDT

[6/6] Cleanup...

==============================================
✅ All E2E Transfer Tests PASSED
==============================================
```

---

## 相关文件 / Related Files

| 文件 / File | 描述 / Description |
|-------------|---------------------|
| `scripts/test_transfer_e2e.sh` | E2E 测试脚本 |
| `scripts/lib/api_auth.py` | API 认证库 |
| `src/transfer/api.rs` | 转账 API 处理 |
| `src/transfer/coordinator.rs` | FSM 协调器 |
| `src/transfer/adapters/funding.rs` | Funding 适配器 |
| `src/transfer/adapters/trading.rs` | Trading 适配器 |
