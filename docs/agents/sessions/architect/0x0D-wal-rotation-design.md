# 0x0D WAL Rotation Mechanism Design

> **Status**: 📋 DRAFT  
> **Author**: Architect Team  
> **Date**: 2024-12-25  
> **Parent**: [0x0D WAL Format Spec](./0x0D-wal-format-spec.md)

---

## 1. Architecture Principles

### 1.1 核心原则

**每个有状态服务必须有自己的 Snapshot + WAL**

- WAL 由**服务自己消费**（备份机制除外）
- 下游恢复时**请求上游重放输出**，不直接读上游 WAL

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    每个服务独立 Snapshot + WAL                               │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐           ┌──────────────┐           ┌──────────────┐     │
│  │   UBSCore    │           │   Matching   │           │  Settlement  │     │
│  │              │           │   Engine     │           │              │     │
│  │ ┌──────────┐ │  重放输出  │ ┌──────────┐ │  重放输出  │ ┌──────────┐ │     │
│  │ │ Snapshot │ │ ◀─请求──  │ │ Snapshot │ │ ◀─请求──  │ │ Snapshot │ │     │
│  │ │ (余额)   │ │           │ │(OrderBook)│ │           │ │ (状态)   │ │     │
│  │ └──────────┘ │           │ └──────────┘ │           │ └──────────┘ │     │
│  │              │           │              │           │              │     │
│  │ ┌──────────┐ │           │ ┌──────────┐ │           │ ┌──────────┐ │     │
│  │ │Order WAL │ │──输出重放▶│ │Trade WAL │ │──输出重放▶│ │状态 WAL  │ │     │
│  │ │(自己消费)│ │  给 ME    │ │(自己消费) │ │ 给结算    │ │(自己消费)│ │     │
│  │ └──────────┘ │           │ └──────────┘ │           │ └──────────┘ │     │
│  └──────────────┘           └──────────────┘           └──────────────┘     │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 WAL 职责

| 服务 | Snapshot | WAL | WAL 消费者 | 输出 (给下游重放) |
|------|----------|-----|------------|------------------|
| **UBSCore** | ✅ 必须 (余额) | Order WAL | **自己** | 重放 ValidOrder 给 ME |
| **Matching** | ✅ 必须 (OrderBook) | Trade WAL | **自己** | 重放 Trade 给 Settlement |
| **Settlement** | ✅ 必须 (状态) | 状态 WAL | **自己** | N/A |

### 1.3 恢复流程

```
ME 重启：
  1. 加载 ME 自己的 Snapshot (OrderBook @ seq=X)
  2. 请求 UBSCore: "请重放 seq > X 的输出"
  3. UBSCore 从自己的 WAL 读取并重放给 ME
  4. ME 追上实时状态

Settlement 重启：
  1. 加载 Settlement 自己的 Snapshot (状态 @ seq=Y)
  2. 请求 ME: "请重放 trade_seq > Y 的输出"
  3. ME 从自己的 WAL 读取并重放给 Settlement
  4. Settlement 追上实时状态
```

---

## 2. Design Goals

| 目标 | 说明 |
|------|------|
| **可控文件大小** | 避免单个 WAL 文件过大 |
| **快速恢复** | 只需重放最近的 WAL 文件 |
| **归档友好** | 旧 WAL 可压缩/删除 |
| **Snapshot 协同** | Rotation 与 Snapshot 对齐 |

---

## 3. WAL 文件命名

```
{service_data_dir}/wal/
├── current.wal                 # 当前活跃 WAL
├── wal-00001-0000000100.wal    # EPOCH-1, seq 100 结束
├── wal-00001-0000000200.wal    # EPOCH-1, seq 200 结束
└── wal-00002-0000000050.wal    # EPOCH-2, seq 50 结束

命名格式: wal-{EPOCH:05d}-{END_SEQ:010d}.wal

例如: data/ubscore-service/wal/wal-00001-0000001000.wal
```

---

## 4. Rotation 触发策略

### 4.1 主要触发条件

```rust
pub struct RotationConfig {
    /// 文件大小阈值 (默认 64MB)
    pub max_file_size: u64,
    
    /// 时间间隔 (默认 1 小时)
    pub max_duration: Duration,
    
    /// 条目数阈值 (默认 1M entries)
    pub max_entries: u64,
    
    /// 是否在 Snapshot 时强制 rotate
    pub rotate_on_snapshot: bool,
}
```

### 4.2 触发判断

```rust
fn should_rotate(&self) -> bool {
    self.current_size >= self.config.max_file_size ||
    self.elapsed_time >= self.config.max_duration ||
    self.entry_count >= self.config.max_entries
}
```

---

## 5. Rotation 流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                    ROTATION SEQUENCE                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Flush current.wal                                              │
│  2. fsync() 确保持久化                                             │
│  3. Close current.wal                                              │
│  4. Rename: current.wal → wal-{EPOCH}-{END_SEQ}.wal               │
│  5. Create new current.wal                                         │
│  6. Write file header                                              │
│  7. Continue writing                                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Rust 实现

```rust
pub fn rotate(&mut self) -> io::Result<PathBuf> {
    // 1. Flush
    self.writer.flush()?;
    self.file.sync_all()?;
    
    // 2. Close and rename
    let old_path = self.current_path.clone();
    let new_name = format!(
        "wal-{:05}-{:010}.wal",
        self.epoch,
        self.seq_id
    );
    let new_path = self.wal_dir.join(&new_name);
    fs::rename(&old_path, &new_path)?;
    
    // 3. Create new file
    self.file = File::create(&self.current_path)?;
    self.writer = BufWriter::new(self.file);
    self.write_file_header()?;
    
    // 4. Reset counters
    self.entry_count = 0;
    self.current_size = 0;
    self.start_time = Instant::now();
    
    Ok(new_path)
}
```

---

## 6. 与 Snapshot 协同

### 6.1 Snapshot 时强制 Rotate

```
Before Snapshot:
  current.wal (seq 150-250 in progress)

Snapshot @ seq=250:
  1. Rotate current.wal → wal-00001-0000000250.wal
  2. Create snapshot (state @ seq=250)
  3. Create new current.wal

After Snapshot:
  data/
  ├── wal/
  │   ├── current.wal              # seq 251+
  │   ├── wal-00001-0000000100.wal # 可归档
  │   └── wal-00001-0000000250.wal # Snapshot 边界
  └── snapshots/
      └── latest → snapshot-250/
```

### 6.2 恢复时只需

```rust
fn recover() {
    let snapshot = load_latest_snapshot(); // state @ seq=250
    let wal_files = find_wal_files_after(snapshot.seq_id);
    // 只需重放 current.wal (seq 251+)
}
```

---

## 7. 保留策略

```rust
pub struct RetentionConfig {
    /// 保留的 WAL 文件数量
    pub keep_wal_files: usize,  // 默认 10
    
    /// 保留天数
    pub keep_days: u32,         // 默认 7
    
    /// Snapshot 后可删除的 WAL
    pub delete_after_snapshot: bool,
}
```

### 清理逻辑

```rust
fn cleanup_old_wal_files(&mut self) -> io::Result<()> {
    let latest_snapshot_seq = self.get_latest_snapshot_seq()?;
    
    for wal_file in self.list_wal_files()? {
        let end_seq = parse_end_seq(&wal_file);
        
        // 只删除 Snapshot 之前的 WAL
        if end_seq < latest_snapshot_seq {
            if self.config.delete_after_snapshot {
                fs::remove_file(&wal_file)?;
            } else {
                // 或者压缩归档
                self.archive_wal(&wal_file)?;
            }
        }
    }
    Ok(())
}
```

---

## 8. 默认配置建议

| 场景 | max_file_size | max_duration | max_entries |
|------|---------------|--------------|-------------|
| **开发** | 16 MB | 5 min | 100K |
| **测试** | 64 MB | 30 min | 500K |
| **生产** | 256 MB | 1 hour | 1M |

---

## 9. 服务隔离存储（必须）

`data/` 是公共可配置的根目录，每个服务在其下创建自己的子目录：

```
data/                              # 公共根目录 (可配置)
├── ubscore-service/               # UBSCore 服务
│   ├── wal/
│   │   ├── current.wal
│   │   └── wal-00001-0000001000.wal
│   └── snapshots/
│       └── latest -> snapshot-1000/
│
├── matching-service/              # 撮合引擎
│   ├── wal/
│   │   ├── current.wal
│   │   └── wal-00001-0000500000.wal
│   └── orderbooks/
│
├── settlement-service/            # 结算服务
│   └── wal/
│       ├── current.wal
│       └── wal-00001-0000100000.wal
│
└── trade-audit-service/           # 审计服务
    └── wal/
        └── ...
```

### 9.1 配置

```yaml
# 全局配置
data:
  base_dir: "/var/lib/zero_x/data"  # 公共根目录

# 各服务会自动在 base_dir 下创建自己的目录
# 例如: /var/lib/zero_x/data/ubscore-service/
```

### 9.2 代码

```rust
pub struct ServiceConfig {
    pub name: String,
    pub data_dir: PathBuf,  // 每个服务独立配置
}

impl ServiceConfig {
    pub fn wal_dir(&self) -> PathBuf {
        self.data_dir.join("wal")
    }
    
    pub fn snapshots_dir(&self) -> PathBuf {
        self.data_dir.join("snapshots")
    }
}
```

### 9.3 服务与数据归档策略

| 服务 | Entry Types | 归档策略 |
|------|-------------|----------|
| ubscore | Order, Deposit, Withdraw | Snapshot 后可删 |
| matching | Order, Cancel | Snapshot 后可删 |
| settlement | Trade, BalanceSettle | 永久保留 |
| trade-audit | Trade | 永久保留 (合规) |

---

*Document updated: 2024-12-25*

