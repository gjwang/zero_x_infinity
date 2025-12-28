# Architect → QA: 0x10.5 Backend Gaps Test Requirements

## 📦 Deliverables

- [x] Test Specification: `docs/src/0x10-qa-test-plan.md`
- [x] Architecture Overview: `docs/src/0x10-backend-gaps.md`
- [x] Design Walkthrough: (See 0x10-backend-gaps 2.3)

## 🎯 Test Goal

**ONE SENTENCE**: Verify that Public Market Data APIs and WebSocket Streams return correct, timely data matching the matching engine's activity.

## 🔑 Key Test Scenarios

### Must Test (P0)
1.  **Public Trade History**: `GET /api/v1/public/trades` returns correct trades after an order match.
2.  **Ticker Accuracy**: `market.ticker` WebSocket updates `v` (Msg Volume) and `c` (Close Price) correctly.
3.  **Real-time Push**: `market.trade` WebSocket event arrives < 10ms after REST `order` response (network permitting).

### Should Test (P1)
1.  **Pagination**: `fromId` parameter correctly pages through trade history.
2.  **Symbol Filter**: Requesting `BTC_USDT` does NOT return `ETH_USDT` trades.

### Can Test (P2)
1.  **High Concurrency**: 1000 WS clients subscribing simultaneously (Load Test).

## ⚠️ Test Difficulty Warning

| Difficulty | Reason | Suggestion |
|------------|--------|------------|
| **Timing Verification** | WS is async; exact 10ms check is hard. | Use automated script to timestamp "Order Ack" vs "WS Event". |
| **Rollover Logic** | 24h Ticker needs sliding window. | Verify "Open Price" changes as window slides (hard to mock 24h). |

## 📝 Test Data Suggestion

- Use `fixtures/test_with_cancel_highbal` to generate initial baseline data.
- Need to create new test data: Continuous trading script (e.g., ping-pong bot) to verify Ticker updates.

## 🔗 Related Documents
- Implementation Plan: `docs/agents/sessions/shared/arch-to-dev-handover.md`

## 📞 Ready for Test Planning

**Architect Signature**: @Antigravity (Architect Role)
**Date**: 2025-12-27
**Status**: ✅ Ready for QA review
