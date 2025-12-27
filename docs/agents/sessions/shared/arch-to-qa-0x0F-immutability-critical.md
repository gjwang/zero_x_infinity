# 🚨 CRITICAL QA Notice: ID Immutability Test Requirements

> **From**: Architect  
> **To**: QA  
> **Date**: 2025-12-26  
> **Priority**: 🔴 CRITICAL

---

## Summary

Per `docs/src/standards/id-specification.md`, certain fields are **IMMUTABLE** after creation.
This MUST have **100% test coverage**.

---

## 🔒 Immutable Fields

### Asset

| Field | Create | Update | Reason |
|-------|--------|--------|--------|
| `asset` | ✅ | ❌ BLOCKED | Asset code cannot change |
| `decimals` | ✅ | ❌ BLOCKED | Precision cannot change (breaks balances) |
| `name` | ✅ | ✅ | Display name can change |
| `status` | ✅ | ✅ | Enable/disable allowed |
| `asset_flags` | ✅ | ✅ | Flags can change |

### Symbol

| Field | Create | Update | Reason |
|-------|--------|--------|--------|
| `symbol` | ✅ | ❌ BLOCKED | Trading pair name cannot change |
| `base_asset_id` | ✅ | ❌ BLOCKED | Base asset cannot change |
| `quote_asset_id` | ✅ | ❌ BLOCKED | Quote asset cannot change |
| `price_decimals` | ✅ | ❌ BLOCKED | Precision cannot change (breaks orders) |
| `qty_decimals` | ✅ | ❌ BLOCKED | Precision cannot change (breaks orders) |
| `min_qty` | ✅ | ✅ | Can adjust minimum |
| `status` | ✅ | ✅ | Trading status can change |
| `symbol_flags` | ✅ | ✅ | Flags can change |
| `base_maker_fee` | ✅ | ✅ | Fee can change |
| `base_taker_fee` | ✅ | ✅ | Fee can change |

---

## 🧪 Required Test Cases

### TC-IMMUTABLE-01: Asset Code Cannot Be Changed

```python
def test_asset_code_immutable():
    """Attempt to change asset code should fail"""
    # Create asset with code "BTC"
    # Try to update asset code to "BITCOIN"
    # EXPECT: Update rejected or field not exposed in update form
```

### TC-IMMUTABLE-02: Asset Decimals Cannot Be Changed

```python
def test_asset_decimals_immutable():
    """Attempt to change asset decimals should fail"""
    # Create asset with decimals=8
    # Try to update decimals to 6
    # EXPECT: Update rejected or field not exposed in update form
```

### TC-IMMUTABLE-03: Symbol Name Cannot Be Changed

```python
def test_symbol_name_immutable():
    """Attempt to change symbol name should fail"""
    # Create symbol "BTC_USDT"
    # Try to update to "BITCOIN_USDT"
    # EXPECT: Update rejected or field not exposed in update form
```

### TC-IMMUTABLE-04: Symbol Base Asset Cannot Be Changed

```python
def test_symbol_base_asset_immutable():
    """Attempt to change base_asset_id should fail"""
    # Create symbol with base_asset_id=1 (BTC)
    # Try to update base_asset_id to 3 (ETH)
    # EXPECT: Update rejected or field not exposed in update form
```

### TC-IMMUTABLE-05: Symbol Quote Asset Cannot Be Changed

```python
def test_symbol_quote_asset_immutable():
    """Attempt to change quote_asset_id should fail"""
    # Same as above for quote_asset_id
```

### TC-IMMUTABLE-06: Symbol Decimals Cannot Be Changed

```python
def test_symbol_decimals_immutable():
    """Attempt to change price_decimals or qty_decimals should fail"""
```

---

## Implementation Status

- ✅ `AssetUpdateSchema` - Only exposes: name, status, asset_flags
- ✅ `SymbolUpdateSchema` - Only exposes: min_qty, status, symbol_flags, fees

---

## Reference

- [ID Specification](file:///docs/src/standards/id-specification.md)
- Commit: See `admin/admin/asset.py` and `admin/admin/symbol.py`

---

*Architect Team*
