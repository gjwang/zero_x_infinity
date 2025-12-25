# Developer → QA: Fee E2E Path Fix Handover

> **From**: Developer (AI Agent)  
> **To**: QA Engineer  
> **Date**: 2025-12-26  
> **Status**: ✅ **Ready for QA Verification**

---

## 📦 交付物清单

- [x] Bug修复: `scripts/lib/db_env.sh` (commit: c64ef9c)
- [x] 验证测试: Fee E2E 5/5 通过
- [x] 回归测试: 5/5 单元测试通过
- [x] QA交接文档更新 (commit: 898b95f)

---

## 🔍 问题分析

### Root Cause
`scripts/lib/db_env.sh` 第 18 行使用了 `SCRIPT_DIR` 变量名：
```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"  # → scripts/lib
```

当 `test_fee_e2e.sh` source 这个文件时：
1. `test_fee_e2e.sh` 设置 `SCRIPT_DIR=/path/to/scripts`
2. source `lib/db_env.sh` 后，`SCRIPT_DIR` 被覆盖为 `/path/to/scripts/lib`
3. 导致后续 `${SCRIPT_DIR}/inject_orders.py` 解析为错误路径

### Fix Applied
将 `db_env.sh` 中的变量重命名为 `_DB_ENV_DIR`，避免命名冲突：
```diff
-SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
-PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
+_DB_ENV_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
+PROJECT_ROOT="$(cd "$_DB_ENV_DIR/../.." && pwd)"
```

---

## 🧪 验证步骤

### 1. 运行 Fee E2E 测试
```bash
./scripts/test_fee_e2e.sh
```

**预期结果**:
```
[Step 1] Checking prerequisites...     ✓
[Step 2] Clearing TDengine database... ✓
[Step 3] Starting Gateway...           ✓
[Step 4] Injecting orders...           ✓
[Step 5] Querying trades API...        ✓

test result: 5 passed; 0 failed; 0 skipped
✅ FEE E2E TEST PASSED
```

### 2. 验证回归
```bash
cargo test --release
```

**预期**: 全部测试通过

### 3. 确认无其他脚本受影响
```bash
# 验证 Transfer E2E 仍正常
./scripts/test_transfer_e2e.sh
```

---

## ✅ 验收标准

- [x] `test_fee_e2e.sh` 5/5 steps 通过
- [x] Fee 字段正确返回 (fee, fee_asset, role)
- [x] Fee 值 > 0 存在
- [ ] 其他 E2E 脚本无回归 (QA验证)
- [ ] 无新增失败测试 (QA验证)

---

## 📝 Git Commits

| Commit | Description |
|--------|-------------|
| `c64ef9c` | fix(test): Rename SCRIPT_DIR to _DB_ENV_DIR in db_env.sh |
| `898b95f` | docs: Mark ISSUE-001 as resolved in QA handover |

---

## 🔗 相关文档

- QA→Dev交接: `docs/agents/sessions/shared/qa-to-dev-handover.md`
- 原始Issue: ISSUE-001 (Fee E2E脚本路径错误)

---

## ⚠️ Breaking Changes

**None**. 内部变量重命名不影响外部调用。

---

## 📞 Ready for QA

Developer: @Developer AI Agent  
Date: 2025-12-26 02:46  
Confidence: **HIGH**  
Status: ✅ Ready for verification
