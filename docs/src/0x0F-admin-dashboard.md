# 0x0F Admin Dashboard Architecture

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: ✅ Verified (E2E 4/4 Pass)
> **Branch**: `0x0F-admin-dashboard`
> **Updated**: 2024-12-27

> **📦 Code Changes**: [View Code](https://github.com/gjwang/zero_x_infinity/tree/v0.0F-admin-dashboard)

---

## 1. Overview

### 1.1 Goal

Build an admin dashboard for exchange operations using **FastAPI Amis Admin + FastAPI-User-Auth**.

### 1.2 Tech Stack

| Component | Technology |
|-----------|------------|
| Backend | FastAPI + SQLAlchemy |
| Admin UI | FastAPI Amis Admin (Baidu Amis) |
| Auth | FastAPI-User-Auth (Casbin RBAC) |
| Database | PostgreSQL (existing) |

### 1.3 Design Highlights ✨

> **Why do these designs matter?** The Admin Dashboard is a core operations system for the exchange. Incorrect operations can lead to fund loss or system failures. The following design principles are **key lessons** we've learned in practice:

| Design Principle | Why? |
|------------------|------|
| **🔒 ID Immutability** | `asset_id`, `symbol_id` cannot be modified after creation. Historical orders and trade records depend on these IDs—changing them would break data relationships. |
| **🔢 DB-Generated IDs** | `asset_id`, `symbol_id` use PostgreSQL `SERIAL` for auto-generation, preventing human input conflicts or errors. |
| **📝 Status as Strings** | Users see `Active`/`Disabled` instead of `1`/`0`, reducing cognitive load and avoiding misinterpretation. |
| **🚫 Base ≠ Quote** | Prevent creation of invalid pairs like `BTC_BTC`—this is a logic bug, not a UX issue. |
| **🔍 Trace ID Evidence Chain** | **Fundamental financial compliance requirement.** Each operation carries a ULID trace_id, forming a complete audit evidence chain. When issues arise: traceable, provable, reproducible. |
| **📜 Mandatory Audit Log** | All operations record before/after states, meeting compliance requirements and supporting incident investigation. |
| **🔄 Gateway Hot-Reload** | Config changes take effect within 5 seconds without service restart—critical for emergency delisting scenarios. |
| **⬇️ Default Descending Sort** | Lists show newest items first—operators typically focus on recent activity. |

> **Tutorial Tip**: These design principles didn't emerge from nothing—they come from real operational pitfalls in exchange systems. Readers should carefully understand each "Why".

### 1.4 Features

| Module | Functions |
|--------|-----------|
| **User Management** | KYC review, VIP level, ban/unban |
| **Asset Management** | Deposit confirm, withdrawal review, freeze |
| **Trading Monitor** | Real-time orders, trades, anomaly alerts |
| **Fee Config** | Symbol fee rates, VIP discounts |
| **System Monitor** | Service health, queue depth, latency |
| **Audit Log** | All admin operations logged |

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Admin Dashboard                        │
├─────────────────────────────────────────────────────────┤
│  FastAPI Amis Admin (UI)                                │
│  ├── User Management                                    │
│  ├── Asset Management                                   │
│  ├── Trading Monitor                                    │
│  ├── Fee Config                                         │
│  └── System Monitor                                     │
├─────────────────────────────────────────────────────────┤
│  FastAPI-User-Auth (RBAC)                               │
│  ├── Page Permissions                                   │
│  ├── Action Permissions                                 │
│  ├── Field Permissions                                  │
│  └── Data Permissions                                   │
├─────────────────────────────────────────────────────────┤
│  PostgreSQL (existing)     │     TDengine (read-only)  │
│  - users_tb                │     - trades_tb           │
│  - balances_tb             │     - balance_events_tb   │
│  - symbols_tb              │     - klines_tb           │
│  - transfers_tb            │                           │
└─────────────────────────────────────────────────────────┘
```

---

## 3. RBAC Roles

| Role | Permissions |
|------|-------------|
| **Super Admin** | All permissions |
| **Risk Officer** | Withdrawal review, user freeze |
| **Operations** | User management, VIP config |
| **Support** | View-only, no modifications |
| **Auditor** | View audit logs only |

---

## 4. Implementation Plan

### Phase 1: MVP - Config Management

**Scope**: Basic login + config CRUD (Asset, Symbol, VIP)

#### Step 1: Project Setup
```bash
mkdir admin && cd admin
python -m venv venv && source venv/bin/activate
pip install fastapi-amis-admin fastapi-user-auth sqlalchemy asyncpg
```

#### Step 2: Database Connection
- Connect to existing PostgreSQL (`zero_x_infinity` database)
- Reuse existing tables: `assets_tb`, `symbols_tb`, `users_tb`

#### Step 3: Admin CRUD

| Model | Table | Operations |
|-------|-------|------------|
| Asset | `assets_tb` | List, Create, Update, **Enable/Disable** |
| Symbol | `symbols_tb` | List, Create, Update, **Trading/Halt** |
| VIP Level | `vip_levels_tb` | List, Create, Update |
| **Audit Log** | `admin_audit_log` | **List (read-only)** |

#### Symbol Status

| Status | Description |
|--------|-------------|
| `trading` | Normal trading |
| `halt` | Suspended (maintenance/emergency) |

#### Step 4: Admin Auth
- Default super admin account
- Login/Logout UI

#### Acceptance Criteria

| ID | Criteria | Verify |
|----|----------|--------|
| AC-01 | Admin can login at `http://localhost:$ADMIN_PORT/admin` | Browser access (dev:8002, ci:8001) |
| AC-02 | Can create Asset (name, symbol, decimals) | UI + DB |
| AC-03 | Can edit Asset | UI + DB |
| AC-04 | Gateway hot-reload Asset config | No restart needed |
| AC-05 | Can create Symbol (base, quote, fees) | UI + DB |
| AC-06 | Can edit Symbol | UI + DB |
| AC-07 | Gateway hot-reload Symbol config | No restart needed |
| AC-08 | Can create/edit VIP Level | UI + DB |
| **AC-09** | **Reject invalid input** (decimals<0, fee>100%) | Boundary tests |
| **AC-10** | **VIP default Normal (level=0, 100% fee)** | Seed data |
| **AC-11** | **Asset Enable/Disable** | Gateway rejects disabled asset |
| **AC-12** | **Symbol Halt** | Gateway rejects new orders |
| **AC-13** | **Audit log** | All CRUD ops queryable |

#### Input Validation Rules

| Field | Rule |
|-------|------|
| `decimals` | 0-18, must be integer |
| `fee_rate` | 0-100%, max 10000 bps |
| `symbol` | Unique, uppercase + underscore |
| `base_asset` / `quote_asset` | Must exist |

#### Future Enhancements (P2)

> **Dual-Confirmation Workflow**:
> 1. **Preview** - Config change preview
> 2. **Second approval** - Another admin approves
> 3. **Apply** - Takes effect after confirmation
>
> For: Symbol delisting, Asset disable, and other irreversible ops

> **Multisig Withdrawal**:
> - Admin can only create "withdrawal proposal", not execute directly
> - Flow: Support submits → Finance reviews → Offline sign/MPC executes
> - Private keys must NEVER touch admin server

---

## 5. Security Requirements (MVP Must-Have)

### 5.1 Mandatory Audit Logging (Middleware)

Every request must be logged:

```python
# FastAPI Middleware
@app.middleware("http")
async def audit_log_middleware(request: Request, call_next):
    response = await call_next(request)
    await AuditLog.create(
        admin_id=request.state.admin_id,
        ip=request.client.host,
        timestamp=datetime.utcnow(),
        action=f"{request.method} {request.url.path}",
        old_value=...,
        new_value=...,
    )
    return response
```

### 5.2 Decimal Precision (Required)

Prevent JSON float precision loss:

```python
from pydantic import BaseModel, field_serializer
from decimal import Decimal

class FeeRateResponse(BaseModel):
    rate: Decimal

    @field_serializer('rate')
    def serialize_rate(self, rate: Decimal, _info):
        return str(rate)  # Serialize as String
```

> ⚠️ All amounts and rates MUST use `Decimal`, output MUST be `String`

#### Naming Consistency (with existing code)

| Entity | Field | Values |
|--------|-------|--------|
| Asset | `status` | 0=disabled, 1=active |
| Symbol | `status` | 0=offline, 1=online, 2=maintenance |

> ⚠️ Implementation MUST match `migrations/001_init_schema.sql`

---

## 6. UX Requirements (Post-QA Review)

> Based on QA feedback from 160+ test cases. These requirements enhance usability and prevent errors.

### 6.1 Asset/Symbol Display Enhancement

**UX-01**: Display Asset names in Symbol creation/edit forms

```
Base Asset: [BTC (ID: 1) ▼]  ← Dropdown with asset code
Quote Asset: [USDT (ID: 2) ▼]
```

**Implementation**: Use SQLAlchemy relationship display in FastAPI Amis Admin.

---

### 6.2 Fee Display Format

**UX-02**: Show fees in both percentage and basis points

```
Maker Fee: 0.10% (10 bps)
Taker Fee: 0.20% (20 bps)
```

**Implementation**: 
```python
@field_serializer('base_maker_fee')
def serialize_fee(self, fee: int, _info):
    pct = fee / 10000
    return f"{pct:.2f}% ({fee} bps)"
```

---

### 6.3 Danger Confirmation Dialog

**UX-03**: Confirm dialog for critical operations (Symbol Halt, Asset Disable)

```
┌─────────────────────────────────┐
│  ⚠️ Halt Symbol: BTC_USDT        │
├─────────────────────────────────┤
│  • Current orders: 1,234        │
│  • 24h volume: $12M             │
│                                 │
│  This action is reversible      │
│                                 │
│    [Confirm Halt]    [Cancel]   │
└─────────────────────────────────┘
```

> **Note**: No "type to confirm" required (action is reversible).

---

### 6.4 Immutable Field Indicators

**UX-04**: Visually mark immutable fields in edit forms

```
Asset Edit:
┌──────────────────────────┐
│ Asset Code: BTC 🔒       │  ← Locked, disabled
│ Decimals: 8 🔒           │  ← Locked, disabled
│ Name: [Bitcoin      ] ✏️  │  ← Editable
│ Status: [Active ▼] ✏️     │  ← Editable
└──────────────────────────┘
```

**Implementation**: Use `readonly_fields` in ModelAdmin.

---

### 6.5 Structured Error Messages

**UX-05**: Provide actionable error responses

```json
{
  "field": "asset",
  "error": "Invalid format",
  "got": "btc!",
  "expected": "Uppercase letters A-Z only (e.g., BTC)",
  "hint": "Remove special character '!'"
}
```

---

### 🚨 6.6 CRITICAL: Base ≠ Quote Validation

**UX-06**: Prevent creating symbols with same base and quote

**This is a LOGIC BUG, not just UX.**

```python
@model_validator(mode='after')
def validate_base_quote_different(self):
    if self.base_asset_id == self.quote_asset_id:
        raise ValueError("Base and Quote assets must be different")
    return self
```

**Test Case**: `BTC_BTC` must be rejected.

---

### 6.7 ID Auto-Generation (DB Responsibility)

**Requirement**: `asset_id` and `symbol_id` are **auto-generated by database**, NOT user input.

**Create Asset Form**:
```
┌──────────────────────────┐
│ Asset Code: [BTC     ]   │  ← User fills
│ Name: [Bitcoin       ]   │  ← User fills
│ Decimals: [8]            │  ← User fills
│                          │
│ asset_id: (auto)         │  ← DB generates (SERIAL)
└──────────────────────────┘
```

**Create Symbol Form**:
```
┌──────────────────────────┐
│ Symbol: [BTC_USDT    ]   │  ← User fills
│ Base Asset: [BTC ▼]      │  ← User selects
│ Quote Asset: [USDT ▼]    │  ← User selects
│                          │
│ symbol_id: (auto)        │  ← DB generates (SERIAL)
└──────────────────────────┘
```

**Implementation**: Use PostgreSQL `SERIAL` or `IDENTITY` columns.

```sql
-- Already in migrations/001_init_schema.sql
CREATE TABLE assets_tb (
    asset_id SERIAL PRIMARY KEY,  -- Auto-increment
    asset VARCHAR(16) NOT NULL UNIQUE,
    ...
);
```

---

### 6.8 Status/Flags String Display

**Requirement**: Display Status and Flags as **human-readable strings**, not raw numbers.

**Asset Status Display**:

| DB Value | Display String | Color |
|----------|----------------|-------|
| 0 | `Disabled` | 🔴 Red |
| 1 | `Active` | 🟢 Green |

**Symbol Status Display**:

| DB Value | Display String | Color |
|----------|----------------|-------|
| 0 | `Offline` | ⚫ Gray |
| 1 | `Online` | 🟢 Green |
| 2 | `Close-Only` | 🟡 Yellow |

**Asset Flags Display** (bitmask):

```
Flags: [Deposit ✓] [Withdraw ✓] [Trade ✓] [Internal Transfer ✓]
```

Instead of: `asset_flags: 23`

**Implementation** (Final Design):

> ⚠️ **API Design**: Status accepts **STRING INPUT ONLY**. Integer input is rejected.

```python
class AssetStatus(IntEnum):
    DISABLED = 0
    ACTIVE = 1

class SymbolStatus(IntEnum):
    OFFLINE = 0
    ONLINE = 1
    CLOSE_ONLY = 2

# Pydantic schema validation (string-only input)
@field_validator('status', mode='before')
def validate_status(cls, v):
    if not isinstance(v, str):
        raise ValueError(f"Status must be a string, got: {type(v).__name__}")
    return AssetStatus[v.upper()]

# Output serialization (always string)
@field_serializer('status')
def serialize_status(self, value: int) -> str:
    return AssetStatus(value).name  # "ACTIVE" or "DISABLED"
```

**Test Count**: 177 unit tests (5 for UX-08 specifically)

---

### 6.9 Default Descending Sorting (UX-09)

**Requirement**: All list views must default to **descending order** (newest items first).
**Reason**: Admins usually want to see recent activity or newly created entities.
**Implementation**: Set `ordering = [Model.pk.desc()]` in `ModelAdmin` classes.

---

### 🔒 6.10 Full Lifecycle Trace ID (UX-10) - CRITICAL

**Requirement**: Every admin operation MUST carry a **unique `trace_id` (ULID)** from entry to exit.

**Why**: Admin Dashboard is critical infrastructure. Full observability is mandatory for:
- Audit compliance
- Debugging production issues
- Security forensics
- Performance monitoring

**Trace Lifecycle**:

```
┌──────────────────────────────────────────────────────────────────┐
│  Request Entry                                                   │
│  trace_id: 01HRC5K8F1ABCDEFG... (ULID generated)                 │
├──────────────────────────────────────────────────────────────────┤
│  [LOG] trace_id=01HRC5K8F1... action=START endpoint=/asset       │
│  [LOG] trace_id=01HRC5K8F1... action=VALIDATE input={...}        │
│  [LOG] trace_id=01HRC5K8F1... action=DB_QUERY sql=SELECT...      │
│  [LOG] trace_id=01HRC5K8F1... action=DB_UPDATE before={} after={}│
│  [LOG] trace_id=01HRC5K8F1... action=AUDIT_LOG written           │
│  [LOG] trace_id=01HRC5K8F1... action=END status=200 duration=45ms│
├──────────────────────────────────────────────────────────────────┤
│  Response Exit                                                   │
│  X-Trace-ID: 01HRC5K8F1ABCDEFG... (returned in header)           │
└──────────────────────────────────────────────────────────────────┘
```

**Implementation**:

```python
import ulid
from fastapi import Request
from contextvars import ContextVar

# Context variable for trace_id
trace_id_var: ContextVar[str] = ContextVar("trace_id", default="")

@app.middleware("http")
async def trace_middleware(request: Request, call_next):
    # Generate ULID for each request
    trace_id = str(ulid.new())
    trace_id_var.set(trace_id)
    
    # Log entry
    logger.info(f"trace_id={trace_id} action=START endpoint={request.url.path}")
    
    response = await call_next(request)
    
    # Log exit
    logger.info(f"trace_id={trace_id} action=END status={response.status_code}")
    
    # Return trace_id in response header
    response.headers["X-Trace-ID"] = trace_id
    return response

# Audit log includes trace_id
class AuditLog(Base):
    trace_id = Column(String(26), nullable=False)  # ULID is 26 chars
    admin_id = Column(BigInteger, nullable=False)
    action = Column(String(32), nullable=False)
    ...
```

**Log Format** (structured JSON):

```json
{
  "timestamp": "2025-12-27T10:25:00Z",
  "trace_id": "01HRC5K8F1ABCDEFGHIJK",
  "admin_id": 1001,
  "action": "DB_UPDATE",
  "entity": "Asset",
  "entity_id": 5,
  "before": {"status": 1},
  "after": {"status": 0},
  "duration_ms": 12
}
```

**Verification**:
- [ ] Every request generates unique ULID trace_id
- [ ] All log lines include trace_id
- [ ] Audit log table has trace_id column
- [ ] Response includes `X-Trace-ID` header
- [ ] Local log files are rotated and retained

## 7. Testing

> **� Full Testing Guide**: [0x0F-admin-testing.md](./0x0F-admin-testing.md)

**Quick Start:**
```bash
./scripts/run_admin_full_suite.sh   # Run all tests
```

**Test Summary:**
| Category | Count | Status |
|----------|-------|--------|
| Rust unit tests | 5 | ✅ |
| Admin unit tests | 178+ | ✅ |
| Admin E2E tests | 4/4 | ✅ |
| UX-10 Trace ID | 16/16 | ✅ |

**Ports:** Dev `8002`, CI `8001`

---

## 8. Future Phases

| Phase | Content |
|-------|---------|
| Phase 2 | User management, balance viewer |
| Phase 3 | TDengine monitoring |
| Phase 4 | Full RBAC, advanced audit |

---

## 7. Directory Structure

```
admin/
├── main.py                 # FastAPI app entry
├── settings.py             # Config
├── models/                 # SQLAlchemy models (shared with main app)
├── admin/
│   ├── user.py            # User admin
│   ├── asset.py           # Asset admin
│   ├── trading.py         # Trading admin
│   └── system.py          # System admin
├── auth/
│   └── rbac.py            # RBAC config
└── requirements.txt
```

---

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: ✅ 已验证 (E2E 4/4 通过)
> **分支**: `0x0F-admin-dashboard`

> **📦 代码变更**: [查看代码](https://github.com/gjwang/zero_x_infinity/tree/v0.0F-admin-dashboard)

---

## 1. 概述

### 1.1 目标

使用 **FastAPI Amis Admin + FastAPI-User-Auth** 构建交易所后台管理系统。

### 1.2 技术栈

| 组件 | 技术 |
|------|------|
| 后端 | FastAPI + SQLAlchemy |
| 管理界面 | FastAPI Amis Admin (百度 Amis) |
| 认证 | FastAPI-User-Auth (Casbin RBAC) |
| 数据库 | PostgreSQL (现有) |

### 1.3 功能模块

| 模块 | 功能 |
|------|------|
| **用户管理** | KYC 审核、VIP 等级、封禁/解封 |
| **资产管理** | 充值确认、提现审核、资产冻结 |
| **交易监控** | 实时订单/成交、异常报警 |
| **费率配置** | Symbol 费率、VIP 折扣 |
| **系统监控** | 服务健康、队列积压、延迟 |
| **审计日志** | 所有管理操作可追溯 |

---

## 2. RBAC 角色

| 角色 | 权限 |
|------|------|
| **超级管理员** | 全部权限 |
| **风控专员** | 提现审核、用户冻结 |
| **运营人员** | 用户管理、VIP 配置 |
| **客服** | 只读，不可修改 |
| **审计员** | 只看审计日志 |

---

## 4. 配置与脚本统一 (2024-12-27)

### 4.1 配置单一源 (Single Source of Truth)

所有环境配置统一从 `scripts/lib/db_env.sh` 导出：

```bash
# 数据库
export PG_HOST, PG_PORT, PG_USER, PG_PASSWORD, PG_DB
export DATABASE_URL, DATABASE_URL_ASYNC

# 服务端口
export GATEWAY_PORT  # 8080
export ADMIN_PORT    # Dev: 8002, CI: 8001
export ADMIN_URL, GATEWAY_URL
```

**端口约定**：
| 环境 | Gateway | Admin |
|------|---------|-------|
| Dev (本地) | 8080 | 8002 |
| CI | 8080 | 8001 |
| QA | 8080 | 8001 |

### 4.2 测试脚本命名规范

| 脚本 | 用途 |
|------|------|
| `run_admin_full_suite.sh` | 统一入口（Rust + Admin Unit + E2E） |
| `run_admin_gateway_e2e.sh` | Admin → Gateway 传播E2E测试 |
| `run_admin_tests_standalone.sh` | 一键完整测试（安装deps+启动server） |

命名规范：`run_<scope>_<type>.sh`

### 4.3 测试结构

```
admin/tests/
├── unit/           # pytest 单元测试
├── e2e/            # pytest E2E测试 (需service running)
└── integration/    # 独立脚本 (通过CI运行)
    └── test_admin_gateway_e2e.py
```

**运行方式**：
```bash
# 运行全部
./scripts/run_admin_full_suite.sh

# 快速模式（跳过unit tests）
./scripts/run_admin_full_suite.sh --quick
```

---

<br>
<div align="right"><a href="#-chinese">↑ 返回顶部</a></div>
<br>
