# QA → Developer: Settlement E2E Bug Report

> **From**: QA Engineer  
> **To**: Developer  
> **Date**: 2025-12-26 03:30  
> **Status**: ⚠️ **Bugs Found - Fix Required**

---

## 📋 Verification Summary

| 测试 | 结果 |
|------|------|
| Transfer E2E (11/11) | ✅ PASS |
| Settlement Unit (9/9) | ✅ PASS |
| Full Unit (286/286) | ✅ PASS |
| **Settlement E2E (14 steps)** | ❌ FAIL (Step 7) |

---

## 🐛 Bugs Found

### BUG-001: `inject_orders.py` 硬编码端口 (P0)

**Location**: `scripts/inject_orders.py:427-429`

**Issue**:
```python
# 当前代码
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(2)
result = sock.connect_ex(('localhost', 8080))  # ❌ 硬编码 8080
```

**Impact**: 当 E2E 脚本使用 `PORT=18082` 时，注入脚本检测 8080 失败，返回错误。

**Fix**:
```python
from urllib.parse import urlparse

# 从 GATEWAY_URL 解析端口
parsed = urlparse(GATEWAY_URL)
host = parsed.hostname or 'localhost'
port = parsed.port or 8080

result = sock.connect_ex((host, port))
```

---

### BUG-002: E2E 脚本空值比较 (P1)

**Location**: `scripts/test_settlement_recovery_e2e.sh:215-217`

**Issue**:
```bash
ACCEPTED=$(echo "$INJECT_RESULT" | grep -o 'Accepted:.*' | ...)

if [ "$ACCEPTED" -eq 0 ]; then  # ❌ 空字符串导致语法错误
```

**Symptom**:
```
./scripts/test_settlement_recovery_e2e.sh: line 217: [: : integer expression expected
```

**Fix**:
```bash
if [ -z "$ACCEPTED" ] || [ "$ACCEPTED" -eq 0 ]; then
```

---

## ✅ Approved Deliverables

### Transfer P0 Fixes
- TC-P0-04 (Precision): ✅ APPROVED
- TC-P0-07 (Idempotency): ✅ APPROVED
- **Ready for merge**

### Settlement WAL Unit Tests
- 9/9 tests passing: ✅ APPROVED
- Code quality good

---

## 🔄 Re-verification After Fix

```bash
# Fix BUG-001 and BUG-002, then:
./scripts/test_settlement_recovery_e2e.sh

# Expected: 14 passed; 0 failed
```

---

*QA → Developer Handover*  
*遵循: `docs/agents/workflows/dev-to-qa-handover.md`*
