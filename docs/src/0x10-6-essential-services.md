# 0x10.6 Essential Services Design (Auth & User Center)

| Status | **DRAFT** |
| :--- | :--- |
| **Date** | 2025-12-27 |
| **Context** | P0 Blocker Resolution for Frontend MVP |

## 1. Overview
To enable the "User Loop" (Register -> Login -> Create API Key -> Trade), we must implement a proper **User Authentication Service** and **User Center**.
Currently, the system only supports *API Key Verification* (machine-to-machine). It lacks *User Session Management* (human-to-machine).

## 2. Architecture Changes

### 2.1 Database Schema (`users` table expansion)
The existing `users` table (implied by `User` struct) needs expansion to support password login.

```sql
-- Existing (implied)
-- user_id, username, email, status, flags, created_at

-- NEW COLUMNS
ALTER TABLE users ADD COLUMN password_hash VARCHAR(255); -- Argon2id
ALTER TABLE users ADD COLUMN salt VARCHAR(64);           -- If not embedded in hash
ALTER TABLE users ADD COLUMN two_factor_secret VARCHAR(64);
```

### 2.2 New Module: `src/user_auth`
Separted from `api_auth` (which is for API Signatures).
*   **Responsibility**: Handle human login, session issuance (JWT), and registration.
*   **Dependencies**: `argon2`, `jsonwebtoken`, `validator`.

## 3. API Specification

### 3.1 Authentication (`/api/v1/auth`)

#### `POST /register`
*   **Input**: `{ "email": "...", "password": "..." }`
*   **Logic**:
    1.  Validate email format.
    2.  Check duplicate email.
    3.  Hash password (Argon2id).
    4.  Insert into `users`.
*   **Output**: `{ "user_id": 123 }` (201 Created)

#### `POST /login`
*   **Input**: `{ "email": "...", "password": "..." }`
*   **Logic**:
    1.  Lookup user by email.
    2.  Verify password hash.
    3.  Generate **JWT Session Token** (Exp: 24h).
*   **Output**: `{ "token": "eyJ...", "expiry": 170... }`

#### `POST /logout`
*   **Input**: Header `Authorization: Bearer <jwt>`
*   **Logic**: (Optional) Blacklist JWT in Redis. Client side clear.

### 3.2 User Center (`/api/v1/user`)

#### `GET /profile`
*   **Auth**: Bearer JWT.
*   **Output**: `{ "user_id": 123, "email": "...", "kyc_level": 1 }`

#### `GET /apikeys`
*   **Auth**: Bearer JWT.
*   **Output**: List of API Keys `{ "id": 1, "api_key": "...", "status": "Active", "created_at": ... }`

#### `POST /apikeys`
*   **Auth**: Bearer JWT.
*   **Input**: `{ "label": "My Trading Bot", "permissions": ["Trade", "Read"] }`
*   **Logic**:
    1.  Generate `Ed25519` Key Pair (Server-side generated for ease of use, OR allow upload).
    2.  **Decision**: For MVP, **Server-Generates key pair**.
        *   Returns `api_key` (Public) and `api_secret` (Private Key Base62).
        *   Server stores `api_key` (Public) in DB.
        *   **CRITICAL**: `api_secret` is SHOWN ONCE to user, never stored.
*   **Output**: `{ "api_key": "...", "api_secret": "..." }` (Show Once)

#### `DELETE /apikeys/{id}`
*   **Auth**: Bearer JWT.
*   **Logic**: Delete/Disable key.

## 4. Implementation Plan

### Step 1: Database Migration
*   Create `migrations/0x10_06_users_auth.sql`.
*   Add `password_hash` column.

### Step 2: Dependencies
*   Add `argon2`, `jsonwebtoken`, `validator`, `rand`.

### Step 3: Implement `src/user_auth`
*   `service.rs`: Hash/Verify logic.
*   `jwt.rs`: Token generation/validation.

### Step 4: Implement Handlers
*   `handlers.rs`: Connect `Gateway` to `UserAuthService`.

### Step 5: Testing
*   Unit Test: Password hashing, JWT signing.
*   E2E Test: Register -> Login -> Create Key -> Trade (using verified key).
