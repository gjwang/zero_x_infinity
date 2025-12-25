# 0x0D WAL & Snapshot Implementation Plan

> **Status**: 📋 READY FOR IMPLEMENTATION  
> **Author**: Architect Team  
> **Date**: 2024-12-25  
> **Target**: Developer Team

---

## 概述

本实施计划基于以下设计文档：
- [WAL Rotation Design](./0x0D-wal-rotation-design.md)
- [Service-Level Design](./0x0D-service-wal-snapshot-design.md)
- [UBSCore Details](./0x0D-ubscore-wal-snapshot.md)
- [Matching Details](./0x0D-matching-wal-snapshot.md)
- [Settlement Details](./0x0D-settlement-wal-snapshot.md)

---

## 实施原则

### 1. 架构原则

- **每个服务独立 Snapshot + WAL**
- **WAL 由服务自己消费** (备份除外)
- **下游请求上游重放输出**
- **Write-Ahead Logging**: 先写 WAL，再更新内存

### 2. 目录约定

```
data/
├── ubscore-service/
│   ├── wal/
│   │   ├── current.wal
│   │   └── wal-{EPOCH}-{END_SEQ}.wal
│   └── snapshots/
│       ├── snapshot-{SEQ}/
│       └── latest -> snapshot-{SEQ}/
├── matching-service/
│   └── ...
└── settlement-service/
    └── ...
```

---

## Phase 1: UBSCore WAL & Snapshot (P0)

### 1.1 模块结构

```
src/
├── ubscore/
│   ├── mod.rs
│   ├── wal.rs           # WAL writer/reader
│   ├── snapshot.rs      # Snapshot creation/loading
│   └── recovery.rs      # Recovery logic
```

### 1.2 实现任务

#### Task 1.1: WAL Writer

```rust
pub struct UBSCoreWalWriter {
    writer: WalWriterV2,
    next_seq_id: u64,
}

impl UBSCoreWalWriter {
    pub fn append_order(&mut self, order: &InternalOrder) -> Result<u64>;
    pub fn append_cancel(&mut self, cancel: &CancelOrder) -> Result<u64>;
    pub fn append_deposit(&mut self, deposit: &Deposit) -> Result<u64>;
    pub fn append_withdraw(&mut self, withdraw: &Withdraw) -> Result<u64>;
    pub fn flush(&mut self) -> Result<()>;
}
```

**验收标准**:
- ✅ 支持 4 种 Entry Type (Order/Cancel/Deposit/Withdraw)
- ✅ 返回递增的 seq_id
- ✅ CRC32 校验正确
- ✅ 单元测试覆盖率 > 90%

#### Task 1.2: Snapshot Creation

```rust
pub struct UBSCoreSnapshotter {
    data_dir: PathBuf,
}

impl UBSCoreSnapshotter {
    pub fn create_snapshot(
        &self,
        accounts: &FxHashMap<UserId, UserAccount>,
        wal_seq_id: u64,
    ) -> Result<PathBuf>;
}
```

**实现步骤**:
1. 创建临时目录 `.tmp-{timestamp}`
2. 序列化 accounts → `accounts.bin`
3. 计算 CRC64 checksum
4. 写 `metadata.json`
5. 写 `COMPLETE` 标记
6. 原子重命名
7. 更新 `latest` 符号链接

**验收标准**:
- ✅ Atomic creation (COMPLETE 标记)
- ✅ Checksum 验证
- ✅ 符号链接正确
- ✅ 崩溃安全测试通过

#### Task 1.3: Recovery Logic

```rust
pub struct UBSCoreRecovery {
    data_dir: PathBuf,
}

impl UBSCoreRecovery {
    pub fn recover(&self) -> Result<RecoveryState>;
}

pub struct RecoveryState {
    pub accounts: FxHashMap<UserId, UserAccount>,
    pub next_seq_id: u64,
}
```

**恢复流程**:
1. 检查 `snapshots/latest`
2. 如果存在，加载 Snapshot
3. 从 `snapshot.wal_seq_id + 1` 重放 WAL
4. 恢复 `accounts` 和 `next_seq_id`

**验收标准**:
- ✅ 冷启动恢复 (无 Snapshot)
- ✅ 热启动恢复 (有 Snapshot + WAL)
- ✅ WAL 损坏检测
- ✅ 恢复后状态一致性验证

#### Task 1.4: Integration

修改 `src/ubscore.rs`:

```rust
pub struct UBSCore {
    accounts: FxHashMap<UserId, UserAccount>,
    wal_writer: UBSCoreWalWriter,
    snapshotter: UBSCoreSnapshotter,
    next_seq_id: u64,
}

impl UBSCore {
    pub fn new_with_recovery(config: UBSCoreConfig) -> Result<Self> {
        let recovery = UBSCoreRecovery::new(&config.data_dir);
        let state = recovery.recover()?;
        
        Ok(Self {
            accounts: state.accounts,
            wal_writer: UBSCoreWalWriter::new(&config.wal_dir)?,
            snapshotter: UBSCoreSnapshotter::new(&config.data_dir),
            next_seq_id: state.next_seq_id,
        })
    }
    
    pub fn process_order(&mut self, order: InternalOrder) 
        -> Result<OrderResult> 
    {
        // 1. 验证
        self.validate_order(&order)?;
        
        // 2. 写 WAL (关键!)
        let seq_id = self.wal_writer.append_order(&order)?;
        
        // 3. 更新内存
        self.lock_balance(&order)?;
        
        // 4. 输出
        Ok(OrderResult::Valid(ValidOrder { seq_id, ...order }))
    }
    
    pub fn create_snapshot(&self) -> Result<()> {
        self.snapshotter.create_snapshot(
            &self.accounts,
            self.next_seq_id - 1,
        )
    }
}
```

**验收标准**:
- ✅ `process_order` 先写 WAL 再更新内存
- ✅ 定期创建 Snapshot (每 10 分钟或 100K 订单)
- ✅ E2E 测试: 写入 → 崩溃模拟 → 恢复 → 验证一致性

---

## Phase 2: Matching Service WAL & Snapshot (P0)

### 2.1 模块结构

```
src/
├── matching/
│   ├── mod.rs
│   ├── wal.rs           # Trade WAL
│   ├── snapshot.rs      # OrderBook Snapshot
│   └── recovery.rs      # Recovery logic
```

### 2.2 实现任务

#### Task 2.1: Trade WAL Writer

```rust
pub struct MatchingWalWriter {
    writer: WalWriterV2,
    next_trade_id: u64,
}

impl MatchingWalWriter {
    pub fn append_trade(&mut self, trade: &Trade) -> Result<u64>;
}
```

**验收标准**:
- ✅ Trade WAL 写入正确
- ✅ trade_id 递增
- ✅ CRC32 校验

#### Task 2.2: OrderBook Snapshot

```rust
pub struct OrderBookSnapshotter {
    data_dir: PathBuf,
}

impl OrderBookSnapshotter {
    pub fn create_snapshot(
        &self,
        orderbooks: &HashMap<SymbolId, OrderBook>,
        last_order_seq: u64,
        next_trade_id: u64,
    ) -> Result<PathBuf>;
}
```

**Snapshot 格式**:
- `metadata.json` (元数据)
- `orderbook-{symbol_id}.bin` (每个交易对一个文件)
- `COMPLETE` 标记

**验收标准**:
- ✅ 多文件 Snapshot
- ✅ 每个 OrderBook 独立 checksum
- ✅ Atomic creation

#### Task 2.3: Recovery + Replay Request

```rust
pub struct MatchingRecovery {
    data_dir: PathBuf,
    ubscore_endpoint: String,
}

impl MatchingRecovery {
    pub fn recover(&self) -> Result<RecoveryState>;
    
    fn request_ubscore_replay(
        &self,
        from_seq: u64,
    ) -> Result<Vec<ValidOrder>>;
}
```

**恢复流程**:
1. 加载 Snapshot (OrderBooks + last_order_seq)
2. 请求 UBSCore: `replay_orders(from_seq = last_order_seq + 1)`
3. 重新撮合，恢复 OrderBooks

**验收标准**:
- ✅ OrderBook 正确恢复
- ✅ UBSCore 重放集成测试
- ✅ 大数据量恢复测试 (10K+ orders)

---

## Phase 3: Settlement Service WAL & Snapshot (P1)

### 3.1 轻量设计

Settlement 的 Snapshot 非常轻量：
- **Snapshot**: 只有 `last_trade_id` (JSON)
- **WAL**: Checkpoint 每 1,000 笔
- **数据**: 全在 TDengine

### 3.2 实现任务

#### Task 3.1: Checkpoint WAL

```rust
pub struct SettlementWalWriter {
    writer: WalWriterV2,
}

impl SettlementWalWriter {
    pub fn append_checkpoint(&mut self, last_trade_id: u64) -> Result<()>;
}
```

**Checkpoint 间隔**: 每 1,000 笔成交

#### Task 3.2: Progress Snapshot

```rust
pub struct SettlementSnapshotter {
    data_dir: PathBuf,
}

impl SettlementSnapshotter {
    pub fn create_snapshot(&self, last_trade_id: u64) -> Result<()>;
}
```

**Snapshot 内容**:
```json
{
  "format_version": 1,
  "last_trade_id": 10000,
  "created_at": "2024-12-25T20:00:00Z"
}
```

#### Task 3.3: Recovery + Replay Request

```rust
impl SettlementRecovery {
    pub fn recover(&self) -> Result<u64>;  // 返回 last_trade_id
    
    fn request_matching_replay(
        &self,
        from_trade_id: u64,
    ) -> Result<Vec<Trade>>;
}
```

**验收标准**:
- ✅ 幂等性保证 (重复 trade_id 检测)
- ✅ Matching 重放集成测试
- ✅ TDengine 数据一致性验证

---

## Phase 4: Replay Protocol (P1)

### 4.1 UBSCore Replay API

```rust
impl UBSCore {
    pub fn replay_orders<F>(
        &self,
        from_seq: u64,
        to_seq: Option<u64>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(ValidOrder) -> bool,  // 返回 false 停止
    {
        // 从 WAL 读取并重放
        self.wal_reader.replay(from_seq, |header, payload| {
            if let Some(to) = to_seq {
                if header.seq_id > to {
                    return false;
                }
            }
            
            match header.entry_type {
                WalEntryType::Order => {
                    let order = bincode::deserialize(payload)?;
                    callback(order)
                }
                _ => true,
            }
        })
    }
}
```

### 4.2 Matching Replay API

```rust
impl MatchingEngine {
    pub fn replay_trades<F>(
        &self,
        from_trade_id: u64,
        to_trade_id: Option<u64>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(Trade) -> bool,
    {
        self.wal_reader.replay(from_trade_id, |header, payload| {
            if let Some(to) = to_trade_id {
                if header.seq_id > to {
                    return false;
                }
            }
            
            if header.entry_type == WalEntryType::Trade {
                let trade = bincode::deserialize(payload)?;
                callback(trade)
            } else {
                true
            }
        })
    }
}
```

**验收标准**:
- ✅ 流式重放 (不加载全部到内存)
- ✅ 范围查询 (from_seq, to_seq)
- ✅ 停止机制 (callback 返回 false)

---

## 测试策略

### Unit Tests (目标: 90% 覆盖率)

- WAL Writer/Reader
- Snapshot Creation/Loading
- Recovery Logic
- 每个 Entry Type

### Integration Tests

**Test 1: UBSCore E2E**
```
1. 写入 1000 订单
2. 创建 Snapshot
3. 继续写入 500 订单
4. 模拟崩溃 (重启)
5. 恢复
6. 验证 1500 订单状态正确
```

**Test 2: Matching E2E**
```
1. ME 处理 500 订单 (生成 200 成交)
2. 创建 Snapshot
3. 模拟崩溃
4. 恢复 (请求 UBSCore 重放)
5. 验证 OrderBook 状态正确
```

**Test 3: 全链路**
```
1. UBSCore → ME → Settlement 完整流程
2. 各服务分别崩溃模拟
3. 恢复并验证数据一致性
```

### Performance Tests

- 恢复速度: 100K 订单 < 5s
- Snapshot 创建: 100K 账户 < 2s
- WAL 写入: > 100K ops/s

---

## 实施顺序

| Phase | 内容 | 优先级 | 预计工时 |
|-------|------|--------|----------|
| **Phase 1** | UBSCore WAL + Snapshot | **P0** | 3-5 天 |
| **Phase 2** | Matching WAL + Snapshot | **P0** | 3-5 天 |
| **Phase 3** | Settlement WAL + Snapshot | **P1** | 2-3 天 |
| **Phase 4** | Replay Protocol | **P1** | 2 天 |
| **Testing** | Integration + E2E | **P0** | 3 天 |

**总计**: 13-18 天

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| WAL 损坏 | 恢复失败 | CRC32 校验 + Epoch 机制 |
| Snapshot 创建失败 | 部分快照 | COMPLETE 标记 + 原子重命名 |
| 重放延迟过长 | ME 启动慢 | 增量 Snapshot 频率 |
| 跨服务 seq 不一致 | 数据错乱 | 重放协议严格验证 |

---

## 验收标准 (整体)

### 功能性

- ✅ 所有服务支持 Snapshot + WAL
- ✅ 崩溃后正确恢复
- ✅ 跨服务重放协议工作正常

### 性能

- ✅ WAL 写入 TPS > 100K
- ✅ 恢复时间 (100K 数据) < 10s
- ✅ Snapshot 创建不阻塞服务 > 500ms

### 可靠性

- ✅ Checksum 验证 100%
- ✅ 原子操作 (无部分文件)
- ✅ 崩溃安全测试通过

---

*Implementation Plan created: 2024-12-25*
