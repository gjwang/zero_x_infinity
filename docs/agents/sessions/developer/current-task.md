# Current Task: Phase 0x10.6 Essential Services

## Session Info
- **Date**: 2025-12-27
- **Role**: Developer
- **Status**: ⏳ **Pending Pickup**

## 🎯 Objective
Implement **User Authentication** and **API Key Management** to unblock the Frontend User Loop.

## 🔗 References
- **Handover Doc**: `docs/agents/sessions/shared/arch-to-dev-handover-0x10-6.md`
- **Design Doc**: `docs/src/0x10-6-essential-services.md`

## 🛠️ Tasks
1.  **Database**: Apply `migrations/0x10_06_users_auth.sql` (Add password fields).
2.  **Auth Service**: Implement `src/user_auth` (Register/Login/JWT).
3.  **User Center**: Implement `POST /api/v1/user/apikeys` (Ed25519 Key Gen).

## 🚨 Constraints
- **Security**: Do NOT store API Private Keys. Store Public Key only. Show Private Key once.
- **Strict Separation**: Do not mix `user_auth` code into `api_auth`.

## ✅ Definition of Done
- User can Register & Login via REST API.
- User can Generate an API Key Set.
- User can use that API Key to call `GET /private/balances`.
