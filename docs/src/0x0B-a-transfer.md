# 0x0B-a Internal Transfer Architecture (Strict FSM)

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-f-integration-test...v0.0B-a)

---

## 1. Problem Statement

### 1.1 System Topology
| System | Role | Source of Truth | Persistence |
|---|---|---|---|
| **PostgreSQL** | Funding Account | `balances_tb` | ACID, Durable |
| **UBSCore** | Trading Account | RAM | WAL + Volatile |

### 1.2 The Core Constraint
These two systems **cannot share a transaction**. There is no XA/2PC database protocol.
Therefore: **We must build our own 2-Phase Commit using an external FSM Coordinator.**

---

## 1.5 Security Pre-Validation (MANDATORY)

> [!CAUTION]
> **Defense-in-Depth**
> All checks below MUST be performed at **every independent module**, not just API layer.
> - **API Layer**: First line of defense, reject obviously invalid requests
> - **Coordinator**: Re-validate, prevent internal calls bypassing API
> - **Adapters**: Final defense, each adapter must independently validate parameters
> - **UBSCore**: Last check before in-memory operations
>
> **Safety > Performance**. The cost of redundant checks is acceptable; security vulnerabilities are not.

### 1.5.1 Identity & Authorization Checks

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **User Authentication** | Forged request | JWT/Session must be valid | `UNAUTHORIZED` |
| **User ID Consistency** | Cross-user transfer attack | `request.user_id == auth.user_id` | `FORBIDDEN` |
| **Account Ownership** | Steal others' funds | Source/Target accounts belong to same `user_id` | `FORBIDDEN` |

### 1.5.2 Account Type Checks

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **from != to** | Infinite wash trading/resource waste | `request.from != request.to` | `SAME_ACCOUNT` |
| **Account Type Valid** | Inject invalid type | `from, to ∈ {FUNDING, SPOT}` | `INVALID_ACCOUNT_TYPE` |
| **Account Type Supported** | Request unlaunched feature | `from, to` both in supported list | `UNSUPPORTED_ACCOUNT_TYPE` |

### 1.5.3 Amount Checks

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **amount > 0** | Zero/negative transfer | `amount > 0` | `INVALID_AMOUNT` |
| **Precision Check** | Precision overflow | `decimal_places(amount) <= asset.precision` | `PRECISION_OVERFLOW` |
| **Minimum Amount** | Dust attack | `amount >= asset.min_transfer_amount` | `AMOUNT_TOO_SMALL` |
| **Maximum Single Amount** | Risk control bypass | `amount <= asset.max_transfer_amount` | `AMOUNT_TOO_LARGE` |
| **Integer Overflow** | u64 overflow attack | `amount <= u64::MAX / safety_factor` | `OVERFLOW` |

### 1.5.4 Asset Checks

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **Asset Exists** | Fake asset_id | `asset_id` exists in system | `INVALID_ASSET` |
| **Asset Status** | Delisted asset | `asset.status == ACTIVE` | `ASSET_SUSPENDED` |
| **Transfer Permission** | Some assets forbid internal transfer | `asset.internal_transfer_enabled == true` | `TRANSFER_NOT_ALLOWED` |

### 1.5.5 Account Status Checks

#### Account Initialization Rules (Overview)

| Account Type | Init Timing | Notes |
|---|---|---|
| **FUNDING** | Created on first deposit request | Triggered by external deposit flow |
| **SPOT** | Created on first internal transfer | Lazy Init |
| **FUTURE** | Created on first internal transfer [P2] | Lazy Init |
| **MARGIN** | Created on first internal transfer [P2] | Lazy Init |

> [!NOTE]
> - Specific initialization behaviors and business rules for each account type are defined in their dedicated documents.
> - Each account has its own state definitions (e.g., whether transfer is allowed); not detailed here.
> - **Default State**: On account initialization, transfer is allowed by default.

#### Account Status Check Table

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **Source Account Exists** | Non-existent account | Source account record must exist | `SOURCE_ACCOUNT_NOT_FOUND` |
| **Target Account Exists/Create** | Non-existent target | FUNDING must exist; SPOT/FUTURE/MARGIN can create | `TARGET_ACCOUNT_NOT_FOUND` (FUNDING only) |
| **Source Not Frozen** | Frozen account transfer out | `source.status != FROZEN` | `ACCOUNT_FROZEN` |
| **Source Not Disabled** | Disabled account operation | `source.status != DISABLED` | `ACCOUNT_DISABLED` |
| **Sufficient Balance** | Insufficient balance direct reject | `source.available >= amount` | `INSUFFICIENT_BALANCE` |


### 1.5.6 Rate Limiting - **[P2 Future Optimization]**

> [!NOTE]
> This is a V2 optimization. V1 may skip this.

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **Requests Per Second** | DoS attack | `user_requests_per_second <= 10` | `RATE_LIMIT_EXCEEDED` |
| **Daily Transfer Count** | Abuse | `user_daily_transfers <= 100` | `DAILY_LIMIT_EXCEEDED` |
| **Daily Transfer Amount** | Large amount risk control | `user_daily_amount <= daily_limit` | `DAILY_AMOUNT_EXCEEDED` |

### 1.5.7 Idempotency Check

| Check | Attack Vector | Validation Logic | Error Code |
|---|---|---|---|
| **cid Unique** | Duplicate submission | If `cid` provided, check if exists | `DUPLICATE_REQUEST` (return original result) |

### 1.5.8 Check Order (Recommended)

```
1. Authentication (JWT valid?)
2. Authorization (user_id match?)
3. Request Format (from/to/amount valid?)
4. Account Type (from != to, type supported?)
5. Asset Check (exists? enabled? transferable?)
6. Amount Check (range? precision? overflow?)
7. Rate Limiting (exceeded?)
8. Idempotency (duplicate?)
9. Balance Check (sufficient?) ← Check last, avoid unnecessary queries
```

---


## 2. FSM Design (The State Machine)

### 2.0 Library Choice: `rust-fsm`

We use the [`rust-fsm`](https://crates.io/crates/rust-fsm) library, providing:
- ✅ **Compile-time validation** - Illegal state transitions cause compile errors.
- ✅ **Declarative DSL** - Clearly defined states and transitions.
- ✅ **Type Safety** - Prevents missing match arms.

**Cargo.toml:**
```toml
[dependencies]
rust-fsm = "0.7"
```

**DSL Definition:**
```rust
use rust_fsm::*;

state_machine! {
    derive(Debug, Clone, Copy, PartialEq, Eq)
    
    TransferFsm(Init)  // Initial State
    
    // State Definitions
    Init => {
        SourceWithdrawOk => SourceDone,
        SourceWithdrawFail => Failed,
    },
    SourceDone => {
        TargetDepositOk => Committed,
        TargetDepositFail => Compensating,
        TargetDepositUnknown => SourceDone [loop],  // Stay, Infinite Retry
    },
    Compensating => {
        RefundOk => RolledBack,
        RefundFail => Compensating [loop],  // Stay, Infinite Retry
    },
    // Terminal States
    Committed,
    Failed,
    RolledBack,
}
```

> [!NOTE]
> The DSL above is used for compile-time validation of state transition validity.
> Actual runtime state is stored in PostgreSQL and updated via CAS.

### 2.0.1 Core State Flow (Top Level)

```
                               ┌─────────────────────────────────────────────────────────┐
                               │              INTERNAL TRANSFER FSM                       │
                               └─────────────────────────────────────────────────────────┘

    ┌─────────────────────────────── Happy Path ────────────────────────────────────────────┐
    │                                                                                       │
    │    ┌─────────┐                    ┌─────────────┐                    ┌───────────────┐  │
    │    │  INIT   │   Source Deduct ✓  │ SOURCE_DONE │   Target Credit ✓  │               │  │
    │    │(Request)│ ─────────────────▶ │ (In-Flight) │ ─────────────────▶ │   COMMITTED   │  │
    │    └─────────┘                    └─────────────┘                    │               │  │
    │         │                               │                            └───────────────┘  │
    │         │                               │                                   ✅          │
    └─────────│───────────────────────────────│───────────────────────────────────────────────┘
              │                               │
              │                               │
              │                               ▼
              │                     ╔══════════════════════════════════════════════════╗
              │                     ║  🔒 ATOMIC COMMIT                               ║
              │                     ║                                                  ║
              │                     ║  IF AND ONLY IF:                                 ║
              │                     ║    FROM.withdraw = SUCCESS  ✓                   ║
              │                     ║    TO.deposit    = SUCCESS  ✓                   ║
              │                     ║                                                  ║
              │                     ║  EXECUTE: CAS(SOURCE_DONE → COMMITTED)           ║
              │                     ║  Must be atomic and non-interruptible.           ║
              │                     ╚══════════════════════════════════════════════════╝
              │                               │
              │ Source Deduction Fail         │ Target Credit Fail (EXPLICIT_FAIL)
              ▼                               ▼
        ┌──────────┐                   ┌──────────────┐
        │  FAILED  │                   │ COMPENSATING │◀───────────┐
        │ (Source) │                   │  (Refunding) │            │ Refund Fail (Infinite Retry)
        └──────────┘                   └──────────────┘────────────┘
             ❌                               │ Refund Success
                                              ▼
                                       ┌─────────────┐
                                       │ ROLLED_BACK │
                                       │ (Restored)  │
                                       └─────────────┘
                                             ↩️

    ╔════════════════════════════════════════════════════════════════════════════════════════╗
    ║  ⚠️ Target Unknown (TIMEOUT/UNKNOWN) → Stay SOURCE_DONE, Infinite Retry, NEVER rollback. ║
    ╚════════════════════════════════════════════════════════════════════════════════════════╝
```

**Core State Description:**
| State | Fund Location | Description |
|---|---|---|
| `INIT` | Source Account | User request accepted, funds haven't moved yet. |
| `SOURCE_DONE` | **In-Flight** | CRITICAL! Funds have left source, haven't reached target. |
| `COMMITTED` | Target Account | Terminal state, transfer succeeded. |
| `FAILED` | Source Account | Terminal state, source deduction failed, no funds moved. |
| `COMPENSATING` | In-Flight | Target credit failed, refunding to source. |
| `ROLLED_BACK` | Source Account | Terminal state, refund succeeded. |

> [!IMPORTANT]
> **`SOURCE_DONE` is the most critical state** - funds have left the source account but have not yet reached the target.
> At this point, the state MUST NOT be lost; it must eventually reach `COMMITTED` or `ROLLED_BACK`.

### 2.1 States (Exhaustive)

| ID | State Name | Entry Condition | Terminal? | Funds Location |
|---|---|---|---|---|
| 0 | `INIT` | User request accepted. | No | Source |
| 10 | `SOURCE_PENDING` | CAS success, Adapter call initiated. | No | Source (Deducting) |
| 20 | `SOURCE_DONE` | Source Adapter returned `OK`. | No | **In-Flight** |
| 30 | `TARGET_PENDING` | CAS success, Target Adapter call initiated. | No | In-Flight (Crediting) |
| 40 | `COMMITTED` | Target Adapter returned `OK`. | **YES** | Target |
| -10 | `FAILED` | Source Adapter returned `FAIL`. | **YES** | Source (Unchanged) |
| -20 | `COMPENSATING` | Target Adapter `FAIL` AND Source is Reversible. | No | In-Flight (Refunding) |
| -30 | `ROLLED_BACK` | Source Refund `OK`. | **YES** | Source (Restored) |

### 2.2 State Transition Rules (Exhaustive)

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                         CANONICAL STATE TRANSITIONS                           │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  INIT ──────[CAS OK]───────► SOURCE_PENDING                                   │
│    │                              │                                           │
│    │                              ├──[Adapter OK]────► SOURCE_DONE            │
│    │                              │                         │                 │
│    │                              └──[Adapter FAIL]──► FAILED (Terminal)      │
│    │                                                        │                 │
│    │                                                        │                 │
│    │                              SOURCE_DONE ──[CAS OK]──► TARGET_PENDING    │
│    │                                                             │            │
│    │                        ┌────────────────────────────────────┤            │
│    │                        │                                    │            │
│    │            [Adapter OK]│                       [Adapter FAIL]            │
│    │                        │                                    │            │
│    │                        ▼                                    ▼            │
│    │                   COMMITTED                     ┌───────────────────┐    │
│    │                   (Terminal)                    │ SOURCE REVERSIBLE?│    │
│    │                                                 └─────────┬─────────┘    │
│    │                                                   YES     │     NO       │
│    │                                                   ▼       │     ▼        │
│    │                                           COMPENSATING    │  INFINITE    │
│    │                                                 │         │   RETRY      │
│    │                                    [Refund OK]  │         │ (Stay in     │
│    │                                         ▼       │         │  TARGET_     │
│    │                                    ROLLED_BACK  │         │  PENDING)    │
│    │                                    (Terminal)   │         │              │
│    │                                                 │         │              │
│    └─────────────────────────────────────────────────┴─────────┴──────────────┘
```

### 2.3 Reversibility Rule (CRITICAL)

**Core Principle**: Only when an Adapter returns an **explicitly defined failure** can we safely rollback.

| Response Type | Meaning | Can Safely Rollback? | Handling |
|---|---|---|---|
| `SUCCESS` | Operation succeeded | N/A | Continue to next step |
| `EXPLICIT_FAIL` | Explicit business failure (e.g., insufficient balance) | **YES** | Can enter `COMPENSATING` |
| `TIMEOUT` | Timeout, state unknown | **NO** | Infinite Retry |
| `PENDING` | Processing, state unknown | **NO** | Infinite Retry |
| `NETWORK_ERROR` | Network error, state unknown | **NO** | Infinite Retry |
| `UNKNOWN` | Any other situation | **NO** | Infinite Retry or Manual Intervention |

> [!CAUTION]
> **Only `EXPLICIT_FAIL` allows safe rollback.**
> Any unknown state (Timeout, Pending, Network Error) means funds are **In-Flight**.
> We cannot know whether the counterparty has processed the request. Rash rollback will cause **Double Spend** or **Fund Loss**.
> Only safe actions: **Infinite Retry** or **Manual Intervention**.

---

## 3. Transfer Scenarios (Step-by-Step)

### 3.1 Scenario A: Funding → Spot (Deposit to Trading)

**Happy Path:**

| Step | Actor | Action | Pre-State | Post-State | Funds |
|---|---|---|---|---|---|
| 1 | API | Validate, Create Record | - | `INIT` | Funding |
| 2 | Coordinator | CAS(`INIT` → `SOURCE_PENDING`) | `INIT` | `SOURCE_PENDING` | Funding |
| 3 | Coordinator | Call `FundingAdapter.withdraw(req_id)` | - | - | - |
| 4 | PG | `UPDATE balances SET amount = amount - X` | - | - | Deducted |
| 5 | Coordinator | On `OK`: CAS(`SOURCE_PENDING` → `SOURCE_DONE`) | `SOURCE_PENDING` | `SOURCE_DONE` | **In-Flight** |
| 6 | Coordinator | CAS(`SOURCE_DONE` → `TARGET_PENDING`) | `SOURCE_DONE` | `TARGET_PENDING` | In-Flight |
| 7 | Coordinator | Call `TradingAdapter.deposit(req_id)` | - | - | - |
| 8 | UBSCore | Credit RAM, Write WAL, Emit Event | - | - | Credited |
| 9 | Coordinator | On Event: CAS(`TARGET_PENDING` → `COMMITTED`) | `TARGET_PENDING` | `COMMITTED` | **Trading** |

**Failure Path (Target Fails):**

| Step | Actor | Action | Pre-State | Post-State | Funds |
|---|---|---|---|---|---|
| 7' | Coordinator | Call `TradingAdapter.deposit(req_id)` → **FAIL/Timeout** | `TARGET_PENDING` | - | In-Flight |
| 8' | Coordinator | Check: Source = **Funding** (Reversible) | - | - | - |
| 9' | Coordinator | CAS(`TARGET_PENDING` → `COMPENSATING`) | `TARGET_PENDING` | `COMPENSATING` | In-Flight |
| 10' | Coordinator | Call `FundingAdapter.refund(req_id)` | - | - | - |
| 11' | PG | `UPDATE balances SET amount = amount + X` | - | - | Refunded |
| 12' | Coordinator | CAS(`COMPENSATING` → `ROLLED_BACK`) | `COMPENSATING` | `ROLLED_BACK` | **Funding** |

---

### 3.2 Scenario B: Spot → Funding (Withdraw from Trading)

**Happy Path:**

| Step | Actor | Action | Pre-State | Post-State | Funds |
|---|---|---|---|---|---|
| 1 | API | Validate, Create Record | - | `INIT` | Trading |
| 2 | Coordinator | CAS(`INIT` → `SOURCE_PENDING`) | `INIT` | `SOURCE_PENDING` | Trading |
| 3 | Coordinator | Call `TradingAdapter.withdraw(req_id)` | - | - | - |
| 4 | UBSCore | Check Balance, Deduct RAM, Write WAL, Emit Event | - | - | Deducted |
| 5 | Coordinator | On Event: CAS(`SOURCE_PENDING` → `SOURCE_DONE`) | `SOURCE_PENDING` | `SOURCE_DONE` | **In-Flight** |
| 6 | Coordinator | CAS(`SOURCE_DONE` → `TARGET_PENDING`) | `SOURCE_DONE` | `TARGET_PENDING` | In-Flight |
| 7 | Coordinator | Call `FundingAdapter.deposit(req_id)` | - | - | - |
| 8 | PG | `INSERT ... ON CONFLICT UPDATE SET amount = amount + X` | - | - | Credited |
| 9 | Coordinator | On `OK`: CAS(`TARGET_PENDING` → `COMMITTED`) | `TARGET_PENDING` | `COMMITTED` | **Funding** |

**Failure Path (Target Fails):**

| Step | Actor | Action | Pre-State | Post-State | Funds |
|---|---|---|---|---|---|
| 7a | Coordinator | Call `FundingAdapter.deposit(req_id)` → **EXPLICIT_FAIL** (e.g., constraint) | `TARGET_PENDING` | - | In-Flight |
| 8a | Coordinator | Check response type = **EXPLICIT_FAIL** (can safely rollback) | - | - | - |
| 9a | Coordinator | CAS(`TARGET_PENDING` → `COMPENSATING`) | `TARGET_PENDING` | `COMPENSATING` | In-Flight |
| 10a | Coordinator | Call `TradingAdapter.refund(req_id)` (refund to UBSCore) | - | - | - |
| 11a | UBSCore | Credit RAM balance, write WAL | - | - | Refunded |
| 12a | Coordinator | CAS(`COMPENSATING` → `ROLLED_BACK`) | `COMPENSATING` | `ROLLED_BACK` | **Trading** |

| Step | Actor | Action | Pre-State | Post-State | Funds |
|---|---|---|---|---|---|
| 7b | Coordinator | Call `FundingAdapter.deposit(req_id)` → **TIMEOUT/UNKNOWN** | `TARGET_PENDING` | - | In-Flight |
| 8b | Coordinator | Check response type = **UNKNOWN** (cannot safely rollback) | - | - | - |
| 9b | Coordinator | **DO NOT TRANSITION**. Stay `TARGET_PENDING`. | `TARGET_PENDING` | `TARGET_PENDING` | In-Flight |
| 10b | Coordinator | Log CRITICAL. Alert Ops. Schedule Retry. | - | - | - |
| 11b | Recovery | Retry `FundingAdapter.deposit(req_id)` **INFINITELY**. | - | - | - |
| 12b | (Eventually) | On `OK`: CAS(`TARGET_PENDING` → `COMMITTED`) | `TARGET_PENDING` | `COMMITTED` | **Funding** |

> [!WARNING]
> **Only enter `COMPENSATING` when Target returns `EXPLICIT_FAIL`.**
> If Timeout or Unknown, funds are In-Flight. Must Infinite Retry or Manual Intervention.

---

## 4. Failure Mode and Effects Analysis (FMEA)

### 4.1 Phase 1 Failures (Source Operation)

| Failure | Cause | Current State | Funds | Resolution |
|---|---|---|---|---|
| Adapter returns `FAIL` | Insufficient balance, DB constraint | `SOURCE_PENDING` | Source | Transition to `FAILED`. User sees error. |
| Adapter returns `PENDING` | Timeout, network issue | `SOURCE_PENDING` | Unknown | **Retry**. Adapter MUST be idempotent. |
| Coordinator crashes after CAS, before call | Process kill | `SOURCE_PENDING` | Source | Recovery Worker retries call. |
| Coordinator crashes after call, before result | Process kill | `SOURCE_PENDING` | Unknown | Recovery Worker retries (idempotent). |

### 4.2 Phase 2 Failures (Target Operation)

| Failure | Cause | Response Type | Current State | Funds | Resolution |
|---|---|---|---|---|---|
| Target explicit reject | Business rule | `EXPLICIT_FAIL` | `TARGET_PENDING` | In-Flight | `COMPENSATING` → Refund. |
| Timeout | Network delay | `TIMEOUT` | `TARGET_PENDING` | Unknown | **Infinite Retry**. |
| Network error | Connection lost | `NETWORK_ERROR` | `TARGET_PENDING` | Unknown | **Infinite Retry**. |
| Unknown error | System exception | `UNKNOWN` | `TARGET_PENDING` | Unknown | **Infinite Retry** or Manual Intervention. |
| Coordinator crashes | Process kill | N/A | `TARGET_PENDING` | In-Flight | Recovery Worker retries. |

### 4.3 Compensation Failures

| Failure | Cause | Current State | Funds | Resolution |
|---|---|---|---|---|
| Refund `FAIL` | PG down, constraint | `COMPENSATING` | In-Flight | **Infinite Retry**. Funds stuck until PG up. |
| Refund `PENDING` | Timeout | `COMPENSATING` | Unknown | **Retry**. |

---

## 5. Idempotency Requirements (MANDATORY)

### 5.1 Why Idempotency?
Retries are the foundation of crash recovery. Without idempotency, a retry will cause **double execution** (double deduction, double credit).

### 5.2 Implementation (Funding Adapter)

**Requirement**: Given the same `req_id`, calling `withdraw()` or `deposit()` multiple times MUST have the same effect as calling it once.

**Mechanism**:
1.  `transfers_tb` has `UNIQUE(req_id)`.
2.  **Atomic Transaction**:
    ```sql
    BEGIN;
    -- Check if already processed
    SELECT state FROM transfers_tb WHERE req_id = $1;
    IF state >= expected_post_state THEN
        RETURN 'AlreadyProcessed';
    END IF;
    
    -- Perform balance update
    UPDATE balances_tb SET amount = amount - $2 WHERE user_id = $3 AND asset_id = $4 AND amount >= $2;
    IF NOT FOUND THEN
        RETURN 'InsufficientBalance';
    END IF;
    
    -- Update state
    UPDATE transfers_tb SET state = $new_state, updated_at = NOW() WHERE req_id = $1;
    COMMIT;
    RETURN 'Success';
    ```

### 5.3 Implementation (Trading Adapter)

**Requirement**: Same as above. UBSCore MUST reject duplicate `req_id`.

**Mechanism**:
1.  `InternalOrder` includes `req_id` field (or `cid`).
2.  UBSCore maintains a `ProcessedTransferSet` (HashSet in RAM, rebuilt from WAL on restart).
3.  On receiving Transfer Order:
    ```
    IF req_id IN ProcessedTransferSet THEN
        RETURN 'AlreadyProcessed' (Success, no-op)
    ELSE
        ProcessTransfer()
        ProcessedTransferSet.insert(req_id)
        WriteWAL(TransferEvent)
        RETURN 'Success'
    END IF
    ```

---

## 6. Recovery Worker (Zombie Handler)

### 6.1 Purpose
On Coordinator startup (or periodically), scan for "stuck" transfers and resume them.

### 6.2 Query
```sql
SELECT * FROM transfers_tb 
WHERE state IN (0, 10, 20, 30, -20) -- INIT, SOURCE_PENDING, SOURCE_DONE, TARGET_PENDING, COMPENSATING
  AND updated_at < NOW() - INTERVAL '1 minute'; -- Stale threshold
```

### 6.3 Recovery Logic

| Current State | Action |
|---|---|
| `INIT` | Call `step()` (will transition to `SOURCE_PENDING`). |
| `SOURCE_PENDING` | Retry `Source.withdraw()`. |
| `SOURCE_DONE` | Call `step()` (will transition to `TARGET_PENDING`). |
| `TARGET_PENDING` | Retry `Target.deposit()`. Apply Reversibility Rule. |
| `COMPENSATING` | Retry `Source.refund()`. |

---

## 7. Data Model

### 7.1 Table: `transfers_tb`

```sql
CREATE TABLE transfers_tb (
    transfer_id   BIGSERIAL PRIMARY KEY,
    req_id        VARCHAR(26) UNIQUE NOT NULL,  -- Server-generated Unique ID (ULID)
    cid           VARCHAR(64) UNIQUE,           -- Client Idempotency Key (Optional)
    user_id       BIGINT NOT NULL,
    asset_id      INTEGER NOT NULL,
    amount        DECIMAL(30, 8) NOT NULL,
    transfer_type SMALLINT NOT NULL,            -- 1 = Funding->Spot, 2 = Spot->Funding
    source_type   SMALLINT NOT NULL,            -- 1 = Funding, 2 = Trading
    state         SMALLINT NOT NULL DEFAULT 0,  -- FSM State ID
    error_message TEXT,                         -- Last error (for debugging)
    retry_count   INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transfers_state ON transfers_tb(state) WHERE state NOT IN (40, -10, -30);
```

### 7.2 Invariant Check
Run periodically to detect data corruption:
```sql
-- Sum of Funding + Trading + In-Flight should be constant per user per asset
-- In-Flight = SUM(amount) WHERE state IN (SOURCE_DONE, TARGET_PENDING, COMPENSATING)
```

---

## 8. API Contract

### 8.1 Endpoint: `POST /api/v1/internal_transfer`

**Request:**
```json
{
  "from": "SPOT",       // Source account type
  "to": "FUNDING",     // Target account type
  "asset": "USDT",
  "amount": "100.00"
}
```

**Account Type Enum (`AccountType`):**
| Value | Meaning | Status |
|---|---|---|
| `FUNDING` | Funding Account (PostgreSQL) | Supported |
| `SPOT` | Spot Trading Account (UBSCore) | Supported |
| `FUTURE` | Futures Account | Future Extension |
| `MARGIN` | Margin Account | Future Extension |

**Response:**
```json
{
  "transfer_id": 12345,
  "req_id": "01JFVQ2X8Z0Y1M3N4P5R6S7T8U",  // Server-generated (ULID)
  "from": "SPOT",
  "to": "FUNDING",
  "state": "COMMITTED",  // or "PENDING" if async
  "message": "Transfer successful"
}
```

### 8.2 Query Endpoint: `GET /api/v1/internal_transfer/:req_id`

**Response:**
```json
{
  "transfer_id": 12345,
  "req_id": "sr-1734912345678901234",
  "from": "SPOT",
  "to": "FUNDING",
  "asset": "USDT",
  "amount": "100.00",
  "state": "COMMITTED",
  "created_at": "2024-12-23T14:00:00Z",
  "updated_at": "2024-12-23T14:00:01Z"
}
```

> [!IMPORTANT]
> **`req_id` is SERVER-GENERATED**, not client.
> If client needs idempotency, use optional `cid` (client_order_id) field. Server will check for duplicates and return existing result.

**Error Codes:**
| Code | Meaning |
|---|---|
| `INSUFFICIENT_BALANCE` | Source account balance < amount. |
| `INVALID_ACCOUNT_TYPE` | `from` or `to` account type is invalid or unsupported. |
| `SAME_ACCOUNT` | `from` and `to` are the same. |
| `DUPLICATE_REQUEST` | `cid` already processed. Return original result. |
| `INVALID_AMOUNT` | amount <= 0 or exceeds precision. |
| `SYSTEM_ERROR` | Internal failure. Advise retry. |

---

## 9. Implementation Pseudocode (Critical State Checks)

### 9.1 API Layer

```pseudo
function handle_transfer_request(request, auth_context):
    // ========== Defense-in-Depth Layer 1: API Layer ==========
    
    // 1. Identity Authentication
    if !auth_context.is_valid():
        return Error(UNAUTHORIZED)
    
    // 2. User ID Consistency (Prevent cross-user attacks)
    if request.user_id != auth_context.user_id:
        return Error(FORBIDDEN, "User ID mismatch")
    
    // 3. Account Type Check
    if request.from == request.to:
        return Error(SAME_ACCOUNT)
    
    if request.from NOT IN [FUNDING, SPOT]:
        return Error(INVALID_ACCOUNT_TYPE)
    
    if request.to NOT IN [FUNDING, SPOT]:
        return Error(INVALID_ACCOUNT_TYPE)
    
    // 4. Amount Check
    if request.amount <= 0:
        return Error(INVALID_AMOUNT)
    
    if decimal_places(request.amount) > asset.precision:
        return Error(PRECISION_OVERFLOW)
    
    // 5. Idempotency Check
    if request.cid:
        existing = db.find_by_cid(request.cid)
        if existing:
            return Success(existing)  // Return existing result
    
    // 6. Asset Check
    asset = db.get_asset(request.asset_id)
    if !asset or asset.status != ACTIVE:
        return Error(INVALID_ASSET)
    
    // 7. Call Coordinator
    result = coordinator.create_and_execute(request)
    return result
```

### 9.2 Coordinator Layer

```pseudo
function create_and_execute(request):
    // ========== Defense-in-Depth Layer 2: Coordinator ==========
    
    // Re-verify (Prevent internal calls bypassing API)
    ASSERT request.from != request.to
    ASSERT request.amount > 0
    ASSERT request.user_id > 0
    
    // Generate unique ID
    req_id = ulid.new()
    
    // Create transfer record (State = INIT)
    transfer = TransferRecord {
        req_id: req_id,
        user_id: request.user_id,
        from: request.from,
        to: request.to,
        asset_id: request.asset_id,
        amount: request.amount,
        state: INIT,
        created_at: now()
    }
    
    db.insert(transfer)
    log.info("Transfer created", req_id)
    
    // Execute FSM
    return execute_fsm(req_id)

function execute_fsm(req_id):
    loop:
        transfer = db.get(req_id)
        
        if transfer.state.is_terminal():
            return transfer
        
        new_state = step(transfer)
        
        if new_state == transfer.state:
            // No progress, wait for retry
            sleep(RETRY_INTERVAL)
            continue
    
function step(transfer):
    match transfer.state:
        INIT:
            return step_init(transfer)
        SOURCE_PENDING:
            return step_source_pending(transfer)
        SOURCE_DONE:
            return step_source_done(transfer)
        TARGET_PENDING:
            return step_target_pending(transfer)
        COMPENSATING:
            return step_compensating(transfer)
        _:
            return transfer.state  // Terminal, no processing
    
function step_init(transfer):
    // CAS: Persist state BEFORE calling adapter (Persist-Before-Call)
    success = db.cas_update(
        req_id = transfer.req_id,
        old_state = INIT,
        new_state = SOURCE_PENDING
    )
    
    if !success:
        return db.get(transfer.req_id).state
    
    // Get source adapter
    source_adapter = get_adapter(transfer.from)
    
    // ========== Defense-in-Depth Layer 3: Adapter ==========
    result = source_adapter.withdraw(
        req_id = transfer.req_id,
        user_id = transfer.user_id,
        asset_id = transfer.asset_id,
        amount = transfer.amount
    )
    
    match result:
        SUCCESS:
            db.cas_update(transfer.req_id, SOURCE_PENDING, SOURCE_DONE)
            return SOURCE_DONE
        
        EXPLICIT_FAIL(reason):
            db.update_with_error(transfer.req_id, SOURCE_PENDING, FAILED, reason)
            return FAILED
        
        TIMEOUT | PENDING | NETWORK_ERROR | UNKNOWN:
            log.warn("Source withdraw unknown state", transfer.req_id)
            return SOURCE_PENDING

function step_source_done(transfer):
    // ========== Enter SOURCE_DONE: Funds In-Flight, must reach terminal state ==========
    
    // CAS update to TARGET_PENDING
    success = db.cas_update(transfer.req_id, SOURCE_DONE, TARGET_PENDING)
    if !success:
        return db.get(transfer.req_id).state
    
    // Get target adapter
    target_adapter = get_adapter(transfer.to)
    
    // ========== Defense-in-Depth Layer 4: Target Adapter ==========
    result = target_adapter.deposit(
        req_id = transfer.req_id,
        user_id = transfer.user_id,
        asset_id = transfer.asset_id,
        amount = transfer.amount
    )
    
    match result:
        SUCCESS:
            // ╔════════════════════════════════════════════════════════════════╗
            // ║  🔒 ATOMIC COMMIT - CRITICAL STEP!                             ║
            // ║                                                                ║
            // ║  At this point:                                                ║
            // ║    FROM.withdraw = SUCCESS ✓ (already confirmed)               ║
            // ║    TO.deposit    = SUCCESS ✓ (just confirmed)                  ║
            // ║                                                                ║
            // ║  Execute Atomic CAS Commit:                                    ║
            // ║    CAS(TARGET_PENDING → COMMITTED)                            ║
            // ║                                                                ║
            // ║  Once this CAS succeeds, the transfer is irreversible!         ║
            // ╚════════════════════════════════════════════════════════════════╝
            
            commit_success = db.cas_update(transfer.req_id, TARGET_PENDING, COMMITTED)
            
            if !commit_success:
                return db.get(transfer.req_id).state
            
            log.info("🔒 ATOMIC COMMIT SUCCESS", transfer.req_id)
            return COMMITTED
        
        EXPLICIT_FAIL(reason):
            db.update_with_error(transfer.req_id, TARGET_PENDING, COMPENSATING, reason)
            return COMPENSATING
        
        TIMEOUT | PENDING | NETWORK_ERROR | UNKNOWN:
            // ========== CRITICAL: Unknown state, MUST NOT compensate! ==========
            log.critical("Target deposit unknown state - INFINITE RETRY", transfer.req_id)
            alert_ops("Transfer stuck in TARGET_PENDING", transfer.req_id)
            return TARGET_PENDING  // Stay and retry

function step_compensating(transfer):
    source_adapter = get_adapter(transfer.from)
    
    result = source_adapter.refund(
        req_id = transfer.req_id,
        user_id = transfer.user_id,
        asset_id = transfer.asset_id,
        amount = transfer.amount
    )
    
    match result:
        SUCCESS:
            db.cas_update(transfer.req_id, COMPENSATING, ROLLED_BACK)
            log.info("Transfer rolled back", transfer.req_id)
            return ROLLED_BACK
        
        _:
            log.critical("Refund failed - MUST RETRY", transfer.req_id)
            return COMPENSATING
```

### 9.3 Adapter Layer (Example: Funding Adapter)

```pseudo
function withdraw(req_id, user_id, asset_id, amount):
    // ========== Defense-in-Depth Layer 3: Adapter Internal Verification ==========
    
    // Re-verify parameters (Do not trust caller)
    ASSERT amount > 0
    ASSERT user_id > 0
    ASSERT asset_id > 0
    
    // Idempotency Check
    existing = db.find_transfer_operation(req_id, "WITHDRAW")
    if existing:
        return existing.result
    
    // Begin transaction
    tx = db.begin_transaction()
    try:
        // SELECT FOR UPDATE
        account = tx.select_for_update(
            "SELECT * FROM balances_tb WHERE user_id = ? AND asset_id = ? AND account_type = 'FUNDING'"
        )
        
        if !account:
            tx.rollback()
            return EXPLICIT_FAIL("SOURCE_ACCOUNT_NOT_FOUND")
        
        if account.status == FROZEN:
            tx.rollback()
            return EXPLICIT_FAIL("ACCOUNT_FROZEN")
        
        if account.available < amount:
            tx.rollback()
            return EXPLICIT_FAIL("INSUFFICIENT_BALANCE")
        
        // Execute deduction
        tx.update("UPDATE balances_tb SET available = available - ? WHERE id = ?", amount, account.id)
        
        // Record operation for idempotency
        tx.insert("INSERT INTO transfer_operations (req_id, op_type, result) VALUES (?, 'WITHDRAW', 'SUCCESS')")
        
        tx.commit()
        return SUCCESS
        
    catch Exception as e:
        tx.rollback()
        log.error("Withdraw failed", req_id, e)
        return UNKNOWN  // Uncertainty requires retry
```

---

## 10. Acceptance Test Plan (Security Critical)

> [!CAUTION]
> **ALL tests below must pass before going production.**
> Any failure indicates potential fund theft, loss, or creation from thin air.

### 10.1 Fund Conservation Tests

| Test ID | Scenario | Expected Result | Verification |
|---|---|---|---|
| **INV-001** | After normal transfer | Total funds = Before | `SUM(source) + SUM(target) = Constant` |
| **INV-002** | After failed transfer | Total funds = Before | Source balance unchanged |
| **INV-003** | After rollback | Total funds = Before | Source balance fully restored |
| **INV-004** | After crash recovery | Total funds = Before | Verify all account balances |

### 10.2 External Attack Tests

| Test ID | Attack Vector | Steps | Expected Result |
|---|---|---|---|
| **ATK-001** | Cross-user transfer | Submits user B's funds with user A's token | `FORBIDDEN` |
| **ATK-002** | user_id Tampering | Modify user_id in request body | `FORBIDDEN` |
| **ATK-003** | Negative Amount | amount = -100 | `INVALID_AMOUNT` |
| **ATK-004** | Zero Amount | amount = 0 | `INVALID_AMOUNT` |
| **ATK-005** | Precision Overflow | amount = 0.000000001 (>8 decimals) | `PRECISION_OVERFLOW` |
| **ATK-006** | Integer Overflow | amount = u64::MAX + 1 | `OVERFLOW` or parse error |
| **ATK-007** | Same Account | from = to = SPOT | `SAME_ACCOUNT` |
| **ATK-008** | Invalid Account Type | from = "INVALID" | `INVALID_ACCOUNT_TYPE` |
| **ATK-009** | Non-existent Asset | asset_id = 999999 | `INVALID_ASSET` |
| **ATK-010** | Duplicate cid | Submit same ID twice | Second returns first result |
| **ATK-011** | No Token | Missing Authorization header | `UNAUTHORIZED` |
| **ATK-012** | Expired Token | Use expired JWT | `UNAUTHORIZED` |
| **ATK-013** | Forged Token | Invalid signature JWT | `UNAUTHORIZED` |

### 10.3 Balance & Status Tests

| Test ID | Scenario | Expected Result |
|---|---|---|
| **BAL-001** | amount > available | `INSUFFICIENT_BALANCE`, no change |
| **BAL-002** | amount = available | Success, balance becomes 0 |
| **BAL-003** | Concurrent: Total > balance | One success, one `INSUFFICIENT_BALANCE` |
| **BAL-004** | Transfer from frozen account | `ACCOUNT_FROZEN` |
| **BAL-005** | Transfer from disabled account | `ACCOUNT_DISABLED` |

### 10.4 FSM State Transition Tests

| Test ID | Scenario | Expected State Flow |
|---|---|---|
| **FSM-001** | Normal Funding→Spot | INIT → SOURCE_PENDING → SOURCE_DONE → TARGET_PENDING → COMMITTED |
| **FSM-002** | Normal Spot→Funding | Same as above |
| **FSM-003** | Source Failure | INIT → SOURCE_PENDING → FAILED |
| **FSM-004** | Target Failure (Explicit) | ... → TARGET_PENDING → COMPENSATING → ROLLED_BACK |
| **FSM-005** | Target Timeout | ... → TARGET_PENDING (Stay, infinite retry) |
| **FSM-006** | Compensation Failure | COMPENSATING (Stay, infinite retry) |

### 10.5 Crash Recovery Tests

| Test ID | Crash Point | Expected Recovery Behavior |
|---|---|---|
| **CRA-001** | After INIT, before SOURCE_PENDING | Recovery reads INIT, restarts step_init |
| **CRA-002** | During SOURCE_PENDING, before call | Recovery retries withdraw (idempotent) |
| **CRA-003** | During SOURCE_PENDING, after call | Recovery retries withdraw (idempotent, returns handled) |
| **CRA-004** | After SOURCE_DONE, before TARGET_PENDING | Recovery executes step_source_done |
| **CRA-005** | During TARGET_PENDING | Recovery retries deposit (idempotent) |
| **CRA-006** | During COMPENSATING | Recovery retries refund (idempotent) |

### 10.6 Concurrency & Race Tests

| Test ID | Scenario | Expected Result |
|---|---|---|
| **CON-001** | Multiple Workers on same req_id | Only one successful CAS, others skip |
| **CON-002** | Concurrent Same Amount Transer | Two separate req_ids, both execute |
| **CON-003** | Transfer + External Withdraw | Sum cannot exceed balance |
| **CON-004** | No-lock balance read | No double deduction (SELECT FOR UPDATE) |

### 10.7 Idempotency Tests

| Test ID | Scenario | Expected Result |
|---|---|---|
| **IDP-001** | Call withdraw twice | Second returns SUCCESS, balance deducted once |
| **IDP-002** | Call deposit twice | Second returns SUCCESS, balance credited once |
| **IDP-003** | Call refund twice | Second returns SUCCESS, balance credited once |
| **IDP-004** | Recovery multiple retries | Final state consistent, balance correct |

### 10.8 Fund Anomaly Tests (Most Critical)

| Test ID | Threat | Method | Verification |
|---|---|---|---|
| **FND-001** | Double Spend | Source deduct twice | Only deduct once (idempotent) |
| **FND-002** | Fund Disappearance | Source success, target fail, no compensation | Must compensate or retry |
| **FND-003** | Money from Nothing | Target credit twice | Only credit once (idempotent) |
| **FND-004** | Lost in Transit | Crash at any point | Recovery restores integrity |
| **FND-005** | State Inconsistency | SOURCE_DONE but DB not updated | WAL + Idempotency parity |
| **FND-006** | Partial Commit | PG Transaction partial success | Atomic transaction (all or none) |

### 10.9 Monitoring & Alerting Tests

| Test ID | Scenario | Expected Alert |
|---|---|---|
| **MON-001** | Stuck in TARGET_PENDING > 1m | CRITICAL Alert |
| **MON-002** | Compensation fail 3 times | CRITICAL Alert |
| **MON-003** | Fund conservation check fail | CRITICAL Alert + HALT Service |
| **MON-004** | Abnormal freq per user | WARNING Alert [P2] |

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-f-integration-test...v0.0B-a)

---

## 1. 问题陈述

### 1.1 系统拓扑
| 系统 | 角色 | 数据源 | 持久化 |
|---|---|---|---|
| **PostgreSQL** | 资金账户 (Funding) | `balances_tb` | ACID, 持久化 |
| **UBSCore** | 交易账户 (Trading) | RAM | WAL + 易失性 |

### 1.2 核心约束
这两个系统 **无法共享事务**。没有 XA/2PC 数据库协议。
因此：**我们必须使用外部 FSM 协调器构建自己的两阶段提交。**

---

## 1.5 安全前置检查 (MANDATORY)

> [!CAUTION]
> **纵深防御 (Defense-in-Depth)**
> 以下所有检查必须在 **每一个独立模块** 中执行，不仅仅是 API 层。
> - **API 层**: 第一道防线，拒绝明显非法请求
> - **Coordinator**: 再次验证，防止内部调用绕过 API
> - **Adapters**: 最终防线，每个适配器必须独立验证参数
> - **UBSCore**: 内存操作前最后一次检查
>
> **安全 > 性能**。重复检查的开销可以接受，安全漏洞不可接受。

### 1.5.1 身份与授权检查

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **用户认证** | 伪造请求 | JWT/Session 必须有效 | `UNAUTHORIZED` |
| **用户 ID 一致性** | 跨用户转账攻击 | `request.user_id == auth.user_id` | `FORBIDDEN` |
| **账户归属** | 转走他人资金 | 源/目标账户都属于同一 `user_id` | `FORBIDDEN` |

### 1.5.2 账户类型检查

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **from != to** | 无限刷单/浪费资源 | `request.from != request.to` | `SAME_ACCOUNT` |
| **账户类型有效** | 注入无效类型 | `from, to ∈ {FUNDING, SPOT}` | `INVALID_ACCOUNT_TYPE` |
| **账户类型支持** | 请求未上线功能 | `from, to` 都在支持列表中 | `UNSUPPORTED_ACCOUNT_TYPE` |

### 1.5.3 金额检查

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **amount > 0** | 零/负数转账 | `amount > 0` | `INVALID_AMOUNT` |
| **精度检查** | 精度溢出 | `decimal_places(amount) <= asset.precision` | `PRECISION_OVERFLOW` |
| **最小金额** | 微额攻击/粉尘攻击 | `amount >= asset.min_transfer_amount` | `AMOUNT_TOO_SMALL` |
| **最大单笔金额** | 风控绕过 | `amount <= asset.max_transfer_amount` | `AMOUNT_TOO_LARGE` |
| **整数溢出** | u64 溢出攻击 | `amount <= u64::MAX / safety_factor` | `OVERFLOW` |

### 1.5.4 资产检查

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **资产存在** | 伪造 asset_id | `asset_id` 在系统中存在 | `INVALID_ASSET` |
| **资产状态** | 已下架资产 | `asset.status == ACTIVE` | `ASSET_SUSPENDED` |
| **转账许可** | 某些资产禁止内部转账 | `asset.internal_transfer_enabled == true` | `TRANSFER_NOT_ALLOWED` |

### 1.5.5 账户状态检查

#### 账户初始化规则（概述）

| 账户类型 | 初始化时机 | 备注 |
|---|---|---|
| **FUNDING** | 首次申请充值时创建 | 外部充值流程触发 |
| **SPOT** | 首次内部转账时创建 | 懒加载 (Lazy Init) |
| **FUTURE** | 首次内部转账时创建 [P2] | 懒加载 |
| **MARGIN** | 首次内部转账时创建 [P2] | 懒加载 |

> [!NOTE]
> - 各账户类型的具体初始化行为和业务规则，请参见各账户类型的专用文档。
> - 每个账户都有自己的状态定义（如是否允许划转），当前不详细定义。
> - **默认状态**：账户初始化时，默认允许划转。

#### 账户状态检查表

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **源账户存在** | 不存在的账户 | 源账户记录必须存在 | `SOURCE_ACCOUNT_NOT_FOUND` |
| **目标账户存在/创建** | 不存在的目标 | FUNDING必须存在；SPOT/FUTURE/MARGIN可创建 | `TARGET_ACCOUNT_NOT_FOUND` (仅FUNDING) |
| **源账户未冻结** | 被冻结账户转出 | `source.status != FROZEN` | `ACCOUNT_FROZEN` |
| **源账户未禁用** | 被禁用账户操作 | `source.status != DISABLED` | `ACCOUNT_DISABLED` |
| **余额充足** | 余额不足直接拒绝 | `source.available >= amount` | `INSUFFICIENT_BALANCE` |


### 1.5.6 频率限制 (Rate Limiting) - **[P2 未来优化]**

> [!NOTE]
> 此部分为 V2 优化项，V1 可不实现。

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **每秒请求数** | DoS 攻击 | `user_requests_per_second <= 10` | `RATE_LIMIT_EXCEEDED` |
| **每日转账次数** | 滥用 | `user_daily_transfers <= 100` | `DAILY_LIMIT_EXCEEDED` |
| **每日转账金额** | 大额风控 | `user_daily_amount <= daily_limit` | `DAILY_AMOUNT_EXCEEDED` |

### 1.5.7 幂等性检查

| 检查项 | 攻击向量 | 验证逻辑 | 错误码 |
|---|---|---|---|
| **cid 唯一** | 重复提交 | 如提供 `cid`，检查是否已存在 | `DUPLICATE_REQUEST` (返回原结果) |

### 1.5.8 检查顺序 (推荐)

```
1. 身份认证 (JWT 有效?)
2. 授权检查 (user_id 匹配?)
3. 请求格式 (from/to/amount 有效?)
4. 账户类型 (from != to, 类型支持?)
5. 资产检查 (存在? 启用? 可转账?)
6. 金额检查 (范围? 精度? 溢出?)
7. 频率限制 (超限?)
8. 幂等性 (重复?)
9. 余额检查 (充足?) ← 最后检查，避免无谓查询
```

---


## 2. FSM 设计 (状态机)

### 2.0 库选择: `rust-fsm`

**使用 [`rust-fsm`](https://crates.io/crates/rust-fsm) 库**，提供：
- ✅ **编译时验证** - 非法状态转换在编译时报错
- ✅ **声明式 DSL** - 清晰定义状态和转换
- ✅ **类型安全** - 防止遗漏分支

**Cargo.toml:**
```toml
[dependencies]
rust-fsm = "0.7"
```

**DSL 定义:**
```rust
use rust_fsm::*;

state_machine! {
    derive(Debug, Clone, Copy, PartialEq, Eq)
    
    TransferFsm(Init)  // 初始状态
    
    // 状态定义
    Init => {
        SourceWithdrawOk => SourceDone,
        SourceWithdrawFail => Failed,
    },
    SourceDone => {
        TargetDepositOk => Committed,
        TargetDepositFail => Compensating,
        TargetDepositUnknown => SourceDone [loop],  // 保持，无限重试
    },
    Compensating => {
        RefundOk => RolledBack,
        RefundFail => Compensating [loop],  // 保持，无限重试
    },
    // 终态
    Committed,
    Failed,
    RolledBack,
}
```

> [!NOTE]
> 上述 DSL 用于编译时验证状态转换的合法性。
> 实际运行时状态存储在 PostgreSQL，使用 CAS 更新。

### 2.0.1 核心状态流程图 (Top Level)

```
                              ┌─────────────────────────────────────────────────────────┐
                              │              INTERNAL TRANSFER FSM                       │
                              └─────────────────────────────────────────────────────────┘

   ┌─────────────────────────────── 正常路径 (Happy Path) ──────────────────────────────────┐
   │                                                                                        │
   │   ┌─────────┐                    ┌─────────────┐                    ┌───────────────┐  │
   │   │  INIT   │   源扣减成功 ✓     │ SOURCE_DONE │   目标入账成功 ✓   │               │  │
   │   │(用户请求)│ ─────────────────▶ │ (资金在途)  │ ─────────────────▶ │   COMMITTED   │  │
   │   └─────────┘                    └─────────────┘                    │               │  │
   │        │                               │                            └───────────────┘  │
   │        │                               │                                   ✅          │
   └────────│───────────────────────────────│───────────────────────────────────────────────┘
            │                               │
            │                               │
            │                               ▼
            │                     ╔══════════════════════════════════════════════════╗
            │                     ║  🔒 ATOMIC COMMIT (原子提交)                     ║
            │                     ║                                                  ║
            │                     ║  当且仅当:                                       ║
            │                     ║    FROM.withdraw = SUCCESS  ✓                   ║
            │                     ║    TO.deposit    = SUCCESS  ✓                   ║
            │                     ║                                                  ║
            │                     ║  执行: CAS(SOURCE_DONE → COMMITTED)             ║
            │                     ║  此操作必须原子，不可中断                         ║
            │                     ╚══════════════════════════════════════════════════╝
            │                               │
            │ 源扣减失败                     │ 目标入账失败 (明确 EXPLICIT_FAIL)
            ▼                               ▼
      ┌──────────┐                   ┌──────────────┐
      │  FAILED  │                   │ COMPENSATING │◀───────────┐
      │ (源失败)  │                   │  (退款中)    │            │ 退款失败 (无限重试)
      └──────────┘                   └──────────────┘────────────┘
           ❌                               │ 退款成功
                                            ▼
                                     ┌─────────────┐
                                     │ ROLLED_BACK │
                                     │  (已回滚)    │
                                     └─────────────┘
                                           ↩️

   ╔════════════════════════════════════════════════════════════════════════════════════════╗
   ║  ⚠️ 目标入账状态未知 (TIMEOUT/UNKNOWN) → 保持 SOURCE_DONE，无限重试，绝不进入 COMPENSATING║
   ╚════════════════════════════════════════════════════════════════════════════════════════╝
```



**核心状态说明:**
| 状态 | 资金位置 | 说明 |
|---|---|---|
| `INIT` | 源账户 | 用户发起请求，资金尚未移动 |
| `SOURCE_DONE` | **在途** | 关键点！资金已离开源，尚未到达目标 |
| `COMMITTED` | 目标账户 | 终态，转账成功 |
| `FAILED` | 源账户 | 终态，源扣减失败，无资金移动 |
| `COMPENSATING` | 在途 | 目标入账失败，正在退款 |
| `ROLLED_BACK` | 源账户 | 终态，退款成功 |

> [!IMPORTANT]
> **`SOURCE_DONE` 是最关键的状态** - 资金已离开源账户但尚未到达目标。
> 此时绝不能丢失状态，必须确保最终到达 `COMMITTED` 或 `ROLLED_BACK`。

---


### 2.1 状态 (穷举)

| ID | 状态名 | 进入条件 | 终态? | 资金位置 |
|---|---|---|---|---|
| 0 | `INIT` | 用户请求已接受 | 否 | 源账户 |
| 10 | `SOURCE_PENDING` | CAS 成功，适配器调用已发起 | 否 | 源账户 (扣减中) |
| 20 | `SOURCE_DONE` | 源适配器返回 `OK` | 否 | **在途** |
| 30 | `TARGET_PENDING` | CAS 成功，目标适配器调用已发起 | 否 | 在途 (入账中) |
| 40 | `COMMITTED` | 目标适配器返回 `OK` | **是** | 目标账户 |
| -10 | `FAILED` | 源适配器返回 `FAIL` | **是** | 源账户 (未变) |
| -20 | `COMPENSATING` | 目标适配器 `FAIL` 且源可逆 | 否 | 在途 (退款中) |
| -30 | `ROLLED_BACK` | 源退款 `OK` | **是** | 源账户 (已恢复) |


### 2.2 状态转换规则 (穷举)

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                              规范状态转换                                       │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  INIT ──────[CAS成功]───────► SOURCE_PENDING                                  │
│    │                              │                                           │
│    │                              ├──[适配器OK]────► SOURCE_DONE              │
│    │                              │                         │                 │
│    │                              └──[适配器FAIL]──► FAILED (终态)            │
│    │                                                        │                 │
│    │                                                        │                 │
│    │                              SOURCE_DONE ──[CAS成功]──► TARGET_PENDING   │
│    │                                                             │            │
│    │                        ┌────────────────────────────────────┤            │
│    │                        │                                    │            │
│    │            [适配器OK]  │                       [适配器FAIL]              │
│    │                        │                                    │            │
│    │                        ▼                                    ▼            │
│    │                   COMMITTED                     ┌───────────────────┐    │
│    │                   (终态)                        │   源可逆?          │    │
│    │                                                 └─────────┬─────────┘    │
│    │                                                   是      │     否       │
│    │                                                   ▼       │     ▼        │
│    │                                           COMPENSATING    │  无限重试    │
│    │                                                 │         │ (保持在      │
│    │                                    [退款OK]     │         │  TARGET_     │
│    │                                         ▼       │         │  PENDING)    │
│    │                                    ROLLED_BACK  │         │              │
│    │                                    (终态)       │         │              │
│    │                                                 │         │              │
│    └─────────────────────────────────────────────────┴─────────┴──────────────┘
```

### 2.3 可逆性规则 (关键)

**核心原则**: 只有当适配器返回 **明确定义的失败** 时，才能安全撤销。

| 响应类型 | 含义 | 可安全撤销? | 处理方式 |
|---|---|---|---|
| `SUCCESS` | 操作成功 | N/A | 继续下一步 |
| `EXPLICIT_FAIL` | 明确业务失败 (如余额不足) | **是** | 可进入 `COMPENSATING` |
| `TIMEOUT` | 超时，状态未知 | **否** | 无限重试 |
| `PENDING` | 处理中，状态未知 | **否** | 无限重试 |
| `NETWORK_ERROR` | 网络错误，状态未知 | **否** | 无限重试 |
| `UNKNOWN` | 任何其他情况 | **否** | 无限重试或人工介入 |

> [!CAUTION]
> **只有 `EXPLICIT_FAIL` 可以安全撤销。**
> 任何状态未知的情况（超时、Pending、网络错误），资金都处于 **In-Flight** 中。
> 我们无法知道对方是否已处理。贸然撤销将导致 **双花** 或 **资金丢失**。
> 唯一安全操作：**无限重试** 或 **人工介入**。

---

## 3. 转账场景 (逐步)

### 3.1 场景 A: 资金 → 交易 (充值到交易账户)

**正常路径:**

| 步骤 | 执行者 | 操作 | 前状态 | 后状态 | 资金 |
|---|---|---|---|---|---|
| 1 | API | 验证，创建记录 | - | `INIT` | 资金账户 |
| 2 | 协调器 | CAS(`INIT` → `SOURCE_PENDING`) | `INIT` | `SOURCE_PENDING` | 资金账户 |
| 3 | 协调器 | 调用 `FundingAdapter.withdraw(req_id)` | - | - | - |
| 4 | PG | `UPDATE balances SET amount = amount - X` | - | - | 已扣减 |
| 5 | 协调器 | 收到 `OK`: CAS(`SOURCE_PENDING` → `SOURCE_DONE`) | `SOURCE_PENDING` | `SOURCE_DONE` | **在途** |
| 6 | 协调器 | CAS(`SOURCE_DONE` → `TARGET_PENDING`) | `SOURCE_DONE` | `TARGET_PENDING` | 在途 |
| 7 | 协调器 | 调用 `TradingAdapter.deposit(req_id)` | - | - | - |
| 8 | UBSCore | 增加RAM余额，写WAL，发出事件 | - | - | 已入账 |
| 9 | 协调器 | 收到事件: CAS(`TARGET_PENDING` → `COMMITTED`) | `TARGET_PENDING` | `COMMITTED` | **交易账户** |

**失败路径 (目标失败):**

| 步骤 | 执行者 | 操作 | 前状态 | 后状态 | 资金 |
|---|---|---|---|---|---|
| 7' | 协调器 | 调用 `TradingAdapter.deposit(req_id)` → **FAIL/超时** | `TARGET_PENDING` | - | 在途 |
| 8' | 协调器 | 检查: 源 = **资金账户** (可逆) | - | - | - |
| 9' | 协调器 | CAS(`TARGET_PENDING` → `COMPENSATING`) | `TARGET_PENDING` | `COMPENSATING` | 在途 |
| 10' | 协调器 | 调用 `FundingAdapter.refund(req_id)` | - | - | - |
| 11' | PG | `UPDATE balances SET amount = amount + X` | - | - | 已退款 |
| 12' | 协调器 | CAS(`COMPENSATING` → `ROLLED_BACK`) | `COMPENSATING` | `ROLLED_BACK` | **资金账户** |

---

### 3.2 场景 B: 交易 → 资金 (从交易账户提现)

**正常路径:**

| 步骤 | 执行者 | 操作 | 前状态 | 后状态 | 资金 |
|---|---|---|---|---|---|
| 1 | API | 验证，创建记录 | - | `INIT` | 交易账户 |
| 2 | 协调器 | CAS(`INIT` → `SOURCE_PENDING`) | `INIT` | `SOURCE_PENDING` | 交易账户 |
| 3 | 协调器 | 调用 `TradingAdapter.withdraw(req_id)` | - | - | - |
| 4 | UBSCore | 检查余额，扣减RAM，写WAL，发出事件 | - | - | 已扣减 |
| 5 | 协调器 | 收到事件: CAS(`SOURCE_PENDING` → `SOURCE_DONE`) | `SOURCE_PENDING` | `SOURCE_DONE` | **在途** |
| 6 | 协调器 | CAS(`SOURCE_DONE` → `TARGET_PENDING`) | `SOURCE_DONE` | `TARGET_PENDING` | 在途 |
| 7 | 协调器 | 调用 `FundingAdapter.deposit(req_id)` | - | - | - |
| 8 | PG | `INSERT ... ON CONFLICT UPDATE SET amount = amount + X` | - | - | 已入账 |
| 9 | 协调器 | 收到 `OK`: CAS(`TARGET_PENDING` → `COMMITTED`) | `TARGET_PENDING` | `COMMITTED` | **资金账户** |

**失败路径 (目标失败):**

| 步骤 | 执行者 | 操作 | 前状态 | 后状态 | 资金 |
|---|---|---|---|---|---|
| 7a | 协调器 | 调用 `FundingAdapter.deposit(req_id)` → **EXPLICIT_FAIL** (如约束违反) | `TARGET_PENDING` | - | 在途 |
| 8a | 协调器 | 检查响应类型 = **EXPLICIT_FAIL** (可安全撤销) | - | - | - |
| 9a | 协调器 | CAS(`TARGET_PENDING` → `COMPENSATING`) | `TARGET_PENDING` | `COMPENSATING` | 在途 |
| 10a | 协调器 | 调用 `TradingAdapter.refund(req_id)` (向UBSCore退款) | - | - | - |
| 11a | UBSCore | 增加RAM余额，写WAL | - | - | 已退款 |
| 12a | 协调器 | CAS(`COMPENSATING` → `ROLLED_BACK`) | `COMPENSATING` | `ROLLED_BACK` | **交易账户** |

| 步骤 | 执行者 | 操作 | 前状态 | 后状态 | 资金 |
|---|---|---|---|---|---|
| 7b | 协调器 | 调用 `FundingAdapter.deposit(req_id)` → **TIMEOUT/UNKNOWN** | `TARGET_PENDING` | - | 在途 |
| 8b | 协调器 | 检查响应类型 = **UNKNOWN** (不可安全撤销) | - | - | - |
| 9b | 协调器 | **不转换状态**。保持 `TARGET_PENDING`。 | `TARGET_PENDING` | `TARGET_PENDING` | 在途 |
| 10b | 协调器 | 记录 CRITICAL 日志。告警运维。安排重试。 | - | - | - |
| 11b | 恢复器 | **无限**重试 `FundingAdapter.deposit(req_id)`。 | - | - | - |
| 12b | (最终) | 收到 `OK`: CAS(`TARGET_PENDING` → `COMMITTED`) | `TARGET_PENDING` | `COMMITTED` | **资金账户** |

> [!WARNING]
> **只有当目标返回 `EXPLICIT_FAIL` 时才能进入 `COMPENSATING`。**
> 如果是超时或未知状态，资金处于 In-Flight，必须无限重试或人工介入。

---

## 4. 失效模式与影响分析 (FMEA)

### 4.1 阶段1失败 (源操作)

| 失败 | 原因 | 当前状态 | 资金 | 解决方案 |
|---|---|---|---|---|
| 适配器返回 `FAIL` | 余额不足，DB约束 | `SOURCE_PENDING` | 源账户 | 转到 `FAILED`。用户看到错误。 |
| 适配器返回 `PENDING` | 超时，网络问题 | `SOURCE_PENDING` | 未知 | **重试**。适配器必须幂等。 |
| 协调器在CAS后、调用前崩溃 | 进程终止 | `SOURCE_PENDING` | 源账户 | 恢复工作器重试调用。 |
| 协调器在调用后、结果前崩溃 | 进程终止 | `SOURCE_PENDING` | 未知 | 恢复工作器重试（幂等）。 |

### 4.2 阶段2失败 (目标操作)

| 失败 | 原因 | 响应类型 | 当前状态 | 资金 | 解决方案 |
|---|---|---|---|---|---|
| 目标明确拒绝 | 业务规则 | `EXPLICIT_FAIL` | `TARGET_PENDING` | 在途 | `COMPENSATING` → 退款。 |
| 超时 | 网络延迟 | `TIMEOUT` | `TARGET_PENDING` | 未知 | **无限重试**。 |
| 网络错误 | 连接断开 | `NETWORK_ERROR` | `TARGET_PENDING` | 未知 | **无限重试**。 |
| 未知错误 | 系统异常 | `UNKNOWN` | `TARGET_PENDING` | 未知 | **无限重试** 或 人工介入。 |
| 协调器崩溃 | 进程终止 | N/A | `TARGET_PENDING` | 在途 | 恢复工作器重试。 |

### 4.3 补偿失败

| 失败 | 原因 | 当前状态 | 资金 | 解决方案 |
|---|---|---|---|---|
| 退款 `FAIL` | PG宕机，约束 | `COMPENSATING` | 在途 | **无限重试**。资金卡住直到PG恢复。 |
| 退款 `PENDING` | 超时 | `COMPENSATING` | 未知 | **重试**。 |

---

## 5. 幂等性要求 (强制)

### 5.1 为什么需要幂等性?
重试是崩溃恢复的基础。没有幂等性，重试将导致 **双重执行**（双重扣减、双重入账）。

### 5.2 实现 (资金适配器)

**要求**: 给定相同的 `req_id`，多次调用 `withdraw()` 或 `deposit()` 必须与调用一次效果相同。

**机制**:
1.  `transfers_tb` 有 `UNIQUE(req_id)`。
2.  **原子事务**:
    ```sql
    BEGIN;
    -- 检查是否已处理
    SELECT state FROM transfers_tb WHERE req_id = $1;
    IF state >= expected_post_state THEN
        RETURN 'AlreadyProcessed';
    END IF;
    
    -- 执行余额更新
    UPDATE balances_tb SET amount = amount - $2 WHERE user_id = $3 AND asset_id = $4 AND amount >= $2;
    IF NOT FOUND THEN
        RETURN 'InsufficientBalance';
    END IF;
    
    -- 更新状态
    UPDATE transfers_tb SET state = $new_state, updated_at = NOW() WHERE req_id = $1;
    COMMIT;
    RETURN 'Success';
    ```

### 5.3 实现 (交易适配器)

**要求**: 同上。UBSCore 必须拒绝重复的 `req_id`。

**机制**:
1.  `InternalOrder` 包含 `req_id` 字段（或 `cid`）。
2.  UBSCore 维护一个 `ProcessedTransferSet`（RAM中的HashSet，重启时从WAL重建）。
3.  收到转账订单时:
    ```
    IF req_id IN ProcessedTransferSet THEN
        RETURN 'AlreadyProcessed' (成功，无操作)
    ELSE
        ProcessTransfer()
        ProcessedTransferSet.insert(req_id)
        WriteWAL(TransferEvent)
        RETURN 'Success'
    END IF
    ```

---

## 6. 恢复工作器 (僵尸处理器)

### 6.1 目的
在协调器启动时（或定期），扫描"卡住"的转账并恢复它们。

### 6.2 查询
```sql
SELECT * FROM transfers_tb 
WHERE state IN (0, 10, 20, 30, -20) -- INIT, SOURCE_PENDING, SOURCE_DONE, TARGET_PENDING, COMPENSATING
  AND updated_at < NOW() - INTERVAL '1 minute'; -- 过期阈值
```

### 6.3 恢复逻辑

| 当前状态 | 操作 |
|---|---|
| `INIT` | 调用 `step()`（将转到 `SOURCE_PENDING`）。 |
| `SOURCE_PENDING` | 重试 `Source.withdraw()`。 |
| `SOURCE_DONE` | 调用 `step()`（将转到 `TARGET_PENDING`）。 |
| `TARGET_PENDING` | 重试 `Target.deposit()`。应用可逆性规则。 |
| `COMPENSATING` | 重试 `Source.refund()`。 |

---

## 7. 数据模型

### 7.1 表: `transfers_tb`

```sql
CREATE TABLE transfers_tb (
    transfer_id   BIGSERIAL PRIMARY KEY,
    req_id        VARCHAR(26) UNIQUE NOT NULL,  -- 服务端生成的唯一 ID (ULID)
    cid           VARCHAR(64) UNIQUE,           -- 客户端幂等键 (可选)
    user_id       BIGINT NOT NULL,
    asset_id      INTEGER NOT NULL,
    amount        DECIMAL(30, 8) NOT NULL,
    transfer_type SMALLINT NOT NULL,            -- 1 = 资金->交易, 2 = 交易->资金
    source_type   SMALLINT NOT NULL,            -- 1 = 资金, 2 = 交易
    state         SMALLINT NOT NULL DEFAULT 0,  -- FSM 状态 ID
    error_message TEXT,                         -- 最后错误（用于调试）
    retry_count   INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transfers_state ON transfers_tb(state) WHERE state NOT IN (40, -10, -30);
```

### 7.2 不变量检查
定期运行以检测数据损坏:
```sql
-- 每个用户每个资产的 资金 + 交易 + 在途 之和应该是常数
-- 在途 = SUM(amount) WHERE state IN (SOURCE_DONE, TARGET_PENDING, COMPENSATING)
```

---

## 8. API 契约

### 8.1 端点: `POST /api/v1/internal_transfer`

**请求:**
```json
{
  "from": "SPOT",       // 源账户类型
  "to": "FUNDING",     // 目标账户类型
  "asset": "USDT",
  "amount": "100.00"
}
```

**账户类型枚举 (`AccountType`):**
| 值 | 含义 | 状态 |
|---|---|---|
| `FUNDING` | 资金账户 (PostgreSQL) | 已支持 |
| `SPOT` | 现货交易账户 (UBSCore) | 已支持 |
| `FUTURE` | 合约账户 | 未来扩展 |
| `MARGIN` | 杠杆账户 | 未来扩展 |

**响应:**
```json
{
  "transfer_id": 12345,
  "req_id": "01JFVQ2X8Z0Y1M3N4P5R6S7T8U",  // 服务端生成 (ULID)
  "from": "SPOT",
  "to": "FUNDING",
  "state": "COMMITTED",  // 或 "PENDING" 如果异步
  "message": "转账成功"
}
```

### 8.2 查询端点: `GET /api/v1/internal_transfer/:req_id`

**响应:**
```json
{
  "transfer_id": 12345,
  "req_id": "sr-1734912345678901234",
  "from": "SPOT",
  "to": "FUNDING",
  "asset": "USDT",
  "amount": "100.00",
  "state": "COMMITTED",
  "created_at": "2024-12-23T14:00:00Z",
  "updated_at": "2024-12-23T14:00:01Z"
}
```

> [!IMPORTANT]
> **`req_id` 由服务端生成**，不是客户端。
> 客户端如果需要幂等性，应使用 `cid` (client_order_id) 字段（可选），服务端会检查重复并返回已有结果。

**错误码:**
| 代码 | 含义 |
|---|---|
| `INSUFFICIENT_BALANCE` | 源账户余额 < 金额。 |
| `INVALID_ACCOUNT_TYPE` | `from` 或 `to` 的账户类型无效或不支持。 |
| `SAME_ACCOUNT` | `from` 和 `to` 相同。 |
| `DUPLICATE_REQUEST` | `cid` 已处理。返回原始结果。 |
| `INVALID_AMOUNT` | 金额 <= 0 或超过精度。 |
| `SYSTEM_ERROR` | 内部失败。建议重试。 |

---

## 9. 实现伪代码 (关键状态检查)

### 9.1 API 层

```pseudo
function handle_transfer_request(request, auth_context):
    // ========== 纵深防御 Layer 1: API 层 ==========
    
    // 1. 身份认证
    if !auth_context.is_valid():
        return Error(UNAUTHORIZED)
    
    // 2. 用户 ID 一致性（防止跨用户攻击）
    if request.user_id != auth_context.user_id:
        return Error(FORBIDDEN, "User ID mismatch")
    
    // 3. 账户类型检查
    if request.from == request.to:
        return Error(SAME_ACCOUNT)
    
    if request.from NOT IN [FUNDING, SPOT]:
        return Error(INVALID_ACCOUNT_TYPE)
    
    if request.to NOT IN [FUNDING, SPOT]:
        return Error(INVALID_ACCOUNT_TYPE)
    
    // 4. 金额检查
    if request.amount <= 0:
        return Error(INVALID_AMOUNT)
    
    if decimal_places(request.amount) > asset.precision:
        return Error(PRECISION_OVERFLOW)
    
    // 5. 幂等性检查
    if request.cid:
        existing = db.find_by_cid(request.cid)
        if existing:
            return Success(existing)  // 返回已存在的结果
    
    // 6. 资产检查
    asset = db.get_asset(request.asset_id)
    if !asset or asset.status != ACTIVE:
        return Error(INVALID_ASSET)
    
    // 7. 调用 Coordinator
    result = coordinator.create_and_execute(request)
    return result
```

### 9.2 Coordinator 层

```pseudo
function create_and_execute(request):
    // ========== 纵深防御 Layer 2: Coordinator ==========
    
    // 再次验证（防止内部调用绕过 API）
    ASSERT request.from != request.to
    ASSERT request.amount > 0
    ASSERT request.user_id > 0
    
    // 生成唯一 ID
    req_id = ulid.new()
    
    // 创建转账记录 (State = INIT)
    transfer = TransferRecord {
        req_id: req_id,
        user_id: request.user_id,
        from: request.from,
        to: request.to,
        asset_id: request.asset_id,
        amount: request.amount,
        state: INIT,
        created_at: now()
    }
    
    db.insert(transfer)
    log.info("Transfer created", req_id)
    
    // 执行 FSM
    return execute_fsm(req_id)

function execute_fsm(req_id):
    loop:
        transfer = db.get(req_id)
        
        if transfer.state.is_terminal():
            return transfer
        
        new_state = step(transfer)
        
        if new_state == transfer.state:
            // 未进展，等待重试
            sleep(RETRY_INTERVAL)
            continue
    
function step(transfer):
    match transfer.state:
        INIT:
            return step_init(transfer)
        SOURCE_PENDING:
            return step_source_pending(transfer)
        SOURCE_DONE:
            return step_source_done(transfer)
        TARGET_PENDING:
            return step_target_pending(transfer)
        COMPENSATING:
            return step_compensating(transfer)
        _:
            return transfer.state  // 终态，不处理

function step_init(transfer):
    // CAS: 先更新状态，再调用适配器（Persist-Before-Call）
    success = db.cas_update(
        req_id = transfer.req_id,
        old_state = INIT,
        new_state = SOURCE_PENDING
    )
    
    if !success:
        // 并发冲突，重新读取
        return db.get(transfer.req_id).state
    
    // 获取源适配器
    source_adapter = get_adapter(transfer.from)
    
    // ========== 纵深防御 Layer 3: Adapter ==========
    result = source_adapter.withdraw(
        req_id = transfer.req_id,
        user_id = transfer.user_id,
        asset_id = transfer.asset_id,
        amount = transfer.amount
    )
    
    match result:
        SUCCESS:
            db.cas_update(transfer.req_id, SOURCE_PENDING, SOURCE_DONE)
            return SOURCE_DONE
        
        EXPLICIT_FAIL(reason):
            // 明确失败，可以安全终止
            db.update_with_error(transfer.req_id, SOURCE_PENDING, FAILED, reason)
            return FAILED
        
        TIMEOUT | PENDING | NETWORK_ERROR | UNKNOWN:
            // 状态未知，保持 SOURCE_PENDING，等待重试
            log.warn("Source withdraw unknown state", transfer.req_id)
            return SOURCE_PENDING

function step_source_done(transfer):
    // ========== 进入 SOURCE_DONE: 资金已在途，必须确保最终到达终态 ==========
    
    // CAS 更新到 TARGET_PENDING
    success = db.cas_update(transfer.req_id, SOURCE_DONE, TARGET_PENDING)
    if !success:
        return db.get(transfer.req_id).state
    
    // 获取目标适配器
    target_adapter = get_adapter(transfer.to)
    
    // ========== 纵深防御 Layer 4: Target Adapter ==========
    result = target_adapter.deposit(
        req_id = transfer.req_id,
        user_id = transfer.user_id,
        asset_id = transfer.asset_id,
        amount = transfer.amount
    )
    
    match result:
        SUCCESS:
            // ╔════════════════════════════════════════════════════════════════╗
            // ║  🔒 ATOMIC COMMIT - 最关键的一步！                             ║
            // ║                                                                ║
            // ║  此时:                                                         ║
            // ║    FROM.withdraw = SUCCESS ✓ (已确认)                         ║
            // ║    TO.deposit    = SUCCESS ✓ (刚确认)                         ║
            // ║                                                                ║
            // ║  执行原子 CAS 提交:                                            ║
            // ║    CAS(TARGET_PENDING → COMMITTED)                            ║
            // ║                                                                ║
            // ║  此 CAS 是最终确认，一旦成功，转账不可逆转！                    ║
            // ╚════════════════════════════════════════════════════════════════╝
            
            commit_success = db.cas_update(transfer.req_id, TARGET_PENDING, COMMITTED)
            
            if !commit_success:
                // 极少发生：另一个 Worker 已经提交，返回当前状态
                return db.get(transfer.req_id).state
            
            log.info("🔒 ATOMIC COMMIT SUCCESS", transfer.req_id)
            return COMMITTED
        
        EXPLICIT_FAIL(reason):
            // 明确失败，可以进入补偿
            db.update_with_error(transfer.req_id, TARGET_PENDING, COMPENSATING, reason)
            return COMPENSATING
        
        TIMEOUT | PENDING | NETWORK_ERROR | UNKNOWN:
            // ========== 关键：状态未知，不能补偿！==========
            log.critical("Target deposit unknown state - INFINITE RETRY", transfer.req_id)
            alert_ops("Transfer stuck in TARGET_PENDING", transfer.req_id)
            return TARGET_PENDING  // 保持状态，等待重试


function step_compensating(transfer):
    source_adapter = get_adapter(transfer.from)
    
    result = source_adapter.refund(
        req_id = transfer.req_id,
        user_id = transfer.user_id,
        asset_id = transfer.asset_id,
        amount = transfer.amount
    )
    
    match result:
        SUCCESS:
            db.cas_update(transfer.req_id, COMPENSATING, ROLLED_BACK)
            log.info("Transfer rolled back", transfer.req_id)
            return ROLLED_BACK
        
        _:
            // 退款失败，必须无限重试
            log.critical("Refund failed - MUST RETRY", transfer.req_id)
            return COMPENSATING
```

### 9.3 Adapter 层 (示例: Funding Adapter)

```pseudo
function withdraw(req_id, user_id, asset_id, amount):
    // ========== 纵深防御 Layer 3: Adapter 内部检查 ==========
    
    // 再次验证参数（不信任调用者）
    ASSERT amount > 0
    ASSERT user_id > 0
    ASSERT asset_id > 0
    
    // 幂等性检查
    existing = db.find_transfer_operation(req_id, "WITHDRAW")
    if existing:
        return existing.result  // 返回已处理的结果
    
    // 开始事务
    tx = db.begin_transaction()
    try:
        // 获取账户并锁定
        account = tx.select_for_update(
            "SELECT * FROM balances_tb WHERE user_id = ? AND asset_id = ? AND account_type = 'FUNDING'"
        )
        
        if !account:
            tx.rollback()
            return EXPLICIT_FAIL("SOURCE_ACCOUNT_NOT_FOUND")
        
        if account.status == FROZEN:
            tx.rollback()
            return EXPLICIT_FAIL("ACCOUNT_FROZEN")
        
        if account.available < amount:
            tx.rollback()
            return EXPLICIT_FAIL("INSUFFICIENT_BALANCE")
        
        // 执行扣减
        tx.update("UPDATE balances_tb SET available = available - ? WHERE id = ?", amount, account.id)
        
        // 记录操作（用于幂等性）
        tx.insert("INSERT INTO transfer_operations (req_id, op_type, result) VALUES (?, 'WITHDRAW', 'SUCCESS')")
        
        tx.commit()
        return SUCCESS
        
    catch Exception as e:
        tx.rollback()
        log.error("Withdraw failed", req_id, e)
        return UNKNOWN  // 不确定是否执行，必须重试
```

---

## 10. 验收测试计划 (安全关键)

> [!CAUTION]
> **以下测试必须全部通过才能上线。**
> 任何失败都可能导致资金被盗、消失或无中生有。

### 10.1 资金守恒测试

| 测试 ID | 场景 | 预期结果 | 验证方法 |
|---|---|---|---|
| **INV-001** | 正常转账后 | 总资金 = 转账前 | `SUM(source) + SUM(target) = 常数` |
| **INV-002** | 失败转账后 | 总资金 = 转账前 | 源账户余额无变化 |
| **INV-003** | 回滚后 | 总资金 = 转账前 | 源账户余额完全恢复 |
| **INV-004** | 系统崩溃恢复后 | 总资金 = 崩溃前 | 遍历所有账户验证 |

### 10.2 外部攻击测试

| 测试 ID | 攻击向量 | 测试步骤 | 预期结果 |
|---|---|---|---|
| **ATK-001** | 跨用户转账 | 用 user_id=A 的 token 请求转 user_id=B 的资金 | `FORBIDDEN` |
| **ATK-002** | user_id 篡改 | 修改请求体中的 user_id | `FORBIDDEN` |
| **ATK-003** | 负数金额 | amount = -100 | `INVALID_AMOUNT` |
| **ATK-004** | 零金额 | amount = 0 | `INVALID_AMOUNT` |
| **ATK-005** | 超精度金额 | amount = 0.000000001 (超过8位) | `PRECISION_OVERFLOW` |
| **ATK-006** | 整数溢出 | amount = u64::MAX + 1 | `OVERFLOW` 或解析失败 |
| **ATK-007** | 相同账户 | from = to = SPOT | `SAME_ACCOUNT` |
| **ATK-008** | 无效账户类型 | from = "INVALID" | `INVALID_ACCOUNT_TYPE` |
| **ATK-009** | 不存在的资产 | asset_id = 999999 | `INVALID_ASSET` |
| **ATK-010** | 重复 cid | 同一 cid 发两次 | 第二次返回第一次结果 |
| **ATK-011** | 无 Token | 不带 Authorization header | `UNAUTHORIZED` |
| **ATK-012** | 过期 Token | 使用过期的 JWT | `UNAUTHORIZED` |
| **ATK-013** | 伪造 Token | 使用无效签名的 JWT | `UNAUTHORIZED` |

### 10.3 余额不足测试

| 测试 ID | 场景 | 预期结果 |
|---|---|---|
| **BAL-001** | 转账金额 > 可用余额 | `INSUFFICIENT_BALANCE`，余额无变化 |
| **BAL-002** | 转账金额 = 可用余额 | 成功，余额变为 0 |
| **BAL-003** | 并发: 两次转账总额 > 余额 | 一个成功，一个 `INSUFFICIENT_BALANCE` |
| **BAL-004** | 冻结账户转出 | `ACCOUNT_FROZEN` |
| **BAL-005** | 禁用账户转出 | `ACCOUNT_DISABLED` |

### 10.4 FSM 状态转换测试

| 测试 ID | 场景 | 预期状态流 |
|---|---|---|
| **FSM-001** | 正常 Funding→Spot | INIT → SOURCE_PENDING → SOURCE_DONE → TARGET_PENDING → COMMITTED |
| **FSM-002** | 正常 Spot→Funding | 同上 |
| **FSM-003** | 源失败 | INIT → SOURCE_PENDING → FAILED |
| **FSM-004** | 目标失败 (明确) | ... → TARGET_PENDING → COMPENSATING → ROLLED_BACK |
| **FSM-005** | 目标超时 | ... → TARGET_PENDING (保持，无限重试) |
| **FSM-006** | 补偿失败 | COMPENSATING (保持，无限重试) |

### 10.5 崩溃恢复测试

| 测试 ID | 崩溃点 | 预期恢复行为 |
|---|---|---|
| **CRA-001** | INIT 后，SOURCE_PENDING 前 | Recovery 读取 INIT，重新执行 step_init |
| **CRA-002** | SOURCE_PENDING 中，适配器调用前 | Recovery 重试 withdraw (幂等) |
| **CRA-003** | SOURCE_PENDING 中，适配器调用后 | Recovery 重试 withdraw (幂等，返回已处理) |
| **CRA-004** | SOURCE_DONE 后，TARGET_PENDING 前 | Recovery 继续执行 step_source_done |
| **CRA-005** | TARGET_PENDING 中 | Recovery 重试 deposit (幂等) |
| **CRA-006** | COMPENSATING 中 | Recovery 重试 refund (幂等) |

### 10.6 并发/竞态测试

| 测试 ID | 场景 | 预期结果 |
|---|---|---|
| **CON-001** | 多个 Worker 处理同一 req_id | 只有一个成功 CAS，其他跳过 |
| **CON-002** | 同时两次相同金额转账 | 两个独立 req_id，各自执行 |
| **CON-003** | 转账 + 外部提现并发 | 只有余额足够的操作成功 |
| **CON-004** | 读取余额时无锁 | 无重复扣减（SELECT FOR UPDATE） |

### 10.7 幂等性测试

| 测试 ID | 场景 | 预期结果 |
|---|---|---|
| **IDP-001** | 同一 req_id 调用 withdraw 两次 | 第二次返回 SUCCESS，余额只扣一次 |
| **IDP-002** | 同一 req_id 调用 deposit 两次 | 第二次返回 SUCCESS，余额只加一次 |
| **IDP-003** | 同一 req_id 调用 refund 两次 | 第二次返回 SUCCESS，余额只加一次 |
| **IDP-004** | Recovery 多次重试同一 transfer | 最终状态一致，余额正确 |

### 10.8 资金异常测试 (最关键)

| 测试 ID | 威胁 | 测试方法 | 验证 |
|---|---|---|---|
| **FND-001** | 双花 (Double Spend) | 源扣减两次 | 只扣一次（幂等） |
| **FND-002** | 资金消失 | 源扣减成功，目标失败，不补偿 | 必须补偿或无限重试 |
| **FND-003** | 资金无中生有 | 目标入账两次 | 只入一次（幂等） |
| **FND-004** | 中途崩溃丢失 | 任意点崩溃 | Recovery 恢复完整性 |
| **FND-005** | 状态不一致 | SOURCE_DONE 但 DB 未更新 | WAL + 幂等保证一致 |
| **FND-006** | 部分提交 | PG 事务部分成功 | 原子事务，全成功或全失败 |

### 10.9 监控告警测试

| 测试 ID | 场景 | 预期告警 |
|---|---|---|
| **MON-001** | 转账卡在 TARGET_PENDING > 1 分钟 | CRITICAL 告警 |
| **MON-002** | 补偿连续失败 3 次 | CRITICAL 告警 |
| **MON-003** | 资金守恒检查失败 | CRITICAL 告警 + 暂停服务 |
| **MON-004** | 单用户转账频率异常 | WARNING 告警 [P2] |

---

<br>
<div align="right"><a href="#-chinese">↑ Back to Top</a></div>
<br>

---

## 📋 Implementation & Verification | 实现与验证

本章的完整实现细节、API 说明、E2E 测试脚本和验证结果请参阅:

For complete implementation details, API documentation, E2E test scripts, and verification results:

👉 **[Phase 0x0B-a: Implementation & Testing Guide](./0x0B-a-transfer-testing.md)**

包含 / Includes:
- 架构实现与核心模块 (Architecture & Core Modules)
- 新增 API 端点 (New API Endpoints)
- 可复用 E2E 测试脚本 (Reusable E2E Test Script)
- 数据库验证方法 (Database Verification)
- 已修复 Bug 清单 (Fixed Bugs)

