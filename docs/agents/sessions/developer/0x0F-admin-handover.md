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

## 🎯 NEW: P0 UX Requirements

### UX-07: ID Auto-Generation (CRITICAL)

**Requirement**: `asset_id` and `symbol_id` are **DB auto-generated** (SERIAL), NOT user input.

Users only fill:
- **Asset**: `asset`, `name`, `decimals`
- **Symbol**: `symbol`, `base_asset_id`, `quote_asset_id`

IDs are generated automatically by PostgreSQL `SERIAL` in `migrations/001_init_schema.sql`.

### UX-08: Status/Flags String Display (CRITICAL)

**Requirement**: Display status and flags as **human-readable strings**, not raw numbers.

| Entity | DB Value | Display String |
|--------|----------|----------------|
| Asset Status | 0 | `Disabled` (🔴 Red) |
| Asset Status | 1 | `Active` (🟢 Green) |
| Symbol Status | 0 | `Offline` (⚫ Gray) |
| Symbol Status | 1 | `Online` (🟢 Green) |
| Symbol Status | 2 | `Close-Only` (🟡 Yellow) |

**Implementation**: Use `field_serializer` or Enum in Pydantic schemas.

### UX-10: Trace ID Evidence Chain (CRITICAL - Financial Compliance)

**Requirement**: Every admin operation MUST carry a **unique `trace_id` (ULID)** from entry to exit.

**Why**: This is a **fundamental requirement for financial audit compliance**:
- **可追溯** (Traceable): Every action links to a unique ID
- **可举证** (Provable): Evidence chain for dispute resolution
- **可复现** (Reproducible): Reconstruct events for investigation

**Implementation**:
```python
import ulid
from contextvars import ContextVar

trace_id_var: ContextVar[str] = ContextVar("trace_id", default="")

@app.middleware("http")
async def trace_middleware(request: Request, call_next):
    trace_id = str(ulid.new())
    trace_id_var.set(trace_id)
    logger.info(f"trace_id={trace_id} action=START endpoint={request.url.path}")
    response = await call_next(request)
    response.headers["X-Trace-ID"] = trace_id
    return response
```

**Requirements**:
- [ ] Each request generates unique ULID `trace_id`
- [ ] All log lines include `trace_id`
- [ ] `admin_audit_log` table has `trace_id` column (VARCHAR 26)
- [ ] Response includes `X-Trace-ID` header

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

### UX-09: Default Descending Sorting (CRITICAL)

**Requirement**: All list views must default to **descending order** (newest items first).
**Reason**: Recent activity is usually most relevant.
**Implementation**: Set `ordering = [Model.pk.desc()]` in `ModelAdmin` classes.

# 3. 参考设计文档
docs/src/0x0F-admin-dashboard.md
```

## Reference

- [Design Doc](file:///docs/src/0x0F-admin-dashboard.md)
- [fastapi-amis-admin Demo](https://github.com/amisadmin/fastapi-amis-admin-demo)

---

*Architect Team*
