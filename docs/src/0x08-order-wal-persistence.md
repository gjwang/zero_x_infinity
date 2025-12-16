# Chapter 0x08: Order Processing Pipeline Architecture

## Overview

This document describes the order processing pipeline architecture for 0xInfinity, 
a high-frequency trading matching engine. The design emphasizes:

- **Durability**: Orders are persisted to WAL before matching
- **Correctness**: Pure state machine for matching, deterministic replay
- **Performance**: Batching and minimal I/O in critical path
- **Modularity**: Clear separation of concerns

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
│  1. PRE-CHECK                                                                    │
│  ├── User exists?                                                                │
│  ├── Balance sufficient?                                                         │
│  └── Lock funds (freeze)                                                         │
│                                                                                   │
│  Result: ValidatedOrder or Reject                                                │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                              ┌───────────────┴───────────────┐
                              │         REJECT                 │──────────► Response
                              └───────────────────────────────┘
                                              │ ACCEPT
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  2. ORDER BUFFER (Pre-WAL)                                                       │
│  ├── Validated orders queue                                                      │
│  └── Batch for WAL write efficiency                                              │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  3. WRITE ORDER_WAL                                                              │
│  ├── Append to WAL (sequential write)                                            │
│  ├── fsync / group commit                                                        │
│  └── Assign seq_num                                                              │
│                                                                                   │
│  Format: [seq_num | op_type | order_id | user_id | side | price | qty | ts]     │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  4. ORDER BUFFER (Post-WAL)                                                      │
│  ├── WAL confirmed orders                                                        │
│  └── Ready for matching                                                          │
└─────────────────────────────────────────────────────────────────────────────────┘
                                              │
                                              ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  5. MATCHING ENGINE (ME)                                                         │
│  ├── Pure state machine (no I/O, no balance ops)                                 │
│  ├── OrderBook (BTreeMap-based)                                                  │
│  ├── Price-Time priority matching                                                │
│  └── Generate trades list                                                        │
│                                                                                   │
│  Input:  ValidatedOrder                                                          │
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
│  ├── Update buyer balance: -quote(frozen), +base(avail)                         │
│  ├── Update seller balance: -base(frozen), +quote(avail)                        │
│  ├── Write ledger entries (audit trail)                                          │
│  └── Emit balance events (for downstream systems)                                │
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

## Component Responsibilities

### 1. PreChecker
**Location**: `pre_check.rs` (to be created)

| Responsibility | Description |
|----------------|-------------|
| User validation | Verify user exists and is active |
| Balance check | Ensure sufficient available balance |
| Fund locking | Freeze required amount for the order |
| Risk checks | Optional: position limits, rate limits |

**Input**: Raw order request  
**Output**: `ValidatedOrder` (with locked_amount) or `Reject`

### 2. OrderWAL
**Location**: `order_wal.rs` (to be created)

| Responsibility | Description |
|----------------|-------------|
| Sequence assignment | Monotonically increasing seq_num |
| Durability | Write to disk before matching |
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

### 3. MatchingEngine (ME)
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

### 4. Settlement
**Location**: `settlement.rs` (to be created)

| Responsibility | Description |
|----------------|-------------|
| Balance transfer | Execute the trade financially |
| Ledger writing | Audit trail of all balance changes |
| Event emission | Notify downstream systems |

**Per Trade Settlement**:
```
Buyer:
  spend_frozen(quote_asset, trade_cost)
  deposit(base_asset, trade_qty)

Seller:
  spend_frozen(base_asset, trade_qty)
  deposit(quote_asset, trade_cost)
```

## Data Flow

```
Order → PreCheck → [balance locked] → WAL → ME → [trades] → Settlement → [balance updated]
                         ↓                              ↓
                   Reject Response              Ledger + Events
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
   │   ├── Skip pre-check (already passed)
   │   ├── Apply to ME
   │   └── Apply settlement
   └── Continue until end of WAL

3. Ready to accept new orders
```

## Module Structure

```
src/
├── lib.rs              # Public API
├── types.rs            # Core type aliases
├── config.rs           # Trading configuration
│
├── pre_check.rs        # [NEW] Balance check and lock
├── order_wal.rs        # [NEW] Write-ahead log
│
├── orderbook.rs        # Order book data structure
├── engine.rs           # Matching engine (pure)
│
├── settlement.rs       # [NEW] Trade settlement
├── ledger.rs           # Audit log
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
- Check balances (done in PreCheck)
- Update balances (done in Settlement)
- Write to disk (done in WAL)
- Emit events (done in Settlement)

Benefits:
- Deterministic replay
- Easy to test
- Clear responsibility

### 2. Lock Before WAL
Funds are locked before writing to WAL, ensuring:
- No double-spend if crash after WAL write
- Failed orders don't consume WAL space
- Rejection is fast (no I/O)

### 3. WAL Before Matching
Order is persisted before matching:
- Guaranteed durability
- Can replay on crash
- Sequence number provides ordering

### 4. Settlement After Matching
Balance updates happen after matching:
- Only executed trades affect balances
- Ledger reflects actual trades
- Atomic commit possible

## Performance Considerations

| Stage | Latency Target | Notes |
|-------|----------------|-------|
| PreCheck | < 1µs | In-memory hash lookup |
| WAL Write | < 10µs | Sequential write, group commit |
| Matching | < 10µs | BTreeMap O(log n) |
| Settlement | < 1µs | In-memory balance update |
| Ledger | < 100µs | Can be async/batched |

**Total target**: < 50µs per order (P99)

## Future Enhancements

1. **Async Settlement**: Process trades in background
2. **Checkpointing**: Periodic snapshots for faster recovery
3. **Sharding**: Multiple order books for different symbols
4. **Replication**: WAL shipping to replicas
