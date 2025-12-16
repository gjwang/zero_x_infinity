# Chapter 0x08: Order Processing Pipeline Architecture

## Overview

This document describes the order processing pipeline architecture for 0xInfinity, 
a high-frequency trading matching engine. The design emphasizes:

- **Durability**: Orders are persisted to WAL before any state changes
- **Correctness**: Pure state machine for matching, deterministic replay
- **Simplicity**: Single-threaded execution per service (no locks, no double-spend)
- **Modularity**: Clear separation of concerns

## Core Design Principle: Single-Threaded Execution

Each service runs in a **single thread** for its critical path:
- No concurrency issues within a service
- No locks needed for balance operations
- Atomic operations are naturally achieved
- **Double-spend is impossible** after WAL write

## Pipeline Stages

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            ORDER PROCESSING PIPELINE                             │
└─────────────────────────────────────────────────────────────────────────────────┘

                    ┌──────────────────────────────────────────────────────────────┐
                    │                      INPUT STAGE                              │
                    │  Order { order_id, user_id, side, price, qty }               │
                    └──────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  1. PRE-CHECK (Soft Filter, NO SIDE EFFECTS)                                     │
│  ├── User exists? (optional quick check)                                         │
│  ├── Basic format validation                                                     │
│  └── Rate limiting (optional)                                                    │
│                                                                                   │
│  Purpose: Reduce garbage orders entering the system                              │
│  NOTE: Some invalid orders may still pass through!                               │
│  Result: Pass or Early Reject (no side effects)                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  2. ORDER BUFFER (Pre-WAL)                                                       │
│  ├── Orders queue (may include some invalid orders)                              │
│  └── Batch for WAL write efficiency                                              │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  3. WRITE ORDER_WAL  ★ POINT OF NO RETURN ★                                     │
│  ├── Append to WAL (sequential write)                                            │
│  ├── fsync / group commit                                                        │
│  └── Assign seq_num                                                              │
│                                                                                   │
│  After this point: Order MUST go through full lifecycle!                         │
│  Format: [seq_num | op_type | order_id | user_id | side | price | qty | ts]     │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  4. BALANCE CHECK + LOCK (Atomic, Single-Threaded)                               │
│  ├── Check user exists                                                           │
│  ├── Check balance sufficient                                                    │
│  └── Lock funds (freeze)                                                         │
│                                                                                   │
│  If FAIL: Order status = Rejected (still recorded in WAL)                        │
│  If PASS: Order status = Accepted, proceed to matching                           │
│                                                                                   │
│  ★ Single-threaded = atomic = no double-spend ★                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                              ┌───────────────┴───────────────┐
                              │      REJECTED (lock failed)   │
                              │  - Record rejection in state  │
                              │  - Emit rejection event       │
                              │  - Continue (order completed) │
                              └───────────────────────────────┘
                                              │ ACCEPTED
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  5. MATCHING ENGINE (ME) - Pure State Machine                                    │
│  ├── OrderBook (BTreeMap-based)                                                  │
│  ├── Price-Time priority matching                                                │
│  └── Generate trades list                                                        │
│                                                                                   │
│  ME does NOT: check balance, update balance, write I/O                          │
│  Input:  Order (already validated)                                               │
│  Output: OrderResult { order, trades: Vec<Trade> }                               │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  6. TRADES BUFFER                                                                │
│  ├── Generated trades queue                                                      │
│  └── Batch for settlement efficiency                                             │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  7. SETTLEMENT                                                                   │
│  ├── For each trade:                                                             │
│  │   ├── Buyer: spend_frozen(quote, cost), deposit(base, qty)                   │
│  │   ├── Seller: spend_frozen(base, qty), deposit(quote, cost)                  │
│  │   ├── Write ledger entries (audit trail)                                      │
│  │   └── Emit balance events                                                     │
│  └── Update order status (Filled/PartiallyFilled)                                │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
                    ┌──────────────────────────────────────────────────────────────┐
                    │                      OUTPUT STAGE                             │
                    │  ├── Order Response (accepted/rejected, fills)               │
                    │  ├── Trade Events (for market data feed)                     │
                    │  └── Balance Events (for user notifications)                 │
                    └──────────────────────────────────────────────────────────────┘
```

## Key Insight: WAL is the Point of No Return

```
PRE-CHECK                    WAL WRITE                 BALANCE LOCK
    │                            │                          │
    │  [no side effects]         │  [persisted]             │  [atomic lock]
    │                            │                          │
    ▼                            ▼                          ▼
 Soft filter              ORDER ACCEPTED           Execute or Reject
 (may miss)               into system              (must complete)
```

**Before WAL**: Order can be silently dropped
**After WAL**: Order MUST complete its lifecycle (accept or reject)

## Component Responsibilities

### 1. PreChecker (Soft Filter)
**Location**: `pre_check.rs` (optional, can be skipped for simplicity)

| Responsibility | Description |
|----------------|-------------|
| Format validation | Basic sanity checks |
| Fast rejection | Obvious bad orders |
| Rate limiting | Optional DoS protection |

**Key**: NO SIDE EFFECTS! No balance changes, no state changes.

### 2. OrderWAL
**Location**: `order_wal.rs` (to be created)

| Responsibility | Description |
|----------------|-------------|
| Sequence assignment | Monotonically increasing seq_num |
| Durability | Write to disk before any state change |
| Group commit | Batch multiple orders for efficiency |
| Recovery | Replay from last checkpoint |

**WAL Entry Format** (binary):
```
┌─────────┬─────────┬──────────┬─────────┬──────┬───────┬─────┬───────────┐
│ seq_num │ op_type │ order_id │ user_id │ side │ price │ qty │ timestamp │
│  8B     │  1B     │  8B      │  8B     │  1B  │  8B   │ 8B  │  8B       │
└─────────┴─────────┴──────────┴─────────┴──────┴───────┴─────┴───────────┘
Total: 50 bytes per entry
```

### 3. BalanceChecker (Post-WAL)
**Location**: Part of order processing, after WAL

| Responsibility | Description |
|----------------|-------------|
| User validation | Verify user exists |
| Balance check | Ensure sufficient available balance |
| Fund locking | Freeze required amount |
| Rejection handling | Mark order as rejected if check fails |

**Critical**: This runs in single thread, making it naturally atomic.

### 4. MatchingEngine (ME)
**Location**: `engine.rs`

| Responsibility | Description |
|----------------|-------------|
| Order matching | Price-time priority algorithm |
| Trade generation | Create trade records |
| Order book management | Maintain bid/ask levels |

**Key Principle**: ME is a **pure state machine**
- No I/O operations
- No balance operations
- Deterministic: same input → same output
- Replayable from WAL

### 5. Settlement
**Location**: `settlement.rs` (to be created)

| Responsibility | Description |
|----------------|-------------|
| Balance transfer | Execute the trade financially |
| Ledger writing | Audit trail of all balance changes |
| Event emission | Notify downstream systems |

## Data Flow

```
Order → [PreCheck] → [OrderBuffer] → WAL → BalanceLock → ME → [TradesBuffer] → Settlement
              │                       │         │                                   │
         (soft filter)           (persisted)  [reject]                        [ledger]
              │                       │         │                                   │
              ▼                       ▼         ▼                                   ▼
         early reject            committed  rejected              balance updated + events
         (no record)              to WAL    (recorded)
```

## Recovery Process

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              RECOVERY FLOW                                       │
└─────────────────────────────────────────────────────────────────────────────────┘

1. Load latest snapshot (checkpoint)
   ├── OrderBook state
   ├── User balances
   └── Last processed seq_num = N

2. Replay WAL from seq_num = N+1
   ├── For each WAL entry:
   │   ├── Check balance & lock (may reject)
   │   ├── If accepted: apply to ME
   │   └── Apply settlement
   └── Continue until end of WAL

3. Ready to accept new orders

Note: Recovery replays the FULL order lifecycle, including
possible rejections due to insufficient balance.
```

## Why Single-Threaded?

| Concern | Multi-threaded | Single-threaded |
|---------|----------------|-----------------|
| Double-spend | Need locks/CAS | Impossible |
| Complexity | High | Low |
| Debugging | Hard | Easy |
| Latency | May have lock contention | Predictable |
| Throughput | Can be higher | Limited by CPU |

For most trading systems, **single-threaded is sufficient** and simpler.
Horizontal scaling is done by sharding (each symbol on different service).

## Module Structure

```
src/
├── lib.rs              # Public API
├── types.rs            # Core type aliases
├── config.rs           # Trading configuration
│
├── order_wal.rs        # [NEW] Write-ahead log
│
├── orderbook.rs        # Order book data structure
├── engine.rs           # Matching engine (pure)
│
├── settlement.rs       # [NEW] Trade settlement + ledger
├── ledger.rs           # Ledger entry types
│
├── balance.rs          # Balance type
├── user_account.rs     # User account management
│
├── models.rs           # Order, Trade structs
├── perf.rs             # Performance metrics
└── csv_io.rs           # CSV utilities (testing)
```

## Key Design Decisions

### 1. ME is Pure
The Matching Engine does NOT:
- Check balances (done after WAL)
- Update balances (done in Settlement)
- Write to disk (done in WAL)
- Emit events (done in Settlement)

### 2. WAL Before Balance Lock
Why not lock before WAL?
- Pre-check is just a soft filter (no side effects)
- Real validation happens after WAL
- This allows batch WAL writes
- Rejected orders are still properly recorded

### 3. Single-Threaded Execution
Why single-threaded?
- Natural atomicity for balance operations
- No double-spend risk
- Simpler to reason about
- Easier recovery

### 4. Full Lifecycle for WAL'd Orders
Once in WAL, an order MUST:
- Complete balance check (accept or reject)
- If accepted: go through ME and Settlement
- Be properly recorded in final state

## Performance Considerations

| Stage | Latency Target | Notes |
|-------|----------------|-------|
| PreCheck | < 1µs | Optional soft filter |
| WAL Write | < 10µs | Sequential write, group commit |
| Balance Lock | < 1µs | In-memory, single-threaded |
| Matching | < 10µs | BTreeMap O(log n) |
| Settlement | < 1µs | In-memory balance update |
| Ledger | < 100µs | Can be async/batched |

**Total target**: < 50µs per order (P99)

## Future Enhancements

1. **Async Settlement**: Process trades in background
2. **Checkpointing**: Periodic snapshots for faster recovery
3. **Sharding**: Multiple order books for different symbols
4. **Replication**: WAL shipping to replicas
