# Architect → Developer: Phase 0x10.5 Handover

## 📦 Design Package
- **Architecture**: `docs/src/0x10-websocket-auth.md`
- **Context**: Unblocking QA Private Channel Testing.

## 🎯 Implementation Goal
Implement **WebSocket Authentication** using JWT Query Parameters, enforcing strict `Option<u64>` semantics.

## ⛔️ STRICT CONSTAINTS (USER MANDATE)
1.  **NO Magic Numbers**: `user_id = 0` is BANNED.
2.  **NO Implicit Fallback**: If a token is provided but invalid, **REJECT** the connection (401). **DO NOT** silently downgrade to Anonymous.
3.  **Explicit Types**: Use `Option<u64>` for user ID throughout the WebSocket stack (`None` = Anonymous, `Some(id)` = Authenticated).

## 📋 Implementation Plan

### Step 1: Handler Refactor (`src/websocket/handler.rs`)
- Remove the "Strict Anonymous" block.
- Parse `?token=` from Query.
- If present -> Verify JWT -> `Some(uid)` or Error.
- If absent -> `None`.

### Step 2: Connection Manager (`src/websocket/connection.rs`)
- Update `Connection` struct to store `user_id: Option<u64>`.
- **Refactor**: Remove `ANONYMOUS_USER_ID` constant if it exists; use `None`.

### Step 3: Subscription Logic
- Enforce the **Permission Matrix**:
    - `Public` Channels: Allow `Any`.
    - `Private` Channels: Require `user_id.is_some()`.
    - **Explicit Error**: Return `AuthError::LoginRequired` if `None` tries to access `Private`.

## 📞 Ready for Development
**Architect Signature**: @Antigravity
**Status**: ✅ Handover Ready (Strict Mode)
