# 0x0F Admin Dashboard - QA Test Plan

> **From**: QA Team (Multi-Agent Review)  
> **To**: Architect / Developer  
> **Date**: 2025-12-26  
> **Status**: ✅ Ready for Testing (Architect Clarified)  
> **Branch**: `0x0F-admin-dashboard`

---

## 🎯 Goal

验证 Admin Dashboard MVP 功能的正确性、安全性和稳定性。

**Definition of DONE**:
- All P0 tests pass
- All P1 tests pass (or documented exceptions)
- Design gaps clarified
- No regression in existing CI

---

## 📊 Executive Summary

| Review Agent | Findings | Severity |
|--------------|----------|----------|
| 🔴 Agent A (Edge Cases) | 13 gaps | **HIGH** |
| 🔵 Agent B (Core Flow) | 3 concerns | **MEDIUM** |
| 🟣 Agent C (Security) | 4 CRITICAL | **CRITICAL** |

---

## ✅ Architect Decisions (Confirmed)

> [!NOTE]
> 以下决策由 Architect 于 2025-12-26 确认，见 [arch-to-qa-0x0F-clarification-response.md](../shared/arch-to-qa-0x0F-clarification-response.md)

| # | Decision | Spec |
|---|----------|------|
| GAP-01 | **Close-Only Mode** | Symbol Halt 时用户可撤单，不可新建订单 |
| GAP-02 | **Reject if Referenced** | Asset 有 Symbol 引用时拒绝禁用/删除 |
| GAP-03 | **5 Seconds SLA** | 配置变更必须 5 秒内生效 |
| GAP-04 | **Strong Password** | 12+ 字符, 大写+数字+特殊字符, 90天过期, 3次历史 |
| GAP-05 | **Session Expiry** | Access 15min, Refresh 24h, Idle 30min, 敏感操作需重认证 |
| GAP-06 | **Integer bps Only** | 只接受整数 bps (0-10000)，拒绝小数 |

### Symbol Status Enum (GAP-01)

```rust
enum SymbolStatus {
    Halt = 0,       // All ops rejected (maintenance)
    Trading = 1,    // Normal trading
    CloseOnly = 2,  // Cancel allowed, new orders rejected
}
```

### Sensitive Operations Requiring Re-auth (GAP-05)

- Asset disable
- Symbol halt  
- VIP level modification

---

## 📋 Test Categories

### Legend

| Priority | Meaning |
|----------|---------|
| **P0** | Blocker - Must pass before merge |
| **P1** | High - Should pass, exceptions documented |
| **P2** | Medium - Nice to have |

---

## 🔴 Agent A: Edge Case Tests

> *"If it can break, I will break it."*

### Input Boundary Tests

| ID | Field | Input | Expected | Priority |
|----|-------|-------|----------|----------|
| TC-EDGE-01 | decimals | `-1` | Reject (400) | **P0** |
| TC-EDGE-02 | decimals | `18.5` (float) | Reject (400) | **P0** |
| TC-EDGE-03 | decimals | `19` | Reject (>18) | **P0** |
| TC-EDGE-04 | decimals | `"eighteen"` | Reject (type) | **P0** |
| TC-EDGE-05 | fee_rate | `10001` bps | Reject (>100%) | **P0** |
| TC-EDGE-06 | fee_rate | `-1` | Reject (<0) | **P0** |
| TC-EDGE-07 | fee_rate | `10000` bps | Accept (100%) | **P0** |
| TC-EDGE-08 | fee_rate | `0` | Accept (0%) | **P0** |
| TC-EDGE-09 | symbol | `""` (empty) | Reject | **P0** |
| TC-EDGE-10 | symbol | `"btc_usdt"` (lowercase) | Reject | **P1** |
| TC-EDGE-11 | symbol | `"BTC USDT"` (space) | Reject | **P1** |
| TC-EDGE-12 | symbol | `"A"*256` (overflow) | Reject | **P0** |
| TC-EDGE-13 | base_asset | `"NOTEXIST"` | Reject (FK) | **P0** |

### State Machine Tests

| ID | Scenario | Expected | Priority |
|----|----------|----------|----------|
| TC-STATE-01 | Create Symbol with disabled Asset | Reject | **P1** |
| TC-STATE-02 | Delete Asset with existing Symbol | Reject (FK) | **P1** |
| TC-STATE-03 | Delete VIP Level 0 | Reject (reserved) | **P1** |
| TC-STATE-04 | Duplicate Symbol name | Reject (unique) | **P0** |
| TC-STATE-05 | Duplicate Asset name | Reject (unique) | **P0** |

### 🚨 Immutability Tests (CRITICAL)

> [!CAUTION]
> 这些字段创建后**不可变更**，必须 100% 测试覆盖。见 [arch-to-qa-0x0F-immutability-critical.md](../shared/arch-to-qa-0x0F-immutability-critical.md)

| ID | Entity | Field | Action | Expected | Priority |
|----|--------|-------|--------|----------|----------|
| TC-IMMUTABLE-01 | Asset | `asset` (code) | Update BTC→BITCOIN | **BLOCKED** | **P0** 🔴 |
| TC-IMMUTABLE-02 | Asset | `decimals` | Update 8→6 | **BLOCKED** | **P0** 🔴 |
| TC-IMMUTABLE-03 | Symbol | `symbol` | Update BTC_USDT→BITCOIN_USDT | **BLOCKED** | **P0** 🔴 |
| TC-IMMUTABLE-04 | Symbol | `base_asset_id` | Update 1→3 | **BLOCKED** | **P0** 🔴 |
| TC-IMMUTABLE-05 | Symbol | `quote_asset_id` | Update 2→4 | **BLOCKED** | **P0** 🔴 |
| TC-IMMUTABLE-06 | Symbol | `price_decimals`, `qty_decimals` | Update 2→4 | **BLOCKED** | **P0** 🔴 |

**Implementation**: 
- ✅ `AssetUpdateSchema` - 只暴露: name, status, asset_flags
- ✅ `SymbolUpdateSchema` - 只暴露: min_qty, status, symbol_flags, fees

### Concurrency Tests

| ID | Scenario | Expected | Priority |
|----|----------|----------|----------|
| TC-CONC-01 | Admin A + B create same Asset | One fails | **P2** |
| TC-CONC-02 | Admin A disables Asset during Gateway read | Atomic update | **P2** |

### Injection Tests

| ID | Attack | Input | Expected | Priority |
|----|--------|-------|----------|----------|
| TC-INJ-01 | SQL Injection | `'; DROP TABLE assets_tb; --` | Reject/Escape | **P0** |
| TC-INJ-02 | XSS | `<script>alert('xss')</script>` | Escape in UI | **P1** |
| TC-INJ-03 | Null byte | `BTC\x00USDT` | Reject | **P1** |

### Precision Tests

| ID | Scenario | Expected | Priority |
|----|----------|----------|----------|
| TC-PREC-01 | fee_rate `0.01%` stored | 1 bps in DB | **P0** |
| TC-PREC-02 | Decimal → String in API | `"0.001"` not `0.001` | **P0** |
| TC-PREC-03 | 0.1 + 0.2 calculation | No float error | **P0** |

---

## 🔵 Agent B: Core Flow Tests

> *"The happy path must work 100%."*

### Functional Tests

| ID | Flow | Steps | Expected | Priority |
|----|------|-------|----------|----------|
| TC-CORE-01 | Admin Login | Navigate → Enter creds → Submit | Dashboard loads | **P0** |
| TC-CORE-02 | Admin Logout | Click logout | Session destroyed | **P0** |
| TC-CORE-03 | Asset Create | Fill form → Submit | Asset in DB | **P0** |
| TC-CORE-04 | Asset List | Navigate to assets | List displayed | **P0** |
| TC-CORE-05 | Asset Update | Edit → Save | Changes in DB | **P0** |
| TC-CORE-06 | Asset Disable | Toggle status=0 | Asset disabled | **P0** |
| TC-CORE-07 | Symbol Create | Select assets → Set fees → Submit | Symbol in DB | **P0** |
| TC-CORE-08 | Symbol Update | Edit fees → Save | Changes in DB | **P0** |
| TC-CORE-09 | Symbol Halt | Toggle status=0 | Symbol halted | **P0** |
| TC-CORE-10 | VIP Create | Set level, discount → Submit | VIP in DB | **P0** |
| TC-CORE-11 | VIP Update | Edit discount → Save | Changes in DB | **P0** |
| TC-CORE-12 | VIP Default | Check Level 0 | Exists, 100% fee | **P0** |

### Hot Reload Tests

| ID | Scenario | Expected | Priority |
|----|----------|----------|----------|
| TC-HOT-01 | Disable Asset → Gateway behavior | Reject ops on asset | **P0** |
| TC-HOT-02 | Halt Symbol → Gateway behavior | Reject new orders | **P0** |
| TC-HOT-03 | Update fee_rate → Gateway behavior | New rate applied | **P0** |
| TC-HOT-04 | Reload timing | Within SLA (5s) | **P0** |

### Regression Tests

| ID | Check | Baseline | Priority |
|----|-------|----------|----------|
| TC-REG-01 | Gateway order latency | <1ms | **P1** |
| TC-REG-02 | Matching engine throughput | 1.3M/s | **P1** |
| TC-REG-03 | All existing CI tests pass | 100% | **P0** |
| TC-REG-04 | TDengine writes unaffected | Normal | **P1** |

---

## 🟣 Agent C: Security Tests

> *"Trust no one. Verify everything."*

### Authentication Tests

| ID | Scenario | Expected | Priority |
|----|----------|----------|----------|
| TC-AUTH-01 | Wrong password 5x | Rate limit (429) | **P0** |
| TC-AUTH-02 | Invalid JWT | 401 Unauthorized | **P0** |
| TC-AUTH-03 | Expired JWT | 401 Unauthorized | **P0** |
| TC-AUTH-04 | Session timeout | Force re-login | **P1** |
| TC-AUTH-05 | Default password first login | Force change | **P0** |
| TC-AUTH-06 | Password in logs | Never logged | **P0** |

### RBAC Tests

| ID | Role | Action | Expected | Priority |
|----|------|--------|----------|----------|
| TC-RBAC-01 | Auditor | POST /admin/asset | 403 Forbidden | **P1** |
| TC-RBAC-02 | Support | PUT /admin/symbol | 403 Forbidden | **P1** |
| TC-RBAC-03 | Operations | DELETE /admin/audit | 403 Forbidden | **P1** |
| TC-RBAC-04 | Tampered JWT role | Any mutation | 401 Invalid | **P0** |
| TC-RBAC-05 | Super Admin | All operations | 200 OK | **P0** |

### Audit Log Tests

| ID | Scenario | Expected | Priority |
|----|----------|----------|----------|
| TC-AUDIT-01 | Any CRUD operation | Entry in audit log | **P0** |
| TC-AUDIT-02 | Audit log contains admin_id | Correct ID | **P0** |
| TC-AUDIT-03 | Audit log contains IP | Client IP recorded | **P0** |
| TC-AUDIT-04 | Audit log old_value/new_value | Correct JSON | **P1** |
| TC-AUDIT-05 | Audit log deletion attempt | **MUST FAIL** | **P0** |
| TC-AUDIT-06 | Audit log tampering | **MUST FAIL** | **P0** |

### Data Protection Tests

| ID | Check | Expected | Priority |
|----|-------|----------|----------|
| TC-DATA-01 | Passwords hashed | bcrypt/argon2 | **P0** |
| TC-DATA-02 | DB credentials | Env var, not code | **P0** |
| TC-DATA-03 | JWT secret | Env var, not code | **P0** |
| TC-DATA-04 | Error responses | No internal details | **P1** |
| TC-DATA-05 | PII in audit logs | Masked if sensitive | **P1** |

---

## 📁 Recommended Test Structure

```
admin/tests/
├── conftest.py                 # Fixtures (admin user, db session)
├── test_auth/
│   ├── test_login.py           # TC-AUTH-01~03, TC-CORE-01~02
│   ├── test_rate_limit.py      # TC-AUTH-01
│   ├── test_session.py         # TC-AUTH-04~05
│   └── test_password.py        # TC-AUTH-06, TC-DATA-01
├── test_rbac/
│   └── test_role_permissions.py # TC-RBAC-01~05
├── test_crud/
│   ├── test_asset_crud.py      # TC-CORE-03~06, TC-STATE-02
│   ├── test_symbol_crud.py     # TC-CORE-07~09, TC-STATE-01
│   ├── test_vip_crud.py        # TC-CORE-10~12, TC-STATE-03
│   └── test_input_validation.py # TC-EDGE-01~13, TC-INJ-01~03
├── test_integration/
│   ├── test_hot_reload.py      # TC-HOT-01~04
│   └── test_gateway_sync.py    # Hot reload + Gateway
├── test_audit/
│   └── test_audit_log.py       # TC-AUDIT-01~06
├── test_precision/
│   └── test_decimal_string.py  # TC-PREC-01~03
└── test_regression/
    └── test_no_regression.py   # TC-REG-01~04
```

---

## ✅ Acceptance Criteria Mapping

| AC ID | Criteria | Test Cases |
|-------|----------|------------|
| AC-01 | Admin can login | TC-CORE-01, TC-AUTH-* |
| AC-02 | Create Asset | TC-CORE-03 |
| AC-03 | Edit Asset | TC-CORE-05 |
| AC-04 | Gateway hot-reload Asset | TC-HOT-01 |
| AC-05 | Create Symbol | TC-CORE-07 |
| AC-06 | Edit Symbol | TC-CORE-08 |
| AC-07 | Gateway hot-reload Symbol | TC-HOT-02, TC-HOT-03 |
| AC-08 | VIP Level CRUD | TC-CORE-10~12 |
| AC-09 | Invalid input rejected | TC-EDGE-* |
| AC-10 | VIP default Normal | TC-CORE-12 |
| AC-11 | Asset Enable/Disable | TC-CORE-06, TC-HOT-01 |
| AC-12 | Symbol Halt | TC-CORE-09, TC-HOT-02 |
| AC-13 | Audit log | TC-AUDIT-* |

---

## 🧪 Test Execution Plan

### Phase 1: Unit/Integration (Developer)

```bash
cd admin
python -m pytest tests/test_crud/ -v
python -m pytest tests/test_auth/ -v
python -m pytest tests/test_precision/ -v
```

### Phase 2: E2E (QA)

```bash
# Start services
docker-compose up -d postgres tdengine
./scripts/start_gateway.sh
cd admin && uvicorn main:app --port 8001

# Run E2E
python -m pytest tests/test_integration/ -v
python -m pytest tests/test_audit/ -v
```

### Phase 3: Regression (CI)

```bash
# Full CI run
./scripts/pre-commit.sh
cargo test
python -m pytest admin/tests/ -v
```

---

## 📝 Sign-off Checklist

### QA Sign-off Conditions

- [ ] All P0 tests pass (52 tests, incl. 6 Immutability 🔴)
- [ ] All P1 tests pass or exceptions documented (18 tests)
- [x] 6 Design gaps clarified by Architect ✅
- [ ] Security review completed (Agent C)
- [ ] No regression in existing CI
- [ ] Hot reload SLA met (<5s)

### Architect Actions ✅ COMPLETED

- [x] GAP-01: Close-Only mode for Symbol Halt
- [x] GAP-02: Reject if Asset referenced by Symbol
- [x] GAP-03: 5 seconds hot reload SLA
- [x] GAP-04: Strong password policy (12+ chars, complexity)
- [x] GAP-05: Session expiry (15min/24h/30min)
- [x] GAP-06: Integer bps only (reject fractional)

---

## 📊 Test Count Summary

| Category | P0 | P1 | P2 | Total |
|----------|----|----|----|-------|
| Edge Cases | 18 | 7 | 2 | 27 |
| **Immutability** 🔴 | **6** | 0 | 0 | **6** |
| Core Flow | 16 | 4 | 0 | 20 |
| Security | 12 | 7 | 0 | 19 |
| **Total** | **52** | **18** | **2** | **72** |

---

## 🔗 References

- [Design Doc](file:///docs/src/0x0F-admin-dashboard.md)
- [Arch→QA Handover](file:///docs/agents/sessions/qa/0x0F-admin-handover.md)
- [Arch Clarification Response](file:///docs/agents/sessions/shared/arch-to-qa-0x0F-clarification-response.md)
- [🔴 Immutability Critical](file:///docs/agents/sessions/shared/arch-to-qa-0x0F-immutability-critical.md)
- [Migration 007](file:///migrations/007_admin_audit_log.sql)

---

*QA Team (Multi-Agent Review)*  
*Generated: 2025-12-26*
