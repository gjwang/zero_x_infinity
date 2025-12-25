#!/usr/bin/env python3
"""
WAL v2 Verification Tool
Phase 0x0D: Read and verify Universal WAL Format files

Wire format (20-byte header):
  [0..2]   payload_len u16 LE
  [2]      entry_type  u8
  [3]      version     u8
  [4..8]   epoch       u32 LE
  [8..16]  seq_id      u64 LE
  [16..20] checksum    u32 LE
"""

import struct
import sys
import os
from pathlib import Path
from dataclasses import dataclass
from typing import Optional, List
import zlib

# WAL constants
WAL_HEADER_SIZE = 20

# Entry types
ENTRY_TYPES = {
    1: "Order",
    2: "Cancel", 
    3: "Trade",
    4: "BalanceSettle",
    5: "Deposit",
    6: "Withdraw",
    7: "SnapshotMarker",
}


@dataclass
class WalHeader:
    seq_id: int
    epoch: int
    checksum: int
    payload_len: int
    entry_type: int
    version: int


@dataclass
class WalEntry:
    header: WalHeader
    payload: bytes
    valid: bool
    error: Optional[str] = None


def crc32(data: bytes) -> int:
    """Calculate CRC32 checksum (same as Rust crc32fast)"""
    return zlib.crc32(data) & 0xFFFFFFFF


def read_header(data: bytes) -> WalHeader:
    """
    Parse 20-byte WAL header
    
    Actual wire format (matches Rust to_bytes()):
      [0..2]   payload_len  u16 LE
      [2]      entry_type   u8
      [3]      version      u8
      [4..8]   epoch        u32 LE
      [8..16]  seq_id       u64 LE
      [16..20] checksum     u32 LE
    """
    if len(data) < WAL_HEADER_SIZE:
        raise ValueError(f"Header too short: {len(data)} bytes")
    
    payload_len = struct.unpack('<H', data[0:2])[0]
    entry_type = data[2]
    version = data[3]
    epoch = struct.unpack('<I', data[4:8])[0]
    seq_id = struct.unpack('<Q', data[8:16])[0]
    checksum = struct.unpack('<I', data[16:20])[0]
    
    return WalHeader(seq_id, epoch, checksum, payload_len, entry_type, version)


def read_wal_file(filepath: str) -> List[WalEntry]:
    """Read all entries from a WAL file"""
    entries = []
    
    with open(filepath, 'rb') as f:
        while True:
            # Read header
            header_data = f.read(WAL_HEADER_SIZE)
            if len(header_data) == 0:
                break  # EOF
            if len(header_data) < WAL_HEADER_SIZE:
                entries.append(WalEntry(
                    header=None, 
                    payload=b'',
                    valid=False,
                    error=f"Incomplete header: {len(header_data)} bytes"
                ))
                break
            
            header = read_header(header_data)
            
            # Read payload
            payload = f.read(header.payload_len)
            if len(payload) < header.payload_len:
                entries.append(WalEntry(
                    header=header,
                    payload=payload,
                    valid=False,
                    error=f"Incomplete payload: got {len(payload)}, expected {header.payload_len}"
                ))
                break
            
            # Verify checksum
            calculated_crc = crc32(payload)
            valid = calculated_crc == header.checksum
            error = None if valid else f"CRC mismatch: got {calculated_crc:08x}, expected {header.checksum:08x}"
            
            entries.append(WalEntry(
                header=header,
                payload=payload,
                valid=valid,
                error=error
            ))
    
    return entries


def verify_wal(filepath: str) -> bool:
    """Verify a WAL file and print report"""
    print(f"\n{'='*60}")
    print(f"WAL v2 Verification: {filepath}")
    print(f"{'='*60}")
    
    if not os.path.exists(filepath):
        print(f"❌ File not found: {filepath}")
        return False
    
    file_size = os.path.getsize(filepath)
    print(f"File size: {file_size} bytes")
    
    entries = read_wal_file(filepath)
    
    print(f"\nEntries found: {len(entries)}")
    print(f"{'-'*60}")
    
    all_valid = True
    for i, entry in enumerate(entries):
        if entry.header is None:
            print(f"[{i+1}] ❌ {entry.error}")
            all_valid = False
            continue
            
        h = entry.header
        entry_type_name = ENTRY_TYPES.get(h.entry_type, f"Unknown({h.entry_type})")
        status = "✅" if entry.valid else "❌"
        
        print(f"[{i+1}] {status} seq={h.seq_id} epoch={h.epoch} type={entry_type_name} "
              f"len={h.payload_len} crc={h.checksum:08x}")
        
        if not entry.valid:
            print(f"    Error: {entry.error}")
            all_valid = False
    
    print(f"{'-'*60}")
    
    if all_valid and len(entries) > 0:
        print(f"✅ WAL verification PASSED ({len(entries)} entries)")
    else:
        print(f"❌ WAL verification FAILED")
    
    print(f"{'='*60}\n")
    
    return all_valid


def main():
    if len(sys.argv) < 2:
        print("Usage: verify_wal.py <wal_file> [wal_file2 ...]")
        print("\nVerifies Universal WAL Format v2 files (20-byte header)")
        sys.exit(1)
    
    all_passed = True
    for filepath in sys.argv[1:]:
        if not verify_wal(filepath):
            all_passed = False
    
    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
