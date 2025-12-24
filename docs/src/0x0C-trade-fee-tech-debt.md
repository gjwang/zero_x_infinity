# 0x0C Trade Fee - Tech Debt

> Phase 1 完成后的剩余工作 / Remaining work after Phase 1

## 已完成 / Completed ✅

| Item | Description | Commit |
|------|-------------|--------|
| Core | Fee calculation + REVENUE income | `08c2bdf` |
| VIP discount | calculate_fee_with_discount | `584a574` |
| balance_events table | TDengine schema + persist | `7d3cbaf`, `7b483d2` |
| Trade.fee removal | Fee only from UBSCore | `30c4db8` |
| VIP DB loading | set_user_vip_level + load | `41a9967` |

## 待完成 / Tech Debt 🔴

### P1: API Fee Field (High Priority)

**问题**: `trades` 表 `fee=0`，API 无法返回真实 fee

**解决方案**:
1. BalanceEvent 添加 `fee_amount: Option<u64>` 字段
2. TDengine `balance_events` schema 添加 `fee` 列
3. `settle_receive` constructor 接受 `fee` 参数
4. `query_trades` JOIN balance_events 获取 fee

**对应测试项**: A01, A02 (fee, fee_asset)

---

### P2: WebSocket Fee Push (Medium)

**问题**: `trade.update` WS 事件缺少 fee 信息

**解决方案**:
- `PushEvent::Trade` 添加 `fee`, `fee_asset`, `is_maker` 字段
- SettlementService push 时填充

**对应测试项**: A05

---

### P3: Unit Tests (Medium)

**缺失测试**:
- U08-U10: 角色分配测试 (Maker/Taker)
- C01-C04: 资产守恒验证

**建议**: 在 `src/fee.rs` 和 `src/ubscore.rs` 添加

---

### P4: API fee_asset Field (Low)

**问题**: TradeApiData 有 `fee` 但缺 `fee_asset`

**解决方案**: 
- 买方 fee_asset = base_asset (收到的资产)
- 卖方 fee_asset = quote_asset

**对应测试项**: A02

---

## 验收标准对照

| AC | Status |
|----|--------|
| AC-1 交易角色扣费 | ✅ |
| AC-2 Fee Ledger 匹配 | ⚠️ trades.fee=0, 查 balance_events |
| AC-3 API 响应 fee | ❌ P1 |
| AC-4 WS 推送 fee | ❌ P2 |
| AC-5 资产守恒 | ✅ |
| AC-6 O(1) 计算 | ✅ |

---

*Created: 2025-12-24*
