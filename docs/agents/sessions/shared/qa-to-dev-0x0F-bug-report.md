# QA → Developer: 0x0F Bug Report

> **From**: QA Team (Agent Leader)  
> **To**: Developer + Architect  
> **Date**: 2025-12-26 (v3 - ID Spec Compliance)  
> **Priority**: 🔴 P0 / 🟡 P1 / 🟠 架构确认  
> **Branch**: `0x0F-admin-dashboard`

---

## 📊 Test Execution Summary

| Category | Tests | Passed | Failed | Skipped |
|----------|-------|--------|--------|---------|
| Input Validation | 26 | 26 | 0 | 0 |
| Immutability | 22 | 22 | 0 | 0 |
| ID Mapping | 17 | 17 | 0 | 0 |
| **ID Spec Compliance** | **17** | **12** | **5** | 0 |
| Edge Cases | 18 | 17 | 1 | 0 |
| Core Flow | 15 | 15 | 0 | 0 |
| Constraints | 11 | 10 | 1 | 0 |
| Security | 12 | 4 | 6 | 2 |
| Integration | 14 | 0 | 0 | 14 |
| Login | 3 | 1 | 2 | 0 |
| **Total** | **160** | **131** | **15** | **14** |

---

## 🐛 BUG-01: Asset Name 无长度限制 [P0]

**TC-EDGE-15**: Asset name overflow

### 复现步骤

```python
from admin.asset import AssetCreateSchema

long_name = "A" * 1000
schema = AssetCreateSchema(
    asset="BTC",
    name=long_name,  # 1000 chars accepted!
    decimals=8,
)
print(len(schema.name))  # → 1000
```

### 预期行为

- Name 长度应限制在 256 字符以内
- 超长 name 应抛出 `ValidationError`

### 实际行为

- 1000 字符被接受，无验证错误

### 文件位置

`admin/admin/asset.py` - `AssetCreateSchema.name` 缺少长度验证

### 建议修复

```python
@field_validator("name")
@classmethod
def validate_name(cls, v: str) -> str:
    if len(v) > 256:
        raise ValueError("Name must be 256 characters or less")
    return v
```

---

## 🐛 BUG-02: AuditLogAdmin 未设置 readonly [P0]

**TC-AUDIT-05, TC-AUDIT-06**: Audit log should be append-only

### 问题

`AuditLogAdmin` 类缺少以下安全配置:
- `enable_bulk_delete = False`
- `readonly = True`

### 预期行为

- 审计日志应为只读，不允许删除或修改
- `DELETE /admin/audit_log/*` 应返回 403

### 文件位置

`admin/admin/audit_log.py` - 需要添加 readonly 配置

---

## 🐛 BUG-03: Password 模块导入失败 [P1]

**TC-AUTH-***: Password validation tests

### 问题

```python
from admin.auth.password import validate_password_strength
# ImportError: cannot import name 'validate_password_strength'
```

### 预期

`admin/auth/password.py` 应包含:
- `validate_password_strength(password: str) -> bool`
- `hash_password(password: str) -> str`
- `verify_password(password: str, hashed: str) -> bool`

### 状态

**BLOCKED** - 需要 Developer 先实现该模块

---

## 🐛 BUG-04: Settings 缺少 Session 配置 [P1]

**TC-AUTH-07**: Session expiry values

### 问题

`admin.settings.settings` 对象缺少以下属性:
- `ACCESS_TOKEN_EXPIRE_MINUTES`
- `REFRESH_TOKEN_EXPIRE_HOURS`
- `IDLE_TIMEOUT_MINUTES`

### 预期值 (per GAP-05)

| Property | Value |
|----------|-------|
| ACCESS_TOKEN_EXPIRE_MINUTES | 15 |
| REFRESH_TOKEN_EXPIRE_HOURS | 24 |
| IDLE_TIMEOUT_MINUTES | 30 |

---

## 🐛 BUG-05: SECRET_KEY 长度不足 [P1]

**TC-DATA-03**: JWT secret security

### 问题

当前 `ADMIN_SECRET_KEY` 默认值:
```python
ADMIN_SECRET_KEY = "change-me-in-production-0x0F"  # 28 chars
```

### 预期

- SECRET_KEY 至少 32 字符
- 不能是明显的默认值

---

## 🐛 BUG-06: Admin 页面返回 404 [P1]

**TC-CORE-01**: Admin login page

### 复现

```bash
curl http://localhost:8001/admin/
# 404 Not Found
```

### 预期

- `/admin/` 应重定向至登录页或返回 302
- `/admin/auth/form/login` 应返回 200

### 备注

可能是路由未正确配置

---

## 🐛 BUG-07: Symbol 允许 base_asset_id = quote_asset_id [P0]

**TC-NEW-01**: Self-referential constraint

### 复现步骤

```python
from admin.symbol import SymbolCreateSchema

schema = SymbolCreateSchema(
    symbol="BTC_BTC",
    base_asset_id=1,
    quote_asset_id=1,  # Same as base!
    price_decimals=2,
    qty_decimals=8,
)
# No ValidationError raised!
```

### 预期行为

- `base_asset_id != quote_asset_id` 应该被校验
- BTC_BTC 这种自引用 Symbol 应该被拒绝

### 实际行为

- 接受 base_asset_id = quote_asset_id

### 文件位置

`admin/admin/symbol.py` - `SymbolCreateSchema` 缺少自引用校验

### 建议修复

```python
@model_validator(mode='after')
def validate_base_not_equal_quote(self):
    if self.base_asset_id == self.quote_asset_id:
        raise ValueError("base_asset_id cannot equal quote_asset_id")
    return self
```

---

## 🐛 BUG-08: Asset 正则过于严格 - 不允许数字 [P0]

**TC-SPEC-01~03**: ID Specification Compliance

### 问题

Per `id-specification.md`:
- 规范正则: `^[A-Z0-9_]{1,16}$`
- 当前实现: `^[A-Z]+$` ❌

### 复现

```python
AssetCreateSchema(asset="BTC2", ...)      # ❌ Rejected (should be valid)
AssetCreateSchema(asset="1INCH", ...)     # ❌ Rejected (should be valid)
AssetCreateSchema(asset="STABLE_COIN",..) # ❌ Rejected (should be valid)
```

### 预期

按规范，以下应该合法：
- `BTC2` (数字)
- `1INCH` (数字开头)
- `STABLE_COIN` (下划线)

### 建议修复

```python
# admin/admin/asset.py
@field_validator("asset")
def validate_asset(cls, v: str) -> str:
    v = v.upper()
    if not re.match(r"^[A-Z0-9_]{1,16}$", v):  # 改为规范的正则
        raise ValueError("Asset must contain only A-Z, 0-9, underscore")
    return v
```

---

## 🐛 BUG-09: Symbol 正则过于严格 - 不允许数字 [P0]

**TC-SPEC-04~05**: ID Specification Compliance

### 问题

Per `id-specification.md`:
- 规范正则: `^[A-Z0-9]+_[A-Z0-9]+$`
- 当前实现: `^[A-Z]+_[A-Z]+$` ❌

### 复现

```python
SymbolCreateSchema(symbol="ETH2_USDT", ...)     # ❌ Rejected
SymbolCreateSchema(symbol="1000SHIB_USDT", ...) # ❌ Rejected
```

### 预期

按规范，以下应该合法：
- `ETH2_USDT`
- `1000SHIB_USDT`

### 建议修复

```python
# admin/admin/symbol.py
@field_validator("symbol")
def validate_symbol(cls, v: str) -> str:
    v = v.upper()
    if not re.match(r"^[A-Z0-9]+_[A-Z0-9]+$", v):  # 改为规范的正则
        raise ValueError("Symbol must be in format BASE_QUOTE")
    return v
```

---

## ✅ 通过的关键测试

| Test | Status |
|------|--------|
| TC-IMMUTABLE-01~06 | ✅ 6/6 全部通过 |
| TC-EDGE-01~13 | ✅ 13/13 输入边界验证 |
| TC-STATE-01~06 | ✅ 6/6 状态机测试 |
| TC-CORE-13~14 | ✅ CloseOnly 状态转换 |
| TC-PREC-01~03 | ✅ 精度测试 |
| **TC-ID-01~17** | ✅ **17/17 ID 映射测试全部通过** |

---

## 📋 Action Items for Developer

### P0 (Blocker - 必须修复)

- [ ] BUG-01: 添加 Asset name 长度验证
- [ ] BUG-02: 设置 AuditLogAdmin readonly=True
- [ ] BUG-07: 添加 base_asset_id != quote_asset_id 校验
- [ ] BUG-08: Asset 正则改为 `^[A-Z0-9_]{1,16}$`
- [ ] BUG-09: Symbol 正则改为 `^[A-Z0-9]+_[A-Z0-9]+$`

### P1 (High - 应该修复)

- [ ] BUG-03: 实现 password.py 模块
- [ ] BUG-04: 添加 Session 过期配置
- [ ] BUG-05: 使用更长的默认 SECRET_KEY
- [ ] BUG-06: 修复 Admin 页面路由

---

## 📊 最终测试统计

| Category | Total | Pass | Fail | Skip |
|----------|-------|------|------|------|
| Input Validation | 26 | 26 | 0 | 0 |
| Immutability | 22 | 22 | 0 | 0 |
| ID Mapping | 17 | 17 | 0 | 0 |
| Edge Cases | 18 | 17 | 1 | 0 |
| Core Flow | 15 | 15 | 0 | 0 |
| Constraints | 11 | 10 | 1 | 0 |
| Security | 12 | 4 | 6 | 2 |
| Integration | 14 | 0 | 0 | 14 |
| Login | 3 | 1 | 2 | 0 |
| **Total** | **143** | **119** | **10** | **14** |

---

## 📊 QA 下一步

1. 等待 Developer 修复 P0 bugs
2. 重新运行失败的测试用例
3. 继续执行集成测试 (需要数据库环境)

---

*QA Team (Agent Leader)*  
*Generated: 2025-12-26 (Updated)*

