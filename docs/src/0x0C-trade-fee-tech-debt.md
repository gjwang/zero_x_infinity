# 0x0C Trade Fee - Tech Debt

> Updated after P3 completion

## ✅ 已完成 / Completed

| Item | Description | Commit |
|------|-------------|--------|
| Core | Fee calculation + REVENUE income | `08c2bdf` |
| VIP discount | calculate_fee_with_discount | `584a574` |
| balance_events table | TDengine schema + persist | `7d3cbaf`, `7b483d2` |
| Trade.fee removal | Fee only from UBSCore | `30c4db8` |
| VIP DB loading | set_user_vip_level + load | `41a9967` |
| **fee_amount persistence** | balance_events.fee_amount | `74cf109` |
| **fee_asset API** | TradeApiData.fee_asset | `8f5d752` |
| **WS fee fields** | trade event fee, fee_asset, role | `f0a2163` |
| **Unit tests** | role + conservation tests | `fe066db` |

## 🔶 剩余 Tech Debt

### API/WS fee 值为 0

**现状**: trades.fee=0, WS fee=0 (ME 阶段无 fee 信息)

**真实 fee 存储位置**: `balance_events.fee_amount`

**未来优化方案**:
1. 添加 `query_user_trade_fees()` 从 balance_events 查询
2. 或将 SymbolManager 传入 SettlementService 计算 fee

---

## 验收标准对照

| AC | Status |
|----|--------|
| AC-1 交易角色扣费 | ✅ |
| AC-2 Fee Ledger 匹配 | ✅ balance_events |
| AC-3 API 响应 fee | ⚠️ fee_asset✅, fee=0 |
| AC-4 WS 推送 fee | ⚠️ fee_asset✅, fee=0 |
| AC-5 资产守恒 | ✅ tested |
| AC-6 O(1) 计算 | ✅ |

---

*Updated: 2025-12-24 (P0-P3 complete)*
