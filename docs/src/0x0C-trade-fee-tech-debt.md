# 0x0C Trade Fee - Tech Debt

> Phase 1 + A01-A02 完成后的剩余工作

## 已完成 / Completed ✅

| Item | Description | Commit |
|------|-------------|--------|
| Core | Fee calculation + REVENUE income | `08c2bdf` |
| VIP discount | calculate_fee_with_discount | `584a574` |
| balance_events table | TDengine schema + persist | `7d3cbaf`, `7b483d2` |
| Trade.fee removal | Fee only from UBSCore | `30c4db8` |
| VIP DB loading | set_user_vip_level + load | `41a9967` |
| **fee_amount persistence** | balance_events.fee_amount | `74cf109` |
| **fee_asset API** | TradeApiData.fee_asset | `8f5d752` |

## 待完成 / Tech Debt 🔴

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

---

### Note: API fee value still 0

trades.fee=0 in TDengine because ME doesn't know fee.
Real fee stored in `balance_events.fee_amount`.

Future: Add `query_user_trade_fees()` to join trades with
balance_events for complete fee display.

---

## 验收标准对照

| AC | Status |
|----|--------|
| AC-1 交易角色扣费 | ✅ |
| AC-2 Fee Ledger 匹配 | ✅ balance_events.fee_amount |
| AC-3 API 响应 fee | ⚠️ fee_asset OK, fee=0 (needs balance_events join) |
| AC-4 WS 推送 fee | ❌ P2 |
| AC-5 资产守恒 | ✅ |
| AC-6 O(1) 计算 | ✅ |

---

*Updated: 2025-12-24*
