# Current Task: Phase 0x10.5 WebSocket Auth

## Session Info
- **Date**: 2025-12-27
- **Role**: Developer
- **Status**: ⏳ **Pending Pickup**

## 🎯 Objective
Implement **WebSocket Authentication** to fix the QA Blocker, stripping out all legacy "Strict Anonymous" code.

## 🔗 References
- **Handover Doc**: `docs/agents/sessions/shared/arch-to-dev-handover-0x10-5.md`
- **Design Doc**: `docs/src/0x10-websocket-auth.md`

## 🛠️ Tasks
1.  **Refactor Handler**: `src/websocket/handler.rs` -> User `Option<u64>`.
2.  **Strict Auth**: Reject invalid tokens (401). No Implicit Downgrade.
3.  **Permission Check**: Enforce `Private` channel requires `Some(uid)`.

## 🚨 Constraints
- **NO Magic Numbers**: `user_id = 0` is forbidden. Use `None`.
- **Security Check**: `test_qa_adversarial.py` must pass.

## ✅ Definition of Done
- WebSocket connection accepts `?token=JWT`.
- Authenticated user can subscribe to `order.update`.
- Anonymous user (no token) can ONLY see `ticker`.
