# 0x0F Admin Dashboard - QA Work Assignment

> **From**: Agent Leader (QA 主编)  
> **Date**: 2025-12-26  
> **Status**: 🔶 Awaiting Developer Implementation  
> **Branch**: `0x0F-admin-dashboard`

---

## 📊 Executive Summary

| Agent | Role | Tests | Priority Focus |
|-------|------|-------|----------------|
| 🔴 A | Edge Cases | 28 | Immutability 🚨, Injection, Boundary |
| 🔵 B | Core Flow | 24 | CRUD, Hot Reload, Regression |
| 🟣 C | Security | 20 | Auth, RBAC, Audit |
| **Total** | | **72** | |

---

## 🔴 Agent A (激进派 QA): Edge Case & Immutability

> *"If it can break, I will break it."*

### Mission

破坏性测试 - 找出所有能绕过验证的方法

### 分配的测试用例 (28 个)

| Category | Test IDs | Count |
|----------|----------|-------|
| **Input Boundary** | TC-EDGE-01 ~ TC-EDGE-13 | 13 |
| **🚨 Immutability** | TC-IMMUTABLE-01 ~ TC-IMMUTABLE-06 | 6 |
| **State Machine** | TC-STATE-01 ~ TC-STATE-05 | 5 |
| **Injection** | TC-INJ-01 ~ TC-INJ-03 | 3 |
| **Precision** | TC-PREC-01 ~ TC-PREC-03 | 3 |

### 执行命令

```bash
# 1. 输入边界测试
pytest admin/tests/test_crud/test_input_validation.py -v

# 2. 不可变字段测试 (CRITICAL)
pytest admin/tests/test_crud/test_immutability.py -v

# 3. 注入测试
pytest admin/tests/test_crud/test_injection.py -v

# 4. 精度测试
pytest admin/tests/test_precision/test_decimal_string.py -v
```

### 重点攻击向量

| # | Attack | Expected Result |
|---|--------|-----------------|
| 1 | 修改 Asset.decimals (8→6) | **BLOCKED** |
| 2 | 修改 Symbol.symbol (BTC_USDT→X) | **BLOCKED** |
| 3 | SQL 注入 `'; DROP TABLE` | **拦截** |
| 4 | fee_rate=10001 bps | **拒绝** |
| 5 | 小数 bps (0.5) | **拒绝** (Integer only) |
| 6 | decimals=-1 | **拒绝** |
| 7 | decimals=19 | **拒绝** (max 18) |
| 8 | 空 symbol "" | **拒绝** |

### 报告模板

```markdown
## Agent A Report: Edge Cases

### Summary
- Tests Run: 28
- Passed: ?
- Failed: ?
- Blocked: ?

### Critical Findings
1. [TC-IMMUTABLE-XX]: ...
2. [TC-EDGE-XX]: ...

### Recommendation
- [ ] Ready for production
- [ ] Needs fixes (list issues)
```

---

## 🔵 Agent B (保守派 QA): Core Flow & Regression

> *"The happy path must work 100%."*

### Mission

稳定性测试 - 确保核心流程 100% 可用

### 分配的测试用例 (24 个)

| Category | Test IDs | Count |
|----------|----------|-------|
| **Functional CRUD** | TC-CORE-01 ~ TC-CORE-12 | 12 |
| **Hot Reload** | TC-HOT-01 ~ TC-HOT-04 | 4 |
| **Regression** | TC-REG-01 ~ TC-REG-04 | 4 |
| **Concurrency** | TC-CONC-01 ~ TC-CONC-02 | 2 |

### 执行命令

```bash
# 1. 核心 CRUD 流程
pytest admin/tests/test_crud/test_asset_crud.py -v
pytest admin/tests/test_crud/test_symbol_crud.py -v
pytest admin/tests/test_crud/test_vip_crud.py -v

# 2. 热加载测试
pytest admin/tests/test_integration/test_hot_reload.py -v

# 3. 回归测试
cargo test  # Rust 测试不能回归
./scripts/pre-commit.sh  # CI 完整流程
```

### 重点验证项

| # | Flow | Pass Criteria |
|---|------|---------------|
| 1 | Admin Login/Logout | Session 正常创建/销毁 |
| 2 | Asset CRUD | 创建、列表、更新、禁用 |
| 3 | Symbol CRUD | 创建、更新、Halt |
| 4 | Symbol Halt | 状态变更 + Gateway 5 秒内生效 |
| 5 | VIP Level 0 | 默认存在，100% fee |
| 6 | Gateway 延迟 | 保持 <1ms，不回归 |
| 7 | Hot Reload SLA | 配置变更 ≤5s 生效 |

### 报告模板

```markdown
## Agent B Report: Core Flow

### Summary
- Tests Run: 24
- Passed: ?
- Failed: ?
- Blocked: ?

### Core Flow Status
| Flow | Status |
|------|--------|
| Login | ✅/❌ |
| Asset CRUD | ✅/❌ |
| Symbol CRUD | ✅/❌ |
| VIP CRUD | ✅/❌ |
| Hot Reload | ✅/❌ |

### Regression Check
| Baseline | Before | After |
|----------|--------|-------|
| Gateway Latency | <1ms | ? |
| Throughput | 1.3M/s | ? |

### Recommendation
- [ ] Ready for production
- [ ] Needs fixes (list issues)
```

---

## 🟣 Agent C (安全专家 QA): Security & Audit

> *"Trust no one. Verify everything."*

### Mission

安全测试 - 确保权限和审计无死角

### 分配的测试用例 (20 个)

| Category | Test IDs | Count |
|----------|----------|-------|
| **Authentication** | TC-AUTH-01 ~ TC-AUTH-06 | 6 |
| **RBAC** | TC-RBAC-01 ~ TC-RBAC-05 | 5 |
| **Audit Log** | TC-AUDIT-01 ~ TC-AUDIT-06 | 6 |
| **Data Protection** | TC-DATA-01 ~ TC-DATA-05 | 5 |

### 执行命令

```bash
# 1. 认证测试
pytest admin/tests/test_auth/test_login.py -v
pytest admin/tests/test_auth/test_rate_limit.py -v
pytest admin/tests/test_auth/test_session.py -v

# 2. RBAC 测试
pytest admin/tests/test_rbac/test_role_permissions.py -v

# 3. 审计日志测试
pytest admin/tests/test_audit/test_audit_log.py -v

# 4. 数据保护测试
pytest admin/tests/test_auth/test_password.py -v
```

### 重点验证项

| # | Security Check | Expected |
|---|----------------|----------|
| 1 | 错误密码 5 次 | Rate Limit (429) |
| 2 | Invalid JWT | 401 Unauthorized |
| 3 | Expired JWT | 401 Unauthorized |
| 4 | Auditor → POST /asset | 403 Forbidden |
| 5 | 审计日志删除 | **MUST FAIL** |
| 6 | 密码策略 | 12+ chars, complexity |
| 7 | Session 过期 | Access 15min, Refresh 24h |
| 8 | 敏感操作重认证 | Asset disable, Symbol halt |

### 安全检查清单

- [ ] JWT Secret 在环境变量，不在代码
- [ ] 密码用 bcrypt/argon2 哈希
- [ ] 错误响应不暴露内部细节
- [ ] 敏感操作需重新认证
- [ ] 审计日志 append-only
- [ ] PII 在日志中脱敏

### 报告模板

```markdown
## Agent C Report: Security

### Summary
- Tests Run: 20
- Passed: ?
- Failed: ?
- Blocked: ?

### Security Status
| Area | Status | Notes |
|------|--------|-------|
| Authentication | ✅/❌ | |
| RBAC | ✅/❌ | |
| Audit Log | ✅/❌ | |
| Data Protection | ✅/❌ | |

### Security Findings
1. [CRITICAL]: ...
2. [HIGH]: ...
3. [MEDIUM]: ...

### Recommendation
- [ ] Ready for production
- [ ] Needs security fixes (list CVEs)
```

---

## 👔 Agent Leader: 协调与汇总

### 执行时间表

```
┌────────────────────────────────────────────────────────┐
│  Phase 1: Parallel Execution (All Agents)              │
│  ├── Agent A: Edge + Immutability tests (28)          │
│  ├── Agent B: CRUD + Hot Reload tests (24)            │
│  └── Agent C: Auth + RBAC + Audit tests (20)          │
├────────────────────────────────────────────────────────┤
│  Phase 2: Cross-Validation                             │
│  ├── Agent A reviews B's concurrency findings         │
│  ├── Agent B reviews A's edge case coverage           │
│  └── Agent C reviews ALL for security implications    │
├────────────────────────────────────────────────────────┤
│  Phase 3: Leader Consolidation                         │
│  └── Merge all reports into final QA sign-off         │
└────────────────────────────────────────────────────────┘
```

### Sign-off Criteria

| Condition | Required |
|-----------|----------|
| Agent A: 0 P0 failures | ✅ |
| Agent B: 0 P0 failures | ✅ |
| Agent C: 0 P0 failures | ✅ |
| Cross-validation complete | ✅ |
| No security blockers | ✅ |
| Regression tests pass | ✅ |

### Final Report Template

```markdown
## 0x0F Admin Dashboard - QA Final Report

### Overall Status: [PASS/FAIL/BLOCKED]

### Agent Reports
| Agent | Tests | Passed | Failed | Status |
|-------|-------|--------|--------|--------|
| A (Edge) | 28 | ? | ? | ✅/❌ |
| B (Core) | 24 | ? | ? | ✅/❌ |
| C (Security) | 20 | ? | ? | ✅/❌ |
| **Total** | **72** | ? | ? | |

### Critical Issues
1. ...
2. ...

### Sign-off
- [ ] QA Lead Approved
- [ ] Ready for Merge
```

---

## 🔗 References

- [Test Plan](./0x0F-admin-test-plan.md)
- [Arch Clarification Response](../shared/arch-to-qa-0x0F-clarification-response.md)
- [Immutability Critical](../shared/arch-to-qa-0x0F-immutability-critical.md)

---

*Agent Leader (QA 主编)*  
*Generated: 2025-12-26*
