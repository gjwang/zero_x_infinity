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
| Asset | `assets_tb` | List, Create, Update |
| Symbol | `symbols_tb` | List, Create, Update |
| VIP Level | `vip_levels_tb` | List, Create, Update |

#### Step 4: Admin Auth
- Default super admin account
- Login/Logout UI

#### Acceptance Criteria

| ID | Criteria | Verify |
|----|----------|--------|
| AC-01 | Admin 可登录 `http://localhost:8001/admin` | 浏览器访问 |
| AC-02 | 可新增 Asset (name, symbol, decimals) | UI + DB |
| AC-03 | 可编辑 Asset | UI + DB |
| AC-04 | Gateway 热加载 Asset 配置 | 无需重启 |
| AC-05 | 可新增 Symbol (base, quote, fees) | UI + DB |
| AC-06 | 可编辑 Symbol | UI + DB |
| AC-07 | Gateway 热加载 Symbol 配置 | 无需重启 |
| AC-08 | 可新增/编辑 VIP Level | UI + DB |
| **AC-09** | **非法输入拒绝** (decimals<0, fee>100%) | 边界测试 |
| **AC-10** | **VIP 默认 Normal (level=0, 100% fee)** | 初始化数据 |

#### Input Validation Rules

| Field | Rule |
|-------|------|
| `decimals` | 0-18, 必须为整数 |
| `fee_rate` | 0-100%, 不超过 10000 bps |
| `symbol` | 唯一，大写字母+下划线 |
| `base_asset` / `quote_asset` | 必须已存在 |

#### 未来优化 (P2)

> **关键配置双确认流程**:
> 1. **预览** - 配置变更预览
> 2. **二次确认** - 另一管理员审批
> 3. **生效** - 确认后才应用
>
> 适用于：Symbol 下架、Asset 禁用等不可撤销操作

---

### Future Phases (Not in MVP)

| Phase | Content |
|-------|---------|
| Phase 2 | User management, balance viewer |
| Phase 3 | TDengine monitoring |
| Phase 4 | Full RBAC, audit logs |

---

## 5. Directory Structure

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
