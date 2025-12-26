# 0x0F Admin Dashboard Architecture

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 📝 Draft
> **Branch**: `0x0F-admin-dashboard`

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

### 1.3 Features

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
| AC-01 | Admin can login at `http://localhost:8001/admin` | Browser access |
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

### UX Priority Implementation

| Feature | Priority | Phase |
|---------|----------|-------|
| **UX-06 Base≠Quote** | 🔴 P0 | **MVP** (blocking bug) |
| UX-01 Asset display | P1 | Post-MVP |
| UX-02 Fee format | P1 | Post-MVP |
| UX-03 Confirm dialog | P1 | Post-MVP |
| UX-04 Readonly fields | P1 | Post-MVP |
| UX-05 Error messages | P2 | Post-MVP |

---

## 7. E2E Tests and Deliverables

### Test Scripts

| Script | Function |
|--------|----------|
| `test_admin_login.py` | Admin login/logout |
| `test_asset_crud.py` | Asset CRUD + disable |
| `test_symbol_crud.py` | Symbol CRUD + halt |
| `test_input_validation.py` | Invalid input rejection |
| `test_hot_reload.py` | Gateway hot-reload verification |

### Deliverables Checklist

| # | Deliverable | Acceptance |
|---|-------------|------------|
| 1 | `admin/` project code | Code Review |
| 2 | Admin UI accessible | Browser at `localhost:8001` |
| 3 | E2E tests all pass | `pytest admin/tests/ -v` |
| 4 | Audit log queryable | Admin UI audit page |
| 5 | Gateway hot-reload works | Config change without restart |

### Future Phases (Not in MVP)

| Phase | Content |
|-------|---------|
| Phase 2 | User management, balance viewer |
| Phase 3 | TDengine monitoring |
| Phase 4 | Full RBAC, audit logs |

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

> **📅 状态**: 📝 草稿
> **分支**: `0x0F-admin-dashboard`

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

## 3. 实现计划

**Phase 1 范围**: 登录 + 配置管理 CRUD

| 功能 | 表 |
|------|-----|
| Asset 管理 | `assets_tb` |
| Symbol 管理 | `symbols_tb` |
| VIP 等级管理 | `vip_levels_tb` |

目标：替换目前 hardcoded 的基础配置。

---

<br>
<div align="right"><a href="#-chinese">↑ 返回顶部</a></div>
<br>
