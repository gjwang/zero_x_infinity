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

### Phase 1: Setup (Day 1)

1. Create `admin/` Python project
2. Install fastapi-amis-admin, fastapi-user-auth
3. Connect to existing PostgreSQL
4. Basic admin login

### Phase 2: Core Modules (Day 2-3)

1. User management CRUD
2. Balance/Asset viewer
3. Withdrawal review workflow
4. Fee config editor

### Phase 3: Monitoring (Day 4)

1. TDengine read-only integration
2. Real-time trading dashboard
3. System health dashboard

### Phase 4: Security (Day 5)

1. RBAC role setup
2. Audit logging
3. Rate limiting
4. IP whitelist

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

| 阶段 | 内容 | 天数 |
|------|------|------|
| Phase 1 | 项目搭建、基础登录 | 1 |
| Phase 2 | 用户/资产/费率管理 | 2-3 |
| Phase 3 | TDengine 监控面板 | 1 |
| Phase 4 | RBAC + 审计日志 | 1 |

---

<br>
<div align="right"><a href="#-chinese">↑ 返回顶部</a></div>
<br>
