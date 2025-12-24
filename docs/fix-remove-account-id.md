# Fix: Remove `account_id` Concept from Codebase

## Background

Documentation has been updated to remove the composite `account_id` concept. We now use `(user_id, account_type)` tuple to identify accounts.

## Changes Required

### 1. Check if `account_id` exists in code

```bash
grep -r "account_id" src/
```

### 2. If found, replace with `(user_id, account_type)` tuple

**Before**:
```rust
pub struct Account {
    pub account_id: u64,  // REMOVE THIS
    pub user_id: u64,
    pub account_type: AccountType,
}
```

**After**:
```rust
pub struct Account {
    pub user_id: u64,
    pub account_type: AccountType,
}
```

### 3. Update HashMap keys

**Before**:
```rust
HashMap<u64, Balance>  // keyed by account_id
```

**After**:
```rust
HashMap<(u64, AccountType), Balance>  // keyed by (user_id, account_type)
```

## Reference

- `docs/src/0x0A-b-id-specification.md` - ID specification
- `docs/src/0x0C-trade-fee.md` - TDengine schema with dual TAGs
