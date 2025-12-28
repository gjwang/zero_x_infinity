# 0x10 Backend Gap Requirements (MASTER LIST)

> **Status**: **ACTIVE** (Updated 2025-12-27)
> **Context**: This document tracks ALL missing backend features required for a fully functional Frontend & Exchange.

## 🚨 P0 - Critical Blockers (Trading Loop)
*Without these, the Frontend cannot perform the core "Register -> Deposit -> Trade" loop.*

### 1. **User Authentication Service** ✅ **IMPLEMENTED**
*   **Status**: ✅ Verified (0x10.6)
*   **Problem**: Current `src/api_auth` only handles *API Key Verification* (Ed25519). There is NO Way for a human to:
    *   Register (Sign Up).
    *   Login (Username/Password).
    *   Obtain a Session/JWT.
*   **Solution**: Implemented `src/user_auth` with Argon2id + JWT.
*   **Requirement**:
    *   `POST /api/v1/user/register`: Email, Password.
    *   `POST /api/v1/user/login`: Returns Session Token / JWT.
    *   `POST /api/v1/user/logout`

### 2. **User Center & API Key Management** ✅ **IMPLEMENTED**
*   **Status**: ✅ Verified (0x10.6)
*   **Problem**: Users cannot create the `Ed25519` keys required to trade.
*   **Solution**: Implemented `POST /api/v1/user/apikeys` (Show Secret Once).
*   **Requirement**:
    *   `POST /api/v1/user/apikeys`: Create new API Key (Generate Key Pair or Upload PubKey).
    *   `GET /api/v1/user/apikeys`: List keys.
    *   `DELETE /api/v1/user/apikeys/{id}`: Revoke.

### 3. **WebSocket Private Channels** ⚠️ DESIGN NEEDED
*   **Problem**: Code exists (`src/websocket/service.rs`) but is blocked by "Strict Anonymous Mode".
*   **Requirement**: Implement **Auth Strategy** (JWT in Query Params?) to unblock `ws_handler`.
*   **Action**: Create `docs/src/0x10-websocket-auth.md`.

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
1.  **Phase 0x10.6 (Essential Services)**: ✅ **COMPLETED** (User Loop Unblocked).
2.  **Phase 0x11 (Deposit & Withdraw)**: 🔄 **IN PROGRESS** (Research & Design).
3.  **Phase 0x10.5 (WebSocket Auth)**: ⚠️ **BLOCKED** (Pending Design).
