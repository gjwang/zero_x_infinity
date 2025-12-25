# 0x0D Snapshot Binary Format Specification

> **Status**: 📋 DRAFT  
> **Author**: Architect (AI)  
> **Date**: 2024-12-25  
> **Parent**: [0x0D Architecture Design](./0x0D-architecture-design.md)

---

## 1. Overview

This document specifies the **exact binary layout** for snapshot files.

### Format Decisions

| Decision | Choice |
|----------|--------|
| Serialization | `bincode` (serde) |
| Compression | None (Phase 1) |
| Byte Order | Little-endian (native) |
| String Encoding | Length-prefixed UTF-8 |

---

## 2. File Layout

```
data/snapshots/
└── 20241225_183000/
    ├── metadata.json      # Human-readable metadata (JSON)
    ├── balances.bin       # Binary serialized balances
    ├── orders.bin         # Binary serialized orders
    └── COMPLETE           # Atomic completion marker (empty file)
```

---

## 3. Metadata File (`metadata.json`)

Human-readable JSON for debugging and version control.

```json
{
  "format_version": 1,
  "compression": "none",
  "created_at": "2024-12-25T18:30:00.000000Z",
  "wal_seq_id": 1234567,
  "trade_id_seq": 890123,
  "user_count": 10000,
  "order_count": 50000,
  "balances_checksum": "a1b2c3d4e5f6",
  "orders_checksum": "f6e5d4c3b2a1",
  "build_version": "0.1.0",
  "build_commit": "abc123def"
}
```

### Rust Struct

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Format version for migrations
    pub format_version: u32,
    
    /// Compression mode: "none", "lz4", "zstd"
    pub compression: String,
    
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    
    /// WAL sequence number at snapshot time
    /// Recovery replays WAL from seq_id + 1
    pub wal_seq_id: u64,
    
    /// Next trade ID to assign
    pub trade_id_seq: u64,
    
    /// Number of users in snapshot
    pub user_count: u64,
    
    /// Number of orders in snapshot
    pub order_count: u64,
    
    /// CRC64 checksum of balances.bin
    pub balances_checksum: String,
    
    /// CRC64 checksum of orders.bin
    pub orders_checksum: String,
    
    /// Build version for debugging
    pub build_version: String,
    
    /// Git commit hash
    pub build_commit: String,
}
```

---

## 4. Balances File (`balances.bin`)

### 4.1 File Structure

```
┌─────────────────────────────────────────────────────────────────────┐
│                     balances.bin                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐                                              │
│  │ Header (16 bytes)│                                              │
│  │  - magic: u32    │  0x42414C53 ("BALS")                        │
│  │  - version: u32  │  1                                           │
│  │  - count: u64    │  Number of UserAccount entries              │
│  └──────────────────┘                                              │
│                                                                     │
│  ┌──────────────────┐                                              │
│  │ UserAccount[0]   │                                              │
│  │  - user_id: u64  │                                              │
│  │  - vip_level: u8 │                                              │
│  │  - asset_count   │                                              │
│  │  - assets: [...]│                                              │
│  └──────────────────┘                                              │
│                                                                     │
│  ┌──────────────────┐                                              │
│  │ UserAccount[1]   │                                              │
│  │  ...             │                                              │
│  └──────────────────┘                                              │
│                                                                     │
│  ... (repeat for all users)                                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Rust Structs (bincode serializable)

```rust
/// File header for balances.bin
#[derive(Serialize, Deserialize)]
pub struct BalancesHeader {
    pub magic: u32,      // 0x42414C53 ("BALS")
    pub version: u32,    // 1
    pub count: u64,      // Number of users
}

/// Snapshot representation of a user's balances
#[derive(Serialize, Deserialize)]
pub struct UserBalanceSnapshot {
    pub user_id: u64,
    pub vip_level: u8,
    pub assets: Vec<AssetBalanceSnapshot>,
}

/// Snapshot of a single asset balance
#[derive(Serialize, Deserialize)]
pub struct AssetBalanceSnapshot {
    pub asset_id: u32,
    pub avail: u64,
    pub frozen: u64,
    pub lock_version: u64,
    pub settle_version: u64,
}
```

### 4.3 Binary Layout (with bincode)

```
UserBalanceSnapshot binary layout:
┌────────────────────────────────────────────────────────────────┐
│ user_id     │ 8 bytes  │ u64 little-endian                    │
│ vip_level   │ 1 byte   │ u8                                   │
│ asset_count │ 8 bytes  │ u64 (Vec length, bincode default)    │
│ asset[0]    │ 36 bytes │ AssetBalanceSnapshot                 │
│ asset[1]    │ 36 bytes │ ...                                  │
│ ...         │          │                                      │
└────────────────────────────────────────────────────────────────┘

AssetBalanceSnapshot binary layout (36 bytes):
┌────────────────────────────────────────────────────────────────┐
│ asset_id       │ 4 bytes  │ u32                               │
│ avail          │ 8 bytes  │ u64                               │
│ frozen         │ 8 bytes  │ u64                               │
│ lock_version   │ 8 bytes  │ u64                               │
│ settle_version │ 8 bytes  │ u64                               │
└────────────────────────────────────────────────────────────────┘
```

---

## 5. Orders File (`orders.bin`)

### 5.1 File Structure

```
┌─────────────────────────────────────────────────────────────────────┐
│                     orders.bin                                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────┐                                              │
│  │ Header (16 bytes)│                                              │
│  │  - magic: u32    │  0x4F524453 ("ORDS")                        │
│  │  - version: u32  │  1                                           │
│  │  - count: u64    │  Number of orders                           │
│  └──────────────────┘                                              │
│                                                                     │
│  ┌──────────────────┐                                              │
│  │ OrderSnapshot[0] │                                              │
│  │  - order_id: u64 │                                              │
│  │  - user_id: u64  │                                              │
│  │  - ...           │                                              │
│  └──────────────────┘                                              │
│                                                                     │
│  ... (repeat for all orders)                                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Rust Structs

```rust
/// File header for orders.bin
#[derive(Serialize, Deserialize)]
pub struct OrdersHeader {
    pub magic: u32,      // 0x4F524453 ("ORDS")
    pub version: u32,    // 1
    pub count: u64,      // Number of orders
}

/// Snapshot of a single order (only resting orders)
#[derive(Serialize, Deserialize)]
pub struct OrderSnapshot {
    pub order_id: u64,
    pub user_id: u64,
    pub symbol_id: u32,
    pub price: u64,
    pub qty: u64,           // Original quantity
    pub filled_qty: u64,    // Already filled
    pub side: u8,           // 0=Buy, 1=Sell
    pub order_type: u8,     // 0=Limit, 1=Market, ...
    pub status: u8,         // OrderStatus as u8
    pub lock_version: u64,
    pub seq_id: u64,
    pub ingested_at_ns: u64,
    pub cid: Option<String>, // Client order ID
}
```

### 5.3 Binary Layout

```
OrderSnapshot binary layout (fixed fields: 73 bytes + variable cid):
┌────────────────────────────────────────────────────────────────┐
│ order_id      │ 8 bytes  │ u64                                │
│ user_id       │ 8 bytes  │ u64                                │
│ symbol_id     │ 4 bytes  │ u32                                │
│ price         │ 8 bytes  │ u64                                │
│ qty           │ 8 bytes  │ u64                                │
│ filled_qty    │ 8 bytes  │ u64                                │
│ side          │ 1 byte   │ u8 (0=Buy, 1=Sell)                 │
│ order_type    │ 1 byte   │ u8                                 │
│ status        │ 1 byte   │ u8                                 │
│ lock_version  │ 8 bytes  │ u64                                │
│ seq_id        │ 8 bytes  │ u64                                │
│ ingested_at_ns│ 8 bytes  │ u64                                │
│ cid_present   │ 1 byte   │ 0=None, 1=Some                     │
│ [cid_len]     │ 8 bytes  │ u64 (if Some)                      │
│ [cid_data]    │ N bytes  │ UTF-8 bytes (if Some)              │
└────────────────────────────────────────────────────────────────┘
```

---

## 6. Enumeration Mappings

### 6.1 Side

```rust
// Serialized as u8
enum Side {
    Buy = 0,
    Sell = 1,
}
```

### 6.2 OrderType

```rust
// Serialized as u8
enum OrderType {
    Limit = 0,
    Market = 1,
    Deposit = 2,
    Withdraw = 3,
}
```

### 6.3 OrderStatus

```rust
// Serialized as u8
enum OrderStatus {
    NEW = 0,
    PARTIALLY_FILLED = 1,
    FILLED = 2,
    CANCELED = 3,
    REJECTED = 4,
    EXPIRED = 5,
}
```

---

## 7. Checksum Calculation

Using CRC64 (XZ polynomial) for checksums:

```rust
use crc::{Crc, CRC_64_XZ};

const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_XZ);

fn calculate_checksum(data: &[u8]) -> String {
    let checksum = CRC64.checksum(data);
    format!("{:016x}", checksum)
}
```

---

## 8. Size Estimates

### 8.1 Per-User Balance Entry

```
UserBalanceSnapshot with 2 assets:
- user_id: 8 bytes
- vip_level: 1 byte
- asset_count: 8 bytes
- 2 × AssetBalanceSnapshot: 2 × 36 = 72 bytes
Total: ~89 bytes per user

For 10,000 users with avg 2 assets:
~890 KB for balances.bin
```

### 8.2 Per-Order Entry

```
OrderSnapshot (no cid):
- Fixed fields: 73 bytes
- cid_present: 1 byte
Total: ~74 bytes per order

For 100,000 resting orders:
~7.4 MB for orders.bin
```

### 8.3 Total Snapshot Size

```
Scenario: 10K users, 100K orders

balances.bin:  ~1 MB
orders.bin:    ~8 MB
metadata.json: ~1 KB
───────────────────
Total:         ~9 MB per snapshot

With 10 snapshots retained: ~90 MB disk usage
```

---

## 9. Implementation Notes

### 9.1 Writing Snapshot

```rust
pub fn write_snapshot(
    dir: &Path,
    ubscore: &UBSCore,
    orderbooks: &HashMap<SymbolId, OrderBook>,
) -> io::Result<()> {
    // 1. Create temp directory
    let temp_dir = dir.join(".tmp");
    fs::create_dir_all(&temp_dir)?;
    
    // 2. Write balances.bin
    let balances_data = serialize_balances(ubscore)?;
    let balances_checksum = calculate_checksum(&balances_data);
    fs::write(temp_dir.join("balances.bin"), &balances_data)?;
    
    // 3. Write orders.bin
    let orders_data = serialize_orders(orderbooks)?;
    let orders_checksum = calculate_checksum(&orders_data);
    fs::write(temp_dir.join("orders.bin"), &orders_data)?;
    
    // 4. Write metadata.json
    let metadata = SnapshotMetadata { ... };
    fs::write(temp_dir.join("metadata.json"), serde_json::to_string_pretty(&metadata)?)?;
    
    // 5. Atomic rename temp -> final
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let final_dir = dir.join(&timestamp);
    fs::rename(&temp_dir, &final_dir)?;
    
    // 6. Create COMPLETE marker
    fs::write(final_dir.join("COMPLETE"), "")?;
    
    // 7. Update "latest" symlink
    let latest = dir.join("latest");
    let _ = fs::remove_file(&latest); // Ignore if not exists
    std::os::unix::fs::symlink(&timestamp, &latest)?;
    
    Ok(())
}
```

### 9.2 Reading Snapshot

```rust
pub fn read_snapshot(dir: &Path) -> io::Result<(Vec<UserBalanceSnapshot>, Vec<OrderSnapshot>, SnapshotMetadata)> {
    // 1. Find latest valid snapshot
    let latest = dir.join("latest");
    let snapshot_dir = fs::read_link(&latest)?;
    
    // 2. Verify COMPLETE marker exists
    if !snapshot_dir.join("COMPLETE").exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Incomplete snapshot"));
    }
    
    // 3. Read and verify metadata
    let metadata: SnapshotMetadata = serde_json::from_str(
        &fs::read_to_string(snapshot_dir.join("metadata.json"))?
    )?;
    
    // 4. Read and verify balances
    let balances_data = fs::read(snapshot_dir.join("balances.bin"))?;
    if calculate_checksum(&balances_data) != metadata.balances_checksum {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Checksum mismatch"));
    }
    let balances: Vec<UserBalanceSnapshot> = bincode::deserialize(&balances_data)?;
    
    // 5. Read and verify orders
    let orders_data = fs::read(snapshot_dir.join("orders.bin"))?;
    if calculate_checksum(&orders_data) != metadata.orders_checksum {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Checksum mismatch"));
    }
    let orders: Vec<OrderSnapshot> = bincode::deserialize(&orders_data)?;
    
    Ok((balances, orders, metadata))
}
```

---

## 10. Version Migration

When `format_version` changes:

```rust
fn load_and_migrate(dir: &Path) -> io::Result<Snapshot> {
    let metadata = load_metadata(dir)?;
    
    match metadata.format_version {
        1 => {
            // Current version, no migration needed
            load_v1(dir)
        }
        0 => {
            // Old version, migrate
            let v0 = load_v0(dir)?;
            Ok(migrate_v0_to_v1(v0))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported snapshot version: {}", metadata.format_version)
        ))
    }
}
```

---

*Document created: 2024-12-25*
