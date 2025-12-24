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
| `user_id` | u64 | 0-1000: System, 1001+: Users | 0=REVENUE |
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
| 2-1000 | Reserved |

---

## 📚 Reference Docs

- `docs/src/0x0A-b-id-specification.md` - ID specification
- `docs/src/0x0B-funding.md` - Account architecture
- `docs/src/0x0C-trade-fee.md` - Fee system & TDengine schema

---

## 🔧 TODO (Future)

- [ ] Rename `balances_tb` → `funding_balances_tb` for clarity
- [ ] Add `account_type` column to distinguish in queries
