# ADR-002: WebSocket Authentication (Listen Key Pattern)

| Status | **PROPOSED** |
| :--- | :--- |
| **Date** | 2025-12-27 |
| **Author** | Architect Agent |
| **Context** | Phase 0x10.5, Response to AR-001 |

## Context
AR-001 highlights that strict anonymous mode (ADR-001) blocks legitimate authenticated users. We need a secure way to authenticate WebSocket connections.
Standard HTTP headers (`Authorization`) are difficult to set in standard browser `WebSocket` APIs. Sending long-term API credentials (API Key + Secret/Signature) in Query Parameters is a security risk (logs, history).

## Decision: Ephemeral "Listen Key" Resource
We will adopt the industry-standard "Listen Key" pattern (used by Binance, etc.).

### 1. The Mechanism
Authentication is performed via a **2-step process**:
1.  **Authorize (REST)**: Client performs a cryptographically signed REST request to obtain a temporary `listenKey`.
2.  **Connect (WS)**: Client connects to WebSocket using `?listenKey=<key>`.

### 2. API Endpoints
New standard endpoints in `Gateway`:

#### `POST /api/v1/userDataStream`
*   **Auth**: Ed25519 Signed (Standard Private API).
*   **Action**: Generates a random `listenKey` (e.g., UUID or 64-char hex).
*   **Storage**: Stores `listenKey -> user_id` in memory (or Redis) with TTL (e.g., 60 minutes).
*   **Response**: `{ "listenKey": "xp8j...32" }`

#### `PUT /api/v1/userDataStream`
*   **Auth**: Public (API Key in Header) or just `listenKey` in body.
*   **Param**: `listenKey`.
*   **Action**: Extends TTL of the key (Keep-alive).

#### `DELETE /api/v1/userDataStream`
*   **Action**: Invalidates the key immediately.

### 3. WebSocket Upgrade Logic
In `ws_handler`:
1.  Extract `listenKey` from query params.
2.  **Validation**: Look up `listenKey` in the `ListenKeyManager` (Global State).
    *   If found: Return mapped `user_id`.
    *   If missing/expired: Return `401 Unauthorized`.
3.  **Binding**: Bind the WebSocket session to the resolved `user_id`.

## Consequences
*   **Security**: Long-term secrets never travel in WS URL. `listenKey` is short-lived.
*   **Performance**: One lookup during handshake. No crypto overhead on WS connect.
*   **Complexity**: Requires implementing 3 new REST endpoints and an in-memory TTL store.

## Implementation Steps (For Developer)
1.  Add `ListenKeyManager` (DashMap<String, u64> + timestamp) to `AppState`.
2.  Implement `POST/PUT/DELETE /api/v1/userDataStream`.
3.  Update `ws_handler` to check `params.listenKey`.
