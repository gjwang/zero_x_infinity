# 0x10 Backend Gap Requirements (MASTER LIST)

> **Status**: **ACTIVE** (Updated 2025-12-27)
> **Context**: This document tracks ALL missing backend features required for a fully functional Frontend & Exchange.

## 🚨 P0 - Critical Blockers (Trading Loop)
*Without these, the Frontend cannot perform the core "Register -> Deposit -> Trade" loop.*

### 1. **User Authentication Service** ❌ MISSING
*   **Problem**: Current `src/api_auth` only handles *API Key Verification* (Ed25519). There is NO Way for a human to:
    *   Register (Sign Up).
    *   Login (Username/Password).
    *   Obtain a Session/JWT.
*   **Requirement**:
    *   `POST /api/v1/user/register`: Email, Password.
    *   `POST /api/v1/user/login`: Returns Session Token / JWT.
    *   `POST /api/v1/user/logout`

### 2. **User Center & API Key Management** ❌ MISSING
*   **Problem**: Users cannot create the `Ed25519` keys required to trade.
*   **Requirement**:
    *   `POST /api/v1/user/apikeys`: Create new API Key (Generate Key Pair or Upload PubKey).
    *   `GET /api/v1/user/apikeys`: List keys.
    *   `DELETE /api/v1/user/apikeys/{id}`: Revoke.

### 3. **WebSocket Private Channels** ⚠️ BLOCKED
*   **Problem**: Code exists (`src/websocket/service.rs`) but is blocked by "Strict Anonymous Mode".
*   **Requirement**: Implement **Auth Strategy** (e.g., Ticket/ListenKey) to unblock `ws_handler`.

---

## 🔸 P1 - Core Financial Features (Money Loop)
*Blocking Real-Money Trading (can be mocked for dev).*

### 4. **Deposit System (Phase 0x11)** ❌ MISSING
*   **Problem**: No way to fund accounts from blockchain.
*   **Requirement**:
    *   Address Generation: `GET /api/v1/capital/deposit/address`.
    *   Chain Listener: Detect on-chain tx -> Mint internal balance.

### 5. **Withdrawal System (Phase 0x11)** ❌ MISSING
*   **Problem**: User cannot cash out.
*   **Requirement**:
    *   `POST /api/v1/capital/withdraw/apply`.
    *   Internal Approval Workflow.
    *   Blockchain Broadcast.

---

## 🔹 P2 - Operational Features
*Required for Production Launch but not Dev Testing.*

### 6. **Admin Dashboard APIs** ⚠️ PARTIAL
*   **Status**: `fastapi_amis_admin` exists but may lack specific User/Asset ops.
*   **Requirement**:
    *   User Management (Ban/Unban).
    *   Asset Config (Enable/Disable Dep/Wdw).
    *   Trade Correction.

### 7. **KYC / Security** ❌ MISSING
*   **Requirement**: 2FA (TOTP), Identity Verification.

---

## 📋 Recommended Roadmap
1.  **Immediate Next (Phase 0x10.6?)**: Implement **User Auth & API Key Mgmt** (Unblocks Frontend "User Center" & Trading).
2.  **Next (Phase 0x11)**: Deposit & Withdraw (Unblocks "Assets" page).
3.  **Finally**: Admin & Operations.
