# 0x0B-a Internal Transfer Architecture (Strict FSM)

> **Author**: System Architect
> **Version**: 2.0 (Exhaustive Specification)
> **Audience**: Implementation Engineers
> **Criticality**: HIGH. Any logical error = Financial Catastrophe.

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
| **client_order_id Unique** | Duplicate submission | If `client_order_id` provided, check if exists | `DUPLICATE_REQUEST` (return original result) |

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
    req_id        VARCHAR(64) UNIQUE NOT NULL,  -- Client Idempotency Key
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
  "req_id": "sr-1734912345678901234",  // Server-generated (Snowflake)
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
> If client needs idempotency, use optional `client_order_id` field. Server will check for duplicates and return existing result.

**Error Codes:**
| Code | Meaning |
|---|---|
| `INSUFFICIENT_BALANCE` | Source account balance < amount. |
| `INVALID_ACCOUNT_TYPE` | `from` or `to` account type is invalid or unsupported. |
| `SAME_ACCOUNT` | `from` and `to` are the same. |
| `DUPLICATE_REQUEST` | `client_order_id` already processed. Return original result. |
| `INVALID_AMOUNT` | amount <= 0 or exceeds precision. |
| `SYSTEM_ERROR` | Internal failure. Advise retry. |

---
