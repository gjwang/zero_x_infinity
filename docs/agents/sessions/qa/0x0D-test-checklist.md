# 0x0D WAL & Snapshot QA Test Checklist

> **Status**: 📋 READY FOR QA  
> **Author**: Architect Team  
> **Date**: 2024-12-25  
> **Target**: QA Team

---

## 测试概述

本测试清单基于：
- [Implementation Plan](../developer/0x0D-implementation-plan.md)
- [Service Designs](../architect/0x0D-service-wal-snapshot-design.md)

---

## Phase 1: UBSCore WAL & Snapshot

### 1.1 WAL Writer Tests

#### Test 1.1.1: Entry Type Coverage
- [ ] **Order** entry 写入成功
- [ ] **Cancel** entry 写入成功
- [ ] **Deposit** entry 写入成功
- [ ] **Withdraw** entry 写入成功
- [ ] seq_id 严格递增 (无跳号)

**验证方法**:
```bash
cargo test ubscore_wal_entry_types
```

#### Test 1.1.2: WAL Integrity
- [ ] CRC32 checksum 计算正确
- [ ] Header 大小 = 20 bytes
- [ ] Payload bincode 序列化无错
- [ ] 损坏数据检测 (修改 1 byte，CRC 失败)

**验证方法**:
```bash
cargo test ubscore_wal_integrity
```

#### Test 1.1.3: Performance
- [ ] 写入 TPS > 100,000 ops/s
- [ ] 单笔写入延迟 < 10 μs (P99)
- [ ] 批量写入 (1000) < 5 ms

**验证方法**:
```bash
cargo bench --bench ubscore_wal_perf
```

---

### 1.2 Snapshot Tests

#### Test 1.2.1: Snapshot Creation
- [ ] 创建 `.tmp-{timestamp}` 临时目录
- [ ] `accounts.bin` 文件存在
- [ ] `metadata.json` 格式正确
  - `format_version`: 1
  - `wal_seq_id`: 正确值
  - `accounts_checksum`: 匹配
- [ ] `COMPLETE` 标记文件存在
- [ ] 原子重命名成功
- [ ] `latest` 符号链接指向最新 Snapshot

**验证方法**:
```bash
cargo test ubscore_snapshot_creation
ls -la data/ubscore-service/snapshots/
```

#### Test 1.2.2: Snapshot Loading
- [ ] 加载 Snapshot 成功
- [ ] Checksum 验证通过
- [ ] accounts 反序列化正确
- [ ] wal_seq_id 读取正确

**验证方法**:
```bash
cargo test ubscore_snapshot_loading
```

#### Test 1.2.3: Crash Safety
- [ ] 创建过程中断 (kill -9) → 重启后无损坏
- [ ] 部分 Snapshot (无 COMPLETE) 被忽略
- [ ] `latest` 链接指向最后完整的 Snapshot

**验证方法**:
```bash
./scripts/test_ubscore_crash_safety.sh
```

---

### 1.3 Recovery Tests

#### Test 1.3.1: Cold Start (无 Snapshot)
- [ ] 从 seq_id=0 开始
- [ ] accounts 为空
- [ ] next_seq_id = 1

**验证方法**:
```bash
rm -rf data/ubscore-service/snapshots
cargo run --bin ubscore
# 验证日志: "Cold start, no snapshot found"
```

#### Test 1.3.2: Hot Start (有 Snapshot)
- [ ] 加载 Snapshot @ seq=1000
- [ ] 从 seq=1001 开始重放 WAL
- [ ] accounts 状态正确
- [ ] next_seq_id = (最后 WAL seq + 1)

**验证方法**:
```bash
cargo test ubscore_recovery_hot_start
```

#### Test 1.3.3: WAL Replay
- [ ] 重放 Order: 锁定余额
- [ ] 重放 Cancel: 解锁余额
- [ ] 重放 Deposit: 增加余额
- [ ] 重放 Withdraw: 减少余额
- [ ] 重放后 accounts 与预期一致

**验证方法**:
```bash
cargo test ubscore_wal_replay
```

---

### 1.4 Integration Tests

#### Test 1.4.1: E2E Flow
```
1. 写入 1,000 订单
2. 创建 Snapshot @ seq=1000
3. 继续写入 500 订单
4. 模拟崩溃 (kill -9)
5. 重启恢复
6. 验证 1,500 订单状态正确
```

- [ ] 恢复后 next_seq_id = 1501
- [ ] 所有账户余额正确
- [ ] 无数据丢失

**验证方法**:
```bash
./scripts/test_ubscore_e2e.sh
```

---

## Phase 2: Matching Service WAL & Snapshot

### 2.1 Trade WAL Tests

#### Test 2.1.1: Trade Entry
- [ ] Trade WAL 写入成功
- [ ] trade_id 递增
- [ ] TradePayload 序列化正确
- [ ] CRC32 校验

**验证方法**:
```bash
cargo test matching_trade_wal
```

---

### 2.2 OrderBook Snapshot Tests

#### Test 2.2.1: Multi-File Snapshot
- [ ] `metadata.json` 正确
- [ ] `orderbook-{symbol_id}.bin` 每个交易对一个文件
- [ ] 每个文件独立 checksum
- [ ] COMPLETE 标记

**验证方法**:
```bash
cargo test matching_orderbook_snapshot
```

#### Test 2.2.2: OrderBook Restore
- [ ] bids 价格降序
- [ ] asks 价格升序
- [ ] 同价格订单按时间排序
- [ ] 所有字段正确 (price, qty, order_id, user_id)

**验证方法**:
```bash
cargo test matching_orderbook_restore
```

---

### 2.3 Recovery + Replay Tests

#### Test 2.3.1: UBSCore Replay Request
- [ ] ME 请求 UBSCore: `replay_orders(from_seq=X)`
- [ ] UBSCore 返回 ValidOrder 流
- [ ] ME 重新撮合
- [ ] OrderBook 恢复正确

**验证方法**:
```bash
cargo test matching_ubscore_replay
```

#### Test 2.3.2: Large Data Recovery
- [ ] 恢复 10,000+ 订单
- [ ] 恢复时间 < 10s
- [ ] OrderBook 深度正确

**验证方法**:
```bash
cargo test matching_large_recovery
```

---

### 2.4 Integration Tests

#### Test 2.4.1: ME E2E Flow
```
1. ME 处理 500 订单 (生成 200 成交)
2. 创建 Snapshot @ last_order_seq=500
3. 模拟崩溃
4. 恢复 (请求 UBSCore 重放)
5. 验证 OrderBook 状态
```

- [ ] OrderBook 深度一致
- [ ] 成交记录完整
- [ ] next_trade_id 正确

**验证方法**:
```bash
./scripts/test_matching_e2e.sh
```

---

## Phase 3: Settlement Service WAL & Snapshot

### 3.1 Checkpoint WAL Tests

#### Test 3.1.1: Checkpoint Writing
- [ ] 每 1,000 笔成交写 Checkpoint
- [ ] Checkpoint payload 正确 (last_trade_id, timestamp)
- [ ] WAL 文件大小合理 (轻量)

**验证方法**:
```bash
cargo test settlement_checkpoint_wal
```

---

### 3.2 Progress Snapshot Tests

#### Test 3.2.1: Lightweight Snapshot
- [ ] `metadata.json` 只包含 `last_trade_id`
- [ ] 文件大小 < 1 KB
- [ ] 创建速度 < 10 ms

**验证方法**:
```bash
cargo test settlement_snapshot
```

---

### 3.3 Recovery + Replay Tests

#### Test 3.3.1: Matching Replay Request
- [ ] Settlement 请求 ME: `replay_trades(from_trade_id=Y)`
- [ ] ME 返回 Trade 流
- [ ] Settlement 重新结算
- [ ] TDengine 数据一致

**验证方法**:
```bash
cargo test settlement_matching_replay
```

#### Test 3.3.2: Idempotency
- [ ] 重复 trade_id 检测
- [ ] TDengine 存在性检查
- [ ] 重复处理不导致余额错误

**验证方法**:
```bash
cargo test settlement_idempotency
```

---

### 3.4 Integration Tests

#### Test 3.4.1: Settlement E2E
```
1. 处理 5,000 笔成交
2. 创建 Snapshot @ trade_id=5000
3. 模拟崩溃
4. 恢复 (请求 ME 重放)
5. 验证 TDengine 数据
```

- [ ] TDengine trade 记录数 = 5000
- [ ] balance_events 数量正确
- [ ] 无重复数据

**验证方法**:
```bash
./scripts/test_settlement_e2e.sh
```

---

## Phase 4: Replay Protocol Tests

### 4.1 UBSCore Replay API

#### Test 4.1.1: Range Replay
- [ ] `replay_orders(from=100, to=200)` 返回 100 条
- [ ] `replay_orders(from=100, to=None)` 返回到最新
- [ ] seq_id 严格连续

**验证方法**:
```bash
cargo test ubscore_replay_range
```

#### Test 4.1.2: Streaming Replay
- [ ] 不加载全部到内存
- [ ] callback 返回 false 停止
- [ ] 大数据量 (100K+) 不 OOM

**验证方法**:
```bash
cargo test ubscore_replay_streaming
```

---

### 4.2 Matching Replay API

#### Test 4.2.1: Trade Replay
- [ ] `replay_trades(from=50, to=100)` 返回 50 笔
- [ ] 只返回 Trade 类型 (过滤其他)
- [ ] trade_id 连续

**验证方法**:
```bash
cargo test matching_replay_trades
```

---

## Full System Integration Tests

### Test I1: 全链路恢复
```
1. UBSCore → ME → Settlement 正常运行
2. 处理 10,000 订单
3. 模拟 3 个服务依次崩溃
4. 恢复顺序: UBSCore → ME → Settlement
5. 验证数据一致性
```

- [ ] UBSCore: 10,000 订单状态正确
- [ ] ME: OrderBook 正确
- [ ] Settlement: TDengine 数据完整

**验证方法**:
```bash
./scripts/test_full_recovery.sh
```

---

### Test I2: 并发恢复
```
1. 3 个服务同时处理数据
2. 同时崩溃
3. 同时重启恢复
4. 验证无数据竞争
```

- [ ] 无数据损坏
- [ ] 无死锁
- [ ] 恢复后可正常服务

**验证方法**:
```bash
./scripts/test_concurrent_recovery.sh
```

---

## Performance Benchmarks

### Benchmark 1: WAL Write Performance
- [ ] UBSCore WAL: > 100K ops/s
- [ ] Matching WAL: > 80K ops/s
- [ ] Settlement WAL: > 50K ops/s

### Benchmark 2: Snapshot Performance
- [ ] UBSCore Snapshot (100K accounts): < 2s
- [ ] Matching Snapshot (10K orders): < 3s
- [ ] Settlement Snapshot: < 100ms

### Benchmark 3: Recovery Performance
- [ ] UBSCore 恢复 (100K orders): < 10s
- [ ] Matching 恢复 (50K orders): < 15s
- [ ] Settlement 恢复 (100K trades): < 5s

**验证方法**:
```bash
cargo bench --bench wal_snapshot_perf
```

---

## Regression Tests

### Regression 1: Data Integrity
- [ ] 100 次随机崩溃恢复
- [ ] 数据完整性 100%
- [ ] 无数据丢失

### Regression 2: Long Running
- [ ] 连续运行 24 小时
- [ ] 无内存泄漏
- [ ] 无文件句柄泄漏
- [ ] Snapshot 自动清理

**验证方法**:
```bash
./scripts/test_long_running.sh
```

---

## Security Tests

### Security 1: Checksum Tampering
- [ ] 修改 WAL 文件 → 检测到损坏
- [ ] 修改 Snapshot 文件 → 检测到损坏
- [ ] 拒绝加载损坏数据

### Security 2: Permission
- [ ] 数据目录权限正确 (700)
- [ ] 文件权限正确 (600)
- [ ] 符号链接安全

---

## 验收标准

### 功能性 (100%)
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过
- [ ] E2E 测试通过

### 性能
- [ ] WAL 写入 > 100K ops/s
- [ ] 恢复时间 < 10s (100K 数据)
- [ ] Snapshot 创建时间达标

### 可靠性
- [ ] 崩溃安全测试 100% 通过
- [ ] Checksum 验证 100%
- [ ] 数据完整性 100%

### 覆盖率
- [ ] 单元测试覆盖率 > 90%
- [ ] 集成测试覆盖所有关键路径

---

## 测试执行顺序

1. **Phase 1**: UBSCore (3-4 天)
2. **Phase 2**: Matching (3-4 天)
3. **Phase 3**: Settlement (2-3 天)
4. **Phase 4**: Replay Protocol (1-2 天)
5. **Integration**: Full System (2-3 天)
6. **Performance**: Benchmarks (1 天)
7. **Regression**: Long-running + Stress (2 天)

**总计**: 14-19 天

---

*QA Test Checklist created: 2024-12-25*
