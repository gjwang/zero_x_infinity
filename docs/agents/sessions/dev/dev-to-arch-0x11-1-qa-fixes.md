# Dev to Arch: Phase 0x11.1 QA Fixes & Security Hardening

| **Milestone** | Phase 0x11.1: QA Fixes & Security Guard |
| :--- | :--- |
| **Status** | 🟢 **RELEASE READY** |
| **From** | Development Team (@Dev) |
| **To** | Technical Architect (@Arch) |
| **Date** | 2025-12-29 |
| **Branch** | `0x11-a-real-chain` |
| **Commit** | `053a9d4` |

## 1. Executive Summary
Following QA feedback (QA-01 to QA-04), we have refined the Funding Service error handling and implemented industry-standard security hardening for the internal mock endpoints.

## 2. Changes Delivered

### 2.1 Error Handling Refinement (QA-02)
- **Problem**: Withdrawal failures (insufficient funds, invalid address) returned `500 Internal Server Error` instead of `400 Bad Request`.
- **Fix**: Refactored `handlers.rs` to use a `match` on specific `WithdrawError` variants, mapping them to:
    - `400 Bad Request` + `INSUFFICIENT_BALANCE` for balance issues.
    - `400 Bad Request` + `INVALID_PARAMETER` for address/amount issues.
    - `500 Internal Server Error` for unexpected database errors.

### 2.2 Mock Endpoint Security (QA-03 + Proactive Hardening)
We implemented a **defense-in-depth** strategy for the `/internal/mock/deposit` endpoint:

| Layer | Mechanism | Status |
|-------|-----------|--------|
| **L1: Compile-Time** | `#[cfg(feature = "mock-api")]` | ✅ Implemented |
| **L2: Runtime** | `X-Internal-Secret` header check | ✅ Already in place |
| **L3: Documentation** | `[SECURITY WARNING]` + `FIXME` tags | ✅ Added |

**Production Build**: `cargo build --release --no-default-features` will **physically exclude** all mock code from the binary.

### 2.3 History API Robustness (QA-01)
- Verified that `get_deposit_history` and `get_withdraw_history` return `200 OK` with empty `[]` when no records exist.
- Mapped `AssetNotFound` errors to `400 Bad Request` instead of `500`.

### 2.4 CI Configuration Protection
- Confirmed `config/ci.yaml` uses port `5432` (standard CI PostgreSQL).
- `config/dev.yaml` uses port `5433` (local Docker mapping).

## 3. Verification Status
All automated tests passed:
```
✅ Agent B (Core Flow): PASS
✅ Agent A (Idempotency/Chaos): PASS
✅ Agent C (Security): PASS
```

## 4. Architectural Recommendations (For Your Review)

### 4.1 Future Cleanup Plan
Once the **Sentinel (Real Chain Scanner)** is fully stable in Phase 0x11-a:
1. Remove `mock-api` from `default` features in `Cargo.toml`.
2. Physically delete `mock_deposit` handler and related routes.
3. Update CI scripts to use real chain simulation instead of mock injection.

### 4.2 Production Checklist Item
Add to deployment runbook:
```bash
# Production build MUST exclude mock endpoints
cargo build --release --no-default-features
```

## 5. Files Modified
- `src/funding/handlers.rs` - Error mapping + `#[cfg(feature = "mock-api")]`
- `src/gateway/mod.rs` - Conditional route registration
- `Cargo.toml` - Added `mock-api` feature flag
- `docs/src/0x11-deposit-withdraw.md` - Security warnings (EN/CN)

Ready for Architectural Review and Merge to `main`.
