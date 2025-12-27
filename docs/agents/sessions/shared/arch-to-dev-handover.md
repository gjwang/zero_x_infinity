# Architect → Developer: 0x10.5 Backend Gaps Handover

## 📦 Design Package Deliverables

- [x] Architecture & Requirements: `docs/src/0x10-backend-gaps.md`
- [x] Frontend Specification: `docs/src/0x10-web-frontend.md`
- [x] Test Checklist (QA): `docs/src/0x10-qa-test-plan.md`

## 🎯 Implementation Goal

**ONE SENTENCE**: Implement the missing Public Market Data APIs and WebSocket Channels required by the 0x10 Frontend MVP.

**Key Metrics**:
- **Availability**: Public endpoints must be cache-friendly and high-availability.
- **Latency**: WebSocket `trade` push < 10ms from matching.
- **Precision**: 100% adherence to `String` format for all prices/quantities.

## 📋 Implementation Plan Summary (Phase 0x10.5)

### Phase 1: Public Trades (Priority P0)
- Task 1.1: Define `PublicTrade` struct in `gateway`.
- Task 1.2: Implement `GET /api/v1/public/trades` querying TDengine.
- Task 1.3: Add pagination support (`limit`, `fromId`).

### Phase 2: WebSocket Broadcast (Priority P0)
- Task 2.1: Refactor `WebSocket` session to separate "Private" (User) from "Public" (Market) subscriptions.
- Task 2.2: Implement `market.trade.{symbol}` stream.
- Task 2.3: Implement `market.ticker` 24h stats rolling aggregator.

## 🔑 Key Design Decisions

| Decision | Rationale | Alternatives |
|----------|-----------|--------------|
| **TDengine for Trades** | Single source of truth for history; specific query support. | Postgres (too slow for high vol), Memory (too varied). |
| **Separated WS Channels** | `market.` prefix distinct from private events for clear security boundary. | Mixed stream (risk of leaking private info). |
| **String Precision** | Frontend requirement for visual consistency (0-gc). | Float (precision loss), Decimal (serialization complexity). |

## ⚠️ Implementation Notes

### DO (Must)
- [ ] Use `format_decimal` utility for all numeric JSON fields.
- [ ] Ensure `PublicTrade` struct derives `Serialize` and `ToSchema` (for OpenAPI).
- [ ] Add unit tests for the specific TDengine query generation.

### DON'T (Forbidden)
- [ ] Do NOT expose `user_id` or `order_id` in public trade stream.
- [ ] Do NOT perform `f64` math for Ticker aggregation (use Decimal/u64).

## 📝 Code Example (PublicTrade)

```rust
#[derive(Serialize, ToSchema)]
pub struct PublicTradeApiData {
    pub id: i64,
    pub price: String, // "45000.00"
    pub qty: String,   // "0.1000"
    pub time: i64,     // 1700000000000
    pub is_buyer_maker: bool,
}
```

## 📞 Ready for Development

**Architect Signature**: @Antigravity (Architect Role)
**Date**: 2025-12-27
**Status**: ✅ Ready for implementation
