# 0x10 Backend Gap Requirements

> **Status**: Draft
> **Priority**: P0 (Frontend Blockers)

This document outlines the backend development tasks required to fully support the **[0x10 Web Frontend](./0x10-web-frontend.md)**. These features are currently marked as "Missing" (❌) or "Partial" and are blockers for a complete Trading MVP.

---

## 1. Public Market Data APIs (REST)

**Goal**: Provide public historical data for charts and "Last Trades" widget.

### 1.1 Public Trade History
*   **Endpoint**: `GET /api/v1/public/trades`
*   **Description**: Get recent trades for a specific symbol.
*   **Parameters**:
    *   `symbol` (required): e.g., `BTC_USDT`
    *   `limit` (optional, default 500, max 1000)
    *   `fromId` (optional): Fetch trades > id (pagination)
*   **Response**: `Vec<PublicTrade>`
    ```json
    [
      {
        "id": 28457,
        "price": "43000.00",
        "qty": "0.15",
        "quote_qty": "6450.00",
        "time": 1703660555000,
        "is_buyer_maker": true,
        "is_best_match": true
      }
    ]
    ```
*   **Implementation Note**:
    *   Query **TDengine** `trades` table.
    *   Filter by `symbol`.
    *   Need to support efficient pagination (time-based or ID-based if strictly monotonic).

---

## 2. WebSocket Push Channels

**Goal**: Provide real-time data for the Trading UI.
**Base URL**: `ws://host:port/ws`

### 2.1 Ticker (Rolling 24h Stats)
*   **Channel**: `market.ticker`
*   **Topic**: `market.ticker.{symbol}` (e.g., `market.ticker.BTC_USDT` or `market.ticker.all`)
*   **Update Frequency**: 1000ms (Throttled)
*   **Payload**:
    ```json
    {
      "e": "24hTicker",
      "s": "BTC_USDT",
      "p": "0.15",      // Price Change
      "P": "0.35",      // Price Change %
      "o": "42850.00",  // Open Price
      "h": "43200.00",  // High Price
      "l": "42800.00",  // Low Price
      "c": "43000.00",  // Current Price
      "v": "1500.25",   // Base Volume
      "q": "64510750.0" // Quote Volume
    }
    ```
*   **Implementation Challenge**:
    *   Need a **Rolling Window** Aggregator.
    *   TDengine caching or in-memory ring buffer aggregation required for performance.

### 2.2 Incremental Depth (Optional) or Diff Depth
*   **Channel**: `market.depth`
*   **Topic**: `market.depth.{symbol}`
*   **Update Frequency**: 100ms or 1000ms
*   **Payload**:
    ```json
    {
      "e": "depthUpdate",
      "s": "BTC_USDT",
      "U": 157, // First update ID
      "u": 160, // Final update ID
      "b": [    // Bids to update [price, qty]
        ["43000.00", "1.5"],
        ["42998.00", "0.0"] // 0.0 means remove level
      ],
      "a": [    // Asks to update
        ["43001.00", "5.2"]
      ]
    }
    ```
*   **MVP Shortcut**: sending `partialBookDepth` (Top 200 levels snapshot) every 1s is acceptable for v1 if diff is too complex.

### 2.3 Public Trade Stream
*   **Channel**: `market.trade`
*   **Topic**: `market.trade.{symbol}`
*   **Payload**: Real-time broadcast of every match.
    ```json
    {
      "e": "trade",
      "s": "BTC_USDT",
      "t": 12345,       // Trade ID
      "p": "43000.00",  // Price
      "q": "0.1",       // Quantity
      "T": 1703660555000, // Time
      "m": true         // Is Buyer Maker?
    }
    ```
*   **Source**: The `Settlement` service already processes trades. It needs to emit a public event (stripping user ID).

---

## 3. Implementation Plan (Phase 0x10.5)

1.  **Step 1: Public Trades API**
    *   Add `get_public_trades` handler in Gateway.
    *   Wire to TDengine.
2.  **Step 2: WebSocket Broadcaster Refactor**
    *   Current `WS` handler only supports private user streams.
    *   Refactor `Session` to support topic subscription (Pub/Sub pattern).
3.  **Step 3: Ticker & Trade Stream**
    *   Implement "Global Broadcast" mechanism in `Settlement` layer.
    *   Connect `Settlement` -> `Gateway` via channel for public messages.

---
