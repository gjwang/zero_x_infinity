# QA Rejection Report: Phase 0x10.5 Backend Gaps

> **Status**: 🔴 **REJECTED** (Critical Security Vulnerability)
> **Date**: 2025-12-27
> **Author**: QA Agent (Antigravity)

## 🚨 Critical Finding: Identity Spoofing (P0)

**The implementation of WebSocket "Public Access" introduces a severe security vulnerability that allows any user to impersonate another user.**

### 1. Vulnerability Description

The WebSocket handshake handler (`src/websocket/handler.rs`) blindly accepts the `user_id` provided in the query parameter without any signature, token, or authentication verification.

```rust
// src/websocket/handler.rs:34
let user_id = params.user_id.unwrap_or(0); // 0 = Anonymous
ws.on_upgrade(move |socket| handle_socket(socket, user_id, manager))
```

This effectively allows any client to assert "I am User 1001" by simply connecting to:
`ws://host:port/ws?user_id=1001`

### 2. Proof of Concept (PoC)

**Script**: `scripts/test_qa_adversarial.py` (Created by QA)

**Execution**:
```bash
python3 scripts/test_qa_adversarial.py
```

**Output**:
```
🕵️  TEST: Identity Spoofing (Connection w/o Token)
   Response: {'type': 'connected', 'user_id': 1001}
   ❌ VULNERABILITY CONFIRMED: Server accepted unauthenticated identity 1001
```

### 3. Impact Analysis

1.  **Identity Theft**: Malicious actors can connect as victim users (e.g., High Net Worth individuals, Admins).
2.  **Private Data Leakage**: Once connected as `user_id=1001`, the attacker can subscribe to any future private channels (e.g., `private.order`, `private.balance`) intended for that user. Even if these channels don't exist *yet*, this architectural flaw poisons the foundation.
3.  **Spoofed Actions**: If the WebSocket allows upstream commands (like `cancel_order`), an attacker could execute them on behalf of the victim.

### 4. Rejection Criteria

This implementation violates the **First Principles of Security**:
> "Never trust client input for identity without cryptographic verification."

The Developer's decision to support "Anonymous" access by making `user_id` optional was valid, but checking the *presence* of `user_id` without checking its *authenicity* (via JWT or Session ID) is unacceptable.

## 🛠 Required Fixes

1.  **Enforce Authentication**: Accessing `/ws` with a non-zero `user_id` **MUST** require a valid JWT or Session Token.
2.  **Strict Anonymous Mode**: If `user_id` is not provided (or 0), the session must be strictly flagged as `Anonymous` and **physically incapable** of subscribing to any `private.*` topics.
3.  **Refactor Handshake**: Prefer `Authorization` header over Query Parameters for sensitive tokens.

## Handover Status

❌ **Returned to Developer for Immediate Remediation.**
