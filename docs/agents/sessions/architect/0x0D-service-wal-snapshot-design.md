# 0x0D Service-Level WAL & Snapshot Design

> **Status**: 📋 DRAFT  
> **Author**: Architect Team  
> **Date**: 2024-12-25  
> **Parent**: [0x0D WAL Rotation Design](./0x0D-wal-rotation-design.md)

---

## 1. UBSCore Service

### 1.1 状态概述

| 状态 | 数据结构 | 说明 |
|------|----------|------|
| **accounts** | `FxHashMap<UserId, UserAccount>` | 所有用户余额 |
| **next_seq_id** | `u64` | 下一个订单序列号 |

### 1.2 WAL 设计

**WAL 类型**: Order WAL (必须)

| Entry Type | 内容 | 说明 |
|------------|------|------|
| `Order` | OrderPayload | 新订单 (pre-trade) |
| `Cancel` | CancelPayload | 撤单 |
| `Deposit` | FundingPayload | 充值 |
| `Withdraw` | FundingPayload | 提现 |

**WAL 路径**: `data/ubscore-service/wal/`

### 1.3 Snapshot 设计

**快照内容**:

```rust
struct UBSCoreSnapshot {
    // Metadata
    format_version: u32,
    created_at: u64,
    wal_seq_id: u64,        // 快照对应的最后 WAL seq
    
    // State
    accounts: Vec<(UserId, UserAccount)>,
}
```

**快照路径**: `data/ubscore-service/snapshots/`

**触发条件**:
- 时间间隔: 每 10 分钟
- 事件阈值: 每 100,000 订单

### 1.4 恢复流程

```
1. 加载 latest Snapshot
2. 从 Snapshot.wal_seq_id 开始重放 Order WAL
3. 恢复 accounts 状态
4. 开始接受新订单
```

### 1.5 重放输出 API

```rust
/// 下游 (ME) 请求重放
pub fn replay_orders(&self, from_seq: u64, callback: impl FnMut(ValidOrder))
```

---

## 2. Matching Service

### 2.1 状态概述

| 状态 | 数据结构 | 说明 |
|------|----------|------|
| **orderbooks** | `HashMap<SymbolId, OrderBook>` | 所有交易对的订单簿 |
| **next_trade_id** | `u64` | 下一个成交 ID |
| **last_order_seq** | `u64` | 最后处理的订单 seq |

### 2.2 WAL 设计

**WAL 类型**: Trade WAL (自己消费 + 给下游重放)

| Entry Type | 内容 | 说明 |
|------------|------|------|
| `Trade` | TradePayload | 成交事件 |
| `OrderUpdate` | OrderUpdatePayload | 订单状态变更 |

**WAL 路径**: `data/matching-service/wal/`

### 2.3 Snapshot 设计

**快照内容**:

```rust
struct MatchingSnapshot {
    format_version: u32,
    created_at: u64,
    last_order_seq: u64,    // 来自 UBSCore 的最后订单 seq
    next_trade_id: u64,
    
    // State
    orderbooks: HashMap<SymbolId, OrderBookSnapshot>,
}

struct OrderBookSnapshot {
    symbol_id: u32,
    bids: Vec<OrderEntry>,  // 按价格排序的买单
    asks: Vec<OrderEntry>,  // 按价格排序的卖单
}
```

**快照路径**: `data/matching-service/snapshots/`

**触发条件**:
- 时间间隔: 每 5 分钟
- 事件阈值: 每 50,000 订单

### 2.4 恢复流程

```
1. 加载 latest Snapshot (OrderBook @ last_order_seq=X)
2. 请求 UBSCore: replay_orders(from_seq=X+1)
3. 重新匹配，恢复 orderbooks
4. 开始正常撮合
```

### 2.5 重放输出 API

```rust
/// 下游 (Settlement) 请求重放
pub fn replay_trades(&self, from_trade_id: u64, callback: impl FnMut(Trade))
```

---

## 3. Settlement Service

### 3.1 状态概述

| 状态 | 数据结构 | 说明 |
|------|----------|------|
| **last_trade_id** | `u64` | 最后处理的成交 ID |
| **pending_settlements** | `Vec<Settlement>` | 待处理结算 |

### 3.2 WAL 设计

**WAL 类型**: 状态 WAL (轻量，只记录进度)

| Entry Type | 内容 | 说明 |
|------------|------|------|
| `SettlementComplete` | trade_id, timestamp | 结算完成标记 |

**WAL 路径**: `data/settlement-service/wal/`

### 3.3 Snapshot 设计

**快照内容**:

```rust
struct SettlementSnapshot {
    format_version: u32,
    created_at: u64,
    last_trade_id: u64,     // 最后结算的 trade_id
}
```

**快照路径**: `data/settlement-service/snapshots/`

**触发条件**:
- 每处理 10,000 笔结算

### 3.4 恢复流程

```
1. 加载 latest Snapshot (last_trade_id=Y)
2. 请求 ME: replay_trades(from_trade_id=Y+1)
3. 继续结算未完成的交易
```

---

## 4. 公共设计

### 4.1 目录结构

```
data/
├── ubscore-service/
│   ├── wal/
│   │   ├── current.wal
│   │   └── wal-00001-0000001000.wal
│   └── snapshots/
│       ├── snapshot-1000/
│       │   ├── metadata.json
│       │   └── accounts.bin
│       └── latest -> snapshot-1000/
│
├── matching-service/
│   ├── wal/
│   │   └── ...
│   └── snapshots/
│       └── ...
│
└── settlement-service/
    ├── wal/
    │   └── ...
    └── snapshots/
        └── ...
```

### 4.2 通用配置

```rust
pub struct ServicePersistenceConfig {
    pub data_dir: PathBuf,              // 服务数据目录
    
    // WAL
    pub wal_max_file_size: u64,         // 默认 256MB
    pub wal_max_duration: Duration,     // 默认 1 小时
    
    // Snapshot
    pub snapshot_interval: Duration,     // 默认 10 分钟
    pub snapshot_event_threshold: u64,   // 默认 100,000
    pub snapshot_keep_count: usize,      // 默认 3
}
```

### 4.3 重放协议

```rust
/// 重放请求
pub struct ReplayRequest {
    pub from_seq: u64,
    pub to_seq: Option<u64>,  // None = 到最新
}

/// 重放响应 (流式)
pub trait ReplayProvider {
    fn replay<F>(&self, request: ReplayRequest, callback: F)
    where F: FnMut(Event) -> bool;  // 返回 false 停止
}
```

---

## 5. 实现优先级

| 阶段 | 服务 | 内容 | 优先级 |
|------|------|------|--------|
| **Phase 1** | UBSCore | Order WAL + Snapshot | **P0** |
| **Phase 2** | Matching | Trade WAL + Snapshot | **P0** |
| **Phase 3** | Settlement | 状态 WAL + Snapshot | **P1** |
| **Phase 4** | 全部 | 重放协议 | **P1** |

---

*Document created: 2024-12-25*
