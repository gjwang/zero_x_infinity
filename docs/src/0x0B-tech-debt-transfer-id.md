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

- [x] Single `transfer_id` column (VARCHAR(26), ULID)
- [x] Code uses `transfer_id` (not `req_id`)
- [x] All tests pass (232 unit + E2E)
- [x] Documentation updated

---

## ✅ Completed (2024-12-24)

**Final Schema** (考虑保留自增 ID 用于 count 估算):
```sql
CREATE TABLE fsm_transfers_tb (
    id            BIGSERIAL PRIMARY KEY,       -- 自增 PK, 可估算 count
    transfer_id   VARCHAR(26) UNIQUE NOT NULL, -- ULID (业务 ID)
    ...
);
```

**Commits**:
| Commit | Description |
|--------|-------------|
| `7bf4386` | refactor(transfer): consolidate transfer_id and req_id |
| `542556c` | chore: remove dead code (1873 lines, 64KB) |
| `d609137` | refactor: rename src/transfer → src/internal_transfer |

**Additional Cleanup**:
- 删除旧的 `src/internal_transfer/` 死代码 (3 files, 64KB)
- 重命名 `src/transfer/` → `src/internal_transfer/` 更清晰
- Migration: `005_consolidate_transfer_id.sql`
