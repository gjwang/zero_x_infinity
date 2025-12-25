# 0x0D Snapshot & Recovery Architecture Design

> **Status**: 📋 DRAFT - Awaiting Review  
> **Author**: Architect (AI)  
> **Date**: 2024-12-25

---

## 1. Executive Summary

### 🎯 Goal
Enable the matching engine to **persist state and recover** after graceful shutdown or crash, with **minimal data loss** and **fast restart**.

### Key Metrics
| Metric | Target |
|--------|--------|
| **Recovery Time (RTO)** | < 5 seconds for 1M orders |
| **Recovery Point (RPO)** | Zero data loss with proper shutdown |
| **Snapshot Size** | ~100MB per 1M active orders |
| **Snapshot Frequency** | Every 10 minutes or N events |

---

## 2. Current Architecture Analysis

### 2.1 Stateful Components

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SYSTEM STATE (In-Memory)                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────┐      ┌─────────────────────┐              │
│  │      UBSCore        │      │     MatchingEngine  │              │
│  │  ┌───────────────┐  │      │  ┌───────────────┐  │              │
│  │  │ accounts:     │  │      │  │ books:        │  │              │
│  │  │ HashMap<      │  │      │  │ HashMap<      │  │              │
│  │  │   UserId,     │  │      │  │   SymbolId,   │  │              │
│  │  │   UserAccount>│  │      │  │   OrderBook>  │  │              │
│  │  └───────────────┘  │      │  └───────────────┘  │              │
│  │  ┌───────────────┐  │      │  ┌───────────────┐  │              │
│  │  │ wal_writer:   │  │      │  │ trade_id_seq  │  │              │
│  │  │ WalWriter     │  │      │  └───────────────┘  │              │
│  │  └───────────────┘  │      │                     │              │
│  └─────────────────────┘      └─────────────────────┘              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                    PERSISTENT STATE (Disk)                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────┐      ┌─────────────────────┐              │
│  │        WAL          │      │      TDengine       │              │
│  │  (Write-Ahead Log)  │      │   (Trading Data)    │              │
│  │  ────────────────   │      │   ────────────────  │              │
│  │  seq_id,timestamp,  │      │   orders, trades,   │              │
│  │  order_id, user_id, │      │   balance_events,   │              │
│  │  price, qty, side   │      │   klines            │              │
│  └─────────────────────┘      └─────────────────────┘              │
│                                                                     │
│  ┌─────────────────────┐      ┌─────────────────────┐              │
│  │     PostgreSQL      │      │     Snapshot        │              │
│  │   (Configuration)   │      │   (NEW - Proposed)  │              │
│  │   ────────────────  │      │   ────────────────  │              │
│  │   users, symbols,   │      │   balances.bin      │              │
│  │   fee_tiers         │      │   orderbook.bin     │              │
│  └─────────────────────┘      │   metadata.json     │              │
│                               └─────────────────────┘              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Existing WAL Design

The WAL is already implemented in `wal.rs`:

```rust
// Current WAL entry structure
pub struct WalEntry {
    pub seq_id: SeqNum,
    pub timestamp_ns: u64,
    pub order_id: u64,
    pub user_id: u64,
    pub symbol_id: u32,
    pub price: u64,
    pub qty: u64,
    pub side: Side,
    pub order_type: OrderType,
}
```

**Key observation**: WAL records **incoming orders**, not state. Recovery requires:
1. **Replaying all orders** from WAL → Slow for large history
2. OR **Snapshot + WAL tail replay** → Fast ✅

---

## 3. Proposed Design

### 3.1 Snapshot Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SNAPSHOT LIFECYCLE                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────┐    ┌──────────────┐    ┌──────────────┐             │
│  │  Running │───▶│ Create       │───▶│   Snapshot   │             │
│  │  System  │    │ Snapshot     │    │   + WAL tail │             │
│  └──────────┘    │ @ seq_id=N   │    └──────────────┘             │
│       │          └──────────────┘           │                      │
│       │                                     ▼                      │
│       │          ┌────────────────────────────────────┐            │
│       │          │        RECOVERY FLOW               │            │
│       │          │  1. Load Snapshot (state @ seq=N)  │            │
│       │          │  2. Replay WAL from seq=N+1        │            │
│       │          │  3. System ready                   │            │
│       │          └────────────────────────────────────┘            │
│       ▼                                     │                      │
│  (Continue                                  │                      │
│   processing)                               ▼                      │
│       │                            ┌──────────┐                    │
│       └───────────────────────────▶│ Recovered│                    │
│                                    │  System  │                    │
│                                    └──────────┘                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Snapshot Contents

```rust
/// Snapshot metadata
struct SnapshotMetadata {
    version: u32,               // Format version (for migrations)
    created_at: DateTime<Utc>,  // Creation timestamp
    wal_seq_id: SeqNum,         // WAL sequence at snapshot time
    trade_id_seq: u64,          // Next trade ID
    order_count: u64,           // Number of orders in snapshot
    user_count: u64,            // Number of users in snapshot
    checksum: u64,              // CRC64 of data files
}

/// Balance snapshot (per user, per asset)
struct BalanceSnapshot {
    user_id: UserId,
    asset_id: AssetId,
    avail: u64,
    frozen: u64,
    lock_version: u64,
    settle_version: u64,
}

/// Order snapshot (resting orders only)
struct OrderSnapshot {
    order_id: u64,
    user_id: u64,
    symbol_id: u32,
    price: u64,
    qty: u64,          // Remaining qty
    filled_qty: u64,
    side: Side,
    status: OrderStatus,
    created_at: u64,
}
```

### 3.3 Snapshot File Layout

```
data/snapshots/
├── latest -> 20241225_183000/           # Symlink to latest valid snapshot
├── 20241225_183000/                     # Snapshot directory (timestamp)
│   ├── metadata.json                    # Snapshot metadata
│   ├── balances.bin                     # Binary serialized balances
│   ├── orders.bin                       # Binary serialized orders
│   └── COMPLETE                         # Marker file (atomic completion)
└── 20241225_180000/                     # Previous snapshot (for rollback)
    └── ...
```

### 3.4 Atomicity Guarantee

```
Snapshot Creation Protocol:
1. Create new directory: 20241225_183000/
2. Serialize balances → balances.bin.tmp
3. Serialize orders → orders.bin.tmp  
4. Rename .tmp → final files (atomic on POSIX)
5. Write metadata.json
6. Create COMPLETE marker file
7. Update "latest" symlink (atomic)
8. Delete old snapshots (keep last N)

If crash during steps 1-6: Incomplete snapshot has no COMPLETE marker → ignored
If crash during step 7: Old symlink still valid → rollback automatic
```

---

## 4. Recovery Protocol

### 4.1 Recovery Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    STARTUP RECOVERY FLOW                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌────────────────┐                                                │
│  │ 1. Check for   │                                                │
│  │    valid       │                                                │
│  │    snapshot    │                                                │
│  └───────┬────────┘                                                │
│          │                                                         │
│          ▼                                                         │
│  ┌────────────────┐      No         ┌────────────────────┐        │
│  │ Snapshot found?├───────────────▶│ Cold start (empty) │        │
│  └───────┬────────┘                 │ or full WAL replay │        │
│          │ Yes                      └────────────────────┘        │
│          ▼                                                         │
│  ┌────────────────┐                                                │
│  │ 2. Load        │                                                │
│  │    metadata    │                                                │
│  │    (get seq_id)│                                                │
│  └───────┬────────┘                                                │
│          │                                                         │
│          ▼                                                         │
│  ┌────────────────┐                                                │
│  │ 3. Load        │                                                │
│  │    balances    │                                                │
│  │    → UBSCore   │                                                │
│  └───────┬────────┘                                                │
│          │                                                         │
│          ▼                                                         │
│  ┌────────────────┐                                                │
│  │ 4. Load orders │                                                │
│  │    → OrderBook │                                                │
│  └───────┬────────┘                                                │
│          │                                                         │
│          ▼                                                         │
│  ┌────────────────┐                                                │
│  │ 5. Replay WAL  │                                                │
│  │    from seq+1  │                                                │
│  │    to end      │                                                │
│  └───────┬────────┘                                                │
│          │                                                         │
│          ▼                                                         │
│  ┌────────────────┐                                                │
│  │ 6. Verify      │                                                │
│  │    consistency │                                                │
│  │    (checksums) │                                                │
│  └───────┬────────┘                                                │
│          │                                                         │
│          ▼                                                         │
│  ┌────────────────┐                                                │
│  │ ✅ READY      │                                                │
│  │    Accept      │                                                │
│  │    new orders  │                                                │
│  └────────────────┘                                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Graceful Shutdown

```rust
async fn graceful_shutdown() {
    // 1. Stop accepting new orders
    gateway.pause();
    
    // 2. Wait for in-flight orders to complete
    pipeline.drain().await;
    
    // 3. Flush WAL
    wal.flush()?;
    
    // 4. Create final snapshot
    snapshot.create(ubscore, orderbooks).await?;
    
    // 5. Shutdown complete
    info!("Graceful shutdown complete at seq={}", wal.current_seq());
}
```

### 4.3 Crash Recovery

```
Crash at any point:
1. Startup finds latest valid snapshot
2. Load snapshot state (state @ seq=N)
3. Replay WAL entries with seq > N
4. Any entry in WAL but without TDengine record = re-execute
5. Continue from last committed state

Idempotency: Orders replayed from WAL must be idempotent
- Use order_id as dedup key
- Skip if order_id already exists in restored state
```

---

## 5. Snapshot Trigger Strategies

### 5.1 Trigger Options

| Trigger | Pros | Cons |
|---------|------|------|
| **Time-based** (every 10 min) | Simple, predictable | May miss busy periods |
| **Event-based** (every N orders) | Adapts to load | Less predictable timing |
| **Hybrid** (whichever first) | Best of both | More complex |
| **Graceful shutdown** | Ensures clean exit | Only on controlled stop |

### 5.2 Recommended: Hybrid Approach

```rust
struct SnapshotConfig {
    time_interval: Duration,    // e.g., 10 minutes
    event_threshold: u64,       // e.g., 100_000 orders
    min_interval: Duration,     // e.g., 1 minute (prevent thrashing)
}

fn should_snapshot(last_snapshot: Instant, events_since: u64, config: &SnapshotConfig) -> bool {
    let time_elapsed = last_snapshot.elapsed();
    
    if time_elapsed < config.min_interval {
        return false; // Too recent
    }
    
    time_elapsed >= config.time_interval || events_since >= config.event_threshold
}
```

---

## 6. Data Consistency Guarantees

### 6.1 Consistency Model

| Scenario | Guarantee |
|----------|-----------|
| **Graceful shutdown** | Zero data loss, exact state recovery |
| **Crash after WAL flush** | Zero data loss (WAL replay) |
| **Crash before WAL flush** | Loss of uncommitted batch (< flush_interval) |
| **Snapshot corruption** | Fall back to previous snapshot |

### 6.2 Verification

```rust
fn verify_recovery_consistency(
    snapshot_meta: &SnapshotMetadata,
    ubscore: &UBSCore,
    orderbooks: &HashMap<SymbolId, OrderBook>,
) -> Result<(), ConsistencyError> {
    // 1. Verify balance invariants
    for (user_id, account) in ubscore.accounts() {
        for (asset_id, balance) in account.balances() {
            // avail + frozen must be non-negative
            assert!(balance.avail().checked_add(balance.frozen()).is_some());
        }
    }
    
    // 2. Verify order book consistency  
    for (symbol_id, book) in orderbooks {
        // All orders must have matching balance locks
        for order in book.iter_orders() {
            // Verify frozen balance >= order locked amount
            let lock_asset = get_lock_asset(order);
            let balance = ubscore.get_balance(order.user_id, lock_asset);
            // ... detailed verification
        }
    }
    
    // 3. Verify sequence numbers
    assert!(ubscore.current_seq() >= snapshot_meta.wal_seq_id);
    
    Ok(())
}
```

---

## 7. Implementation Phases

### Phase 1: Core Snapshot (3-4 days)
- [ ] `SnapshotWriter`: Serialize UBSCore + OrderBook to binary
- [ ] `SnapshotReader`: Deserialize snapshot files
- [ ] Atomic snapshot creation with marker file
- [ ] Unit tests for serialization/deserialization

### Phase 2: Recovery Integration (2-3 days)
- [ ] Modify `main.rs` startup to check for snapshots
- [ ] Implement recovery flow (load + WAL replay)
- [ ] Add idempotency checks for WAL replay
- [ ] Integration tests for recovery scenarios

### Phase 3: Graceful Shutdown (1-2 days)
- [ ] Add shutdown signal handler (SIGTERM, SIGINT)
- [ ] Implement `graceful_shutdown()` flow
- [ ] Test graceful shutdown + restart cycle

### Phase 4: Production Hardening (2-3 days)
- [ ] Add snapshot retention policy (keep last N)
- [ ] Add corruption detection (checksums)
- [ ] Add metrics (snapshot time, size, recovery time)
- [ ] Documentation + E2E tests

---

## 8. Test Acceptance Checklist

### Unit Tests
- [ ] Snapshot serialization round-trip for Balance
- [ ] Snapshot serialization round-trip for OrderBook
- [ ] Atomic file creation with marker
- [ ] Checksum calculation and verification

### Integration Tests
- [ ] Create snapshot → Stop → Restart → Verify state matches
- [ ] Simulate crash (kill -9) → Restart → Verify WAL replay
- [ ] Corrupt snapshot → Verify fallback to previous
- [ ] Large dataset (1M orders) → Recovery time < 5s

### E2E Tests
- [ ] Full trading cycle → Graceful shutdown → Restart → Continue trading
- [ ] Crash during order processing → Restart → No duplicate fills
- [ ] Multiple snapshots → Verify oldest are cleaned up

---

## 9. ADR: Architecture Decision Record

### ADR-007: Snapshot + WAL Recovery Strategy

**Status**: Proposed

**Context**: 
The matching engine needs to recover state after restart. Options:
1. Full WAL replay from beginning
2. Snapshot + WAL tail replay
3. Database-only recovery (TDengine queries)

**Decision**: 
Use **Snapshot + WAL tail replay**.

**Rationale**:
- Full WAL replay is O(total_orders), too slow for production
- Database-only requires reconstructing order book from trades (complex, slower)
- Snapshot + tail is O(orders_since_snapshot), typically < 100K

**Consequences**:
- Need to implement snapshot serialization
- Need to manage snapshot files (retention, corruption)
- Recovery is fast and deterministic

---

## 10. Technical Decisions (Finalized)

### ADR-008: Serialization Format

| 项目 | 决定 |
|------|------|
| **格式** | `bincode` |
| **理由** | 最快序列化，零配置，项目已依赖 serde |
| **版本控制** | `metadata.json` 中的 `format_version` 字段 |
| **迁移策略** | 版本匹配 + migration 函数 |

### ADR-009: Compression Strategy

| 阶段 | 策略 | 理由 |
|------|------|------|
| **Phase 1** | 不压缩 | 最低延迟，不占用 CPU |
| **Phase 2 首选** | LZ4 | ~800 MB/s 压缩，~4 GB/s 解压 |
| **Phase 2 备选** | Zstd Level 1 | 更好压缩率 (30-40%) |

```rust
// 预留字段设计
enum CompressionMode {
    None,      // Phase 1 ✅
    Lz4,       // Phase 2 首选
    ZstdFast,  // Phase 2 备选
}

struct SnapshotMetadata {
    format_version: u32,
    compression: CompressionMode,  // 预留
    // ...
}
```

### ADR-010: Encryption

| 项目 | 决定 |
|------|------|
| **加密** | 否 |
| **理由** | 内部使用，文件系统权限保护足够 |
| **未来选项** | 如需加密，使用 AES-256-GCM |

---

*Document finalized: 2024-12-25*  
*Status: ✅ APPROVED*

