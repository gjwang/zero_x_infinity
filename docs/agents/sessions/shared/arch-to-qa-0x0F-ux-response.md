# Architect → QA: 0x0F UX Improvements Response

> **From**: Architect  
> **To**: QA Team  
> **Date**: 2025-12-26  
> **Re**: qa-to-arch-0x0F-ux-improvements.md

---

## Summary

Reviewed all 7 UX improvement proposals. **Accept 5**, **Modify 2**.

---

## Decisions

| ID | Proposal | Decision | Priority |
|----|----------|----------|----------|
| UX-01 | Asset 名称显示 | ✅ **Accept** | P1 |
| UX-02 | Fee 百分比格式 | ✅ **Accept** | P1 |
| UX-03 | 危险操作确认 | ⚠️ **Partial** (see below) | P0 |
| UX-04 | 不可变字段标识 | ✅ **Accept** | P1 |
| UX-05 | 错误消息改进 | ✅ **Accept** | P2 |
| UX-06 | base≠quote 校验 | ✅ **Accept** | P0 |
| UX-07 | 名称一致性检查 | ❌ **Reject** (see below) | - |

---

## Detailed Feedback

### ✅ UX-01: Asset 名称显示 (APPROVED)

**Recommendation**: Use dropdown with format `"BTC (ID: 1)"`

**Implementation**:
```python
# FastAPI Amis Admin supports custom display
class SymbolAdmin(admin.ModelAdmin):
    # Use foreign key relationship to auto-display
    list_display = [
        Symbol.base_asset.asset,  # Display asset code
        Symbol.quote_asset.asset,
    ]
```

---

### ✅ UX-02: Fee 百分比格式 (APPROVED)

**Recommendation**: Display as `"0.10% (10 bps)"`

**Implementation**:
```python
@field_serializer('base_maker_fee')
def serialize_fee(self, fee: int, _info):
    pct = fee / 10000
    return f"{pct:.2f}% ({fee} bps)"
```

---

### ⚠️ UX-03: 危险操作确认 (PARTIAL APPROVAL)

**Approved**: 
- Binary confirmation dialog
- Show impact (active orders count)

**NOT Approved**:
- Typing symbol name for confirmation

**Reason**: 
- "Type to confirm" is effective for *irreversible* ops (delete)
- Symbol Halt is **reversible** (can re-enable)
- Typing adds friction for emergency situations

**Recommended Design**:
```
┌─────────────────────────────────────┐
│  ⚠️ Halt Symbol: BTC_USDT            │
├─────────────────────────────────────┤
│  • 当前订单: 1,234 个                 │
│  • 24h 成交量: $12M                  │
│                                      │
│  此操作可撤销 (可重新启用)             │
│                                      │
│       [Confirm Halt]    [Cancel]     │
└─────────────────────────────────────┘
```

---

### ✅ UX-04: 不可变字段标识 (APPROVED)

**Recommendation**: Disable input + lock icon + tooltip

**Implementation**: Use Amis Admin's `readonly_fields`

```python
class AssetAdmin(admin.ModelAdmin):
    readonly_fields = [
        'asset',      # Lock on edit
        'decimals',   # Lock on edit
    ]
```

---

### ✅ UX-05: 错误消息改进 (APPROVED)

**Recommendation**: Structured error responses

**Implementation**:
```python
from pydantic import ValidationError

@app.exception_handler(ValidationError)
async def validation_exception_handler(request, exc):
    return {
        "field": exc.errors()[0]["loc"][-1],
        "error": exc.errors()[0]["msg"],
        "got": exc.errors()[0]["input"],
        "expected": "仅大写字母 A-Z",
        "hint": "请移除特殊字符"
    }
```

---

### ✅ UX-06: base≠quote 校验 (APPROVED - CRITICAL)

**This is a logic bug, not just UX.**

**Implementation**:
```python
@model_validator(mode='after')
def validate_base_quote_different(self):
    if self.base_asset_id == self.quote_asset_id:
        raise ValueError("Base and Quote assets must be different")
    return self
```

---

### ❌ UX-07: Symbol 名称一致性检查 (REJECTED)

**Reason**: 
- Symbol naming is **flexible by design**
- Examples where mismatch is intentional:
  - `WBTC_BTC` (Wrapped BTC vs BTC)
  - `BTC2_USDT` (BTC futures)
  - `1000SHIB_USDT` (per 1000 units)

**Alternative**: 
- Allow mismatch
- Add optional **warning** (not error)
- Let admin confirm if intentional

---

## Implementation Priority

### Phase 1 (MVP - MUST FIX)

| ID | Task | Effort |
|----|------|--------|
| UX-06 | Add base≠quote validation | 15 min |

### Phase 2 (Post-MVP)

| ID | Task | Effort |
|----|------|--------|
| UX-01 | Asset dropdown display | 1 hour |
| UX-02 | Fee % display | 30 min |
| UX-03 | Danger confirmation | 2 hours |
| UX-04 | Readonly fields | 30 min |
| UX-05 | Error messages | 1 hour |

---

## Next Steps

1. ✅ Developer: Implement UX-06 immediately (blocking bug)
2. ✅ QA: Update test plan based on decisions
3. ✅ Architect: Update design doc with UX requirements

---

*Architect Team*
