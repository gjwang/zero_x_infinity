# QA → Developer: L4 Matching Engine Bug Report

## 📦 Bug Report Summary

**Report ID**: SEC-AUDIT-2024-1230-L4  
**Reporter**: QA Security Expert (AI Agent)  
**Date**: 2025-12-30  
**Priority**: P0 (Critical)

### 发现的缺陷

| ID | 描述 | 严重性 | 状态 |
|----|------|--------|------|
| SEC-001 | Maker订单状态不更新为FILLED | P0 | 待修复 |
| SEC-002 | Maker交易记录缺失 | P0 | 待修复 |
| SEC-003 | Maker余额未更新 | P0 | 待修复 |
| SEC-004 | `/trades` API泄露全局数据 | P1 | 待修复 |

---

## 🧪 复现测试

已创建分解测试用于隔离和验证每个Bug:

```bash
cd scripts/tests/0x11b_sentinel

# L4d: 隔离 SEC-001/002/003 (Maker bug)
uv run python3 L4d_maker_verification.py

# L4e: 隔离 SEC-004 (数据泄露)
uv run python3 L4e_data_isolation.py

# 或运行全部分解测试
./run_L4_decomposed.sh
```

### 预期结果
- L4a/L4b/L4c: ✅ PASS (基础功能正常)
- L4d: ❌ FAIL (确认Maker bug)
- L4e: ❌ FAIL (确认数据泄露)

---

## 🔍 根本原因分析

### SEC-001/002/003 (Maker问题)

**症状**: Taker订单正常成交,但Maker订单:
- 状态保持`NEW`(应为`FILLED`)
- 无交易记录写入TDengine
- Spot余额未变化

**根因**: `OrderExecutedEvent`事件链只处理了Taker侧
```
MatchingEngine → OrderExecutedEvent → Sentinel → UBS → Gateway
                      ↑
                   BUG: Maker端事件未触发
```

### SEC-004 (数据泄露)

**症状**: 用户A调用`/trades`能看到用户B的交易

**根因**: trades查询缺少`user_id`过滤
```sql
-- 当前 (BUG)
SELECT * FROM trades WHERE symbol = ?

-- 应该是
SELECT * FROM trades WHERE symbol = ? AND user_id = ?
```

---

## ✅ 验收标准

修复完成后,需满足:

1. [ ] `L4d_maker_verification.py` 通过
2. [ ] `L4e_data_isolation.py` 通过
3. [ ] 原始 `L4_two_user_matching.py` 通过
4. [ ] Maker订单状态正确更新为FILLED
5. [ ] Maker交易记录正确写入
6. [ ] 每个用户只能看到自己的trades

---

## 📁 交付的测试文件

| 文件 | 用途 |
|------|------|
| `L4a_user_isolation.py` | 用户隔离验证 |
| `L4b_order_placement.py` | 下单API验证 |
| `L4c_taker_verification.py` | Taker成交验证 |
| `L4d_maker_verification.py` | **Maker bug隔离** |
| `L4e_data_isolation.py` | **数据泄露隔离** |
| `run_L4_decomposed.sh` | 分解测试运行器 |

---

## 📞 Ready for Developer

QA签名: @QA Security Expert AI  
Date: 2025-12-30 20:35  
Status: ⚠️ **P0 BLOCKERS - 需要Developer修复**
