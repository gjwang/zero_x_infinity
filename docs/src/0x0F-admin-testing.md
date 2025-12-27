# 0x0F Admin Dashboard - Testing Guide

> This document contains detailed test cases and scripts for the Admin Dashboard.
> For architecture overview, see [Admin Dashboard](./0x0F-admin-dashboard.md).

---

## Test Scripts

### One-Click Testing

```bash
# Run all tests (Rust + Admin Unit + E2E)
./scripts/run_admin_full_suite.sh

# Quick mode (skip Unit Tests)
./scripts/run_admin_full_suite.sh --quick

# Run only Admin → Gateway propagation E2E
./scripts/run_admin_gateway_e2e.sh
```

### Script Reference

| Script | Purpose |
|--------|---------|
| `run_admin_full_suite.sh` | Unified entry (Rust + Admin Unit + E2E) |
| `run_admin_gateway_e2e.sh` | Admin → Gateway propagation tests |
| `run_admin_tests_standalone.sh` | One-click full test (install deps + start server) |

### Port Configuration

| Environment | Admin Port | Gateway Port |
|-------------|------------|--------------|
| Dev (local) | 8002 | 8080 |
| CI | 8001 | 8080 |

---

## Test Files

| Script | Function |
|--------|----------|
| `verify_e2e.py` | Admin login/logout, health check |
| `test_admin_login.py` | Authentication tests |
| `test_constraints.py` | Database constraint validation |
| `test_core_flow.py` | Asset/Symbol CRUD workflows |
| `test_input_validation.py` | Invalid input rejection |
| `test_security.py` | Security and authentication |
| `tests/e2e/test_asset_lifecycle.py` | Asset enable/disable lifecycle |
| `tests/e2e/test_symbol_lifecycle.py` | Symbol trading status management |
| `tests/e2e/test_fee_update.py` | Fee configuration updates |
| `tests/e2e/test_audit_log.py` | Audit trail verification |
| `tests/test_ux10_trace_id.py` | UX-10 Trace ID verification |

### Running Individual Tests

```bash
cd admin && source venv/bin/activate
pytest tests/test_core_flow.py -v
pytest tests/e2e/test_asset_lifecycle.py -v
pytest tests/test_ux10_trace_id.py -v
```

---

## Test Coverage

**Total: 198+ tests**
- Rust unit tests: 5 passed
- Admin unit tests: 178+ passed
- Admin E2E tests: 4/4 passed
- UX-10 Trace ID tests: 16/16 passed

---

## UX Requirements Test Matrix

| UX ID | Requirement | Test File |
|-------|-------------|-----------|
| UX-06 | Base ≠ Quote validation | `test_constraints.py` |
| UX-07 | ID Auto-Generation | `test_id_mapping.py` |
| UX-08 | Status String Display | `test_ux08_status_strings.py` |
| UX-09 | Default Descending Sort | `test_core_flow.py` |
| UX-10 | Trace ID Evidence Chain | `test_ux10_trace_id.py` |

---

## Acceptance Criteria

| # | Deliverable | Verification |
|---|-------------|--------------|
| 1 | Admin UI accessible | Browser at `localhost:$ADMIN_PORT` |
| 2 | One-click E2E test | `./scripts/run_admin_full_suite.sh` passes |
| 3 | All tests pass | 198+ tests green |
| 4 | Audit log queryable | Admin UI audit page |
| 5 | Gateway hot-reload | Config change without restart |
