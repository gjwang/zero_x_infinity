# Architect → QA Handover: Phase 0x14-c Money Safety

> **Branch**: `0x14-c-money-safety`
> **Design Spec**: [docs/src/0x14-c-money-safety.md](../../src/0x14-c-money-safety.md)
> **Dev Handover**: [0x14-c-money-safety-handover.md](./0x14-c-money-safety-handover.md)
> **Date**: 2025-12-31
> **Architect**: Arch-Agent

---

## 1. Phase Overview

本阶段实现 **Money Type Safety** 的全面落地：

| 任务 | 目标 |
|------|------|
| CI 审计脚本 | 检测并阻止 `10u64.pow` 在白名单外使用 |
| Gateway 改造 | 订单 API 使用类型安全的金额解析 |
| 存量代码迁移 | 消除 6 个文件中的手工缩放 |

---

## 2. Test Scope (验收范围)

### 2.1 CI 审计验证

```bash
# 脚本必须存在且可执行
ls -la scripts/audit_money_safety.sh

# 脚本必须通过
./scripts/audit_money_safety.sh
```

**预期结果**: `✅ Money safety audit passed!`

### 2.2 单元测试

```bash
cargo test money::
```

**预期结果**: 所有 money 模块测试通过

### 2.3 API 层验证

测试 Gateway 订单接口的金额验证：

| Test Case | Input | Expected Response |
|-----------|-------|-------------------|
| 有效数量 | `"quantity": "1.5"` | 200 OK |
| 精度超限 | `"quantity": "1.123456789"` | 400 PRECISION_EXCEEDED |
| 零值数量 | `"quantity": "0"` | 400 ZERO_NOT_ALLOWED |
| 负数 | `"quantity": "-1.0"` | 400 INVALID_AMOUNT |
| 溢出 | `"quantity": "999...999"` | 400 AMOUNT_OVERFLOW |
| 格式错误 | `"quantity": ".5"` | 400 INVALID_FORMAT |

### 2.4 回归测试

```bash
# 全量测试必须通过
cargo test
```

**预期结果**: 370+ 测试全部通过

---

## 3. Test Checklist

| ID | Test Case | Status |
|----|-----------|--------|
| TC-001 | `audit_money_safety.sh` 脚本存在 | ⬜ |
| TC-002 | `audit_money_safety.sh` 执行通过 | ⬜ |
| TC-003 | CI workflow 包含审计步骤 | ⬜ |
| TC-004 | `cargo test money::` 全绿 | ⬜ |
| TC-005 | Gateway 精度超限返回正确错误 | ⬜ |
| TC-006 | Gateway 零值返回正确错误 | ⬜ |
| TC-007 | Gateway 负数返回正确错误 | ⬜ |
| TC-008 | 全量 `cargo test` 通过 | ⬜ |
| TC-009 | 无 `10u64.pow` 在白名单外 | ⬜ |

---

## 4. Acceptance Criteria

- [ ] 所有 TC-001 ~ TC-009 通过
- [ ] 无新增 P0/P1 Defects
- [ ] CI Pipeline 全绿

---

## 5. Notes

- **白名单文件**: `money.rs`, `symbol_manager.rs`
- **设计文档**: 详见 [0x14-c-money-safety.md](../../src/0x14-c-money-safety.md)
- **规范文档**: 详见 [money-type-safety.md](../../standards/money-type-safety.md)
