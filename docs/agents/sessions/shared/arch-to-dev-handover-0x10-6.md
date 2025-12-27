# Architect → Developer: Phase 0x10.6 Handover

## 📦 Design Package
- **Architecture**: `docs/src/0x10-6-essential-services.md`
- **Context**: Critical P0 dependency for Frontend "User Loop".

## 🎯 Implementation Goal
Implement **User Authentication Service** (Login/Register) and **User Center** (API Key Management).

## 📋 Implementation Plan (Phase 0x10.6)

### Phase 1: Database Schema (Priority P0)
- Task 1.1: Create migration `migrations/0x10_06_users_auth.sql`.
    - Add `password_hash`, `salt` to `users` table.

### Phase 2: User Auth Service (Priority P0)
- Task 2.1: Add dependencies (`argon2`, `jsonwebtoken`, `validator`, `rand`).
- Task 2.2: Implement `src/user_auth` module (Service & Handlers).
    - `POST /api/v1/auth/register`
    - `POST /api/v1/auth/login` (Issue JWT)
    - `POST /api/v1/auth/logout`

### Phase 3: User Center (Priority P0)
- Task 3.1: Implement API Key Management.
    - `POST /api/v1/user/apikeys` (Generate & Show Secret ONCE)
    - `GET /api/v1/user/apikeys`
    - `DELETE /api/v1/user/apikeys/{id}`

## 🔑 Key Design Decisions
- **Argon2id**: Use strictly for password hashing (industry standard).
- **JWT**: Stateless session management. Use `HS256` or `EdDSA`.
- **API Secret Security**: **NEVER STORE** the raw API Secret. Show it to user once, store only the hash (or just rely on signature verification which uses Public Key, so we only store Public Key? **Correction**: Ed25519 is Public/Private. We generate both. We store Public Key in DB. We give Private Key to user. We NEVER store Private Key. Correct.)

## 📝 Code Guidelines
- **Strict Separation**: `src/user_auth` (Humans) vs `src/api_auth` (Machines).
- **Security**:
    - Validate email format.
    - Enforce password complexity (min 8 chars).
    - Rate limit login attempts (optional for now, but good to have).

## 📞 Ready for Development
**Architect Signature**: @Antigravity
**Status**: ✅ Handover Ready
