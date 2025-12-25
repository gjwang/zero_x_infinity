# 0x0D WAL Rotation Mechanism Design

> **Status**: 📋 DRAFT  
> **Author**: Architect Team  
> **Date**: 2024-12-25  
> **Parent**: [0x0D WAL Format Spec](./0x0D-wal-format-spec.md)

---

## 1. Design Goals

| 目标 | 说明 |
|------|------|
| **可控文件大小** | 避免单个 WAL 文件过大 |
| **快速恢复** | 只需重放最近的 WAL 文件 |
| **归档友好** | 旧 WAL 可压缩/删除 |
| **Snapshot 协同** | Rotation 与 Snapshot 对齐 |

---

## 2. WAL 文件命名

```
data/wal/
├── current.wal                 # 当前活跃 WAL
├── wal-00001-0000000100.wal    # EPOCH-1, seq 100 结束
├── wal-00001-0000000200.wal    # EPOCH-1, seq 200 结束
└── wal-00002-0000000050.wal    # EPOCH-2, seq 50 结束

命名格式: wal-{EPOCH:05d}-{END_SEQ:010d}.wal
```

---

## 3. Rotation 触发策略

### 3.1 主要触发条件

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

### 3.2 触发判断

```rust
fn should_rotate(&self) -> bool {
    self.current_size >= self.config.max_file_size ||
    self.elapsed_time >= self.config.max_duration ||
    self.entry_count >= self.config.max_entries
}
```

---

## 4. Rotation 流程

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

## 5. 与 Snapshot 协同

### 5.1 Snapshot 时强制 Rotate

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

### 5.2 恢复时只需

```rust
fn recover() {
    let snapshot = load_latest_snapshot(); // state @ seq=250
    let wal_files = find_wal_files_after(snapshot.seq_id);
    // 只需重放 current.wal (seq 251+)
}
```

---

## 6. 保留策略

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

## 7. 默认配置建议

| 场景 | max_file_size | max_duration | max_entries |
|------|---------------|--------------|-------------|
| **开发** | 16 MB | 5 min | 100K |
| **测试** | 64 MB | 30 min | 500K |
| **生产** | 256 MB | 1 hour | 1M |

---

## 8. 服务隔离存储（必须）

每个服务有**独立的 data 目录**，不同服务的数据完全隔离：

```
# 每个服务配置自己的 data_dir (可配置)

ubscore-service/
└── data/                          # UBSCore 的 data_dir
    ├── wal/
    │   ├── current.wal
    │   └── wal-00001-0000001000.wal
    └── snapshots/
        └── latest -> snapshot-1000/

matching-service/
└── data/                          # Matching Engine 的 data_dir
    ├── wal/
    │   ├── current.wal
    │   └── wal-00001-0000500000.wal
    └── orderbooks/

settlement-service/
└── data/                          # Settlement 的 data_dir
    └── wal/
        ├── current.wal
        └── wal-00001-0000100000.wal

trade-audit-service/
└── data/                          # 审计服务的 data_dir
    └── wal/
        └── ...
```

### 8.1 服务配置

每个服务通过配置文件或环境变量指定自己的 `data_dir`：

```yaml
# ubscore-service config.yaml
service:
  name: "ubscore"
  data_dir: "/var/lib/zero_x/ubscore/data"  # 可配置

# matching-service config.yaml
service:
  name: "matching"
  data_dir: "/var/lib/zero_x/matching/data"
```

### 8.2 配置代码

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

### 8.3 服务与数据归档策略

| 服务 | Entry Types | 归档策略 |
|------|-------------|----------|
| ubscore | Order, Deposit, Withdraw | Snapshot 后可删 |
| matching | Order, Cancel | Snapshot 后可删 |
| settlement | Trade, BalanceSettle | 永久保留 |
| trade-audit | Trade | 永久保留 (合规) |

---

*Document updated: 2024-12-25*

