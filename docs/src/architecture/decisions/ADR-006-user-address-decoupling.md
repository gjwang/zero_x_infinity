# ADR-006: User Address Decoupling for Account-Based Chains

**Date**: 2025-12-30
**Status**: Accepted
**Context**: Replaces `user_addresses` definition in `migrations/010_deposit_withdraw.sql` to enable "Hot Listing".

## 1. Problem Statement
The current schema for user addresses matches Assets, not Chains:

```sql
-- OLD (Flawed)
PRIMARY KEY (user_id, asset, chain_slug)
```

**The Loophole**:
1.  User A has ETH Address `0x123`. DB Record: `(UserA, 'ETH', 'ETH', '0x123')`.
2.  Ops lists `UNI` (ERC20).
3.  User A deposits `UNI` to `0x123`.
4.  Sentinel parses `Transfer(0x123, val)`.
5.  Sentinel looks up: `SELECT user_id FROM user_addresses WHERE address='0x123' AND asset='UNI'`.
6.  **Result**: NULL. Deposit Ignored.

**Impact**: Users must manually "Generate UNI Address" (redundant action) before depositing, or funds are lost/stuck. This breaks the "Ops List -> Immediate Deposit" workflow.

## 2. Solution: Chain-Centric Address Model
We must recognize that for Account-Based Chains (ETH, TRON, BSC, SOL), an address belongs to the **Chain Account**, not the individual Asset.

### 2.1 Schema Change
We split the concept into "Account Bindings".

```sql
-- migration/012_user_chain_addresses.sql

CREATE TABLE user_chain_addresses (
    user_id BIGINT NOT NULL,
    chain_slug VARCHAR(32) NOT NULL REFERENCES chains_tb(chain_slug),
    address VARCHAR(255) NOT NULL,
    
    -- Metadata
    memo_tag VARCHAR(64), -- For XRP/EOS destination tags
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Constraint: One address per user per chain (simplified model)
    -- Or Multiple? For now, 1:1 is sufficient for MVP.
    PRIMARY KEY (user_id, chain_slug),
    UNIQUE (chain_slug, address) -- Reverse lookup must be unique
);
```

### 2.2 Sentinel Lookup Logic
When `EthScanner` detects a `Transfer(to, value, contract)`:
1.  **Identify Asset**: Match `contract` -> `asset_id` (via `chain_assets_tb`).
2.  **Identify User**: Match `to` -> `user_id` (via `user_chain_addresses` WHERE `chain_slug`='ETH').
3.  **Insert Deposit**: `deposit_history(user_id, asset_id, amount)`.

**Outcome**: The `asset_id` comes from the *Contract*, the `user_id` comes from the *Address*. They are decoupled.

## 3. Handling UTXO (BTC)
BTC addresses are generally single-use or per-intent. However, for an Exchange Deposit model, we typically generate one "Deposit Address" per User per Chain (or rotate them).
Currently, we can treat BTC the same: "User's BTC Deposit Address".
If we need `asset`-specific addresses (e.g. OMNI USDT vs BTC), that's a legacy edge case we might ignore for MVP, or handle via `chain_slug` variants (e.g. `btc-omni` vs `btc-native`? No, usually same chain).

**Decision**: The Schema `user_chain_addresses(user_id, chain_slug)` works for BTC too if we assume "One Checkable Address per User" or "List of Addresses".
Refinement: `PRIMARY KEY (chain_slug, address)` is the real physical truth. A user maps to an address.
An address maps to a user.

## 4. Operational Workflow (Final)
1.  **Listing**: Ops lists `UNI` (Contract `0x...`) -> `chain_assets_tb`.
2.  **Sentinel**: Refreshes map `0x...` -> `UNI`.
3.  **User**: Sends `UNI` to their *existing* ETH address.
4.  **Sentinel**:
    *   Sees `0x...` -> Knows it's `UNI`.
    *   Sees `Receiver` -> Knows it's `User A`.
    *   **Success**.

## 5. Status
Accepted
