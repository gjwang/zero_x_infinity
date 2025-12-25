# 0x0D Universal WAL Format Specification

> **Status**: 📋 DRAFT  
> **Author**: Architect Team  
> **Date**: 2024-12-25  
> **Parent**: [0x0D Architecture Design](./0x0D-architecture-design.md)

---

## 1. Design Goal

**通用 WAL 格式**：Header 与 Payload 分离，支持多种事件类型扩展。

### 设计原则

| 原则 | 实现 |
|------|------|
| **最小开销** | Header 仅 16 bytes |
| **类型扩展** | entry_type 区分事件类型 |
| **版本兼容** | version 字段支持格式演进 |
| **完整性** | CRC32 校验 |

---

## 2. WAL Header (20 bytes)

```
┌────────────┬───────────┬────────────────────────────────────┐
│ payload_len│ 2 bytes   │ Payload size (max 64KB)            │
│ entry_type │ 1 byte    │ Event type (Order/Trade/...)       │
│ version    │ 1 byte    │ Payload format version (0-255)     │
│ epoch      │ 4 bytes   │ EPOCH (restarts from new epoch)    │
│ seq_id     │ 8 bytes   │ Monotonic sequence within EPOCH    │
│ checksum   │ 4 bytes   │ CRC32 of payload                   │
└────────────┴───────────┴────────────────────────────────────┘
Total: 20 bytes
```

### EPOCH Concept

当重启恢复时发现 WAL 有 gap 无法对齐，从最后可对齐点开始，使用新 EPOCH：

```
EPOCH=1: seq 1,2,3,4,[损坏],7,8   ← 无法确定 5,6
EPOCH=2: seq 1,2,3...             ← 从快照恢复，新 EPOCH
```

### Rust 定义

```rust
/// Universal WAL header (20 bytes)
#[repr(C)]
pub struct WalHeader {
    pub payload_len: u16,    // 2: Payload size
    pub entry_type: u8,      // 1: WalEntryType enum
    pub version: u8,         // 1: Payload format version
    pub epoch: u32,          // 4: EPOCH number
    pub seq_id: u64,         // 8: Monotonic sequence
    pub checksum: u32,       // 4: CRC32 of payload
}

const WAL_HEADER_SIZE: usize = 20;
```

---

## 3. Entry Types

```rust
#[repr(u8)]
pub enum WalEntryType {
    Order = 1,           // Place order
    Cancel = 2,          // Cancel order
    Deposit = 3,         // Deposit funds
    Withdraw = 4,        // Withdraw funds
    SnapshotMarker = 5,  // Snapshot taken marker
    // Future extensions...
}
```

---

## 4. Payload Definitions

### 4.1 Order Payload (entry_type = 1)

```rust
#[derive(Serialize, Deserialize)]
pub struct OrderPayload {
    pub order_id: u64,
    pub user_id: u64,
    pub symbol_id: u32,
    pub price: u64,
    pub qty: u64,
    pub side: u8,           // 0=Buy, 1=Sell
    pub order_type: u8,     // 0=Limit, 1=Market
    pub ingested_at_ns: u64,
}
// ~50 bytes
```

### 4.2 Cancel Payload (entry_type = 2)

```rust
#[derive(Serialize, Deserialize)]
pub struct CancelPayload {
    pub order_id: u64,
    pub user_id: u64,
}
// 16 bytes
```

### 4.3 Funding Payload (entry_type = 3, 4)

```rust
#[derive(Serialize, Deserialize)]
pub struct FundingPayload {
    pub user_id: u64,
    pub asset_id: u32,
    pub amount: u64,
    pub request_id: u64,
}
// 28 bytes
```

### 4.4 Snapshot Marker (entry_type = 5)

```rust
#[derive(Serialize, Deserialize)]
pub struct SnapshotMarkerPayload {
    pub snapshot_dir: String,
    pub timestamp_ns: u64,
}
```

---

## 5. WAL File Format

```
┌─────────────────────────────────────────────────────────────────────┐
│                     WAL FILE STRUCTURE                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐      │
│  │ File Header (8 bytes, once per file)                     │      │
│  │  - magic: u32 (0x57414C31 = "WAL1")                      │      │
│  │  - file_version: u16                                     │      │
│  │  - reserved: u16                                         │      │
│  └──────────────────────────────────────────────────────────┘      │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐      │
│  │ Entry[0]: Header (16 bytes) + Payload (N bytes)          │      │
│  └──────────────────────────────────────────────────────────┘      │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐      │
│  │ Entry[1]: Header (16 bytes) + Payload (N bytes)          │      │
│  └──────────────────────────────────────────────────────────┘      │
│                                                                     │
│  ... (repeat)                                                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 6. Size Estimates

| 事件类型 | Header | Payload | Total |
|----------|--------|---------|-------|
| Order | 16 | ~50 | ~66 bytes |
| Cancel | 16 | 16 | 32 bytes |
| Deposit/Withdraw | 16 | 28 | 44 bytes |

### 1M Orders WAL Size

```
1,000,000 orders × 66 bytes = ~66 MB
```

---

## 7. Version Evolution

每个 entry_type 可独立演进 version：

| entry_type | version | 含义 |
|------------|---------|------|
| Order (1) | 0 | 初始格式 |
| Order (1) | 1 | 未来：增加字段 |
| Cancel (2) | 0 | 初始格式 |

读取时根据 `(entry_type, version)` 选择解析器。

---

## 8. Merkle Checkpoint Extension (Phase 2)

### 8.1 概述

周期性生成 Merkle Root，用于防篡改验证和轻客户端证明。

### 8.2 高效实现：复用 CRC32

**核心优化**：不重复 hash 整个 payload，复用已计算的 checksum。

```
叶子节点 = xxhash64(seq_id || checksum)
         = 8 bytes seq_id + 4 bytes crc32 → 8 bytes hash
         
开销：~5ns per entry (vs ~100ns hash full payload)
```

### 8.3 Entry Types 扩展

```rust
#[repr(u8)]
pub enum WalEntryType {
    Order = 1,
    Cancel = 2,
    Trade = 3,
    BalanceSettle = 4,
    Deposit = 5,
    Withdraw = 6,
    SnapshotMarker = 7,
    MerkleCheckpoint = 8,  // ← 新增
}
```

### 8.4 MerkleCheckpoint Payload

```rust
#[derive(Serialize, Deserialize)]
pub struct MerkleCheckpointPayload {
    pub start_seq: u64,      // 起始 seq_id
    pub end_seq: u64,        // 结束 seq_id
    pub merkle_root: u64,    // xxhash64 root (8 bytes)
    pub entry_count: u32,    // 条目数
}
// 28 bytes
```

### 8.5 实现

```rust
use xxhash_rust::xxh3::xxh3_64;

const CHECKPOINT_INTERVAL: usize = 1024;

struct MerkleAccumulator {
    hashes: Vec<u64>,
    start_seq: u64,
}

impl MerkleAccumulator {
    /// 每条 entry 调用，传入 header 中已有的 seq_id 和 checksum
    #[inline]
    fn append(&mut self, seq_id: u64, checksum: u32) {
        let mut buf = [0u8; 12];
        buf[0..8].copy_from_slice(&seq_id.to_le_bytes());
        buf[8..12].copy_from_slice(&checksum.to_le_bytes());
        self.hashes.push(xxh3_64(&buf));
    }

    #[inline]
    fn should_checkpoint(&self) -> bool {
        self.hashes.len() >= CHECKPOINT_INTERVAL
    }

    fn compute_root(&self) -> u64 {
        let mut level = self.hashes.clone();
        while level.len() > 1 {
            let mut next = Vec::with_capacity((level.len() + 1) / 2);
            for chunk in level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    xxh3_64(&[chunk[0].to_le_bytes(), chunk[1].to_le_bytes()].concat())
                } else {
                    chunk[0]
                };
                next.push(hash);
            }
            level = next;
        }
        level.first().copied().unwrap_or(0)
    }
}
```

### 8.6 安全模式（可选）

如需抵抗恶意攻击，替换哈希算法：

| 模式 | 叶子计算 | 每条开销 |
|------|----------|----------|
| **Fast (默认)** | `xxh3_64(seq_id \|\| crc32)` | ~5ns |
| **Secure** | `sha256(seq_id \|\| sha256(payload))[0..8]` | ~500ns |

安全模式需将 header 中的 `checksum` 改为 `sha256(payload)[0..4]`。

---

*Document updated: 2024-12-25*

