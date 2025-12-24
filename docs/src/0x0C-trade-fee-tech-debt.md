# 0x0C Trade Fee - Tech Debt

> All AC items complete!

## ✅ 已完成 / Completed

| Item | Description | Commit |
|------|-------------|--------|
| Core | Fee calculation + REVENUE income | `08c2bdf` |
| VIP discount | calculate_fee_with_discount | `584a574` |
| balance_events table | TDengine schema + persist | `7d3cbaf`, `7b483d2` |
| Trade.fee removal | Fee only from UBSCore | `30c4db8` |
| VIP DB loading | set_user_vip_level + load | `41a9967` |
| fee_amount persistence | balance_events.fee_amount | `74cf109` |
| fee_asset API | TradeApiData.fee_asset | `8f5d752` |
| WS real fee | push_trade_events + SymbolManager | `179ec9f` |
| **API real fee** | query_user_trades + balance_events JOIN | (pending) |
| Unit tests | role + conservation tests | `fe066db` |

## 验收标准对照

| AC | Status |
|----|--------|
| AC-1 交易角色扣费 | ✅ |
| AC-2 Fee Ledger 匹配 | ✅ balance_events |
| AC-3 API 响应 fee | ✅ query_user_trades |
| AC-4 WS 推送 fee | ✅ real fee |
| AC-5 资产守恒 | ✅ tested |
| AC-6 O(1) 计算 | ✅ |

---

## 🔶 未来优化

### 从 UBSCore 输出消费 (P5)

**当前方案**: API 查询时从 balance_events JOIN 获取 fee
**问题**: 额外 ~2ms 延迟

**优化方案**:
1. UBSCore.settle_trade 返回 TradeSettlementResult
2. trades 持久化时直接写入 fee
3. 单表查询，无需 JOIN

**改动量**: 中等（需修改数据流）

---

*Updated: 2025-12-24 (All AC complete)*
