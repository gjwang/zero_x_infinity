# Architect → QA: 0x0F Clarification Response

> **From**: Architect  
> **To**: QA  
> **Date**: 2025-12-26  
> **Re**: qa-to-arch-0x0F-clarification.md

---

## 📋 Decisions

| GAP | Decision | Rationale |
|-----|----------|-----------|
| GAP-01 | **Option 3: Close-Only** | Users can cancel orders but cannot place new ones. Industry standard. |
| GAP-02 | **Option 1: Reject** | Data integrity first. Prevent accidental cascade failures. |
| GAP-03 | **Option 1: 5 seconds** | Fast response needed for emergency symbol halt. |
| GAP-04 | **QA Recommended** | Security compliance standard |
| GAP-05 | **QA Recommended** | Security compliance standard |
| GAP-06 | **Option 1: Reject** | Integer bps only. Simplifies logic, avoids float precision issues. |

---

## GAP-01: Symbol Halt Order Handling

**Decision**: Close-Only mode

- Users retain control to cancel their orders
- No forced cancellation avoids customer complaints
- Future: Can extend to per-symbol configurable setting

**Implementation**:
```rust
enum SymbolStatus {
    Trading = 1,     // Normal
    CloseOnly = 2,   // Cancel allowed, new orders rejected
    Halt = 0,        // All operations rejected (maintenance)
}
```

---

## GAP-02: Asset Cascade Behavior

**Decision**: Reject if referenced

- If any Symbol references the Asset, reject disable/delete
- Operator must manually halt/remove dependent Symbols first
- Prevents accidental production impact

**Error Message**:
```json
{
  "error": "ASSET_IN_USE",
  "message": "Cannot disable asset BTC: referenced by symbols [BTC_USDT, BTC_ETH]"
}
```

---

## GAP-03: Hot Reload SLA

**Decision**: 5 seconds max

- Use config_watcher with broadcast channel
- Gateway polls or receives push notification
- Critical for emergency symbol halt scenarios

---

## GAP-04: Password Policy

**Decision**: Accept all QA recommendations

| Property | Value |
|----------|-------|
| Minimum length | 12 |
| Require uppercase | Yes |
| Require number | Yes |
| Require special char | Yes |
| Maximum age | 90 days |
| History | 3 previous |

---

## GAP-05: Session Expiry

**Decision**: Accept all QA recommendations

| Property | Value |
|----------|-------|
| Access token expiry | 15 min |
| Refresh token expiry | 24 hours |
| Idle timeout | 30 min |
| Force re-auth for sensitive ops | Yes |

**Sensitive Operations** (require re-auth):
- Asset disable
- Symbol halt
- VIP level modification

---

## GAP-06: Fee Precision

**Decision**: Integer bps only

- Accept only whole number bps (0, 1, 2, ... 10000)
- Reject fractional bps like 0.5
- Simplifies frontend and backend precision handling

**Validation Rule**:
```python
def validate_fee_bps(value: int) -> bool:
    return 0 <= value <= 10000  # Integer only
```

---

## Next Steps

1. ✅ QA updates test plan with these constraints
2. ✅ Developer implements per these decisions
3. ✅ QA verifies edge cases based on decisions

---

*Architect Team*
