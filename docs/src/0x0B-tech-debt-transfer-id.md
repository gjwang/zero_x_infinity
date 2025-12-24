# Tech Debt Fix: Consolidate transfer_id and req_id

**Branch**: `0x0C-trade-fee` (fix before main work)  
**Priority**: P1

---

## Current Problem

```sql
CREATE TABLE fsm_transfers_tb (
    transfer_id   BIGSERIAL PRIMARY KEY,     -- ❌ Redundant
    req_id        VARCHAR(26) UNIQUE NOT NULL, -- ULID (unique)
    ...
);
```

Two IDs for the same entity:
- `transfer_id`: Auto-increment, used as PK
- `req_id` (InternalTransferId): ULID, already unique

---

## Required Fix

Use ULID (`InternalTransferId`) directly as the primary key:

```sql
CREATE TABLE fsm_transfers_tb (
    transfer_id   VARCHAR(26) PRIMARY KEY,   -- ULID, was req_id
    ...
);
```

### Code Changes

| Location | Change |
|----------|--------|
| `src/transfer/types.rs` | Remove `transfer_id: Option<i64>` from `TransferRecord` |
| `src/transfer/db.rs` | Use `InternalTransferId` as PK |
| `migrations/*.sql` | Update schema |
| `src/transfer/api.rs` | Simplify response (use `transfer_id` only) |
| Documentation | Update `0x0B-a-transfer.md` schema section |

### Naming Convention

**Before** (confusing):
- `req_id` in code
- `transfer_id` in DB

**After** (consistent):
- `transfer_id` everywhere (code + DB)
- Type: `InternalTransferId`

---

## Acceptance Criteria

- [ ] Single `transfer_id` column (VARCHAR(26), ULID)
- [ ] Code uses `transfer_id` (not `req_id`)
- [ ] All tests pass
- [ ] Documentation updated
