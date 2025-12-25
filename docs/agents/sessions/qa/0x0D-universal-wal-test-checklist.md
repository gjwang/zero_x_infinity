# 0x0D QA Test Acceptance Checklist

> **From**: Architect  
> **To**: QA Engineer  
> **Date**: 2024-12-25

---

## Unit Tests

### Header
- [ ] `WalHeader` size = 20 bytes
- [ ] All fields correct byte offset

### CRC32
- [ ] Checksum matches payload
- [ ] Corrupted payload → detected

### Serialization
- [ ] All `WalEntryType` round-trip
- [ ] `OrderPayload` round-trip

---

## Integration Tests

### Write-Read
- [ ] Write 100 entries → Read back → Match
- [ ] Mixed types → Correct dispatch

### Epoch
- [ ] Epoch=0 on fresh start
- [ ] Epoch increments on reset

### Edge Cases
- [ ] Empty payload → valid
- [ ] Max 64KB payload → valid
- [ ] Partial write → reader stops safely

---

*Ref*: `docs/agents/sessions/architect/0x0D-wal-format-spec.md`
