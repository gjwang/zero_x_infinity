# 0x0F Admin Dashboard - QA Handover

> **From**: Architect  
> **To**: QA  
> **Date**: 2025-12-26  
> **Branch**: `0x0F-admin-dashboard`

---

## Task Summary

验证 Admin Dashboard MVP 功能：Asset/Symbol/VIP 配置管理。

## Test Scope

| 模块 | 测试重点 |
|------|----------|
| 登录 | 正确密码登录成功，错误密码拒绝 |
| Asset | CRUD + Enable/Disable |
| Symbol | CRUD + Trading/Halt |
| VIP Level | CRUD + 默认值 |
| Audit Log | 所有操作有日志 (AdminID, IP, Action) |
| 输入验证 | 非法输入拒绝 |
| 热加载 | 配置变更无需重启 Gateway |
| **Decimal 精度** | 费率 API 返回 String，非 float |

## Acceptance Criteria

| ID | Criteria | Verify |
|----|----------|--------|
| AC-01 | Admin 可登录 | 浏览器访问 |
| AC-02 | 可新增 Asset | UI + DB |
| AC-03 | 可编辑 Asset | UI + DB |
| AC-04 | Gateway 热加载 Asset | 无需重启 |
| AC-05 | 可新增 Symbol | UI + DB |
| AC-06 | 可编辑 Symbol | UI + DB |
| AC-07 | Gateway 热加载 Symbol | 无需重启 |
| AC-08 | 可新增/编辑 VIP Level | UI + DB |
| AC-09 | 非法输入拒绝 | 边界测试 |
| AC-10 | VIP 默认 Normal | 初始化数据 |
| AC-11 | Asset Enable/Disable | 禁用后 Gateway 拒绝 |
| AC-12 | Symbol Halt | 暂停后拒绝新订单 |
| AC-13 | 操作日志记录 | 可查询 |

---

## 🎯 NEW: P0 UX Test Requirements

### UX-07: ID Auto-Generation

**Test Cases**:
- [ ] TC-UX-07-01: Creating Asset does NOT require `asset_id` input
- [ ] TC-UX-07-02: Creating Symbol does NOT require `symbol_id` input
- [ ] TC-UX-07-03: After creation, `asset_id` is auto-assigned (DB SERIAL)
- [ ] TC-UX-07-04: After creation, `symbol_id` is auto-assigned (DB SERIAL)
- [ ] TC-UX-07-05: IDs are sequential (1, 2, 3, ...)

### UX-08: Status/Flags String Display

**Test Cases**:
- [ ] TC-UX-08-01: Asset status displays as `"Disabled"` (not `0`)
- [ ] TC-UX-08-02: Asset status displays as `"Active"` (not `1`)
- [ ] TC-UX-08-03: Symbol status displays as `"Offline"` (not `0`)
- [ ] TC-UX-08-04: Symbol status displays as `"Online"` (not `1`)
- [ ] TC-UX-08-05: Symbol status displays as `"Close-Only"` (not `2`)
- [ ] TC-UX-08-06: Asset flags display as checkboxes/labels (not `23`)

---

## Test Cases (建议)

### 输入验证

| Case | Input | Expected |
|------|-------|----------|
| Invalid decimals | -1 | 拒绝 |
| Invalid fee | 101% | 拒绝 |
| Duplicate symbol | BTC_USDT 重复 | 拒绝 |
| Non-existent asset | base=XYZ | 拒绝 |

### 状态变更

| Case | Action | Expected |
|------|--------|----------|
| Disable Asset | status=0 | Gateway 拒绝相关操作 |
| Halt Symbol | status=0 | Gateway 拒绝新订单 |

## E2E Tests

```bash
pytest admin/tests/ -v
```

| 脚本 | 功能 |
|------|------|
| `test_admin_login.py` | 登录/登出 |
| `test_asset_crud.py` | Asset 增删改查 |
| `test_symbol_crud.py` | Symbol 增删改查 |
| `test_input_validation.py` | 非法输入拒绝 |
| `test_hot_reload.py` | Gateway 热加载 |

## Reference

- [Design Doc](file:///docs/src/0x0F-admin-dashboard.md)

---

*Architect Team*
