# ADR-005: Unified Chain-Asset Schema & Admin Integration

**Date**: 2025-12-30
**Status**: Proposed
**Supersedes**: ADR-004 (Partial), Design Doc 0x11-c (Draft)
**Context**: Reconciling conflict between Admin's Logical Assets (`assets_tb`) and Sentinel's Physical Chains (`chain_assets`).

## 1. Problem Statement
The system currently has ambiguity regarding where "Asset Definition" lives:
1.  **Admin (`assets_tb`)**: Defines "USDT" (Logical), Symbol, Decimal (Internal).
2.  **Sentinel (`chain_assets`)**: Needs "USDT" on ETH (Physical), Contract, Decimal (Chain).
3.  **Conflict**: Potential data duplication (redundancy) and unclear ownership.

## 2. Architectural Decision: Layered Asset Model
We explicitly separate the domain model into two strictly defined layers.

### Layer 1: Logical Asset (Master) -> `assets_tb`
*   **Owner**: Admin Dashboard (Existing).
*   **Scope**: Business logic, User Balances, Trading Pairs.
*   **Key Fields**:
    *   `asset_id` (PK)
    *   `asset` (Unique Identifier, e.g., "USDT")
    *   `decimals` (System Precision, e.g., 8)
    *   `status` (Global Switch)

### Layer 2: Physical Binding (Extension) -> `chain_assets_tb`
*   **Owner**: Operations (via Admin Extension).
*   **Scope**: Blockchain adapters, Deposit/Withdrawal addresses, Sentinel config.
*   **Key Fields**:
    *   `chain_slug` (FK to `chains_tb`)
    *   `asset_id` (FK to `assets_tb`)
    *   `contract_address` (Physical ID)
    *   `decimals` (Physical Precision)
*   **Constraint**: No re-definition of Logical fields (Symbol, Name).

## 3. Schema Specification (Finalized)

```sql
-- 1. Chains (Infrastructure)
CREATE TABLE chains_tb (
    chain_slug VARCHAR(32) PRIMARY KEY,  -- 'ETH', 'BTC' (Renamed from chain_id)
    chain_name VARCHAR(64) NOT NULL,
    network_id VARCHAR(32),              -- '1', 'regtest'
    rpc_urls TEXT[] NOT NULL,
    confirmation_blocks INT DEFAULT 1,
    is_active BOOLEAN DEFAULT TRUE
);

-- 2. Chain Assets (Physical Extension)
CREATE TABLE chain_assets_tb (
    id SERIAL PRIMARY KEY,
    chain_slug VARCHAR(32) NOT NULL REFERENCES chains_tb(chain_slug),
    asset_id INT NOT NULL REFERENCES assets_tb(asset_id),
    
    -- Physical Properties Only
    contract_address VARCHAR(128),  -- Mutually Exclusive Unique ID per chain
    decimals SMALLINT NOT NULL,     -- The mapping factor (Chain -> System)
    
    -- Chain-Specific Overrides
    min_deposit DECIMAL(30, 8) DEFAULT 0,
    min_withdraw DECIMAL(30, 8) DEFAULT 0,
    withdraw_fee DECIMAL(30, 8) DEFAULT 0,
    is_active BOOLEAN DEFAULT FALSE, -- Safety: Inactive by default until verified
    
    -- Constraints
    UNIQUE(chain_slug, asset_id),        -- 1 Asset per Chain (for now, can look into bridging later)
    UNIQUE(chain_slug, contract_address) -- 1 Contract = 1 Asset
);
```

## 4. Admin Integration Scope
Admin code currently does **not** support Layer 2. To strictly follow this architecture, Admin must be updated in a future iteration:
1.  **New Model**: `ChainAsset` mapping to `chain_assets_tb`.
2.  **New View**: "Chain Configurations" tab under Asset details.
3.  **Logic**: When viewing "USDT", allow adding "ETH Binding" (Contract: 0x..., Decimals: 6).

## 5. Migration Strategy (Immediate)
For Phase 0x11-b (Sentinel Hardening), we implement the Schema and Manual Seeding (Migration 012). Admin UI updates are deferred, but the *Schema* is future-proofed to support them perfectly.
