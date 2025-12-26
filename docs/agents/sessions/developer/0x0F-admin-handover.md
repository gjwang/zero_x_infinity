# 0x0F Admin Dashboard - Developer Handover

> **From**: Architect  
> **To**: Developer  
> **Date**: 2025-12-26  
> **Branch**: `0x0F-admin-dashboard`

---

## Task Summary

实现 Admin Dashboard MVP，用于管理 Asset/Symbol/VIP 配置。

## Tech Stack

- FastAPI + SQLAlchemy
- FastAPI Amis Admin (UI)
- FastAPI-User-Auth (认证)
- PostgreSQL (现有数据库)

## Phase 1 Scope

| 模块 | 功能 |
|------|------|
| Asset | CRUD + status (0=disabled, 1=active) |
| Symbol | CRUD + status (0=offline, 1=online, 2=maintenance) |
| VIP Level | CRUD (默认 level=0, 100% fee) |
| Audit Log | 只读查询 |

## Key Requirements

1. **命名一致性**: 使用 `status` 字段，值与 `migrations/001_init_schema.sql` 一致
2. **输入验证**: 严格拒绝非法输入
3. **热加载**: 配置变更后 Gateway 无需重启
4. **审计日志**: 使用 Middleware 记录所有操作 (AdminID, IP, Action, OldValue, NewValue)
5. **Decimal 精度**: 所有金额/费率用 `Decimal`，序列化为 `String` (防止 float 精度丢失)

---

## 🚨 CRITICAL: ID Immutability

Per `docs/src/standards/id-specification.md`, these fields are **IMMUTABLE** after creation:

### Asset

| Field | Create | Update |
|-------|--------|--------|
| `asset` | ✅ | ❌ **BLOCKED** |
| `decimals` | ✅ | ❌ **BLOCKED** |
| `name` | ✅ | ✅ |
| `status` | ✅ | ✅ |

### Symbol

| Field | Create | Update |
|-------|--------|--------|
| `symbol` | ✅ | ❌ **BLOCKED** |
| `base_asset_id` | ✅ | ❌ **BLOCKED** |
| `quote_asset_id` | ✅ | ❌ **BLOCKED** |
| `price_decimals` | ✅ | ❌ **BLOCKED** |
| `qty_decimals` | ✅ | ❌ **BLOCKED** |
| `min_qty` / `status` / `fees` | ✅ | ✅ |

**Implementation**: Use separate `CreateSchema` and `UpdateSchema` in Pydantic.

See: `admin/admin/asset.py` and `admin/admin/symbol.py` for reference.

---

## Acceptance Criteria

| ID | Criteria |
|----|----------|
| AC-01 | Admin 可登录 `localhost:8001/admin` |
| AC-02~08 | Asset/Symbol/VIP CRUD |
| AC-09 | 非法输入拒绝 |
| AC-10 | VIP 默认 Normal |
| AC-11 | Asset Enable/Disable |
| AC-12 | Symbol Halt |
| AC-13 | 操作日志记录 |

## Quick Start

```bash
# 1. 创建分支 (已创建)
git checkout 0x0F-admin-dashboard

# 2. 创建项目
mkdir admin && cd admin
python -m venv venv && source venv/bin/activate
pip install fastapi-amis-admin fastapi-user-auth sqlalchemy asyncpg

# 3. 参考设计文档
docs/src/0x0F-admin-dashboard.md
```

## Reference

- [Design Doc](file:///docs/src/0x0F-admin-dashboard.md)
- [fastapi-amis-admin Demo](https://github.com/amisadmin/fastapi-amis-admin-demo)

---

*Architect Team*
