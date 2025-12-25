# 0x0D Implementation Plan: Universal WAL Format

> **From**: Architect  
> **To**: Developer  
> **Date**: 2024-12-25

---

## Goal

重构 `src/wal.rs` 为 **Universal WAL Format** (20-byte header)。

> ⚠️ **无需兼容**: 目前没有老数据，直接替换。

---

## Changes

### [MODIFY] `src/wal.rs`

```rust
#[repr(C)]
pub struct WalHeader {
    pub payload_len: u16,
    pub entry_type: u8,
    pub version: u8,
    pub epoch: u32,
    pub seq_id: u64,
    pub checksum: u32,
}

#[repr(u8)]
pub enum WalEntryType {
    Order = 1,
    Cancel = 2,
    Trade = 3,
    BalanceSettle = 4,
    Deposit = 5,
    Withdraw = 6,
    SnapshotMarker = 7,
}
```

### [MODIFY] `Cargo.toml`

```toml
crc32fast = "1.3"
bincode = "1.3"
```

---

## Acceptance

- [ ] `WalHeader` = 20 bytes
- [ ] CRC32 校验
- [ ] 读写 round-trip

---

*Ref*: `docs/agents/sessions/architect/0x0D-wal-format-spec.md`
