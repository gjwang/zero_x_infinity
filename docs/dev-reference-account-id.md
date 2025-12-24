# Developer Reference: Account & ID System

## 📋 Quick Reference

### Account Types

| Type | ID | Storage | Purpose |
|------|------|---------|---------|
| **Funding** | 2 | PostgreSQL `balances_tb` | Deposit/Withdraw/Transfer |
| **Spot** | 1 | UBSCore RAM | Trading (in-memory) |

### ID System

| ID | Type | Range | Note |
|----|------|-------|------|
| `user_id` | u64 | 0-1023: System, 1024+: Users | 0=REVENUE |
| `account_type` | u8 | 1=Spot, 2=Funding | Enum value |
| ~~`account_id`~~ | - | **REMOVED** | Use `(user_id, account_type)` tuple |

---

## ⚠️ Important Notes

### 1. No Composite Account ID
```rust
// ❌ OLD (DO NOT USE)
account_id = (user_id << 8) | account_type

// ✅ CURRENT
AccountKey { user_id, account_type }
```

### 2. balances_tb is Funding Only
```sql
-- This table is for FUNDING account (account_type=2)
-- Spot balances are in UBSCore memory, NOT in database
SELECT * FROM balances_tb WHERE user_id = 1001;
```

### 3. TDengine Uses Dual TAGs
```sql
-- Partition by (user_id, account_type)
CREATE STABLE balance_events (...) 
TAGS (user_id BIGINT, account_type TINYINT);
```

### 4. System Reserved IDs

| user_id | Purpose |
|---------|---------|
| 0 | REVENUE (platform fee income) |
| 1 | INSURANCE (future) |
| 2-1023 | Reserved |

---

## 📚 Reference Docs

- `docs/src/0x0A-b-id-specification.md` - ID specification
- `docs/src/0x0B-funding.md` - Account architecture
- `docs/src/0x0C-trade-fee.md` - Fee system & TDengine schema

---

## 🔧 TODO (Future)

- [ ] Rename `balances_tb` → `funding_balances_tb` for clarity
- [ ] Add `account_type` column to distinguish in queries

---

## 💡 Future Consideration: System ID Range

**Current**: System IDs use 0-1023 (1024 total), users start at 1024.

**Problem**: Test data might accidentally use 1, 2, 3... which conflicts with system IDs.

**Alternative**: Use `u64::MAX` downward for system accounts:
```rust
const REVENUE_ID: u64 = u64::MAX;        // 18446744073709551615
const INSURANCE_ID: u64 = u64::MAX - 1;  // 18446744073709551614
const SYSTEM_MIN: u64 = u64::MAX - 1000; // Boundary

fn is_system_account(user_id: u64) -> bool {
    user_id > SYSTEM_MIN
}
```

**Benefits**:
- ✅ Users can start from 1, more natural
- ✅ Test data never conflicts with system IDs
- ✅ Clear separation: low = users, high = system
